//! Unified provider for Gitea/Forgejo-style Git hosting APIs.
//!
//! GitHub and Codeberg (Forgejo) share ~95% of their HTTP shape: same fork/PR/
//! branch URL paths, same PR body schema, same `json_to_pull_request` mapping.
//! The genuine differences — auth scheme, page-size, JSON key names, response
//! envelopes — are captured in [`GiteaStyleConfig`] as data, and
//! [`GiteaStyleProvider`] implements [`HostingProvider`] generically over that
//! config.
//!
//! Adding a new Gitea-style provider (Gitea self-hosted, Forgejo, etc.) is a
//! new `const` config + constructor pair, not ~320 lines of copy-pasted trait
//! impl.

use crate::http;
use crate::models::{CreatePullRequestRequest, HostingAccount, PullRequest, RemoteRepo};
use crate::provider::HostingProvider;
use anyhow::Result;
use async_trait::async_trait;

/// Per-provider configuration captured as plain data.
struct GiteaStyleConfig {
    id: &'static str,
    display_name: &'static str,
    default_base_url: &'static str,
    default_web_url: &'static str,
    api_suffix: &'static str,
    auth_scheme: &'static str,
    accept: Option<&'static str>,
    page_param: &'static str,
    page_size: usize,
    sort_repos: bool,
    stars_key: &'static str,
    display_name_key: &'static str,
    list_repos_path: &'static str,
    search_repos_path: &'static str,
    /// JSON key wrapping the repo array in list/search responses, or "" for a
    /// bare top-level array.
    list_repos_envelope: &'static str,
    search_repos_envelope: &'static str,
}

const GITHUB: GiteaStyleConfig = GiteaStyleConfig {
    id: "github",
    display_name: "GitHub",
    default_base_url: "https://api.github.com",
    default_web_url: "https://github.com",
    api_suffix: "",
    auth_scheme: "Bearer",
    accept: Some("application/vnd.github+json"),
    page_param: "per_page",
    page_size: 100,
    sort_repos: true,
    stars_key: "stargazers_count",
    display_name_key: "name",
    list_repos_path: "/user/repos",
    search_repos_path: "/search/repositories",
    list_repos_envelope: "",
    search_repos_envelope: "items",
};

const CODEBERG: GiteaStyleConfig = GiteaStyleConfig {
    id: "codeberg",
    display_name: "Codeberg",
    default_base_url: "https://codeberg.org/api/v1",
    default_web_url: "https://codeberg.org",
    api_suffix: "/api/v1",
    auth_scheme: "token",
    accept: None,
    page_param: "limit",
    page_size: 50,
    sort_repos: false,
    stars_key: "stars_count",
    display_name_key: "full_name",
    list_repos_path: "/user/repos",
    search_repos_path: "/repos/search",
    list_repos_envelope: "",
    search_repos_envelope: "data",
};

/// A hosting provider for Gitea/Forgejo-style APIs (GitHub, Codeberg, etc.).
pub struct GiteaStyleProvider {
    base_url: String,
    web_url: String,
    config: &'static GiteaStyleConfig,
}

impl GiteaStyleProvider {
    pub fn new_github() -> Self {
        Self::new(&GITHUB)
    }

    pub fn new_codeberg() -> Self {
        Self::new(&CODEBERG)
    }

    pub fn with_url_github(base_url: String) -> Self {
        Self::with_url(&GITHUB, base_url)
    }

    pub fn with_url_codeberg(base_url: String) -> Self {
        Self::with_url(&CODEBERG, base_url)
    }

    fn new(config: &'static GiteaStyleConfig) -> Self {
        Self {
            base_url: config.default_base_url.to_string(),
            web_url: config.default_web_url.to_string(),
            config,
        }
    }

    fn with_url(config: &'static GiteaStyleConfig, base_url: String) -> Self {
        let web_url = base_url.trim_end_matches(config.api_suffix).to_string();
        Self {
            base_url,
            web_url,
            config,
        }
    }

    fn make_client(&self, token: &str) -> reqwest::Client {
        let mut headers = reqwest::header::HeaderMap::new();
        if let Some(accept) = self.config.accept {
            headers.insert(reqwest::header::ACCEPT, accept.parse().unwrap());
        }
        headers.insert(
            reqwest::header::AUTHORIZATION,
            format!("{} {}", self.config.auth_scheme, token)
                .parse()
                .unwrap(),
        );
        http::make_client(headers)
    }
}

impl Default for GiteaStyleProvider {
    fn default() -> Self {
        Self::new_github()
    }
}

#[async_trait]
impl HostingProvider for GiteaStyleProvider {
    fn name(&self) -> &str {
        self.config.display_name
    }

    fn base_url(&self) -> &str {
        &self.base_url
    }

    async fn authenticate(&self, token: &str) -> Result<HostingAccount> {
        let client = self.make_client(token);
        let response = client
            .get(format!("{}/user", self.base_url))
            .send()
            .await?;
        let response = http::ensure_success(
            response,
            &format!("{} authentication failed", self.config.display_name),
        )
        .await?;

        let user: serde_json::Value = response.json().await?;
        let login = user["login"].as_str().unwrap_or("unknown").to_string();
        let name = user[self.config.display_name_key]
            .as_str()
            .unwrap_or(&login)
            .to_string();
        let avatar = user["avatar_url"].as_str().map(|s| s.to_string());

        let token_key = format!("{}:{}", self.config.id, login);
        HostingAccount::store_token(&token_key, token)?;

        Ok(HostingAccount {
            provider: self.config.id.to_string(),
            username: login,
            display_name: name,
            avatar_url: avatar,
            token_key,
            created_at: Some(chrono::Utc::now()),
        })
    }

