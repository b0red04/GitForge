use std::env::consts::ARCH;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context as _, Result};
use gpui::{App, AppContext as _, AsyncApp, Context, Entity, Global, Task, Window, actions};
use semver::Version;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;

use crate::detect::{auto_update_supported, update_block_reason};
use crate::github::{
    ReleaseAsset, fetch_latest_release, is_newer_version, make_http_client, select_checksum_url,
    verify_downloaded_checksum,
};
use crate::install::{install_release_linux, linux_rsync_install_hint};
use crate::restart::set_pending_restart_path;

const POLL_INTERVAL: Duration = Duration::from_secs(60 * 60);
const SHOULD_SHOW_UPDATE_NOTIFICATION_KEY: &str = "gitforge-update-should-show-notification";
const PENDING_UPDATE_VERSION_KEY: &str = "gitforge-update-pending-version";

actions!(
    gitforge_update,
    [
        /// Checks for available updates.
        Check,
    ]
);

#[derive(Debug)]
struct MissingDependencyError(String);

impl std::fmt::Display for MissingDependencyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for MissingDependencyError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VersionCheckType {
    Semantic(Version),
}

#[derive(Clone, Debug)]
pub enum AutoUpdateStatus {
    Idle,
    Checking,
    Downloading { version: VersionCheckType },
    Installing { version: VersionCheckType },
    Updated { version: VersionCheckType },
    Errored { error: Arc<anyhow::Error> },
}

impl PartialEq for AutoUpdateStatus {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Idle, Self::Idle) => true,
            (Self::Checking, Self::Checking) => true,
            (Self::Downloading { version: v1 }, Self::Downloading { version: v2 }) => v1 == v2,
            (Self::Installing { version: v1 }, Self::Installing { version: v2 }) => v1 == v2,
            (Self::Updated { version: v1 }, Self::Updated { version: v2 }) => v1 == v2,
            (Self::Errored { error: e1 }, Self::Errored { error: e2 }) => {
                e1.to_string() == e2.to_string()
            }
            _ => false,
        }
    }
}

