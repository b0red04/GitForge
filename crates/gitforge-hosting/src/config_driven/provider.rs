//! [`ConfigDrivenProvider`] implementation and pull-request creation hooks.

use crate::error::{HostingError, HostingResult};
use crate::http;
use crate::models::{CreatePullRequestRequest, HostingAccount, PullRequest, RemoteRepo};
use crate::provider::HostingProvider;
use async_trait::async_trait;

use super::config::{
    self, AuthStyle, EndpointAction, ProviderConfig, PullRequestStrategy, SearchStyle,
    config_for_id,
};
use super::mappers::{extract_envelope, json_to_pull_request, json_to_remote_repo};

/// A hosting provider driven by a static [`ProviderConfig`] row.
pub struct ConfigDrivenProvider {
    base_url: String,
    web_url: String,
    config: &'static ProviderConfig,
}

impl ConfigDrivenProvider {
    pub fn from_id(id: &str) -> Option<Self> {
        config_for_id(id).map(Self::new)
    }

    pub fn with_url_from_id(id: &str, base_url: String) -> Option<Self> {
        config_for_id(id).map(|cfg| {
            let web_url = Self::derive_web_url(cfg, &base_url);
            Self::with_url(cfg, base_url, web_url)
        })
    }

    pub fn new_github() -> Self {
        Self::from_id("github").expect("github config row exists")
    }

    pub fn new_codeberg() -> Self {
        Self::from_id("codeberg").expect("codeberg config row exists")
    }

    pub fn new_gitlab() -> Self {
        Self::from_id("gitlab").expect("gitlab config row exists")
    }

    pub fn with_url_github(base_url: String) -> Self {
        Self::with_url_from_id("github", base_url).expect("github config row exists")
    }

    pub fn with_url_codeberg(base_url: String) -> Self {
        Self::with_url_from_id("codeberg", base_url).expect("codeberg config row exists")
    }

    pub fn with_url_gitlab(base_url: String) -> Self {
        Self::with_url_from_id("gitlab", base_url).expect("gitlab config row exists")
    }

    fn new(config: &'static ProviderConfig) -> Self {
        Self {
            base_url: config.default_base_url.to_string(),
            web_url: config.default_web_url.to_string(),
            config,
        }
    }

    fn with_url(config: &'static ProviderConfig, base_url: String, web_url: String) -> Self {
        Self {
            base_url,
            web_url,
            config,
        }
    }

    /// Derive the browser host from the API base URL.
    ///
    /// Gitea-style and GitLab APIs hang off the web host under a versioned path
    /// (`/api/v1`, `/api/v4`); stripping that suffix recovers the web URL.
    /// GitHub's API lives on a separate host (`api.github.com`) with no
    /// versioned suffix, so fall back to the configured default web URL rather
    /// than leaving the browser pointed at the API host.
    fn derive_web_url(config: &ProviderConfig, base_url: &str) -> String {
        let suffix = config.api_suffix;
        if !suffix.is_empty() && base_url.ends_with(suffix) {
            base_url.trim_end_matches(suffix).to_string()
        } else {
            config.default_web_url.to_string()
        }
    }

    fn make_client(&self, token: &str) -> HostingResult<reqwest::Client> {
        let mut headers = reqwest::header::HeaderMap::new();
        if let Some(accept) = self.config.accept {
            headers.insert(
                reqwest::header::ACCEPT,
                accept.parse().map_err(|e| {
                    anyhow::anyhow!("invalid accept header for {}: {e}", self.config.display_name)
                })?,
            );
        }
        match self.config.auth {
            AuthStyle::Authorization { scheme } => {
                let value = format!("{scheme} {token}").parse().map_err(|e| {
                    anyhow::anyhow!("invalid {} auth token: {e}", self.config.display_name)
                })?;
                headers.insert(reqwest::header::AUTHORIZATION, value);
            }
            AuthStyle::HeaderToken { name } => {
                let header_name =
                    reqwest::header::HeaderName::from_bytes(name.as_bytes()).map_err(|e| {
                        anyhow::anyhow!(
                            "invalid auth header name for {}: {e}",
                            self.config.display_name
                        )
                    })?;
                let value = token.parse().map_err(|e| {
                    anyhow::anyhow!("invalid {} auth token: {e}", self.config.display_name)
                })?;
                headers.insert(header_name, value);
            }
        }
        Ok(http::make_client(headers))
    }
}

impl Default for ConfigDrivenProvider {
    fn default() -> Self {
        Self::new_github()
    }
}

#[async_trait]
impl HostingProvider for ConfigDrivenProvider {
    fn name(&self) -> &str {
        self.config.display_name
    }

