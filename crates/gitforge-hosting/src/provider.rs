use crate::models::{CreatePullRequestRequest, HostingAccount, PullRequest, RemoteRepo};
use anyhow::Result;
use async_trait::async_trait;

#[async_trait]
pub trait HostingProvider: Send + Sync {
    fn name(&self) -> &str;
    fn base_url(&self) -> &str;

    async fn authenticate(&self, token: &str) -> Result<HostingAccount>;
    async fn list_repos(&self, account: &HostingAccount) -> Result<Vec<RemoteRepo>>;
    async fn list_org_repos(&self, account: &HostingAccount, org: &str) -> Result<Vec<RemoteRepo>>;

    async fn search_repos(&self, account: &HostingAccount, query: &str) -> Result<Vec<RemoteRepo>>;
    async fn create_fork(
        &self,
        account: &HostingAccount,
        owner: &str,
        repo: &str,
    ) -> Result<RemoteRepo>;
    fn file_url(&self, repo_full_name: &str, sha: &str, path: &str, line: Option<u32>) -> String;
    fn commit_url(&self, repo_full_name: &str, sha: &str) -> String;
    fn repo_url(&self, repo_full_name: &str) -> String;

    async fn create_pull_request(
        &self,
        account: &HostingAccount,
        req: &CreatePullRequestRequest,
    ) -> Result<PullRequest>;

    async fn list_pull_requests(
        &self,
        account: &HostingAccount,
        owner: &str,
        repo: &str,
    ) -> Result<Vec<PullRequest>>;

    async fn list_branches(
        &self,
        account: &HostingAccount,
        owner: &str,
        repo: &str,
    ) -> Result<Vec<String>>;
}