impl AutoUpdateStatus {
    pub fn is_updated(&self) -> bool {
        matches!(self, Self::Updated { .. })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UpdateCheckType {
    Automatic,
    Manual,
}

impl UpdateCheckType {
    pub fn is_manual(self) -> bool {
        self == Self::Manual
    }
}

pub struct AutoUpdater {
    status: AutoUpdateStatus,
    current_version: Version,
    http_client: reqwest::Client,
    pending_poll: Option<Task<Option<()>>>,
    polling_task: Option<Task<Result<()>>>,
    update_check_type: UpdateCheckType,
}

#[derive(Default)]
struct GlobalAutoUpdate(Option<Entity<AutoUpdater>>);

impl Global for GlobalAutoUpdate {}

pub(crate) struct GlobalUpdateState(pub(crate) std::collections::HashMap<String, String>);

impl Default for GlobalUpdateState {
    fn default() -> Self {
        Self(std::collections::HashMap::new())
    }
}

impl Global for GlobalUpdateState {}

pub fn init(cx: &mut App) {
    let current_version = installed_version();
    let http_client = make_http_client();
    let auto_updater = cx.new(|_cx| AutoUpdater {
        status: AutoUpdateStatus::Idle,
        current_version,
        http_client,
        pending_poll: None,
        polling_task: None,
        update_check_type: UpdateCheckType::Automatic,
    });
    cx.set_global(GlobalAutoUpdate(Some(auto_updater)));
    cx.set_global(GlobalUpdateState(std::collections::HashMap::new()));
}

pub fn set_auto_update_enabled(enabled: bool, cx: &mut App) {
    let Some(updater) = AutoUpdater::get(cx) else {
        return;
    };
    updater.update(cx, |updater, cx| {
        if enabled && auto_update_supported() {
            if updater.polling_task.is_none() {
                updater.polling_task = Some(updater.start_polling(cx));
            }
        } else {
            updater.polling_task.take();
        }
    });
}

pub fn check(_: &Check, window: &mut Window, cx: &mut App) {
    if let Some(reason) = update_block_reason() {
        drop(window.prompt(
            gpui::PromptLevel::Info,
            "Could not check for updates",
            Some(reason.message()),
            &["OK"],
            cx,
        ));
        return;
    }

    if let Some(updater) = AutoUpdater::get(cx) {
        if updater.read(cx).status().is_updated() {
            drop(window.prompt(
                gpui::PromptLevel::Info,
                "Update ready",
                Some("Restart GitForge to finish applying the update."),
                &["OK"],
                cx,
            ));
            return;
        }
        updater.update(cx, |updater, cx| updater.poll(UpdateCheckType::Manual, cx));
    } else {
        drop(window.prompt(
            gpui::PromptLevel::Info,
            "Could not check for updates",
            Some("Auto-updater is not initialized."),
            &["OK"],
            cx,
        ));
    }
}

pub fn notify_if_app_was_updated<F>(cx: &mut App, mut on_notify: F)
where
    F: FnMut(&str, &mut App),
{
    let Some(updater) = AutoUpdater::get(cx) else {
        return;
    };

    let should_show = updater.read(cx).should_show_update_notification(cx);
    if !should_show {
        return;
    }

    let version = AutoUpdater::pending_update_notification_version(cx)
        .unwrap_or_else(|| updater.read(cx).current_version().to_string());
    on_notify(&version, cx);
    updater.update(cx, |updater, cx| updater.clear_update_notification(cx));
}

impl AutoUpdater {
    pub fn get(cx: &mut App) -> Option<Entity<Self>> {
        cx.default_global::<GlobalAutoUpdate>().0.clone()
    }

    pub fn start_polling(&self, cx: &mut Context<Self>) -> Task<Result<()>> {
        cx.spawn(async move |this, cx| {
            loop {
                this.update(cx, |this, cx| this.poll(UpdateCheckType::Automatic, cx))?;
                cx.background_executor().timer(POLL_INTERVAL).await;
            }
        })
    }

    pub fn update_check_type(&self) -> UpdateCheckType {
        self.update_check_type
    }

    pub fn poll(&mut self, check_type: UpdateCheckType, cx: &mut Context<Self>) {
        if matches!(self.status, AutoUpdateStatus::Updated { .. })
            && check_type == UpdateCheckType::Automatic
        {
            return;
        }

        if self.pending_poll.is_some() {
            if self.update_check_type == UpdateCheckType::Automatic {
                self.update_check_type = check_type;
                cx.notify();
            }
            return;
        }
        self.update_check_type = check_type;
        cx.notify();

        let status_before_poll = self.status.clone();
        self.pending_poll = Some(cx.spawn(async move |this, cx| {
            let result = Self::update(this.upgrade()?, cx).await;
            this.update(cx, |this, cx| {
                this.pending_poll = None;
                if let Err(error) = result {
                    let is_missing_dependency =
                        error.downcast_ref::<MissingDependencyError>().is_some();
                    this.status = match check_type {
                        UpdateCheckType::Automatic if is_missing_dependency => {
                            tracing::warn!("auto-update: {error}");
                            AutoUpdateStatus::Errored {
                                error: Arc::new(error),
                            }
                        }
                        UpdateCheckType::Automatic if status_before_poll.is_updated() => {
                            tracing::info!("auto-update check failed while update is pending restart: {error:?}");
                            status_before_poll.clone()
                        }
                        UpdateCheckType::Automatic => {
                            tracing::info!("auto-update check failed: {error:?}");
                            AutoUpdateStatus::Idle
                        }
                        UpdateCheckType::Manual if status_before_poll.is_updated() => {
                            tracing::info!("manual update check skipped; restart is required");
                            status_before_poll.clone()
                        }
                        UpdateCheckType::Manual => {
                            tracing::error!("auto-update failed: {error:?}");
                            AutoUpdateStatus::Errored {
                                error: Arc::new(error),
                            }
                        }
                    };
                    cx.notify();
                }
            })
            .ok();
            Some(())
        }));
    }

    pub fn current_version(&self) -> Version {
        self.current_version.clone()
    }

    pub fn status(&self) -> AutoUpdateStatus {
        self.status.clone()
    }

    pub fn dismiss(&mut self, cx: &mut Context<Self>) -> bool {
        if matches!(self.status, AutoUpdateStatus::Idle) {
            return false;
        }
        self.status = AutoUpdateStatus::Idle;
        cx.notify();
        true
    }

    fn should_show_update_notification(&self, cx: &App) -> bool {
        cx.try_global::<GlobalUpdateState>()
            .and_then(|state| state.0.get(SHOULD_SHOW_UPDATE_NOTIFICATION_KEY).cloned())
            .is_some_and(|value| value == "true")
    }

    fn set_should_show_update_notification(&mut self, show: bool, cx: &mut App) {
        let state = cx.default_global::<GlobalUpdateState>();
        if show {
            state.0.insert(
                SHOULD_SHOW_UPDATE_NOTIFICATION_KEY.to_string(),
                "true".into(),
            );
            if let AutoUpdateStatus::Updated {
                version: VersionCheckType::Semantic(version),
            } = &self.status
            {
                state
                    .0
                    .insert(PENDING_UPDATE_VERSION_KEY.to_string(), version.to_string());
            }
        } else {
            state.0.remove(SHOULD_SHOW_UPDATE_NOTIFICATION_KEY);
            state.0.remove(PENDING_UPDATE_VERSION_KEY);
        }
    }

    fn clear_update_notification(&mut self, cx: &mut App) {
        self.set_should_show_update_notification(false, cx);
    }

    pub fn pending_update_notification_version(cx: &App) -> Option<String> {
        cx.try_global::<GlobalUpdateState>()
            .and_then(|state| state.0.get(PENDING_UPDATE_VERSION_KEY).cloned())
    }

    async fn update(this: Entity<Self>, cx: &mut AsyncApp) -> Result<()> {
        let (http_client, installed_version, previous_status) = this.read_with(cx, |this, _| {
            (
                this.http_client.clone(),
                this.current_version.clone(),
                this.status.clone(),
            )
        })?;

        if matches!(previous_status, AutoUpdateStatus::Updated { .. }) {
            return Ok(());
        }

        Self::check_dependencies()?;

        let _ = this.update(cx, |this, cx| {
            this.status = AutoUpdateStatus::Checking;
            tracing::info!("Auto Update: checking for updates");
            cx.notify();
        });

        let release = fetch_latest_release(&http_client).await?;
        let asset = crate::github::select_update_asset(&release, ARCH)?;
        let newer_version = Self::check_if_fetched_version_is_newer(
            &installed_version,
            &asset.version,
            previous_status.clone(),
        )?;

        let Some(newer_version) = newer_version else {
            let _ = this.update(cx, |this, cx| {
                let status = match previous_status {
                    AutoUpdateStatus::Updated { .. } => previous_status,
                    _ => AutoUpdateStatus::Idle,
                };
                this.status = status;
                cx.notify();
            });
            return Ok(());
        };

        let _ = this.update(cx, |this, cx| {
            this.status = AutoUpdateStatus::Downloading {
                version: newer_version.clone(),
            };
            cx.notify();
        });

        let installer_dir = tempfile::Builder::new()
            .prefix("gitforge-auto-update")
            .tempdir()
            .context("failed to create installer dir")?;
        let target_path = installer_dir.path().join("gitforge.tar.gz");
        download_release(&target_path, &asset, &http_client)
            .await
            .with_context(|| format!("failed to download update to {}", target_path.display()))?;
        verify_downloaded_checksum(
            &target_path,
            select_checksum_url(&release, ARCH)?,
            &http_client,
        )
        .await
        .context("downloaded update failed checksum verification")?;

        let _ = this.update(cx, |this, cx| {
            this.status = AutoUpdateStatus::Installing {
                version: newer_version.clone(),
            };
            cx.notify();
        });

        let running_app_path = cx.update(|cx| cx.app_path())??;
        let new_binary_path = install_release_linux(&target_path, running_app_path)
            .await
            .with_context(|| format!("failed to install update at: {}", target_path.display()))?;

        if let Some(new_binary_path) = new_binary_path {
            let _ = cx.update(|cx| {
                cx.set_restart_path(new_binary_path.clone());
                set_pending_restart_path(new_binary_path, cx);
            });
        }

        let _ = this.update(cx, |this, cx| {
            this.status = AutoUpdateStatus::Updated {
                version: newer_version,
            };
            this.set_should_show_update_notification(true, cx);
            cx.notify();
        });
        Ok(())
    }

    fn check_if_fetched_version_is_newer(
        installed_version: &Version,
        fetched_version: &Version,
        status: AutoUpdateStatus,
    ) -> Result<Option<VersionCheckType>> {
        if let AutoUpdateStatus::Updated {
            version: VersionCheckType::Semantic(cached_version),
        } = status
        {
            return Ok(is_newer_version(&cached_version, fetched_version)
                .then(|| VersionCheckType::Semantic(fetched_version.clone())));
        }

        Ok(is_newer_version(installed_version, fetched_version)
            .then(|| VersionCheckType::Semantic(fetched_version.clone())))
    }

    fn check_dependencies() -> Result<()> {
        if which::which("rsync").is_err() {
            let install_hint = linux_rsync_install_hint();
            return Err(MissingDependencyError(format!(
                "rsync is required for auto-updates but is not installed. {install_hint}"
            ))
            .into());
        }
        Ok(())
    }
}

async fn download_release(
    target_path: &PathBuf,
    release: &ReleaseAsset,
    client: &reqwest::Client,
) -> Result<()> {
    let mut target_file = File::create(target_path).await?;
    let response = client
        .get(&release.url)
        .send()
        .await
        .context("failed to download update")?;
    anyhow::ensure!(
        response.status().is_success(),
        "failed to download update: {:?}",
        response.status()
    );
    let bytes = response
        .bytes()
        .await
        .context("failed to read update body")?;
    target_file.write_all(&bytes).await?;
    tracing::info!("downloaded update. path:{target_path:?}");
    Ok(())
}

fn installed_version() -> Version {
    env!("CARGO_PKG_VERSION")
        .parse()
        .expect("workspace version must be valid semver")
}
