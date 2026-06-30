//! Characterization tests for `GitLabProvider`.
//!
//! GitLab diverges from the GitHub/Codeberg shape in several ways that the
//! refactor must preserve:
//!   - Auth header is `PRIVATE-TOKEN: <t>` (not `Bearer`/`token`).
//!   - Project paths in URLs are percent-encoded (`owner/repo` → `owner%2Frepo`).
//!   - `create_pull_request` for cross-fork MERs must resolve a numeric
//!     `target_project_id` via a separate `GET /projects/:id` call.
//!   - JSON keys differ: `iid`/`source_branch`/`web_url`/`last_activity_at`/
//!     `path_with_namespace`/`star_count`/`visibility`.

mod common;

use common::{ensure_test_tokens, test_account};
use gitforge_hosting::CreatePullRequestRequest;
use gitforge_hosting::ConfigDrivenProvider;
use gitforge_hosting::provider::HostingProvider;
use httpmock::Method::{GET, POST};
use httpmock::prelude::*;

fn gl_project_json(path: &str, updated: &str, stars: u64, visibility: &str) -> serde_json::Value {
    serde_json::json!({
        "name": path.split('/').next().unwrap_or(path),
        "path_with_namespace": path,
        "description": null,
        "http_url_to_repo": format!("https://gitlab.com/{}.git", path),
        "ssh_url_to_repo": format!("git@gitlab.com:{}.git", path),
        "web_url": format!("https://gitlab.com/{}", path),
        "visibility": visibility,
        "default_branch": "main",
        "star_count": stars,
        "last_activity_at": updated,
    })
}

fn gl_mr_json(iid: u64, title: &str) -> serde_json::Value {
    serde_json::json!({
        "iid": iid,
        "title": title,
        "web_url": format!("https://gitlab.com/o/r/-/merge_requests/{}", iid),
        "state": "opened",
        "source_branch": "feature",
        "draft": false,
    })
}

fn provider(server: &MockServer) -> ConfigDrivenProvider {
    ConfigDrivenProvider::with_url_gitlab(server.base_url())
}

#[tokio::test]
async fn authenticate_uses_username_key() {
    ensure_test_tokens();
    let server = MockServer::start_async().await;
    server.mock(|when, then| {
        when.method(GET)
            .path("/user")
            .header("PRIVATE-TOKEN", "gl_test");
        then.status(200).body(
            r#"{"username":"bob","name":"Bob Smith","avatar_url":"https://gitlab.com/a.png"}"#,
        );
    });

    let account = provider(&server)
        .authenticate("gl_test")
        .await
        .expect("auth");
    assert_eq!(account.provider, "gitlab");
    assert_eq!(account.username, "bob");
    assert_eq!(account.display_name, "Bob Smith");
    assert_eq!(account.token_key, "gitlab:bob");
}

#[tokio::test]
async fn list_repos_paginates_projects_endpoint() {
    let server = MockServer::start_async().await;
    let account = test_account("gitlab", "bob");

    let page1: Vec<_> = (0..100)
        .map(|i| gl_project_json(&format!("u/p{}", i), "2020-01-01T00:00:00Z", i, "public"))
        .collect();
    let page2 = vec![gl_project_json(
        "u/new",
        "2024-06-01T00:00:00Z",
        999,
        "public",
    )];

    server.mock(|when, then| {
        when.method(GET)
            .path("/projects")
            .query_param("membership", "true")
            .query_param("page", "1")
            .query_param("per_page", "100");
        then.status(200)
            .body(serde_json::to_string(&page1).unwrap());
    });
    server.mock(|when, then| {
        when.method(GET)
            .path("/projects")
            .query_param("page", "2")
            .query_param("per_page", "100");
        then.status(200)
            .body(serde_json::to_string(&page2).unwrap());
    });

    let repos = provider(&server).list_repos(&account).await.expect("list");
    assert_eq!(repos.len(), 101);
    // GitLab does NOT sort — items appear in page order.
    assert_eq!(repos[0].full_name, "u/p0");
    assert!(repos.iter().any(|r| r.full_name == "u/new"));
}

#[tokio::test]
async fn search_repos_hits_projects_search_endpoint() {
    let server = MockServer::start_async().await;
    let account = test_account("gitlab", "bob");

    server.mock(|when, then| {
        when.method(GET)
            .path("/projects")
            .query_param("search", "foo");
        then.status(200).body(
            serde_json::json!([gl_project_json(
                "u/foo",
                "2024-01-01T00:00:00Z",
                42,
                "private"
            )])
            .to_string(),
        );
    });

    let repos = provider(&server)
        .search_repos(&account, "foo")
        .await
        .expect("search");
    assert_eq!(repos.len(), 1);
    assert_eq!(repos[0].full_name, "u/foo");
    assert!(
        repos[0].is_private,
        "visibility=private should map to is_private=true"
    );
}

