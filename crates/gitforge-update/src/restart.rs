use gpui::App;

/// Relaunch GitForge to run the updated binary.
///
/// The updated binary path is configured at install time via `App::set_restart_path`,
/// so we delegate to GPUI's restart. GPUI spawns a detached watcher that waits for this
/// process to exit before relaunching, avoiding races on shared resources (app_id / DBus)
/// with the still-running instance.
pub fn restart_to_apply_update(cx: &mut App) {
    tracing::info!("restarting to apply update");

    // Close windows before restarting, matching Zed's `workspace::reload` flow.
    let windows = cx.windows();
    cx.defer(move |cx| {
        for window in windows {
            let _ = window.update(cx, |_, window, _| window.remove_window());
        }
        cx.restart();
    });
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    use gpui::TestAppContext;

    use super::*;

    #[gpui::test]
    fn restart_to_apply_update_reaches_restart(cx: &mut TestAppContext) {
        let restarted = Arc::new(AtomicBool::new(false));
        let flag = restarted.clone();
        let mut restart_subscription = None;
        cx.update(|app| {
            app.set_restart_path("/tmp/gitforge-restart-test".into());
            restart_subscription = Some(app.on_app_restart(move |_| {
                flag.store(true, Ordering::SeqCst);
            }));
        });

        cx.update(restart_to_apply_update);
        cx.run_until_parked();

        assert!(
            restarted.load(Ordering::SeqCst),
            "restart_to_apply_update should defer window teardown and call cx.restart()"
        );
    }
}
