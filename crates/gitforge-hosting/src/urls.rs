use crate::provider::HostingProvider;
use crate::{GitLabProvider, GiteaStyleProvider};

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
    detect_provider_id(url).map(|id| match id {
        "github" => Box::new(GiteaStyleProvider::new_github()) as Box<dyn HostingProvider>,
        "gitlab" => Box::new(GitLabProvider::new()),
        "codeberg" => Box::new(GiteaStyleProvider::new_codeberg()),
        _ => unreachable!(),
    })
}

pub fn detect_provider_id(url: &str) -> Option<&'static str> {
    let host = url_host(url);
    if host == "github.com" || host == "www.github.com" {
        Some("github")
    } else if host == "gitlab.com" || host.contains("gitlab") {
        Some("gitlab")
    } else if host == "codeberg.org" {
        Some("codeberg")
    } else {
        None
    }
}

pub fn provider_label(id: &str) -> &str {
    match id {
        "github" => "GitHub",
        "gitlab" => "GitLab",
        "codeberg" => "Codeberg",
        _ => id,
    }
}

/// Extracts the host component from a (possibly normalised) remote URL.
///
/// Handles `https://host/...`, `http://host/...`, and bare `host/...`
/// forms (the latter arising from SSH URLs after `normalize_remote_url`).
fn url_host(url: &str) -> &str {
    let url = url
        .trim_start_matches("https://")
        .trim_start_matches("http://");
    let url = url.strip_prefix("git@").unwrap_or(url);
    url.split('/').next().unwrap_or(url)
}

pub fn split_repo_full_name(full_name: &str) -> Option<(&str, &str)> {
    full_name.rsplit_once('/')
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
