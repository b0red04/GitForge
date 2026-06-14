//! Characterization tests for `CodebergProvider`.
//!
//! Codeberg runs Forgejo (a Gitea fork) whose API closely mirrors GitHub's,
//! but differs in: auth header (`token <t>` vs `Bearer <t>`), page-param name
//! (`limit` vs `per_page`), page size (50 vs 100), the `data` response envelope,
//! and JSON key names (`stars_count`, `full_name` for display).

mod common;

use common::{ensure_test_tokens, test_account};
use gitforge_hosting::CreatePullRequestRequest;
use gitforge_hosting::gitea_style::GiteaStyleProvider;
use gitforge_hosting::provider::HostingProvider;
use httpmock::Method::{GET, POST};
use httpmock::prelude::*;

fn cb_repo_json(full_name: &str, updated: &str, stars: u64) -> serde_json::Value {
    serde_json::json!({
        "name": full_name.split('/').next().unwrap_or(full_name),
        "full_name": full_name,
        "description": null,
        "clone_url": format!("https://codeberg.org/{}.git", full_name),
        "ssh_url": format!("git@codeberg.org:{}.git", full_name),
        "html_url": format!("https://codeberg.org/{}", full_name),
        "fork": false,
        "private": false,
        "default_branch": "main",
        "stars_count": stars,
        "updated_at": updated,
    })
}

fn provider(server: &MockServer) -> GiteaStyleProvider {
    GiteaStyleProvider::with_url_codeberg(server.base_url())
}

#[tokio::test]
async fn authenticate_uses_full_name_for_display() {
    ensure_test_tokens();
    let server = MockServer::start_async().await;
    server.mock(|when, then| {
        when.method(GET).path("/user").header("Authorization", "token cb_test");
        then.status(200).body(r#"{"login":"alice","full_name":"Alice Doe","avatar_url":"https://codeberg.org/a.png"}"#);
    });

    let account = provider(&server).authenticate("cb_test").await.expect("auth");
    assert_eq!(account.provider, "codeberg");
    assert_eq!(account.username, "alice");
    assert_eq!(account.display_name, "Alice Doe");
    assert_eq!(account.token_key, "codeberg:alice");
}

#[tokio::test]
async fn authenticate_falls_back_to_login_when_full_name_missing() {
    ensure_test_tokens();
    let server = MockServer::start_async().await;
    server.mock(|when, then| {
        when.method(GET).path("/user");
        then.status(200).body(r#"{"login":"alice","full_name":null}"#);
    });

    let account = provider(&server).authenticate("cb_test").await.expect("auth");
    assert_eq!(account.display_name, "alice");
}

#[tokio::test]
async fn list_repos_paginates_with_data_envelope_and_limit_50() {
    let server = MockServer::start_async().await;
    let account = test_account("codeberg", "alice");

    let page1: Vec<_> = (0..50)
        .map(|i| cb_repo_json(&format!("u/r{}", i), "2020-01-01T00:00:00Z", i))
        .collect();
    let page2 = vec![cb_repo_json("u/new", "2024-06-01T00:00:00Z", 999)];

    server.mock(|when, then| {
        when.method(GET)
            .path("/repos/search")
            .query_param("page", "1")
            .query_param("limit", "50");
        then.status(200).body(serde_json::json!({"data": page1}).to_string());
    });
    server.mock(|when, then| {
        when.method(GET)
            .path("/repos/search")
            .query_param("page", "2")
            .query_param("limit", "50");
        then.status(200).body(serde_json::json!({"data": page2}).to_string());
    });

    let repos = provider(&server).list_repos(&account).await.expect("list");
    assert_eq!(repos.len(), 51);
    // Codeberg does NOT sort — items appear in page order.
    assert_eq!(repos[0].full_name, "u/r0");
    assert!(repos.iter().any(|r| r.full_name == "u/new"));
    assert_eq!(repos[50].stars, 999);
}

#[tokio::test]
async fn search_repos_unwraps_data_envelope() {
    let server = MockServer::start_async().await;
    let account = test_account("codeberg", "alice");

    server.mock(|when, then| {
        when.method(GET)
            .path("/repos/search")
            .query_param("q", "foo");
        then.status(200).body(
            serde_json::json!({"data": [cb_repo_json("u/foo", "2024-01-01T00:00:00Z", 42)]})
                .to_string(),
        );
    });

    let repos = provider(&server)
        .search_repos(&account, "foo")
        .await
        .expect("search");
    assert_eq!(repos.len(), 1);
    assert_eq!(repos[0].stars, 42);
}

#[tokio::test]
async fn create_fork_posts_to_repos_forks() {
    let server = MockServer::start_async().await;
    let account = test_account("codeberg", "alice");

    server.mock(|when, then| {
        when.method(POST).path("/repos/owner/repo/forks");
        then.status(200).body(cb_repo_json("alice/repo", "2024-01-01T00:00:00Z", 0).to_string());
    });

    let fork = provider(&server)
        .create_fork(&account, "owner", "repo")
        .await
        .expect("fork");
    assert_eq!(fork.full_name, "alice/repo");
}

#[tokio::test]
async fn create_pull_request_sends_head_colon_branch_for_cross_owner() {
    let server = MockServer::start_async().await;
    let account = test_account("codeberg", "alice");

    let req = CreatePullRequestRequest {
        owner: "owner".into(),
        repo: "repo".into(),
        title: "T".into(),
        body: "B".into(),
        head_owner: "alice".into(),
        head_branch: "feature".into(),
        base_branch: "main".into(),
        draft: false,
    };

    server.mock(|when, then| {
        when.method(POST)
            .path("/repos/owner/repo/pulls")
            .body_includes("\"head\":\"alice:feature\"");
        then.status(200).body(serde_json::json!({
            "number": 3, "title": "T",
            "html_url": "https://codeberg.org/owner/repo/pulls/3",
            "state": "open",
            "head": {"ref": "feature"}, "draft": false
        }).to_string());
    });

    let pr = provider(&server)
        .create_pull_request(&account, &req)
        .await
        .expect("pr");
    assert_eq!(pr.number, 3);
    assert_eq!(pr.head_branch.as_deref(), Some("feature"));
}

#[tokio::test]
async fn list_pull_requests_uses_limit_50() {
    let server = MockServer::start_async().await;
    let account = test_account("codeberg", "alice");

    server.mock(|when, then| {
        when.method(GET)
            .path("/repos/owner/repo/pulls")
            .query_param("limit", "50");
        then.status(200).body(
            serde_json::json!([{"number":1,"title":"a","html_url":"","state":"open","head":{"ref":"f"},"draft":false}])
                .to_string(),
        );
    });

    let prs = provider(&server)
        .list_pull_requests(&account, "owner", "repo")
        .await
        .expect("list prs");
    assert_eq!(prs.len(), 1);
}

#[tokio::test]
async fn list_branches_extracts_names() {
    let server = MockServer::start_async().await;
    let account = test_account("codeberg", "alice");

    server.mock(|when, then| {
        when.method(GET).path("/repos/owner/repo/branches");
        then.status(200).body(
            serde_json::json!([{"name": "main"}, {"name": "dev"}]).to_string(),
        );
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
    let account = test_account("codeberg", "alice");

    server.mock(|when, then| {
        when.method(GET).path("/repos/search");
        then.status(401).body("unauthorized");
    });

    provider(&server)
        .list_repos(&account)
        .await
        .expect_err("should fail");
}

#[test]
fn repo_url_uses_web_url() {
    let cb = GiteaStyleProvider::new_codeberg();
    assert_eq!(cb.repo_url("foo/bar"), "https://codeberg.org/foo/bar");
}
