use std::path::PathBuf;
use std::process::Stdio;

use gpui::App;

const PENDING_RESTART_PATH_KEY: &str = "gitforge-update-pending-restart-path";

pub fn set_pending_restart_path(path: PathBuf, cx: &mut App) {
    let state = cx.default_global::<super::auto_update::GlobalUpdateState>();
    state.0.insert(
        PENDING_RESTART_PATH_KEY.to_string(),
        path.display().to_string(),
    );
}

pub fn pending_restart_path(cx: &App) -> Option<PathBuf> {
    cx.try_global::<super::auto_update::GlobalUpdateState>()
        .and_then(|state| state.0.get(PENDING_RESTART_PATH_KEY).cloned())
        .map(PathBuf::from)
}

/// Relaunch GitForge to run the updated binary.
pub fn restart_to_apply_update(cx: &mut App) {
    let binary = pending_restart_path(cx).or_else(|| cx.app_path().ok());

    let Some(binary) = binary else {
        tracing::warn!(
            "restart requested but no binary path is available; falling back to GPUI restart"
        );
        cx.restart();
        return;
    };

    tracing::info!("restarting to apply update from {:?}", binary);

    match std::process::Command::new(&binary)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(_) => cx.quit(),
        Err(error) => {
            tracing::error!("failed to spawn updated binary at {:?}: {error}", binary);
            cx.set_restart_path(binary);
            cx.restart();
        }
    }
}
