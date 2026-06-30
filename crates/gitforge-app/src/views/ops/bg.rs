//! Background-op seam. A single method that spawns a repo-independent blocking
//! `op` on the blocking pool and routes its result through the unified
//! `dispatch::run_op_full` shell — every operation, blocking or async, reaches
//! the one pure classifier (`dispatch::plan_dispatch`).
//!
//! Variance (error channel, refresh flags) travels as data in `OpEffects`: pass
//! `OpEffects::QUIET` for the auto-toast case, or an `OpEffects` with
//! `ErrorChannel::Silent` for the banner-only case. See `CONTEXT.md` →
//! "Operation Dispatch" and `docs/adr/0004-operation-dispatch-shell-collapse.md`.

use gitforge_git::GitError;
use gpui::Context;

use crate::views::app::GitForgeApp;
use crate::views::ops::dispatch::{AppError, OpEffects, RemoteError};

impl GitForgeApp {
    /// Handle-less blocking-op seam: spawns `op` on the blocking pool and routes
    /// the result through `run_op_full`. `on_error(detail)` runs on ANY failure
    /// (before the auto-toast, if any) so callers can clear transient state or
    /// write the persistent `last_error` banner. The error channel — toast vs
    /// silent — is chosen by `fx.error_channel`:
    ///
    /// - `OpEffects::QUIET` (or any `ErrorChannel::Toast`): auto-toasts on
    ///   failure; `on_error` is for pre-toast cleanup (ignore `detail` if you
    ///   don't need it).
    /// - `ErrorChannel::Silent`: does NOT auto-toast; `on_error(detail)` is the
    ///   sole error destination (typically the persistent `last_error` banner).
    ///
    /// For ops that need an open repo, use `run_git_blocking` (which owns the
    /// readiness guard and the `with_repo_blocking` lock-dance).
    pub(crate) fn run_blocking<T, F>(
        &mut self,
        label: &str,
        cx: &mut Context<Self>,
        fx: OpEffects,
        op: F,
        on_success: impl FnOnce(&mut Self, T, &mut Context<Self>) + Send + 'static,
        on_error: impl FnOnce(&mut Self, String, &mut Context<Self>) + Send + 'static,
    ) where
        F: FnOnce() -> Result<T, GitError> + Send + 'static,
        T: Send + 'static,
    {
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
