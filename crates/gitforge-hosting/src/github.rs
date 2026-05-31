use anyhow::Result;
use async_trait::async_trait;
use crate::models::{HostingAccount, RemoteRepo};
use crate::provider::HostingProvider;

pub struct GitHubProvider {
    base_url: String,
    web_url: String,
}

impl GitHubProvider {
    pub fn new() -> Self {
        Self {
            base_url: "https://api.github.com".to_string(),
            web_url: "https://github.com".to_string(),
        }
    }

    pub fn with_url(base_url: String) -> Self {
        let web_url = base_url
            .trim_end_matches("/api")
            .trim_end_matches(".com")
            .to_string();
        Self { base_url, web_url }
    }
}

impl Default for GitHubProvider {
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
                reqwest::header::ACCEPT,
                "application/vnd.github+json".parse().unwrap(),
            );
            headers.insert(
                reqwest::header::AUTHORIZATION,
                format!("Bearer {}", token).parse().unwrap(),
            );
            headers
        })
        .build()
        .unwrap_or_default()
}

#[async_trait]
impl HostingProvider for GitHubProvider {
    fn name(&self) -> &str {
        "GitHub"
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
            anyhow::bail!("GitHub authentication failed: {}", response.status());
        }

        let user: serde_json::Value = response.json().await?;
        let login = user["login"].as_str().unwrap_or("unknown").to_string();
        let name = user["name"].as_str().unwrap_or(&login).to_string();
        let avatar = user["avatar_url"].as_str().map(|s| s.to_string());

        let token_key = format!("github:{}", login);
        HostingAccount::store_token(&token_key, token)?;

        Ok(HostingAccount {
            provider: "github".to_string(),
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
                    "{}/user/repos?page={}&per_page=100&sort=updated",
                    self.base_url, page
                ))
                .send()
                .await?;

            if !response.status().is_success() {
                anyhow::bail!("Failed to list repos: {}", response.status());
            }

            let repos: Vec<serde_json::Value> = response.json().await?;
            if repos.is_empty() {
                break;
            }

            for repo in &repos {
                all_repos.push(json_to_remote_repo(repo));
            }

            page += 1;
            if repos.len() < 100 {
                break;
            }
        }

        all_repos.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        Ok(all_repos)
    }

    async fn list_org_repos(&self, account: &HostingAccount, org: &str) -> Result<Vec<RemoteRepo>> {
        let token = account.token()?;
        let client = make_client(&token);
        let mut all_repos = Vec::new();
        let mut page = 1;

        loop {
            let response = client
                .get(format!(
                    "{}/orgs/{}/repos?page={}&per_page=100&sort=updated",
                    self.base_url, org, page
                ))
                .send()
                .await?;

            if !response.status().is_success() {
                anyhow::bail!("Failed to list org repos: {}", response.status());
            }

            let repos: Vec<serde_json::Value> = response.json().await?;
            if repos.is_empty() {
                break;
            }

            for repo in &repos {
                all_repos.push(json_to_remote_repo(repo));
            }

            page += 1;
            if repos.len() < 100 {
                break;
            }
        }

        Ok(all_repos)
    }

    async fn search_repos(&self, account: &HostingAccount, query: &str) -> Result<Vec<RemoteRepo>> {
        let token = account.token()?;
        let client = make_client(&token);
        let response = client
            .get(format!(
                "{}/search/repositories?q={}&per_page=30&sort=updated",
                self.base_url, query
            ))
            .send()
            .await?;

        if !response.status().is_success() {
            anyhow::bail!("Failed to search repos: {}", response.status());
        }

        let result: serde_json::Value = response.json().await?;
        let items = result["items"].as_array().cloned().unwrap_or_default();
        Ok(items.iter().map(json_to_remote_repo).collect())
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
            Some(l) => format!("{}/{}/blob/{}/{}#L{}", self.web_url, repo_full_name, sha, path, l),
            None => format!("{}/{}/blob/{}/{}", self.web_url, repo_full_name, sha, path),
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
        stars: repo["stargazers_count"].as_u64().unwrap_or(0),
        updated_at,
    }
}
