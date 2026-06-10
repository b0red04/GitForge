use crate::models::{HostingAccount, RemoteRepo};
use crate::provider::HostingProvider;
use anyhow::Result;
use async_trait::async_trait;

pub struct GitLabProvider {
    base_url: String,
    web_url: String,
}

impl GitLabProvider {
    pub fn new() -> Self {
        Self {
            base_url: "https://gitlab.com/api/v4".to_string(),
            web_url: "https://gitlab.com".to_string(),
        }
    }

    pub fn with_url(base_url: String) -> Self {
        let web_url = base_url.trim_end_matches("/api/v4").to_string();
        Self { base_url, web_url }
    }
}

impl Default for GitLabProvider {
    fn default() -> Self {
        Self::new()
    }
}

fn make_client(token: &str) -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent("gitforge")
        .default_headers({
            let mut headers = reqwest::header::HeaderMap::new();
            headers.insert("PRIVATE-TOKEN", token.parse().unwrap());
            headers
        })
        .build()
        .unwrap_or_default()
}

fn url_encode(s: &str) -> String {
    s.replace('%', "%25")
        .replace('/', "%2F")
        .replace(' ', "%20")
}

#[async_trait]
impl HostingProvider for GitLabProvider {
    fn name(&self) -> &str {
        "GitLab"
    }

    fn base_url(&self) -> &str {
        &self.base_url
    }

    async fn authenticate(&self, token: &str) -> Result<HostingAccount> {
        let client = make_client(token);
        let response = client.get(format!("{}/user", self.base_url)).send().await?;

        if !response.status().is_success() {
            anyhow::bail!("GitLab authentication failed: {}", response.status());
        }

        let user: serde_json::Value = response.json().await?;
        let username = user["username"].as_str().unwrap_or("unknown").to_string();
        let name = user["name"].as_str().unwrap_or(&username).to_string();
        let avatar = user["avatar_url"].as_str().map(|s| s.to_string());

        let token_key = format!("gitlab:{}", username);
        HostingAccount::store_token(&token_key, token)?;

        Ok(HostingAccount {
            provider: "gitlab".to_string(),
            username,
            display_name: name,
            avatar_url: avatar,
            token_key,
            created_at: Some(chrono::Utc::now()),
        })
    }

    async fn list_repos(&self, account: &HostingAccount) -> Result<Vec<RemoteRepo>> {
        let token = account.token()?;
        let client = make_client(&token);
        let mut all_repos = Vec::new();
        let mut page = 1;

        loop {
            let response = client
                .get(format!(
                    "{}/projects?membership=true&page={}&per_page=100&order_by=updated_at",
                    self.base_url, page
                ))
                .send()
                .await?;

            if !response.status().is_success() {
                anyhow::bail!("Failed to list GitLab projects: {}", response.status());
            }

            let projects: Vec<serde_json::Value> = response.json().await?;
            if projects.is_empty() {
                break;
            }

            for project in &projects {
                all_repos.push(json_to_remote_repo(project));
            }

            page += 1;
            if projects.len() < 100 {
                break;
            }
        }

        Ok(all_repos)
    }

    async fn list_org_repos(
        &self,
        account: &HostingAccount,
        group: &str,
    ) -> Result<Vec<RemoteRepo>> {
        let token = account.token()?;
        let client = make_client(&token);
        let response = client
            .get(format!(
                "{}/groups/{}/projects?per_page=100",
                self.base_url,
                url_encode(group)
            ))
            .send()
            .await?;

        if !response.status().is_success() {
            anyhow::bail!(
                "Failed to list GitLab group projects: {}",
                response.status()
            );
        }

        let projects: Vec<serde_json::Value> = response.json().await?;
        Ok(projects.iter().map(json_to_remote_repo).collect())
    }

    async fn search_repos(&self, account: &HostingAccount, query: &str) -> Result<Vec<RemoteRepo>> {
        let token = account.token()?;
        let client = make_client(&token);
        let response = client
            .get(format!(
                "{}/projects?search={}&per_page=30&order_by=updated_at",
                self.base_url, query
            ))
            .send()
            .await?;

        if !response.status().is_success() {
            anyhow::bail!("Failed to search GitLab projects: {}", response.status());
        }

        let projects: Vec<serde_json::Value> = response.json().await?;
        Ok(projects.iter().map(json_to_remote_repo).collect())
    }

    async fn create_fork(
        &self,
        account: &HostingAccount,
        owner: &str,
        repo: &str,
    ) -> Result<RemoteRepo> {
        let token = account.token()?;
        let client = make_client(&token);
        let project_path = url_encode(&format!("{}/{}", owner, repo));
        let response = client
            .post(format!("{}/projects/{}/fork", self.base_url, project_path))
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("Failed to fork {}/{}: {} - {}", owner, repo, status, body);
        }

        let fork: serde_json::Value = response.json().await?;
        Ok(json_to_remote_repo(&fork))
    }

    fn file_url(&self, repo_full_name: &str, sha: &str, path: &str, line: Option<u32>) -> String {
        let encoded = url_encode(repo_full_name);
        match line {
            Some(l) => format!(
                "{}/{}/-/blob/{}/{}#L{}",
                self.web_url, encoded, sha, path, l
            ),
            None => format!("{}/{}/-/blob/{}/{}", self.web_url, encoded, sha, path),
        }
    }

    fn commit_url(&self, repo_full_name: &str, sha: &str) -> String {
        format!(
            "{}/{}/-/commit/{}",
            self.web_url,
            url_encode(repo_full_name),
            sha
        )
    }

    fn repo_url(&self, repo_full_name: &str) -> String {
        format!("{}/{}", self.web_url, url_encode(repo_full_name))
    }
}

fn json_to_remote_repo(project: &serde_json::Value) -> RemoteRepo {
    use chrono::{DateTime, Utc};

    let updated_at = project["last_activity_at"]
        .as_str()
        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.with_timezone(&Utc));

    let http_url = project["http_url_to_repo"]
        .as_str()
        .unwrap_or("")
        .to_string();
    let ssh_url = project["ssh_url_to_repo"].as_str().map(|s| s.to_string());
    let web_url = project["web_url"].as_str().unwrap_or("").to_string();

    RemoteRepo {
        name: project["name"].as_str().unwrap_or("").to_string(),
        full_name: project["path_with_namespace"]
            .as_str()
            .unwrap_or("")
            .to_string(),
        description: project["description"].as_str().map(|s| s.to_string()),
        clone_url: http_url,
        ssh_url,
        html_url: web_url,
        is_fork: false,
        is_private: project["visibility"].as_str() == Some("private"),
        default_branch: project["default_branch"].as_str().map(|s| s.to_string()),
        stars: project["star_count"].as_u64().unwrap_or(0),
        updated_at,
    }
}