#[tokio::test]
async fn create_fork_posts_to_url_encoded_project_fork() {
    let server = MockServer::start_async().await;
    let account = test_account("gitlab", "bob");

    server.mock(|when, then| {
        when.method(POST).path("/projects/owner%2Frepo/fork");
        then.status(200)
            .body(gl_project_json("bob/repo", "2024-01-01T00:00:00Z", 0, "public").to_string());
    });

    let fork = provider(&server)
        .create_fork(&account, "owner", "repo")
        .await
        .expect("fork");
    assert_eq!(fork.full_name, "bob/repo");
}

#[tokio::test]
async fn create_pull_request_same_fork_uses_source_branch_body() {
    let server = MockServer::start_async().await;
    let account = test_account("gitlab", "bob");

    let req = CreatePullRequestRequest {
        owner: "owner".into(),
        repo: "repo".into(),
        title: "T".into(),
        body: "B".into(),
        head_owner: "owner".into(),
        head_branch: "feature".into(),
        base_branch: "main".into(),
        draft: false,
    };

    server.mock(|when, then| {
        when.method(POST)
            .path("/projects/owner%2Frepo/merge_requests")
            .body_includes("\"source_branch\":\"feature\"")
            .body_includes("\"target_branch\":\"main\"");
        then.status(200).body(gl_mr_json(5, "T").to_string());
    });

    let pr = provider(&server)
        .create_pull_request(&account, &req)
        .await
        .expect("pr");
    assert_eq!(pr.number, 5);
    assert_eq!(pr.head_branch.as_deref(), Some("feature"));
}

#[tokio::test]
async fn create_pull_request_cross_fork_resolves_target_project_id() {
    let server = MockServer::start_async().await;
    let account = test_account("gitlab", "bob");

    let req = CreatePullRequestRequest {
        owner: "upstream".into(),
        repo: "repo".into(),
        title: "Cross".into(),
        body: "B".into(),
        head_owner: "bob".into(),
        head_branch: "feature".into(),
        base_branch: "main".into(),
        draft: false,
    };

    server.mock(|when, then| {
        when.method(GET).path("/projects/upstream%2Frepo");
        then.status(200)
            .body(r#"{"id": 4242, "path_with_namespace": "upstream/repo"}"#);
    });

    server.mock(|when, then| {
        when.method(POST)
            .path("/projects/bob%2Frepo/merge_requests")
            .body_includes("\"target_project_id\":4242")
            .body_includes("\"source_branch\":\"feature\"");
        then.status(200).body(gl_mr_json(9, "Cross").to_string());
    });

    let pr = provider(&server)
        .create_pull_request(&account, &req)
        .await
        .expect("pr");
    assert_eq!(pr.number, 9);
    assert_eq!(pr.title, "Cross");
}

#[tokio::test]
async fn list_pull_requests_hits_merge_requests_endpoint() {
    let server = MockServer::start_async().await;
    let account = test_account("gitlab", "bob");

    server.mock(|when, then| {
        when.method(GET)
            .path("/projects/owner%2Frepo/merge_requests")
            .query_param("state", "opened")
            .query_param("per_page", "100");
        then.status(200)
            .body(serde_json::json!([gl_mr_json(1, "a"), gl_mr_json(2, "b")]).to_string());
    });

    let prs = provider(&server)
        .list_pull_requests(&account, "owner", "repo")
        .await
        .expect("list prs");
    assert_eq!(prs.len(), 2);
    assert_eq!(prs[0].number, 1);
}

#[tokio::test]
async fn list_branches_hits_repository_branches_endpoint() {
    let server = MockServer::start_async().await;
    let account = test_account("gitlab", "bob");

    server.mock(|when, then| {
        when.method(GET)
            .path("/projects/owner%2Frepo/repository/branches");
        then.status(200)
            .body(serde_json::json!([{"name": "main"}, {"name": "dev"}]).to_string());
    });

    let branches = provider(&server)
        .list_branches(&account, "owner", "repo")
        .await
        .expect("list branches");
    assert_eq!(branches, vec!["main", "dev"]);
}

#[tokio::test]
async fn error_on_non_2xx() {
    let server = MockServer::start_async().await;
    let account = test_account("gitlab", "bob");

    server.mock(|when, then| {
        when.method(GET).path("/projects");
        then.status(500).body("server error");
    });

    provider(&server)
        .list_repos(&account)
        .await
        .expect_err("should fail");
}

#[test]
fn repo_url_url_encodes_slashes_in_full_name() {
    let gl = ConfigDrivenProvider::new_gitlab();
    assert_eq!(
        gl.repo_url("group/sub/project"),
        "https://gitlab.com/group%2Fsub%2Fproject"
    );
}
