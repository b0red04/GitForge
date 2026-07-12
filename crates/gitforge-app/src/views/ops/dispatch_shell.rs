//! GPUI-bound operation shells — spawn work, classify via [`super::dispatch`],
//! execute the returned [`DispatchAction`] on the UI thread.

use std::future::Future;
use std::sync::Arc;

use gitforge_git::{GitError, Repository};
use gpui::Context;
use parking_lot::Mutex;

use crate::views::app::GitForgeApp;
use crate::views::ops::dispatch::{
    AppError, DispatchAction, OpEffects, Surface, plan_dispatch, with_repo_blocking,
};
use crate::views::ops::pr_ops::PullRequestRefreshMode;
use crate::views::repo_session::GitOpReadiness;
use crate::views::toasts::ToastKind;

pub use super::lifecycle::BusyFlag;

pub(crate) type ErrorHandler =
    Box<dyn FnOnce(&mut GitForgeApp, String, &mut Context<GitForgeApp>) + Send>;
pub(crate) type SurfaceHandler =
    Box<dyn FnOnce(&mut GitForgeApp, Surface, &mut Context<GitForgeApp>) + Send>;
type FinallyHandler = Box<dyn FnOnce(&mut GitForgeApp, &mut Context<GitForgeApp>) + Send>;

