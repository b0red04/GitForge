//! Background-op seams. The two methods here are now thin adapters over the
//! unified `dispatch::run_op_full` shell — every operation, blocking or async,
//! routes through the one pure classifier (`dispatch::plan_dispatch`). The
//! former `dispatch_bg_result` / `dispatch_bg_result_silent` free functions
//! (and their hand-rolled 3-arm match) are gone; that logic now lives in the
//! classifier and the shell.
//!
//! See `CONTEXT.md` → "Operation Dispatch".

use gitforge_git::GitError;
use gpui::Context;

use crate::views::app::GitForgeApp;
use crate::views::ops::dispatch::{AppError, ErrorChannel, OpEffects, RemoteError};

impl GitForgeApp {
    /// Handle-less blocking-op seam: spawns `op` on the blocking pool and
    /// surfaces a toast on error (auto-toasts). Adapter over `run_op_full` with
    /// `OpEffects::QUIET`. `on_error` runs on any failure (before the toast) so
    /// callers can clear transient state (e.g. `remote_status`).
    ///
    /// For ops that need an open repo, use `run_git_blocking` /
    /// `run_git_op_returning`. For ops that must NOT auto-toast (banner-only
    /// error destinations), use [`Self::run_blocking_op_silent`].
    pub(crate) fn run_blocking_op_returning<T, F>(
        &mut self,
        label: &str,
        cx: &mut Context<Self>,
        op: F,
        on_success: impl FnOnce(&mut Self, T, &mut Context<Self>) + Send + 'static,
        on_error: impl FnOnce(&mut Self, &mut Context<Self>) + Send + 'static,
    ) where
        F: FnOnce() -> Result<T, GitError> + Send + 'static,
        T: Send + 'static,
    {
        self.run_op_full(
            label,
            cx,
            OpEffects::QUIET,
            move || async move {
                tokio::task::spawn_blocking(op)
                    .await
                    .map_err(|e| AppError::Remote(RemoteError::error(e.to_string())))
                    .and_then(|r| r.map_err(AppError::Git))
            },
            on_success,
            Some(Box::new(move |this, _detail, cx| on_error(this, cx))),
            None,
        );
    }

    /// Handle-less blocking-op seam that does NOT auto-toast. The caller's
    /// `on_error(detail)` is the sole error handler. Adapter over `run_op_full`
    /// with `ErrorChannel::Silent` — failures surface via the caller's callback
    /// (typically the persistent `last_error` banner). Used by
    /// repo-creation/discovery sites.
    pub(crate) fn run_blocking_op_silent<T, F>(
        &mut self,
        label: &str,
        cx: &mut Context<Self>,
        op: F,
        on_success: impl FnOnce(&mut Self, T, &mut Context<Self>) + Send + 'static,
        on_error: impl FnOnce(&mut Self, String, &mut Context<Self>) + Send + 'static,
    ) where
        F: FnOnce() -> Result<T, GitError> + Send + 'static,
        T: Send + 'static,
    {
        let fx = OpEffects {
            refresh_repo: false,
            refresh_prs: false,
            remote_status: None,
            error_channel: ErrorChannel::Silent,
        };
        self.run_op_full(
            label,
            cx,
            fx,
            move || async move {
                tokio::task::spawn_blocking(op)
                    .await
                    .map_err(|e| AppError::Remote(RemoteError::error(e.to_string())))
                    .and_then(|r| r.map_err(AppError::Git))
            },
            on_success,
            Some(Box::new(on_error)),
            None,
        );
    }
}
