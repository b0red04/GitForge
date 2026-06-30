//! Characterization tests for `GitHubProvider`.
//!
//! These pin the current observable behaviour (URLs, pagination, JSON mappings,
//! error handling) so the refactor cannot silently regress it. The providers are
//! pointed at an in-process `httpmock` server via `GitHubProvider::with_url`.

mod common;

use common::{ensure_test_tokens, test_account};
use gitforge_hosting::CreatePullRequestRequest;
use gitforge_hosting::ConfigDrivenProvider;
use gitforge_hosting::provider::HostingProvider;
use httpmock::Method::{GET, POST};
use httpmock::prelude::*;

fn gh_repo_json(full_name: &str, updated: &str, stars: u64) -> serde_json::Value {
    serde_json::json!({
        "name": full_name.split('/').next().unwrap_or(full_name),
        "full_name": full_name,
        "description": "desc",
        "clone_url": format!("https://github.com/{}.git", full_name),
        "ssh_url": format!("git@github.com:{}.git", full_name),
        "html_url": format!("https://github.com/{}", full_name),
        "fork": false,
        "private": false,
        "default_branch": "main",
        "stargazers_count": stars,
        "updated_at": updated,
    })
}

fn gh_pr_json(number: u64, title: &str) -> serde_json::Value {
    serde_json::json!({
        "number": number,
        "title": title,
        "html_url": format!("https://github.com/o/r/pull/{}", number),
        "state": "open",
        "head": {"ref": "feature"},
        "draft": false,
    })
}

fn provider(server: &MockServer) -> ConfigDrivenProvider {
    ConfigDrivenProvider::with_url_github(server.base_url())
}

#[tokio::test]
async fn authenticate_maps_user_fields_and_stores_token() {
    ensure_test_tokens();
    let server = MockServer::start_async().await;
    server.mock(|when, then| {
        when.method(GET)
            .path("/user")
            .header("Authorization", "Bearer ghp_test");
        then.status(200).body(
            r#"{"login":"octocat","name":"The Octocat","avatar_url":"https://github.com/a.png"}"#,
        );
    });

    let account = provider(&server)
        .authenticate("ghp_test")
        .await
        .expect("auth");
    assert_eq!(account.provider, "github");
    assert_eq!(account.username, "octocat");
    assert_eq!(account.display_name, "The Octocat");
    assert_eq!(
        account.avatar_url.as_deref(),
        Some("https://github.com/a.png")
    );
    assert_eq!(account.token_key, "github:octocat");
}

#[tokio::test]
async fn authenticate_falls_back_to_login_for_display_name() {
    ensure_test_tokens();
    let server = MockServer::start_async().await;
    server.mock(|when, then| {
        when.method(GET).path("/user");
        then.status(200)
            .body(r#"{"login":"octocat","name":null,"avatar_url":null}"#);
    });

    let account = provider(&server)
        .authenticate("ghp_test")
        .await
        .expect("auth");
    assert_eq!(account.display_name, "octocat");
    assert!(account.avatar_url.is_none());
}

#[tokio::test]
async fn list_repos_paginates_and_sorts_by_updated_desc() {
    let server = MockServer::start_async().await;
    let account = test_account("github", "octocat");

    let page1: Vec<_> = (0..100)
        .map(|i| gh_repo_json(&format!("u/r{}", i), "2020-01-01T00:00:00Z", i))
        .collect();
    let page2 = vec![gh_repo_json("u/new", "2024-06-01T00:00:00Z", 999)];

    server.mock(|when, then| {
        when.method(GET)
            .path("/user/repos")
            .query_param("page", "1")
            .query_param("per_page", "100");
        then.status(200)
            .body(serde_json::to_string(&page1).unwrap());
    });
    server.mock(|when, then| {
        when.method(GET)
            .path("/user/repos")
            .query_param("page", "2")
            .query_param("per_page", "100");
        then.status(200)
            .body(serde_json::to_string(&page2).unwrap());
    });

    let repos = provider(&server).list_repos(&account).await.expect("list");
    assert_eq!(repos.len(), 101);
    assert_eq!(repos[0].full_name, "u/new");
    assert_eq!(repos[0].stars, 999);
}

#[tokio::test]
async fn search_repos_unwraps_items_envelope() {
    let server = MockServer::start_async().await;
    let account = test_account("github", "octocat");

    server.mock(|when, then| {
        when.method(GET)
            .path("/search/repositories")
            .query_param("q", "foo");
        then.status(200).body(
            serde_json::json!({
                "items": [gh_repo_json("u/foo", "2024-01-01T00:00:00Z", 42)]
            })
            .to_string(),
        );
    });

    let repos = provider(&server)
        .search_repos(&account, "foo")
        .await
        .expect("search");
    assert_eq!(repos.len(), 1);
    assert_eq!(repos[0].full_name, "u/foo");
    assert_eq!(repos[0].stars, 42);
}

#[tokio::test]
async fn create_fork_posts_to_repo_forks_endpoint() {
    let server = MockServer::start_async().await;
    let account = test_account("github", "octocat");

    server.mock(|when, then| {
        when.method(POST).path("/repos/owner/repo/forks");
        then.status(200)
            .body(gh_repo_json("octocat/repo", "2024-01-01T00:00:00Z", 0).to_string());
    });

    let fork = provider(&server)
        .create_fork(&account, "owner", "repo")
        .await
        .expect("fork");
    assert_eq!(fork.full_name, "octocat/repo");
}

#[tokio::test]
async fn create_pull_request_sends_github_body_shape() {
    let server = MockServer::start_async().await;
    let account = test_account("github", "octocat");

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
            .path("/repos/owner/repo/pulls")
            .body_includes("\"head\":\"feature\"");
        then.status(200).body(gh_pr_json(7, "T").to_string());
    });

    let pr = provider(&server)
        .create_pull_request(&account, &req)
        .await
        .expect("pr");
    assert_eq!(pr.number, 7);
    assert_eq!(pr.title, "T");
    assert_eq!(pr.head_branch.as_deref(), Some("feature"));
}