    async fn list_repos(&self, account: &HostingAccount) -> Result<Vec<RemoteRepo>> {
        let token = account.token()?;
        let client = self.make_client(&token);
        let cfg = self.config;
        let mut repos = http::paginate(
            &client,
            |page| {
                format!(
                    "{}{}?page={}&{}={}&sort=updated",
                    self.base_url, cfg.list_repos_path, page, cfg.page_param, cfg.page_size
                )
            },
            cfg.page_size,
            "Failed to list repos",
            |json| extract_envelope(json, cfg.list_repos_envelope),
            |item| Some(json_to_remote_repo(item, cfg.stars_key)),
        )
        .await?;
        if cfg.sort_repos {
            repos.sort_by_key(|r| std::cmp::Reverse(r.updated_at));
        }
        Ok(repos)
    }

    async fn search_repos(&self, account: &HostingAccount, query: &str) -> Result<Vec<RemoteRepo>> {
        let token = account.token()?;
        let client = self.make_client(&token);
        let response = client
            .get(format!(
                "{}{}?q={}&{}={}&sort=updated",
                self.base_url,
                self.config.search_repos_path,
                http::url_encode_query(query),
                self.config.page_param,
                self.config.page_size
            ))
            .send()
            .await?;
        let response =
            http::ensure_success(response, "Failed to search repos").await?;

        let result: serde_json::Value = response.json().await?;
        let items = extract_envelope(&result, self.config.search_repos_envelope);
        Ok(items
            .iter()
            .map(|r| json_to_remote_repo(r, self.config.stars_key))
            .collect())
    }

    async fn create_fork(
        &self,
        account: &HostingAccount,
        owner: &str,
        repo: &str,
    ) -> Result<RemoteRepo> {
        let token = account.token()?;
        let client = self.make_client(&token);
        let response = client
            .post(format!("{}/repos/{}/{}/forks", self.base_url, owner, repo))
            .send()
            .await?;
        let response = http::ensure_success(
            response,
            &format!("Failed to fork {}/{}", owner, repo),
        )
        .await?;

        let fork: serde_json::Value = response.json().await?;
        Ok(json_to_remote_repo(&fork, self.config.stars_key))
    }

    fn repo_url(&self, repo_full_name: &str) -> String {
        format!("{}/{}", self.web_url, repo_full_name)
    }

    async fn create_pull_request(
        &self,
        account: &HostingAccount,
        req: &CreatePullRequestRequest,
    ) -> Result<PullRequest> {
        let token = account.token()?;
        let client = self.make_client(&token);

        let head = if req.head_owner == req.owner {
            req.head_branch.clone()
        } else {
            format!("{}:{}", req.head_owner, req.head_branch)
        };

        let body = serde_json::json!({
            "title": req.title,
            "body": req.body,
            "head": head,
            "base": req.base_branch,
            "draft": req.draft,
        });

        let response = client
            .post(format!(
                "{}/repos/{}/{}/pulls",
                self.base_url, req.owner, req.repo
            ))
            .json(&body)
            .send()
            .await?;
        let response =
            http::ensure_success(response, "Failed to create pull request").await?;

        let pr: serde_json::Value = response.json().await?;
        Ok(json_to_pull_request(&pr))
    }

    async fn list_pull_requests(
        &self,
        account: &HostingAccount,
        owner: &str,
        repo: &str,
    ) -> Result<Vec<PullRequest>> {
        let token = account.token()?;
        let client = self.make_client(&token);
        http::paginate(
            &client,
            |page| {
                format!(
                    "{}/repos/{}/{}/pulls?state=open&page={}&{}={}",
                    self.base_url, owner, repo, page, self.config.page_param, self.config.page_size
                )
            },
            self.config.page_size,
            "Failed to list pull requests",
            |json| json.as_array().cloned().unwrap_or_default(),
            |item| Some(json_to_pull_request(item)),
        )
        .await
    }

    async fn list_branches(
        &self,
        account: &HostingAccount,
        owner: &str,
        repo: &str,
    ) -> Result<Vec<String>> {
        let token = account.token()?;
        let client = self.make_client(&token);
        http::paginate(
            &client,
            |page| {
                format!(
                    "{}/repos/{}/{}/branches?page={}&{}={}",
                    self.base_url, owner, repo, page, self.config.page_param, self.config.page_size
                )
            },
            self.config.page_size,
            "Failed to list branches",
            |json| json.as_array().cloned().unwrap_or_default(),
            |b| b["name"].as_str().map(|s| s.to_string()),
        )
        .await
    }
}

fn extract_envelope(json: &serde_json::Value, envelope: &str) -> Vec<serde_json::Value> {
    if envelope.is_empty() {
        json.as_array().cloned().unwrap_or_default()
    } else {
        json[envelope].as_array().cloned().unwrap_or_default()
    }
}

fn json_to_pull_request(pr: &serde_json::Value) -> PullRequest {
    PullRequest {
        number: pr["number"].as_u64().unwrap_or(0),
        title: pr["title"].as_str().unwrap_or("").to_string(),
        html_url: pr["html_url"].as_str().unwrap_or("").to_string(),
        state: pr["state"].as_str().unwrap_or("open").to_string(),
        head_branch: pr["head"]["ref"].as_str().map(|s| s.to_string()),
        draft: pr["draft"].as_bool().unwrap_or(false),
    }
}

fn json_to_remote_repo(repo: &serde_json::Value, stars_key: &str) -> RemoteRepo {
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
        stars: repo[stars_key].as_u64().unwrap_or(0),
        updated_at,
    }
}
