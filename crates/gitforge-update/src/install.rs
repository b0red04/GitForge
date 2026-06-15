use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context as _, Result};
use tempfile::TempDir;
use tokio::process::Command;
use tokio::time::timeout;

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

#[cfg(not(target_os = "windows"))]
pub async fn install_release_linux(
    downloaded_tar_gz: &Path,
    running_app_path: PathBuf,
) -> Result<Option<PathBuf>> {
    let installer_dir = InstallerDir::new()?;

    let extracted = installer_dir.path().join("extracted");
    tokio::fs::create_dir_all(&extracted)
        .await
        .context("failed to create directory into which to extract update")?;

    let output = timeout(
        Duration::from_secs(120),
        Command::new("tar")
            .arg("-xzf")
            .arg(downloaded_tar_gz)
            .arg("-C")
            .arg(&extracted)
            .output(),
    )
    .await
    .context("tar timed out after 120 seconds")?
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

    let output = timeout(
        Duration::from_secs(300),
        Command::new("rsync")
            .args(["-av", "--delete"])
            .arg(&from)
            .arg(&to)
            .output(),
    )
    .await
    .context("rsync timed out after 300 seconds")?
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

#[cfg(not(target_os = "windows"))]
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
        id == "fedora" || id == "rhel" || id == "centos" || id == "rocky" || id == "almalinux"
    }) {
        return "Install it with: sudo dnf install rsync";
    }
    if distribution_ids.iter().any(|id| id == "nixos") {
        return "Install pkgs.rsync from nixpkgs";
    }

    "Please install rsync using your package manager"
}

/// Windows installer: extracts the downloaded zip into `<install_dir>/install/`,
/// writes a `.pending` flag file, and returns the path to the update helper
/// that should be spawned after the app exits.
///
/// The helper (a separate binary) performs the actual file swap using the
/// Windows Restart Manager to release locked handles. This two-stage approach
/// is necessary because Windows does not allow overwriting a running `.exe`.
#[cfg(target_os = "windows")]
pub async fn install_release_windows(
    downloaded_zip: &Path,
    running_app_path: PathBuf,
) -> Result<Option<PathBuf>> {
    let install_dir = running_app_path
        .parent()
        .context("running binary has no parent directory")?
        .to_path_buf();

    let staging = install_dir.join("install");
    let _ = tokio::fs::remove_dir_all(&staging).await;
    tokio::fs::create_dir_all(&staging)
        .await
        .context("failed to create install staging directory")?;

    let zip_data = std::fs::read(downloaded_zip)
        .with_context(|| format!("failed to read zip at {}", downloaded_zip.display()))?;
    extract_zip(&zip_data, &staging)?;

    // Write the pending-update flag so `finalize_auto_update_on_quit` knows
    // there is a staged update when the app quits without restarting.
    let updates_dir = install_dir.join("updates");
    tokio::fs::create_dir_all(&updates_dir).await.ok();
    tokio::fs::write(updates_dir.join(".pending"), b"")
        .await
        .ok();

    let helper_path = install_dir.join("gitforge-update-helper.exe");
    if !helper_path.exists() {
        anyhow::bail!(
            "update helper not found at {} — the installation may be corrupted",
            helper_path.display()
        );
    }
    Ok(Some(helper_path))
}

/// Extract a zip archive to the given directory. Synchronous because the
/// `zip` crate's reader is not async.
#[cfg(target_os = "windows")]
fn extract_zip(zip_data: &[u8], dest: &Path) -> Result<()> {
    let cursor = std::io::Cursor::new(zip_data);
    let mut archive = zip::ZipArchive::new(cursor).context("failed to open zip archive")?;

    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .with_context(|| format!("failed to read zip entry {i}"))?;
        let outpath = match file.enclosed_name() {
            Some(path) => dest.join(path),
            None => continue,
        };

        if file.is_dir() {
            std::fs::create_dir_all(&outpath)
                .with_context(|| format!("failed to create dir {}", outpath.display()))?;
        } else {
            if let Some(parent) = outpath.parent() {
                std::fs::create_dir_all(parent).ok();
            }
            let mut buf = Vec::new();
            std::io::Read::read_to_end(&mut file, &mut buf)
                .with_context(|| format!("failed to read zip file entry {i}"))?;
            std::fs::write(&outpath, &buf)
                .with_context(|| format!("failed to write {}", outpath.display()))?;
        }
    }
    Ok(())
}

/// Called from GPUI's `on_app_quit` on Windows. If a staged update is
/// pending (the `.pending` flag file exists in `updates\`), spawns the
/// update helper with `--launch false` so the file swap completes without
/// relaunching GitForge.
#[cfg(target_os = "windows")]
pub fn finalize_auto_update_on_quit() {
    let Some(install_dir) = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
    else {
        return;
    };

    let flag_file = install_dir.join("updates").join(".pending");
    if !flag_file.exists() {
        return;
    }

    let helper = install_dir.join("gitforge-update-helper.exe");
    if !helper.exists() {
        let _ = std::fs::remove_file(&flag_file);
        return;
    }

    let _ = std::process::Command::new(&helper)
        .arg("--launch")
        .arg("false")
        .spawn();
}
