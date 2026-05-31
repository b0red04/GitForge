use anyhow::Result;
use async_trait::async_trait;
use crate::models::{HostingAccount, RemoteRepo};
use crate::provider::HostingProvider;

pub struct CodebergProvider {
    base_url: String,
    web_url: String,
}

impl CodebergProvider {
    pub fn new() -> Self {
        Self {
            base_url: "https://codeberg.org/api/v1".to_string(),
            web_url: "https://codeberg.org".to_string(),
        }
    }
}

impl Default for CodebergProvider {
    fn default() -> Self {
        Self::new()
    }
}

fn make_client(token: &str) -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent("gitforge")
        .default_headers({
            let mut headers = reqwest::header::HeaderMap::new();
            headers.insert(
                reqwest::header::AUTHORIZATION,
                format!("token {}", token).parse().unwrap(),
            );
            headers
        })
        .build()
        .unwrap_or_default()
}

#[async_trait]
impl HostingProvider for CodebergProvider {
    fn name(&self) -> &str {
        "Codeberg"
    }

    fn base_url(&self) -> &str {
        &self.base_url
    }

    async fn authenticate(&self, token: &str) -> Result<HostingAccount> {
        let client = make_client(token);
        let response = client
            .get(format!("{}/user", self.base_url))
            .send()
            .await?;

        if !response.status().is_success() {
            anyhow::bail!("Codeberg authentication failed: {}", response.status());
        }

        let user: serde_json::Value = response.json().await?;
        let login = user["login"].as_str().unwrap_or("unknown").to_string();
        let name = user["full_name"].as_str().unwrap_or(&login).to_string();
        let avatar = user["avatar_url"].as_str().map(|s| s.to_string());

        let token_key = format!("codeberg:{}", login);
        HostingAccount::store_token(&token_key, token)?;

        Ok(HostingAccount {
            provider: "codeberg".to_string(),
            username: login,
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
                    "{}/repos/search?page={}&limit=50&sort=updated",
                    self.base_url, page
                ))
                .send()
                .await?;

            if !response.status().is_success() {
                anyhow::bail!("Failed to list Codeberg repos: {}", response.status());
            }

            let result: serde_json::Value = response.json().await?;
            let repos = result["data"].as_array().cloned().unwrap_or_default();
            if repos.is_empty() {
                break;
            }

            for repo in &repos {
                all_repos.push(json_to_remote_repo(repo));
            }

            page += 1;
            if repos.len() < 50 {
                break;
            }
        }

        Ok(all_repos)
    }

    async fn list_org_repos(&self, account: &HostingAccount, org: &str) -> Result<Vec<RemoteRepo>> {
        let token = account.token()?;
        let client = make_client(&token);
        let response = client
            .get(format!("{}/orgs/{}/repos?limit=100", self.base_url, org))
            .send()
            .await?;

        if !response.status().is_success() {
            anyhow::bail!("Failed to list Codeberg org repos: {}", response.status());
        }

        let repos: Vec<serde_json::Value> = response.json().await?;
        Ok(repos.iter().map(json_to_remote_repo).collect())
    }

    async fn search_repos(&self, account: &HostingAccount, query: &str) -> Result<Vec<RemoteRepo>> {
        let token = account.token()?;
        let client = make_client(&token);
        let response = client
            .get(format!(
                "{}/repos/search?q={}&limit=30&sort=updated",
                self.base_url, query
            ))
            .send()
            .await?;

        if !response.status().is_success() {
            anyhow::bail!("Failed to search Codeberg repos: {}", response.status());
        }

        let result: serde_json::Value = response.json().await?;
        let repos = result["data"].as_array().cloned().unwrap_or_default();
        Ok(repos.iter().map(json_to_remote_repo).collect())
    }

    async fn create_fork(&self, account: &HostingAccount, owner: &str, repo: &str) -> Result<RemoteRepo> {
        let token = account.token()?;
        let client = make_client(&token);
        let response = client
            .post(format!("{}/repos/{}/{}/forks", self.base_url, owner, repo))
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
        match line {
            Some(l) => format!("{}/{}/src/{}/{}#L{}", self.web_url, repo_full_name, sha, path, l),
            None => format!("{}/{}/src/{}/{}", self.web_url, repo_full_name, sha, path),
        }
    }

    fn commit_url(&self, repo_full_name: &str, sha: &str) -> String {
        format!("{}/{}/commit/{}", self.web_url, repo_full_name, sha)
    }

    fn repo_url(&self, repo_full_name: &str) -> String {
        format!("{}/{}", self.web_url, repo_full_name)
    }
}

fn json_to_remote_repo(repo: &serde_json::Value) -> RemoteRepo {
    use chrono::{DateTime, Utc};

    let updated_at = repo["updated_at"]
        .as_str()
        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.with_timezone(&Utc));

    RemoteRepo {
        name: repo["name"].as_str().unwrap_or("").to_string(),
        full_name: repo["full_name"].as_str().unwrap_or("").to_string(),
        description: repo["description"].as_str().map(|s| s.to_string()),
        clone_url: repo["clone_url"].as_str().unwrap_or("").to_string(),
        ssh_url: repo["ssh_url"].as_str().map(|s| s.to_string()),
        html_url: repo["html_url"].as_str().unwrap_or("").to_string(),
        is_fork: repo["fork"].as_bool().unwrap_or(false),
        is_private: repo["private"].as_bool().unwrap_or(false),
        default_branch: repo["default_branch"].as_str().map(|s| s.to_string()),
        stars: repo["stars_count"].as_u64().unwrap_or(0),
        updated_at,
    }
}