    fn base_url(&self) -> &str {
        &self.base_url
    }

    async fn authenticate(&self, token: &str) -> HostingResult<HostingAccount> {
        let client = self.make_client(token)?;
        let ctx = format!("{} authentication failed", self.config.display_name);
        let user: serde_json::Value =
            http::get_json(&client, format!("{}/user", self.base_url), &ctx).await?;
        let username = user[self.config.username_key]
            .as_str()
            .unwrap_or("unknown")
            .to_string();
        let name = user[self.config.display_name_key]
            .as_str()
            .unwrap_or(&username)
            .to_string();
        let avatar = user["avatar_url"].as_str().map(|s| s.to_string());

        let token_key = format!("{}:{}", self.config.id, username);
        HostingAccount::store_token(&token_key, token)?;

        Ok(HostingAccount {
            provider: self.config.id.to_string(),
            username,
            display_name: name,
            avatar_url: avatar,
            token_key,
            created_at: Some(chrono::Utc::now()),
        })
    }

    async fn list_repos(&self, account: &HostingAccount) -> HostingResult<Vec<RemoteRepo>> {
        let token = account.token()?;
        let client = self.make_client(&token)?;
        let cfg = self.config;
        let base_url = self.base_url.clone();
        let extra = cfg.list_repos_query_extra;
        let repo_keys = &cfg.family.repo_keys;
        let mut repos = http::paginate(
            &client,
            move |page| {
                let mut url = format!(
                    "{}{}?page={}&{}={}",
                    base_url, cfg.list_repos_path, page, cfg.page_param, cfg.page_size
                );
                if !extra.is_empty() {
                    url.push('&');
                    url.push_str(extra);
                }
                url
            },
            cfg.page_size,
            "Failed to list repos",
            |json| extract_envelope(json, cfg.list_repos_envelope),
            |item| Some(json_to_remote_repo(item, repo_keys)),
        )
        .await?;
        if cfg.sort_repos {
            repos.sort_by_key(|r| std::cmp::Reverse(r.updated_at));
        }
        Ok(repos)
    }

    async fn search_repos(
        &self,
        account: &HostingAccount,
        query: &str,
    ) -> HostingResult<Vec<RemoteRepo>> {
        let token = account.token()?;
        let client = self.make_client(&token)?;
        let encoded = http::url_encode_query(query);
        let repo_keys = &self.config.family.repo_keys;

        let repos = match self.config.search_style {
            SearchStyle::Dedicated {
                path,
                envelope,
                query_param,
            } => {
                let url = format!(
                    "{}{}?{}={}&{}={}&sort=updated",
                    self.base_url,
                    path,
                    query_param,
                    encoded,
                    self.config.page_param,
                    self.config.page_size
                );
                let result: serde_json::Value =
                    http::get_json(&client, url, "Failed to search repos").await?;
                extract_envelope(&result, envelope)
            }
            SearchStyle::ProjectFilter {
                query_param,
                page_size,
                query_extra,
            } => {
                let url = format!(
                    "{}{}?{}={}&{}={}&{}",
                    self.base_url,
                    self.config.list_repos_path,
                    query_param,
                    encoded,
                    self.config.page_param,
                    page_size,
                    query_extra
                );
                http::get_json(&client, url, "Failed to search GitLab projects").await?
            }
        };

        Ok(repos
            .iter()
            .map(|r| json_to_remote_repo(r, repo_keys))
            .collect())
    }

    async fn create_fork(
        &self,
        account: &HostingAccount,
        owner: &str,
        repo: &str,
    ) -> HostingResult<RemoteRepo> {
        let token = account.token()?;
        let client = self.make_client(&token)?;
        let ctx = format!("Failed to fork {}/{}", owner, repo);
        let url = config::collection_url(&self.base_url, self.config, owner, repo, EndpointAction::Fork);
        let fork: serde_json::Value = http::post_json(&client, url, None::<&()>, &ctx).await?;
        Ok(json_to_remote_repo(&fork, &self.config.family.repo_keys))
    }

    fn repo_url(&self, repo_full_name: &str) -> String {
        let path = if self.config.encode_web_repo_path {
            http::url_encode_path(repo_full_name)
        } else {
            repo_full_name.to_string()
        };
        format!("{}/{}", self.web_url, path)
    }

    async fn create_pull_request(
        &self,
        account: &HostingAccount,
        req: &CreatePullRequestRequest,
    ) -> HostingResult<PullRequest> {
        match self.config.family.pr_strategy {
            PullRequestStrategy::HeadColonOnTarget => {
                create_pr_head_colon_on_target(self, account, req).await
            }
            PullRequestStrategy::TargetProjectIdOnSource => {
                create_pr_target_project_id_on_source(self, account, req).await
            }
        }
    }

