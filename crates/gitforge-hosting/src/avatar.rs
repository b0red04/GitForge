use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::models::HostingAccount;

pub fn avatar_cache_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("gitforge")
        .join("avatars")
}

pub fn avatar_cache_path(provider: &str, username: &str) -> PathBuf {
    let provider = sanitize_cache_key(provider);
    let username = sanitize_cache_key(username);
    avatar_cache_dir().join(format!("{provider}-{username}.png"))
}

/// Reduce a free-form provider/username value to a safe filename component,
/// preventing path traversal via separators or traversal sequences. External
/// API responses (e.g. a forged `username`) feed this path, so strip anything
/// that is not alphanumeric, `-`, or `_`, falling back to a stable placeholder
/// when the result would be empty.
fn sanitize_cache_key(input: &str) -> String {
    let cleaned: String = input
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .collect();
    if cleaned.is_empty() {
        "unknown".to_string()
    } else {
        cleaned
    }
}

/// Returns the cached avatar path when the file exists on disk.
pub fn cached_avatar_path(account: &HostingAccount) -> Option<PathBuf> {
    let path = avatar_cache_path(&account.provider, &account.username);
    path.exists().then_some(path)
}

/// Downloads the avatar when missing and returns the cache path.
pub async fn ensure_avatar_cached(account: &HostingAccount) -> Result<Option<PathBuf>> {
    let path = avatar_cache_path(&account.provider, &account.username);
    if path.exists() {
        return Ok(Some(path));
    }
    let Some(url) = account.avatar_url.as_deref() else {
        return Ok(None);
    };
    download_avatar(url, &path).await?;
    Ok(Some(path))
}

pub async fn download_avatar(url: &str, path: &Path) -> Result<()> {
    let client = reqwest::Client::builder()
        .user_agent("GitForge")
        .build()
        .context("failed to create HTTP client")?;
    let response = client
        .get(url)
        .send()
        .await
        .context("avatar download failed")?
        .error_for_status()
        .context("avatar download failed")?;
    let bytes = response
        .bytes()
        .await
        .context("failed to read avatar response")?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).context("failed to create avatar cache directory")?;
    }
    // Write to a sibling temp file first, then atomically rename it into place.
    // This prevents a partial file from being treated as a valid cache entry by
    // the `path.exists()` check if the write is interrupted.
    let temp_path = path.with_extension("part");
    std::fs::write(&temp_path, bytes).context("failed to write avatar cache file")?;
    std::fs::rename(&temp_path, path).context("failed to finalize avatar cache file")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitizes_traversal_and_separators() {
        let path = avatar_cache_path("github", "../etc/passwd");
        assert!(path.ends_with("github-etcpasswd.png"));
    }

    #[test]
    fn sanitizes_empty_components_to_placeholder() {
        let path = avatar_cache_path("../", "***");
        assert!(path.ends_with("unknown-unknown.png"));
    }

    #[test]
    fn keeps_safe_characters_intact() {
        let path = avatar_cache_path("codeberg", "alice_dev-01");
        assert!(path.ends_with("codeberg-alice_dev-01.png"));
    }
}
