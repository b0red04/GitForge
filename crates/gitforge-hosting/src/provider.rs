use crate::error::HostingResult;
use crate::models::{CreatePullRequestRequest, HostingAccount, PullRequest, RemoteRepo};
use async_trait::async_trait;

#[async_trait]
pub trait HostingProvider: Send + Sync {
    fn name(&self) -> &str;
    fn base_url(&self) -> &str;

    async fn authenticate(&self, token: &str) -> HostingResult<HostingAccount>;
    async fn list_repos(&self, account: &HostingAccount) -> HostingResult<Vec<RemoteRepo>>;

    async fn search_repos(
        &self,
        account: &HostingAccount,
        query: &str,
    ) -> HostingResult<Vec<RemoteRepo>>;
    async fn create_fork(
        &self,
        account: &HostingAccount,
        owner: &str,
        repo: &str,
    ) -> HostingResult<RemoteRepo>;
    fn repo_url(&self, repo_full_name: &str) -> String;

    async fn create_pull_request(
        &self,
        account: &HostingAccount,
        req: &CreatePullRequestRequest,
    ) -> HostingResult<PullRequest>;

    async fn list_pull_requests(
        &self,
        account: &HostingAccount,
        owner: &str,
        repo: &str,
    ) -> HostingResult<Vec<PullRequest>>;

    async fn list_branches(
        &self,
        account: &HostingAccount,
        owner: &str,
        repo: &str,
    ) -> HostingResult<Vec<String>>;
}