impl GitForgeApp {
    /// The full operation shell. Spawns an async `op` off the UI thread and
    /// routes its result through [`plan_dispatch`]; the classifier owns the
    /// result→surface decision, this shell owns spawn + executing the returned
    /// [`DispatchAction`] (refresh, `on_error`, surface, `on_success`,
    /// lifecycle).
    ///
    /// `op` is a future-factory (built inside the spawn) so it can capture the
    /// repo handle / provider by value. For staged ops (AI/PR), use
    /// [`with_repo_blocking`] for the blocking-git part inside `op`.
    ///
    /// Callbacks (all `Send + 'static`, all run on the UI thread inside the
    /// spawn's `update`):
    /// - `on_success(value)` — value consumption ONLY. Generic effects come
    ///   from [`OpEffects`]. Runs on success.
    /// - `on_error(detail)` — runs on ANY error (regardless of channel), BEFORE
    ///   the surface; receives the raw redacted detail. Use it to clear
    ///   transient state or write a banner. `None` for ops that just want the
    ///   auto-toast.
    /// - `finally` — runs in EVERY arm (success and error), for caller-specific
    ///   lifecycle cleanup beyond [`OpEffects::busy`] / [`OpEffects::remote_status`].
    ///   `None` for ops with no extra cleanup.
    ///
    /// Most ops use [`Self::run_git_op`] (fire-and-forget git) or
    /// [`Self::run_hosting_op`] (async hosting). Reach for `run_op_full` when you
    /// need a custom [`OpEffects`] (including [`OpEffects::busy`] /
    /// [`OpEffects::remote_status`]), a Silent-channel `on_error`, or caller
    /// `finally` cleanup beyond shell-owned lifecycle.
    /// `on_surface` — when `Some`, replaces the default surface→toast mapping
    /// (used by commit-push to morph a progress toast instead of pushing new ones).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn run_op_full<T, Fut, Op, FOk>(
        &mut self,
        label: &str,
        cx: &mut Context<Self>,
        fx: OpEffects,
        op: Op,
        on_success: FOk,
        on_error: Option<ErrorHandler>,
        on_surface: Option<SurfaceHandler>,
        finally: Option<FinallyHandler>,
    ) where
        Op: FnOnce() -> Fut + Send + 'static,
        Fut: Future<Output = Result<T, AppError>> + Send + 'static,
        T: Send + 'static,
        FOk: FnOnce(&mut Self, T, &mut Context<Self>) + Send + 'static,
    {
        let (clear_status, busy, label) = self.begin_dispatch_op(label, &fx, cx);
        cx.spawn(async move |this, cx| {
            let result = op().await;
            let action = plan_dispatch(&label, result, &fx);
            this.update(cx, |this, cx| {
                this.apply_dispatch_action(
                    action,
                    clear_status,
                    busy.as_ref(),
                    on_success,
                    on_error,
                    on_surface,
                    finally,
                    cx,
                );
            })
            .ok();
        })
        .detach();
    }

    pub(crate) fn begin_dispatch_op(
        &mut self,
        label: &str,
        fx: &OpEffects,
        cx: &mut Context<Self>,
    ) -> (bool, Option<BusyFlag>, String) {
        if let Some(status) = fx.remote_status.clone() {
            self.repo_session.remote_status = status;
            cx.notify();
        }
        if let Some(busy) = &fx.busy {
            busy.set(self, true);
            cx.notify();
        }
        let clear_status = fx.remote_status.is_some();
        let busy = fx.busy.clone();
        (clear_status, busy, label.to_string())
    }

    /// Map a dispatch [`Surface`] onto an in-flight progress toast.
    pub(crate) fn apply_surface_to_progress_toast(
        &mut self,
        progress_id: u64,
        surface: Surface,
        cx: &mut Context<Self>,
    ) {
        match surface {
            Surface::Info(msg) => self.finish_progress_toast(progress_id, ToastKind::Info, msg, cx),
            Surface::Error(msg) => {
                self.finish_progress_toast(progress_id, ToastKind::Error, msg, cx)
            }
            Surface::Silent => self.dismiss_toast(progress_id, cx),
        }
    }

    /// [`SurfaceHandler`] that morphs a progress toast instead of pushing new ones.
    pub(crate) fn surface_handler_for_progress_toast(progress_id: u64) -> SurfaceHandler {
        Box::new(move |this, surface, cx| {
            this.apply_surface_to_progress_toast(progress_id, surface, cx);
        })
    }

    /// Execute a [`DispatchAction`] on the UI thread — shared by the dispatch
    /// shells and bespoke staged ops that need mid-flight progress updates.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn apply_dispatch_action<T, FOk>(
        &mut self,
        action: DispatchAction<T>,
        clear_status: bool,
        busy: Option<&BusyFlag>,
        on_success: FOk,
        on_error: Option<ErrorHandler>,
        on_surface: Option<SurfaceHandler>,
        finally: Option<FinallyHandler>,
        cx: &mut Context<Self>,
    ) where
        FOk: FnOnce(&mut Self, T, &mut Context<Self>),
    {
        if action.refresh_repo {
            self.refresh_repository(cx);
        }
        if action.refresh_prs {
            self.refresh_pull_requests(cx, PullRequestRefreshMode::Initial);
        }
        if let Some(detail) = action.error_detail
            && let Some(handler) = on_error
        {
            handler(self, detail, cx);
        }
        if let Some(value) = action.value {
            on_success(self, value, cx);
        } else if let Some(handler) = on_surface {
            handler(self, action.surface, cx);
        } else {
            match action.surface {
                Surface::Silent => {}
                Surface::Info(msg) => self.push_toast(ToastKind::Info, msg, cx),
                Surface::Error(msg) => self.push_toast(ToastKind::Error, msg, cx),
            }
        }
        if clear_status {
            self.repo_session.remote_status.clear();
        }
        if let Some(b) = busy
            && b.should_clear_on_complete(self)
        {
            b.set(self, false);
        }
        if let Some(fin) = finally {
            fin(self, cx);
        }
        cx.notify();
    }

    /// Readiness guard for bespoke [`Self::run_op_full`] call sites that need
    /// async work or custom lifecycle. Returns the repo handle when the active
    /// tab is loaded; sets `last_error` on NoRepo and optionally notifies.
    /// Loading is silent.
    pub(crate) fn git_op_handle(
        &mut self,
        cx: &mut Context<Self>,
        notify_on_no_repo: bool,
    ) -> Option<Arc<Mutex<Option<Repository>>>> {
        match self.repo_session.git_op_readiness() {
            GitOpReadiness::Ready(handle) => Some(handle),
            GitOpReadiness::NoRepo => {
                self.repo_session.last_error = Some("No repository open".into());
                if notify_on_no_repo {
                    cx.notify();
                }
                None
            }
            GitOpReadiness::Loading => None,
        }
    }

    /// Sugar for the common blocking-git op: the single readiness guard
    /// (`RepoSession::git_op_readiness` — handle present AND tab not loading),
    /// wraps the sync `op` in [`with_repo_blocking`], and delegates to
    /// [`Self::run_op_full`]. Effects (refresh flags, `remote_status`) travel in
    /// `fx`. Used directly by call sites that need a custom `OpEffects`, and via
    /// the [`Self::run_git_op`] fire-and-forget sugar for the common
    /// refresh-on-success case.
    pub(crate) fn run_git_blocking<T, Op, FOk>(
        &mut self,
        label: &str,
        cx: &mut Context<Self>,
        fx: OpEffects,
        op: Op,
        on_success: FOk,
    ) where
        Op: FnOnce(&Repository) -> Result<T, GitError> + Send + 'static,
        T: Send + 'static,
        FOk: FnOnce(&mut Self, T, &mut Context<Self>) + Send + 'static,
    {
        let handle = match self.repo_session.git_op_readiness() {
            GitOpReadiness::Ready(handle) => handle,
            GitOpReadiness::NoRepo => {
                self.repo_session.last_error = Some("No repository open".into());
                tracing::warn!("{label}: no active repo handle");
                self.push_toast(ToastKind::Warning, "No repository open", cx);
                return;
            }
            GitOpReadiness::Loading => {
                tracing::debug!("{label}: skipped, repo still loading");
                return;
            }
        };
        self.run_op_full(
            label,
            cx,
            fx,
            move || with_repo_blocking(handle, op),
            on_success,
            None,
            None,
            None,
        );
    }
}
