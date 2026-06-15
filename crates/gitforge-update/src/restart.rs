use gpui::App;

/// Relaunch GitForge to run the updated binary.
///
/// The updated binary path is configured at install time via `App::set_restart_path`,
/// so we delegate to GPUI's restart. GPUI spawns a detached watcher that waits for this
/// process to exit before relaunching, avoiding races on shared resources (app_id / DBus)
/// with the still-running instance.
pub fn restart_to_apply_update(cx: &mut App) {
    tracing::info!("restarting to apply update");
    cx.restart();
}
