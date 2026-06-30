pub mod avatar;
pub mod config_driven;
pub mod http;
pub mod models;
pub mod provider;
pub mod secrets;
pub mod urls;

pub use avatar::{avatar_cache_path, cached_avatar_path, ensure_avatar_cached};
pub use config_driven::ConfigDrivenProvider;
pub use models::{CreatePullRequestRequest, HostingAccount, PullRequest, RemoteRepo};
pub use provider::HostingProvider;

/// Deprecated re-export path for existing callers.
#[deprecated(note = "use config_driven module")]
pub mod gitea_style {
    pub use super::config_driven::ConfigDrivenProvider;
    #[deprecated(note = "use ConfigDrivenProvider")]
    pub type GiteaStyleProvider = super::ConfigDrivenProvider;
}

/// Type alias preserving the historical name for GitHub.
#[deprecated(note = "use ConfigDrivenProvider")]
pub type GitHubProvider = ConfigDrivenProvider;
/// Type alias preserving the historical name for Codeberg.
#[deprecated(note = "use ConfigDrivenProvider")]
pub type CodebergProvider = ConfigDrivenProvider;
/// Type alias preserving the historical name for GitLab.
#[deprecated(note = "use ConfigDrivenProvider")]
pub type GitLabProvider = ConfigDrivenProvider;

pub fn get_provider(name: &str) -> Option<Box<dyn HostingProvider>> {
    ConfigDrivenProvider::from_id(name).map(|p| Box::new(p) as Box<dyn HostingProvider>)
}