#[tokio::test]
async fn list_pull_requests_paginates() {
    let server = MockServer::start_async().await;
    let account = test_account("github", "octocat");

    let page1: Vec<_> = (0..100).map(|i| gh_pr_json(i, "t")).collect();
    let page2 = vec![gh_pr_json(100, "last")];

    server.mock(|when, then| {
        when.method(GET)
            .path("/repos/owner/repo/pulls")
            .query_param("page", "1");
        then.status(200)
            .body(serde_json::to_string(&page1).unwrap());
    });
    server.mock(|when, then| {
        when.method(GET)
            .path("/repos/owner/repo/pulls")
            .query_param("page", "2");
        then.status(200)
            .body(serde_json::to_string(&page2).unwrap());
    });

    let prs = provider(&server)
        .list_pull_requests(&account, "owner", "repo")
        .await
        .expect("list prs");
    assert_eq!(prs.len(), 101);
}

#[tokio::test]
async fn list_branches_paginates_and_extracts_names() {
    let server = MockServer::start_async().await;
    let account = test_account("github", "octocat");

    server.mock(|when, then| {
        when.method(GET).path("/repos/owner/repo/branches");
        then.status(200)
            .body(serde_json::json!([{"name": "main"}, {"name": "develop"}]).to_string());
    });

    let branches = provider(&server)
        .list_branches(&account, "owner", "repo")
        .await
        .expect("list branches");
    assert_eq!(branches, vec!["main", "develop"]);
}

#[tokio::test]
async fn error_on_non_2xx_response() {
    let server = MockServer::start_async().await;
    let account = test_account("github", "octocat");

    server.mock(|when, then| {
        when.method(GET).path("/user/repos");
        then.status(403).body("forbidden");
    });

    let err = provider(&server)
        .list_repos(&account)
        .await
        .expect_err("should fail");
    let msg = format!("{err}");
    assert!(
        msg.contains("403") || msg.contains("Forbidden") || msg.contains("forbidden"),
        "unexpected error message: {msg}"
    );
}

#[test]
fn repo_url_uses_web_url() {
    let gh = ConfigDrivenProvider::new_github();
    assert_eq!(gh.repo_url("foo/bar"), "https://github.com/foo/bar");
}

#[test]
fn repo_url_with_url_keeps_github_web_host() {
    // GitHub's API host (api.github.com) differs from its web host; pointing the
    // API base URL at it must not drag the browser URL along.
    let gh = ConfigDrivenProvider::with_url_github("https://api.github.com".to_string());
    assert_eq!(gh.repo_url("foo/bar"), "https://github.com/foo/bar");
}
