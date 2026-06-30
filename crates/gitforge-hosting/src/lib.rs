pub mod avatar;
pub mod error;
pub mod gitea_style;
pub mod gitlab;
pub mod http;
pub mod models;
pub mod provider;
pub mod secrets;
pub mod urls;

pub use avatar::{avatar_cache_path, cached_avatar_path, ensure_avatar_cached};
pub use error::{HostingError, HostingResult};
pub use gitea_style::GiteaStyleProvider;
pub use gitlab::GitLabProvider;
pub use models::{CreatePullRequestRequest, HostingAccount, PullRequest, RemoteRepo};
pub use provider::HostingProvider;

/// Type alias preserving the historical name for GitHub.
pub type GitHubProvider = GiteaStyleProvider;
/// Type alias preserving the historical name for Codeberg.
pub type CodebergProvider = GiteaStyleProvider;

pub fn get_provider(name: &str) -> Option<Box<dyn HostingProvider>> {
    match name {
        "github" => Some(Box::new(GiteaStyleProvider::new_github())),
        "gitlab" => Some(Box::new(GitLabProvider::new())),
        "codeberg" => Some(Box::new(GiteaStyleProvider::new_codeberg())),
        _ => None,
    }
}
