use std::time::Duration;

use anyhow::{Context as _, Result};
use semver::Version;
use serde::Deserialize;

pub const GITHUB_REPO: &str = "b0red04/gitforge";
pub const LATEST_RELEASE_URL: &str =
    "https://api.github.com/repos/b0red04/gitforge/releases/latest";

#[derive(Debug, Clone, Deserialize)]
pub struct GitHubRelease {
    pub tag_name: String,
    pub assets: Vec<GitHubAsset>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GitHubAsset {
    pub name: String,
    pub browser_download_url: String,
}

#[derive(Debug, Clone)]
pub struct ReleaseAsset {
    pub version: Version,
    pub url: String,
}

pub fn parse_release(body: &str) -> Result<GitHubRelease> {
    serde_json::from_str(body).context("failed to parse GitHub release JSON")
}

pub fn tag_to_version(tag_name: &str) -> Result<Version> {
    let trimmed = tag_name.strip_prefix('v').unwrap_or(tag_name);
    trimmed
        .parse::<Version>()
        .with_context(|| format!("invalid release tag version: {tag_name}"))
}

pub fn select_update_asset(release: &GitHubRelease, arch: &str) -> Result<ReleaseAsset> {
    let version = tag_to_version(&release.tag_name)?;
    let expected_name = tarball_asset_name(&version, arch);
    let asset = release
        .assets
        .iter()
        .find(|asset| asset.name == expected_name)
        .with_context(|| format!("release asset not found: {expected_name}"))?;
    Ok(ReleaseAsset {
        version,
        url: asset.browser_download_url.clone(),
    })
}

pub fn tarball_asset_name(version: &Version, arch: &str) -> String {
    format!("GitForge-{version}-{arch}.tar.gz")
}

pub fn normalize_installed_version(mut version: Version) -> Version {
    version.pre = semver::Prerelease::EMPTY;
    version.build = semver::BuildMetadata::EMPTY;
    version
}

pub fn is_newer_version(installed: &Version, fetched: &Version) -> bool {
    let installed = normalize_installed_version(installed.clone());
    let fetched = normalize_installed_version(fetched.clone());
    fetched > installed
}

pub fn checksum_asset_name(version: &Version, arch: &str) -> String {
    format!("{}.sha256", tarball_asset_name(version, arch))
}

pub fn select_checksum_url(release: &GitHubRelease, arch: &str) -> Result<String> {
    let version = tag_to_version(&release.tag_name)?;
    let expected_name = checksum_asset_name(&version, arch);
    release
        .assets
        .iter()
        .find(|asset| asset.name == expected_name)
        .map(|asset| asset.browser_download_url.clone())
        .with_context(|| format!("release checksum asset not found: {expected_name}"))
}

pub fn make_http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent("gitforge")
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(60))
        .build()
        .expect("reqwest client build only fails for invalid builder config")
}

pub async fn verify_downloaded_checksum(
    downloaded_path: &std::path::Path,
    checksum_url: String,
    client: &reqwest::Client,
) -> Result<()> {
    use sha2::{Digest, Sha256};

    let response = client
        .get(checksum_url)
        .send()
        .await
        .context("failed to download release checksum")?;
    let status = response.status();
    let body = response
        .text()
        .await
        .context("failed to read release checksum body")?;
    anyhow::ensure!(
        status.is_success(),
        "failed to download release checksum: {status} - {body}"
    );
    let expected_hash = body
        .split_whitespace()
        .next()
        .context("release checksum file is empty")?;

    let bytes = tokio::fs::read(downloaded_path).await.with_context(|| {
        format!(
            "failed to read downloaded update at {}",
            downloaded_path.display()
        )
    })?;
    let actual_hash = format!("{:x}", Sha256::digest(bytes));
    anyhow::ensure!(
        expected_hash == actual_hash,
        "downloaded update checksum mismatch: expected {expected_hash}, got {actual_hash}"
    );
    Ok(())
}

pub async fn fetch_latest_release(client: &reqwest::Client) -> Result<GitHubRelease> {
    let response = client
        .get(LATEST_RELEASE_URL)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .context("failed to request latest GitHub release")?;
    let status = response.status();
    let body = response
        .text()
        .await
        .context("failed to read GitHub release response body")?;
    anyhow::ensure!(
        status.is_success(),
        "failed to fetch latest release: {status} - {body}"
    );
    parse_release(&body)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = r#"{
    "tag_name": "v1.2.3",
    "assets": [
      {
        "name": "GitForge-1.2.3-x86_64.tar.gz",
        "browser_download_url": "https://example.com/GitForge-1.2.3-x86_64.tar.gz"
      },
      {
        "name": "GitForge-1.2.3-aarch64.tar.gz",
        "browser_download_url": "https://example.com/GitForge-1.2.3-aarch64.tar.gz"
      }
    ]
  }"#;

    #[test]
    fn parses_release_json() {
        let release = parse_release(FIXTURE).unwrap();
        assert_eq!(release.tag_name, "v1.2.3");
        assert_eq!(release.assets.len(), 2);
    }

    #[test]
    fn selects_arch_specific_asset() {
        let release = parse_release(FIXTURE).unwrap();
        let asset = select_update_asset(&release, "x86_64").unwrap();
        assert_eq!(asset.version, Version::new(1, 2, 3));
        assert_eq!(
            asset.url,
            "https://example.com/GitForge-1.2.3-x86_64.tar.gz"
        );
    }

    #[test]
    fn compares_semver_versions() {
        let installed = Version::new(1, 0, 0);
        let fetched = Version::new(1, 0, 1);
        assert!(is_newer_version(&installed, &fetched));
        assert!(!is_newer_version(&fetched, &installed));
    }

    #[test]
    fn ignores_prerelease_and_build_metadata() {
        let mut installed = Version::new(1, 0, 0);
        installed.build = semver::BuildMetadata::new("abc").unwrap();
        let fetched = Version::new(1, 0, 0);
        assert!(!is_newer_version(&installed, &fetched));
    }

    #[test]
    fn tag_to_version_strips_v_prefix() {
        let version = tag_to_version("v2.0.1").unwrap();
        assert_eq!(version, Version::new(2, 0, 1));
    }
}
