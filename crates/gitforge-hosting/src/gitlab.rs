use crate::error::{HostingError, HostingNet, HostingResult};
use crate::http;
use crate::models::{CreatePullRequestRequest, HostingAccount, PullRequest, RemoteRepo};
use crate::provider::HostingProvider;
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
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert("PRIVATE-TOKEN", token.parse().unwrap());
    http::make_client(headers)
}

fn url_encode(s: &str) -> String {
    s.replace('%', "%25")
        .replace('/', "%2F")
        .replace(' ', "%20")
}

async fn fetch_project_id(
    client: &reqwest::Client,
    base_url: &str,
    full_path: &str,
) -> HostingResult<u64> {
    let context = format!("Failed to resolve GitLab project id for {}", full_path);
    let response = client
        .get(format!("{}/projects/{}", base_url, url_encode(full_path)))
        .send()
        .await
        .hosting_context(&context)?;
    let response = http::ensure_success(response, &context).await?;

    let project: serde_json::Value = response.json().await.hosting_context(&context)?;
    project["id"].as_u64().ok_or_else(|| HostingError::MissingProjectId {
        path: full_path.to_string(),
    })
}

#[async_trait]
impl HostingProvider for GitLabProvider {
    fn name(&self) -> &str {
        "GitLab"
    }

    fn base_url(&self) -> &str {
        &self.base_url
    }

    async fn authenticate(&self, token: &str) -> HostingResult<HostingAccount> {
        let client = make_client(token);
        let context = "GitLab authentication failed";
        let response = client
            .get(format!("{}/user", self.base_url))
            .send()
            .await
            .hosting_context(context)?;
        let response = http::ensure_success(response, context).await?;

        let user: serde_json::Value = response.json().await.hosting_context(context)?;
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

    async fn list_repos(&self, account: &HostingAccount) -> HostingResult<Vec<RemoteRepo>> {
        let token = account.token()?;
        let client = make_client(&token);
        http::paginate(
            &client,
            |page| {
                format!(
                    "{}/projects?membership=true&page={}&per_page=100&order_by=updated_at",
                    self.base_url, page
                )
            },
            100,
            "Failed to list GitLab projects",
            |json| json.as_array().cloned().unwrap_or_default(),
            |item| Some(json_to_remote_repo(item)),
        )
        .await
    }

    async fn search_repos(
        &self,
        account: &HostingAccount,
        query: &str,
    ) -> HostingResult<Vec<RemoteRepo>> {
        let token = account.token()?;
        let client = make_client(&token);
        let context = "Failed to search GitLab projects";
        let response = client
            .get(format!(
                "{}/projects?search={}&per_page=30&order_by=updated_at",
                self.base_url,
                http::url_encode_query(query)
            ))
            .send()
            .await
            .hosting_context(context)?;
        let response = http::ensure_success(response, context).await?;

        let projects: Vec<serde_json::Value> = response.json().await.hosting_context(context)?;
        Ok(projects.iter().map(json_to_remote_repo).collect())
    }

    async fn create_fork(
        &self,
        account: &HostingAccount,
        owner: &str,
        repo: &str,
    ) -> HostingResult<RemoteRepo> {
        let token = account.token()?;
        let client = make_client(&token);
        let context = format!("Failed to fork {}/{}", owner, repo);
        let project_path = url_encode(&format!("{}/{}", owner, repo));
        let response = client
            .post(format!("{}/projects/{}/fork", self.base_url, project_path))
            .send()
            .await
            .hosting_context(&context)?;
        let response = http::ensure_success(response, &context).await?;

        let fork: serde_json::Value = response.json().await.hosting_context(&context)?;
        Ok(json_to_remote_repo(&fork))
    }

    fn repo_url(&self, repo_full_name: &str) -> String {
        format!("{}/{}", self.web_url, url_encode(repo_full_name))
    }

    async fn create_pull_request(
        &self,
        account: &HostingAccount,
        req: &CreatePullRequestRequest,
    ) -> HostingResult<PullRequest> {
        let token = account.token()?;
        let client = make_client(&token);

        let is_cross_fork = req.head_owner != req.owner;

        let (project_path, body) = if is_cross_fork {
            let source_path = format!("{}/{}", req.head_owner, req.repo);
            let target_path = format!("{}/{}", req.owner, req.repo);
            let target_project_id = fetch_project_id(&client, &self.base_url, &target_path).await?;
            let body = serde_json::json!({
                "source_branch": req.head_branch,
                "target_branch": req.base_branch,
                "title": req.title,
                "description": req.body,
                "draft": req.draft,
                "target_project_id": target_project_id,
            });
            (url_encode(&source_path), body)
        } else {
            let path = format!("{}/{}", req.owner, req.repo);
            let body = serde_json::json!({
                "source_branch": req.head_branch,
                "target_branch": req.base_branch,
                "title": req.title,
                "description": req.body,
                "draft": req.draft,
            });
            (url_encode(&path), body)
        };

        let context = "Failed to create merge request";
        let response = client
            .post(format!(
                "{}/projects/{}/merge_requests",
                self.base_url, project_path
            ))
            .json(&body)
            .send()
            .await
            .hosting_context(context)?;
        let response = http::ensure_success(response, context).await?;

        let mr: serde_json::Value = response.json().await.hosting_context(context)?;
        Ok(json_to_pull_request(&mr))
    }

    async fn list_pull_requests(
        &self,
        account: &HostingAccount,
        owner: &str,
        repo: &str,
    ) -> HostingResult<Vec<PullRequest>> {
        let token = account.token()?;
        let client = make_client(&token);
        let project_path = url_encode(&format!("{}/{}", owner, repo));
        http::paginate(
            &client,
            |page| {
                format!(
                    "{}/projects/{}/merge_requests?state=opened&page={}&per_page=100",
                    self.base_url, project_path, page
                )
            },
            100,
            "Failed to list merge requests",
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
    ) -> HostingResult<Vec<String>> {
        let token = account.token()?;
        let client = make_client(&token);
        let project_path = url_encode(&format!("{}/{}", owner, repo));
        http::paginate(
            &client,
            |page| {
                format!(
                    "{}/projects/{}/repository/branches?page={}&per_page=100",
                    self.base_url, project_path, page
                )
            },
            100,
            "Failed to list branches",
            |json| json.as_array().cloned().unwrap_or_default(),
            |b| b["name"].as_str().map(|s| s.to_string()),
        )
        .await
    }
}

fn json_to_pull_request(mr: &serde_json::Value) -> PullRequest {
    PullRequest {
        number: mr["iid"].as_u64().unwrap_or(0),
        title: mr["title"].as_str().unwrap_or("").to_string(),
        html_url: mr["web_url"].as_str().unwrap_or("").to_string(),
        state: mr["state"].as_str().unwrap_or("opened").to_string(),
        head_branch: mr["source_branch"].as_str().map(|s| s.to_string()),
        draft: mr["draft"].as_bool().unwrap_or(false),
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
        is_fork: project["forked_from_project"].is_object(),
        is_private: project["visibility"].as_str() == Some("private"),
        default_branch: project["default_branch"].as_str().map(|s| s.to_string()),
        stars: project["star_count"].as_u64().unwrap_or(0),
        updated_at,
    }
}
