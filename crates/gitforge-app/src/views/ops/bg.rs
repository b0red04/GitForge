//! Background-op seams and the shared dispatchers that own the 3-arm
//! `spawn_blocking` result match.
//!
//! Four sibling seams funnel through the two dispatchers in this file:
//! - [` GitForgeApp::run_git_op_returning`] (in `git_ops.rs`) — requires an
//!   open repo handle; the op receives `&Repository`. Auto-toasts on error.
//! - [`GitForgeApp::run_blocking_op_returning`] (here) — handle-less; the op
//!   receives nothing. Auto-toasts on error. Used for SSH key management,
//!   credential storage, clone, and other blocking work.
//! - [`GitForgeApp::run_blocking_op_silent`] (here) — handle-less, does NOT
//!   auto-toast. The caller's `on_error` is the sole error handler. Used by
//!   repo-creation/discovery sites that surface failures via the persistent
//!   `last_error` banner rather than a transient toast.
//!
//! The async (non-blocking) hosting seam in `hosting_ops.rs` has a different
//! shape (2-arm match, `anyhow::Error`, future-factory) and is intentionally
//! kept separate.

use gitforge_git::GitError;
use gpui::Context;

use crate::views::app::GitForgeApp;

impl GitForgeApp {
    /// Handle-less blocking-op seam: spawns `op` on the blocking pool and
    /// dispatches the result through [`dispatch_bg_result`] (auto-toasts on
    /// error). The op does not receive a `&Repository`. On either failure arm,
    /// `on_error` runs before the toast.
    ///
    /// For ops that need an open repo, use
    /// [`Self::run_git_op_returning`](super::git_ops::GitForgeApp::run_git_op_returning).
    /// For ops that must NOT auto-toast (banner-only error destinations), use
    /// [`Self::run_blocking_op_silent`].
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
        let label_owned = label.to_string();
        cx.spawn(async move |this, cx| {
            let result = tokio::task::spawn_blocking(op).await;
            this.update(cx, |this, cx| {
                dispatch_bg_result(this, cx, &label_owned, result, on_success, on_error);
            })
            .ok();
        })
        .detach();
    }

    /// Handle-less blocking-op seam that does NOT auto-report errors. The
    /// caller's `on_error` receives the error message and is the sole error
    /// handler — no toast is produced. Used by repo-creation/discovery sites
    /// (`init_repository`, `start_loading_repo_tab`, `clone_repository` when
    /// banner-style errors are preferred) that surface failures via the
    /// persistent `last_error` banner.
    ///
    /// `on_success` receives the op's value; it is also the right place to
    /// perform side effects that must happen on the main thread after the
    /// blocking op completes (e.g. writing the repo handle, opening a tab).
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
        let label_owned = label.to_string();
        cx.spawn(async move |this, cx| {
            let result = tokio::task::spawn_blocking(op).await;
            this.update(cx, |this, cx| {
                dispatch_bg_result_silent(this, cx, &label_owned, result, on_success, on_error);
            })
            .ok();
        })
        .detach();
    }
}

/// Dispatch the result of a `spawn_blocking` op with auto-toasting. Three arms:
/// - `Ok(Ok(value))` → `on_success(this, value, cx)`
/// - `Ok(Err(git_err))` → `on_error(this, cx)`, then `report_git_error`
///   (structured toast kind/message)
/// - `Err(join_err)` → `on_error(this, cx)`, then `report_op_error` (lossy)
///
/// Both git-op seams route through here so the 3-arm match and the
/// dual-reporter convention live in one place. `on_error` is shared across
/// both failure arms; for success-only side effects use `on_success`.
pub(super) fn dispatch_bg_result<T>(
    this: &mut GitForgeApp,
    cx: &mut Context<GitForgeApp>,
    label: &str,
    result: Result<Result<T, GitError>, tokio::task::JoinError>,
    on_success: impl FnOnce(&mut GitForgeApp, T, &mut Context<GitForgeApp>),
    on_error: impl FnOnce(&mut GitForgeApp, &mut Context<GitForgeApp>),
) {
    match result {
        Ok(Ok(value)) => on_success(this, value, cx),
        Ok(Err(e)) => {
            tracing::error!("{} failed: {}", label, e);
            on_error(this, cx);
            this.report_git_error(label, &e, cx);
        }
        Err(e) => {
            tracing::error!("{} task panicked: {}", label, e);
            on_error(this, cx);
            this.report_op_error(label, &e.to_string(), cx);
        }
    }
}

/// Dispatch the result of a `spawn_blocking` op WITHOUT auto-toasting. Three
/// arms:
/// - `Ok(Ok(value))` → `on_success(this, value, cx)`
/// - `Ok(Err(git_err))` → `on_error(this, git_err.to_string(), cx)`
/// - `Err(join_err)` → `on_error(this, join_err.to_string(), cx)`
///
/// The caller's `on_error` is the sole error handler. Used by sites that
/// surface failures via the persistent `last_error` banner rather than a
/// transient toast. The `tracing::error!` log still records which arm fired
/// and the full error detail.
pub(super) fn dispatch_bg_result_silent<T>(
    this: &mut GitForgeApp,
    cx: &mut Context<GitForgeApp>,
    label: &str,
    result: Result<Result<T, GitError>, tokio::task::JoinError>,
    on_success: impl FnOnce(&mut GitForgeApp, T, &mut Context<GitForgeApp>),
    on_error: impl FnOnce(&mut GitForgeApp, String, &mut Context<GitForgeApp>),
) {
    match result {
        Ok(Ok(value)) => on_success(this, value, cx),
        Ok(Err(e)) => {
            tracing::error!("{} failed: {}", label, e);
            on_error(this, e.to_string(), cx);
        }
        Err(e) => {
            tracing::error!("{} task panicked: {}", label, e);
            on_error(this, e.to_string(), cx);
        }
    }
}
