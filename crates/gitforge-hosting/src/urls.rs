use crate::provider::HostingProvider;
use crate::{CodebergProvider, GitHubProvider, GitLabProvider};

pub fn normalize_remote_url(url: &str) -> String {
    let url = url.trim();
    if url.starts_with("git@") {
        let without_prefix = url.strip_prefix("git@").unwrap_or(url);
        without_prefix
            .replacen(':', "/", 1)
            .trim_end_matches(".git")
            .to_string()
    } else if url.starts_with("ssh://") {
        url.trim_end_matches(".git")
            .replacen("ssh://", "", 1)
            .replacen(':', "/", 1)
            .to_string()
    } else if url.starts_with("https://") || url.starts_with("http://") {
        url.trim_end_matches(".git").to_string()
    } else {
        url.to_string()
    }
}

pub fn detect_provider(url: &str) -> Option<Box<dyn HostingProvider>> {
    if url.contains("github.com") {
        Some(Box::new(GitHubProvider::new()))
    } else if url.contains("gitlab.com") || url.contains("gitlab") {
        Some(Box::new(GitLabProvider::new()))
    } else if url.contains("codeberg.org") {
        Some(Box::new(CodebergProvider::new()))
    } else {
        None
    }
}

pub fn extract_repo_full_name(url: &str) -> String {
    let parts: Vec<&str> = url.split('/').collect();
    let len = parts.len();
    if len >= 2 {
        format!("{}/{}", parts[len - 2], parts[len - 1])
    } else {
        url.to_string()
    }
}