    async fn list_pull_requests(
        &self,
        account: &HostingAccount,
        owner: &str,
        repo: &str,
    ) -> HostingResult<Vec<PullRequest>> {
        let token = account.token()?;
        let client = self.make_client(&token)?;
        let base_url = self.base_url.clone();
        let cfg = self.config;
        let pr_keys = &cfg.family.pr_keys;
        let owner = owner.to_string();
        let repo = repo.to_string();
        let open_state = cfg.pr_open_state;
        http::paginate(
            &client,
            move |page| {
                format!(
                    "{}?state={}&page={}&{}={}",
                    config::collection_url(&base_url, cfg, &owner, &repo, EndpointAction::PullRequest),
                    open_state,
                    page,
                    cfg.page_param,
                    cfg.page_size
                )
            },
            cfg.page_size,
            "Failed to list pull requests",
            |json| json.as_array().cloned().unwrap_or_default(),
            |item| Some(json_to_pull_request(item, pr_keys)),
        )
        .await
    }

    async fn list_branches(
        &self,
        account: &HostingAccount,
        owner: &str,
        repo: &str,
    ) -> HostingResult<Vec<String>> {
        let token = account.token()?;
        let client = self.make_client(&token)?;
        let base_url = self.base_url.clone();
        let cfg = self.config;
        let owner = owner.to_string();
        let repo = repo.to_string();
        http::paginate(
            &client,
            move |page| {
                format!(
                    "{}?page={}&{}={}",
                    config::collection_url(&base_url, cfg, &owner, &repo, EndpointAction::Branch),
                    page,
                    cfg.page_param,
                    cfg.page_size
                )
            },
            cfg.page_size,
            "Failed to list branches",
            |json| json.as_array().cloned().unwrap_or_default(),
            |b| b["name"].as_str().map(|s| s.to_string()),
        )
        .await
    }
}

async fn create_pr_head_colon_on_target(
    provider: &ConfigDrivenProvider,
    account: &HostingAccount,
    req: &CreatePullRequestRequest,
) -> HostingResult<PullRequest> {
    let token = account.token()?;
    let client = provider.make_client(&token)?;
    let keys = &provider.config.family.pr_keys;

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

    let url = config::collection_url(
        &provider.base_url,
        provider.config,
        &req.owner,
        &req.repo,
        EndpointAction::PullRequest,
    );
    let ctx = "Failed to create pull request";
    let pr: serde_json::Value = http::post_json(&client, url, Some(&body), ctx).await?;
    Ok(json_to_pull_request(&pr, keys))
}

async fn create_pr_target_project_id_on_source(
    provider: &ConfigDrivenProvider,
    account: &HostingAccount,
    req: &CreatePullRequestRequest,
) -> HostingResult<PullRequest> {
    let token = account.token()?;
    let client = provider.make_client(&token)?;
    let keys = &provider.config.family.pr_keys;

    let mut body = serde_json::json!({
        "source_branch": req.head_branch,
        "target_branch": req.base_branch,
        "title": req.title,
        "description": req.body,
        "draft": req.draft,
    });

    let (owner, repo) = if req.head_owner != req.owner {
        let target_path = format!("{}/{}", req.owner, req.repo);
        let target_project_id =
            fetch_project_id(&client, &provider.base_url, &target_path).await?;
        body["target_project_id"] = target_project_id.into();
        (req.head_owner.as_str(), req.repo.as_str())
    } else {
        (req.owner.as_str(), req.repo.as_str())
    };

    let url = config::collection_url(
        &provider.base_url,
        provider.config,
        owner,
        repo,
        EndpointAction::PullRequest,
    );
    let ctx = "Failed to create merge request";
    let mr: serde_json::Value = http::post_json(&client, url, Some(&body), ctx).await?;
    Ok(json_to_pull_request(&mr, keys))
}

async fn fetch_project_id(
    client: &reqwest::Client,
    base_url: &str,
    full_path: &str,
) -> HostingResult<u64> {
    let ctx = format!("Failed to resolve GitLab project id for {full_path}");
    let url = format!(
        "{}/projects/{}",
        base_url,
        http::url_encode_path(full_path)
    );
    let project: serde_json::Value = http::get_json(client, url, &ctx).await?;
    project["id"].as_u64().ok_or(HostingError::MissingProjectId {
        path: full_path.to_string(),
    })
}
