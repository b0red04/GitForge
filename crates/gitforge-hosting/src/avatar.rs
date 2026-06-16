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
    avatar_cache_dir().join(format!("{provider}-{username}.png"))
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
    std::fs::write(path, bytes).context("failed to write avatar cache file")?;
    Ok(())
}
