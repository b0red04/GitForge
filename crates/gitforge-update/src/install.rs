use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result};
use tempfile::TempDir;
use tokio::process::Command;

use crate::detect::{app_folder_name, binary_suffix, default_install_prefix};

struct InstallerDir(TempDir);

impl InstallerDir {
  fn new() -> Result<Self> {
    Ok(Self(
      tempfile::Builder::new()
        .prefix("gitforge-auto-update")
        .tempdir()?,
    ))
  }

  fn path(&self) -> &Path {
    self.0.path()
  }
}

pub async fn install_release_linux(
  downloaded_tar_gz: &Path,
  running_app_path: PathBuf,
) -> Result<Option<PathBuf>> {
  let installer_dir = InstallerDir::new()?;

  let extracted = installer_dir.path().join("extracted");
  tokio::fs::create_dir_all(&extracted)
    .await
    .context("failed to create directory into which to extract update")?;

  let output = Command::new("tar")
    .arg("-xzf")
    .arg(downloaded_tar_gz)
    .arg("-C")
    .arg(&extracted)
    .output()
    .await
    .context("failed to run tar")?;

  anyhow::ensure!(
    output.status.success(),
    "failed to extract {:?} to {:?}: {:?}",
    downloaded_tar_gz,
    extracted,
    String::from_utf8_lossy(&output.stderr)
  );

  let folder_name = app_folder_name();
  let from = extracted.join(folder_name);
  let mut to = default_install_prefix();

  let expected_suffix = format!("{folder_name}{}", binary_suffix());
  if let Some(prefix) = running_app_path
    .to_str()
    .and_then(|path| path.strip_suffix(&expected_suffix))
  {
    to = PathBuf::from(prefix);
  }

  let output = Command::new("rsync")
    .args(["-av", "--delete"])
    .arg(&from)
    .arg(&to)
    .output()
    .await
    .context("failed to run rsync")?;

  anyhow::ensure!(
    output.status.success(),
    "failed to copy GitForge update from {:?} to {:?}: {:?}",
    from,
    to,
    String::from_utf8_lossy(&output.stderr)
  );

  Ok(Some(to.join(expected_suffix)))
}

pub fn linux_rsync_install_hint() -> &'static str {
  let os_release = match std::fs::read_to_string("/etc/os-release") {
    Ok(os_release) => os_release,
    Err(_) => return "Please install rsync using your package manager",
  };

  let mut distribution_ids = Vec::new();
  for line in os_release.lines() {
    let trimmed = line.trim();
    if let Some(value) = trimmed.strip_prefix("ID=") {
      distribution_ids.push(value.trim_matches('"').to_ascii_lowercase());
    } else if let Some(value) = trimmed.strip_prefix("ID_LIKE=") {
      for id in value.trim_matches('"').split_whitespace() {
        distribution_ids.push(id.to_ascii_lowercase());
      }
    }
  }

  if distribution_ids.iter().any(|id| id == "arch") {
    return "Install it with: sudo pacman -S rsync";
  }
  if distribution_ids
    .iter()
    .any(|id| id == "debian" || id == "ubuntu")
  {
    return "Install it with: sudo apt install rsync";
  }
  if distribution_ids.iter().any(|id| {
    id == "fedora"
      || id == "rhel"
      || id == "centos"
      || id == "rocky"
      || id == "almalinux"
  }) {
    return "Install it with: sudo dnf install rsync";
  }
  if distribution_ids.iter().any(|id| id == "nixos") {
    return "Install pkgs.rsync from nixpkgs";
  }

  "Please install rsync using your package manager"
}
