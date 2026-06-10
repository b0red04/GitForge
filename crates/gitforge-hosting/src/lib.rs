pub mod codeberg;
pub mod github;
pub mod gitlab;
pub mod models;
pub mod provider;
pub mod urls;

pub use codeberg::CodebergProvider;
pub use github::GitHubProvider;
pub use gitlab::GitLabProvider;
pub use models::{HostingAccount, RemoteRepo};
pub use provider::HostingProvider;

pub fn get_provider(name: &str) -> Option<Box<dyn HostingProvider>> {
    match name {
        "github" => Some(Box::new(GitHubProvider::new())),
        "gitlab" => Some(Box::new(GitLabProvider::new())),
        "codeberg" => Some(Box::new(CodebergProvider::new())),
        _ => None,
    }
}

pub fn find_account<'a>(
    accounts: &'a [HostingAccount],
    provider: &str,
) -> Option<&'a HostingAccount> {
    accounts.iter().find(|a| a.provider == provider)
}
