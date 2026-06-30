//! Operation dispatch — the GPUI-free layer that turns an operation's result
//! into a single [`DispatchAction`] the UI shell can execute blindly, plus the
//! reusable lock-dance helper the shells route blocking git reads through.
//!
//! Two concerns live here, both free of GPUI types:
//! - the **classifier** ([`plan_dispatch`]) — a pure decision: which surface
//!   (toast / `on_error` / silent), which refresh flags fire, whether the value
//!   is handed to `on_success`. Fully unit-testable with no GPUI context.
//! - the **blocking helpers** — [`with_repo_blocking`] (the lock-dance: clones
//!   the per-tab handle, grabs the lock on the blocking pool, and runs an `op`
//!   against the open `Repository`) and its repo-independent companion
//!   [`spawn_blocking_ok`] (for blocking steps with no handle, e.g. AI provider
//!   construction). GPUI-free but tokio-bound.
//!
//! The thin shells on `GitForgeApp` (`run_op`, `run_git_blocking`) — which DO
//! depend on GPUI — spawn the work, hand the result to [`plan_dispatch`], and
//! execute the returned action. They live elsewhere (migration step 3); see
//! `CONTEXT.md` → "Operation Dispatch".
//!
//! ## Error model
//! One error type ([`AppError`]) spans both failure origins:
//! - [`AppError::Git`] carries [`gitforge_git::GitError`], which already
//!   classifies info-vs-error ([`GitError::is_info`]) and redacts credentials
//!   ([`GitError::toast_message`]).
//! - [`AppError::Remote`] carries [`RemoteError`], built at the app boundary
//!   from `anyhow` results returned by `gitforge-hosting` / `gitforge-ai`.
//!   `RemoteError` scrubs credential URL userinfo at construction using the
//!   same redactor as `GitError`, so no toast or banner can leak a token.

use std::future::Future;
use std::sync::Arc;

use gitforge_git::{GitError, Repository, first_line, redact_credentials};
use gpui::Context;
use parking_lot::Mutex;

use crate::views::app::GitForgeApp;
use crate::views::repo_session::GitOpReadiness;
use crate::views::toasts::ToastKind;

/// One error type across git, hosting, and AI operations.
///
/// Construct via `?` from operations returning [`GitError`] or
/// `anyhow::Error` (both have `From` impls), or build a [`RemoteError`]
/// explicitly when a severity other than `Error` is needed (e.g. a soft
/// "nothing to do" condition).
#[derive(Debug)]
pub enum AppError {
    /// A git operation failed. Classified via [`GitError::is_info`] /
    /// [`GitError::toast_message`].
    Git(GitError),
    /// A hosting/AI (or other non-git) operation failed. Carries its own
    /// severity and an already-redacted message.
    Remote(RemoteError),
}

impl From<GitError> for AppError {
    fn from(e: GitError) -> Self {
        AppError::Git(e)
    }
}

impl From<RemoteError> for AppError {
    fn from(e: RemoteError) -> Self {
        AppError::Remote(e)
    }
}

/// Default conversion from `anyhow`: severity [`Error`](Severity::Error),
/// message redacted. This is how hosting/AI failures enter the classifier
/// unless the op builds a [`RemoteError::info`] explicitly.
impl From<anyhow::Error> for AppError {
    fn from(e: anyhow::Error) -> Self {
        AppError::Remote(RemoteError::from_anyhow(e))
    }
}

/// User-facing severity of a remote failure. Mirrors the info/error split that
/// [`GitError::is_info`] provides for the git arm.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    /// An informational condition, not a failure (e.g. "no staged changes to
    /// generate a message from"). Surfaced as an info toast, or suppressed
    /// entirely under [`ErrorChannel::Silent`].
    Info,
    /// A real failure. Surfaced as an error toast, or routed to the caller's
    /// `on_error` under [`ErrorChannel::Silent`].
    Error,
}

/// A non-git failure (hosting API, AI provider, task panic) with its severity
/// and an already-redacted, single-line message.
///
/// The message is scrubbed with the same credential redactor as
/// [`GitError::toast_message`], so tokens embedded in URLs can never reach a
/// toast or banner. Construct via [`RemoteError::error`] /
/// [`RemoteError::info`] / [`RemoteError::from_anyhow`].
#[derive(Debug, Clone)]
pub struct RemoteError {
    severity: Severity,
    message: String,
}

impl RemoteError {
    /// An error-severity remote failure. The message is redacted and reduced
    /// to its first line before storage.
    pub fn error(msg: impl Into<String>) -> Self {
        Self {
            severity: Severity::Error,
            message: redact_for_display(&msg.into()),
        }
    }

    /// An informational remote outcome (not a failure). Treated gently by the
    /// classifier: an info toast under [`ErrorChannel::Toast`], or suppressed
    /// entirely under [`ErrorChannel::Silent`].
    pub fn info(msg: impl Into<String>) -> Self {
        Self {
            severity: Severity::Info,
            message: redact_for_display(&msg.into()),
        }
    }

    /// Build an error-severity [`RemoteError`] from an `anyhow` result. This is
    /// the default conversion for hosting/AI failures; reach for
    /// [`RemoteError::info`] when the condition is informational.
    pub fn from_anyhow(e: anyhow::Error) -> Self {
        Self::error(e.to_string())
    }

    pub fn severity(&self) -> Severity {
        self.severity
    }

    /// The redacted, single-line message. Never contains credential userinfo.
    pub fn message(&self) -> &str {
        &self.message
    }
}

/// Where the classifier sends an error. Set per-op via [`OpEffects`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorChannel {
    /// Surface failures automatically (info toast / error toast). The common
    /// case for git porcelain and most hosting operations.
    Toast,
    /// Do NOT auto-surface. Error-severity failures are handed to the caller's
    /// `on_error` callback (which typically writes the persistent `last_error`
    /// banner and performs recovery); info-severity outcomes are suppressed
    /// entirely. Used by repo discovery/init and AI generation.
    Silent,
}

/// What an operation wants, declared by the caller. Carried through to
/// [`plan_dispatch`], which resolves the result-dependent parts into a
/// [`DispatchAction`]. Lifecycle effects not derived from the result (clearing
/// `remote_status`, clearing busy flags) stay with the shell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpEffects {
    /// Refresh `RepoState` from the repository on success.
    pub refresh_repo: bool,
    /// Refresh pull requests on success (e.g. after fetch/pull/push).
    pub refresh_prs: bool,
    /// A `remote_status` label to show while the op runs. `Some` makes the
    /// shell set `repo_session.remote_status` before spawn and clear it in
    /// every arm. Owned because callers pass runtime-built strings (e.g.
    /// `format!("Fetching from {remote}…")`).
    pub remote_status: Option<String>,
    /// Where failures go. See [`ErrorChannel`].
    pub error_channel: ErrorChannel,
}

impl OpEffects {
    /// No side effects, errors toast. The minimal declaration.
    pub const QUIET: Self = Self {
        refresh_repo: false,
        refresh_prs: false,
        remote_status: None,
        error_channel: ErrorChannel::Toast,
    };

    /// Refresh the repo on success, errors toast. The common git-op declaration.
    pub const GIT: Self = Self {
        refresh_repo: true,
        refresh_prs: false,
        remote_status: None,
        error_channel: ErrorChannel::Toast,
    };

    /// No refresh, errors routed to the caller's `on_error` (banner-only). The
    /// declaration for repo discovery / init, where `apply_repo_state` handles
    /// the post-op snapshot and failures surface via the persistent
    /// `last_error` banner rather than a toast.
    pub const SILENT: Self = Self {
        refresh_repo: false,
        refresh_prs: false,
        remote_status: None,
        error_channel: ErrorChannel::Silent,
    };

    /// A git network op (fetch / push / pull) that shows `status` while it runs,
    /// refreshes the repo on success, and surfaces failures as toasts.
    /// `refresh_prs` is deliberately false: `refresh_repository`'s success
    /// callback already refreshes pull requests, so a flag-driven refresh here
    /// would fire a duplicate, racing API call before the repo state has
    /// updated.
    pub fn git_with_status(status: impl Into<String>) -> Self {
        Self {
            refresh_repo: true,
            refresh_prs: false,
            remote_status: Some(status.into()),
            error_channel: ErrorChannel::Toast,
        }
    }
}

impl Default for OpEffects {
    fn default() -> Self {
        Self::GIT
    }
}

/// How the shell should surface an op's result (the auto-toast decision).
/// Produced by [`plan_dispatch`]; the shell maps each variant to a toast or
/// nothing. Separate from `error_detail` (below): an error fires the caller's
/// `on_error` callback **regardless of channel**, then the surface decides
/// whether a toast also appears.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Surface {
    /// Nothing to surface: success, or any error under [`ErrorChannel::Silent`]
    /// (the caller's `on_error` handles it; no auto-toast).
    Silent,
    /// Push an info toast. Carries the final, label-joined message.
    Info(String),
    /// Push an error toast. Carries the final, label-joined message.
    Error(String),
}

/// The full dispatch decision for one op result. The shell executes this
/// blindly: apply refresh flags, fire `on_error` (if `error_detail` is set),
/// map [`Surface`] to a toast, hand `value` to `on_success`.
#[derive(Debug, Clone, PartialEq)]
pub struct DispatchAction<T> {
    /// `Some` only on success. Passed to the caller's `on_success`.
    pub value: Option<T>,
    /// The auto-toast decision (label-joined message). See [`Surface`].
    pub surface: Surface,
    /// Raw redacted detail, `Some` on ANY error (regardless of channel). The
    /// shell hands this to the caller's `on_error` callback before surfacing,
    /// so the callback can clear transient state or write a banner. `None` on
    /// success.
    pub error_detail: Option<String>,
    /// Refresh `RepoState`. Only set on success.
    pub refresh_repo: bool,
    /// Refresh pull requests. Only set on success.
    pub refresh_prs: bool,
}

/// The classifier. Pure.
///
/// Turns an op result plus the caller's [`OpEffects`] into a [`DispatchAction`]
/// the shell executes. The decision table:
///
/// | result             | channel | surface                                  | error_detail |
/// |--------------------|---------|------------------------------------------|--------------|
/// | `Ok(value)`        | any     | [`Surface::Silent`]                      | `None`       |
/// | git/remote `Info`  | `Toast` | [`Surface::Info`] `"{label}: {detail}"`  | `Some(detail)` |
/// | git/remote `Info`  | `Silent`| [`Surface::Silent`]                      | `Some(detail)` |
/// | git/remote `Error` | `Toast` | [`Surface::Error`] `"{label}: {detail}"` | `Some(detail)` |
/// | git/remote `Error` | `Silent`| [`Surface::Silent`]                      | `Some(detail)` |
///
/// `error_detail` is `Some` for every error, so the shell fires `on_error`
/// uniformly; the `error_channel` only controls whether a toast also appears.
/// Refresh flags fire only on success.
pub fn plan_dispatch<T>(
    label: &str,
    result: Result<T, AppError>,
    fx: &OpEffects,
) -> DispatchAction<T> {
    match result {
        Ok(value) => DispatchAction {
            value: Some(value),
            surface: Surface::Silent,
            error_detail: None,
            refresh_repo: fx.refresh_repo,
            refresh_prs: fx.refresh_prs,
        },
        Err(err) => {
            let (severity, detail) = classify(&err);
            let surface = match (fx.error_channel, severity) {
                (ErrorChannel::Toast, Severity::Info) => Surface::Info(join_label(label, &detail)),
                (ErrorChannel::Toast, Severity::Error) => {
                    Surface::Error(join_label(label, &detail))
                }
                (ErrorChannel::Silent, _) => Surface::Silent,
            };
            DispatchAction {
                value: None,
                surface,
                error_detail: Some(detail),
                refresh_repo: false,
                refresh_prs: false,
            }
        }
    }
}

/// Severity + redacted detail for an [`AppError`]. The git arm delegates to
/// `GitError`'s own classifier; the remote arm reads the pre-redacted message
/// stored at construction.
fn classify(err: &AppError) -> (Severity, String) {
    match err {
        AppError::Git(g) => {
            let severity = if g.is_info() {
                Severity::Info
            } else {
                Severity::Error
            };
            (severity, g.toast_message())
        }
        AppError::Remote(r) => (r.severity(), r.message().to_string()),
    }
}

/// Join a label and detail as `"{label}: {detail}"`, or just `"{label}"` when
/// the detail is empty. Matches the convention used by the existing toast
/// reporters (`report_git_error` / `report_op_error`).
fn join_label(label: &str, detail: &str) -> String {
    if detail.is_empty() {
        label.to_string()
    } else {
        format!("{label}: {detail}")
    }
}

/// Reduce a raw string to a single line and scrub credential URL userinfo. The
/// same treatment [`GitError::toast_message`] applies to `OperationFailed`.
fn redact_for_display(s: &str) -> String {
    redact_credentials(&first_line(s))
}

/// The single repo lock-dance. Takes the per-tab repository handle by value
/// (clone at the call site if you need to keep it), grabs the lock on the
/// blocking pool, and runs `op` against the open [`Repository`] (or returns
/// [`GitError::OperationFailed`] if the tab's repo has been closed).
///
/// `JoinError` (task panic) becomes [`AppError::Remote`]; a [`GitError`] from
/// `op` becomes [`AppError::Git`]. Both shells and staged ops (AI/PR) route
/// their blocking git reads through here so the lock-dance lives in exactly one
/// place — replacing the four prior copies of `lock → as_ref → op(repo)`.
pub async fn with_repo_blocking<R>(
    handle: Arc<Mutex<Option<Repository>>>,
    op: impl FnOnce(&Repository) -> Result<R, GitError> + Send + 'static,
) -> Result<R, AppError>
where
    R: Send + 'static,
{
    match tokio::task::spawn_blocking(move || {
        let guard = handle.lock();
        let Some(repo) = guard.as_ref() else {
            return Err(GitError::OperationFailed("No repository open".into()));
        };
        op(repo)
    })
    .await
    {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(git_err)) => Err(AppError::Git(git_err)),
        Err(join_err) => Err(AppError::Remote(RemoteError::error(join_err.to_string()))),
    }
}

/// Run a blocking, repo-independent `anyhow` op on the blocking pool, mapping a
/// task panic ([`tokio::task::JoinError`]) and the inner [`anyhow::Error`] to
/// [`AppError::Remote`]. The non-repo companion to [`with_repo_blocking`]:
/// used by staged ops (AI generation) for the blocking-but-handle-free step,
/// e.g. keychain-backed AI provider construction.
pub async fn spawn_blocking_ok<T>(
    op: impl FnOnce() -> Result<T, anyhow::Error> + Send + 'static,
) -> Result<T, AppError>
where
    T: Send + 'static,
{
    match tokio::task::spawn_blocking(op).await {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(e)) => Err(AppError::from(e)),
        Err(join_err) => Err(AppError::Remote(RemoteError::error(join_err.to_string()))),
    }
}

// ── shells (GPUI-bound) ─────────────────────────────────────────────────────
//
// The classifier and lock-dance above are GPUI-free; these shells are the thin
// GPUI-bound wrappers that spawn the work, hand the result to `plan_dispatch`,
// and execute the returned `DispatchAction` on the UI thread. They are methods
// on `GitForgeApp` so they can reach the refresh/toast/remote-status surface.

type ErrorHandler = Box<dyn FnOnce(&mut GitForgeApp, String, &mut Context<GitForgeApp>) + Send>;
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
    /// - `finally` — runs in EVERY arm (success and error), for lifecycle
    ///   cleanup like clearing busy flags. `None` for ops with no busy flag.
    ///
    /// Most ops want the [`Self::run_op`] convenience instead (no `on_error` /
    /// `finally`). Reach for `run_op_full` only when you need an error callback
    /// (Silent-channel recovery) or a `finally` (busy-flag lifecycle).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn run_op_full<T, Fut, Op, FOk>(
        &mut self,
        label: &str,
        cx: &mut Context<Self>,
        fx: OpEffects,
        op: Op,
        on_success: FOk,
        on_error: Option<ErrorHandler>,
        finally: Option<FinallyHandler>,
    ) where
        Op: FnOnce() -> Fut + Send + 'static,
        Fut: Future<Output = Result<T, AppError>> + Send + 'static,
        T: Send + 'static,
        FOk: FnOnce(&mut Self, T, &mut Context<Self>) + Send + 'static,
    {
        if let Some(status) = fx.remote_status.clone() {
            self.repo_session.remote_status = status;
            cx.notify();
        }
        let clear_status = fx.remote_status.is_some();
        let label = label.to_string();
        cx.spawn(async move |this, cx| {
            let result = op().await;
            let action = plan_dispatch(&label, result, &fx);
            this.update(cx, |this, cx| {
                if action.refresh_repo {
                    this.refresh_repository(cx);
                }
                if action.refresh_prs {
                    this.refresh_pull_requests(cx);
                }
                if let Some(detail) = action.error_detail
                    && let Some(handler) = on_error
                {
                    handler(this, detail, cx);
                }
                match action.surface {
                    Surface::Silent => {}
                    Surface::Info(msg) => this.push_toast(ToastKind::Info, msg, cx),
                    Surface::Error(msg) => this.push_toast(ToastKind::Error, msg, cx),
                }
                if let Some(value) = action.value {
                    on_success(this, value, cx);
                }
                if clear_status {
                    this.repo_session.remote_status.clear();
                }
                if let Some(fin) = finally {
                    fin(this, cx);
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
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
        // The single git-op readiness guard. `git_op_readiness` is GPUI-free and
        // unit-tested; both skip reasons (no repo / still loading) surface here,
        // preserving the prior NoRepo (warn + Warning toast) and Loading
        // (silent debug log) behaviours.
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
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOAST: OpEffects = OpEffects {
        refresh_repo: true,
        refresh_prs: true,
        remote_status: None,
        error_channel: ErrorChannel::Toast,
    };
    const SILENT: OpEffects = OpEffects {
        refresh_repo: false,
        refresh_prs: false,
        remote_status: None,
        error_channel: ErrorChannel::Silent,
    };

    /// Run the classifier on an error with `T = ()`. Error-case tests don't
    /// inspect `value`, so this anchors the type parameter once instead of
    /// annotating every call site.
    fn err(label: &str, error: AppError, fx: &OpEffects) -> DispatchAction<()> {
        plan_dispatch(label, Err(error), fx)
    }

    #[test]
    fn success_passes_value_and_refresh_flags() {
        let action = plan_dispatch("Stage", Ok::<_, AppError>(7), &TOAST);
        assert_eq!(action.value, Some(7));
        assert_eq!(action.surface, Surface::Silent);
        assert!(action.refresh_repo);
        assert!(action.refresh_prs);
    }

    #[test]
    fn success_suppresses_surface_even_on_toast_channel() {
        let action = plan_dispatch("Stage", Ok::<_, AppError>(()), &TOAST);
        assert_eq!(action.surface, Surface::Silent);
    }

    #[test]
    fn git_error_on_toast_channel_surfaces_error_with_label() {
        let e = GitError::OperationFailed("disk full".into());
        let action = err("Commit", AppError::Git(e), &TOAST);
        assert_eq!(action.value, None);
        assert_eq!(action.surface, Surface::Error("Commit: disk full".into()));
        // error_detail carries the raw redacted detail (no label) for on_error.
        assert_eq!(action.error_detail, Some("disk full".into()));
        assert!(!action.refresh_repo);
        assert!(!action.refresh_prs);
    }

    #[test]
    fn git_info_on_toast_channel_surfaces_info() {
        let action = err("Commit", AppError::Git(GitError::EmptyCommit), &TOAST);
        assert_eq!(
            action.surface,
            Surface::Info("Commit: Nothing to commit".into())
        );
        assert_eq!(action.error_detail, Some("Nothing to commit".into()));
    }

    #[test]
    fn git_local_changes_overwritten_on_git_channel_surfaces_error() {
        // Regression: a `git pull` that aborts with "Your local changes ... would
        // be overwritten" used to be silently swallowed (or surfaced as a raw
        // OperationFailed). It now carries a structured variant that classifies
        // as a real error and surfaces on the default Toast channel.
        let e = GitError::LocalChangesOverwritten {
            command: "pull".into(),
            paths: vec!["README.md".into()],
            stderr: "...".into(),
        };
        let action = err("Pull", AppError::Git(e), &OpEffects::GIT);
        match action.surface {
            Surface::Error(msg) => {
                assert!(msg.starts_with("Pull: "), "surface: {msg}");
                assert!(msg.contains("commit or stash"), "surface: {msg}");
            }
            other => panic!("expected Surface::Error, got {other:?}"),
        }
        assert!(action.error_detail.is_some());
        assert!(!action.refresh_repo, "errors must not refresh");
    }

    #[test]
    fn git_error_on_silent_channel_is_silent_but_fires_on_error() {
        let e = GitError::OperationFailed("no upstream".into());
        let action = err("Push", AppError::Git(e), &SILENT);
        // No toast (Silent), but on_error still gets the raw detail.
        assert_eq!(action.surface, Surface::Silent);
        assert_eq!(action.error_detail, Some("no upstream".into()));
    }

    #[test]
    fn git_info_on_silent_channel_is_silent_but_fires_on_error() {
        let action = err("Commit", AppError::Git(GitError::EmptyCommit), &SILENT);
        assert_eq!(action.surface, Surface::Silent);
        assert_eq!(action.error_detail, Some("Nothing to commit".into()));
    }

    #[test]
    fn remote_error_on_toast_channel_surfaces_error() {
        let action = err(
            "Fetch PRs",
            AppError::Remote(RemoteError::error("503")),
            &TOAST,
        );
        assert_eq!(action.surface, Surface::Error("Fetch PRs: 503".into()));
        assert_eq!(action.error_detail, Some("503".into()));
    }

    #[test]
    fn remote_info_on_toast_channel_surfaces_info() {
        let action = err(
            "Generate",
            AppError::Remote(RemoteError::info("no staged changes")),
            &TOAST,
        );
        assert_eq!(
            action.surface,
            Surface::Info("Generate: no staged changes".into())
        );
        assert_eq!(action.error_detail, Some("no staged changes".into()));
    }

    #[test]
    fn remote_error_on_silent_channel_is_silent_but_fires_on_error() {
        let action = err(
            "Generate",
            AppError::Remote(RemoteError::error("rate limited")),
            &SILENT,
        );
        assert_eq!(action.surface, Surface::Silent);
        assert_eq!(action.error_detail, Some("rate limited".into()));
    }

    #[test]
    fn remote_info_on_silent_channel_is_silent_but_fires_on_error() {
        let action = err(
            "Generate",
            AppError::Remote(RemoteError::info("nothing to do")),
            &SILENT,
        );
        assert_eq!(action.surface, Surface::Silent);
        assert_eq!(action.error_detail, Some("nothing to do".into()));
    }

    #[test]
    fn errors_never_refresh() {
        let action = err("Op", AppError::Remote(RemoteError::error("boom")), &TOAST);
        assert!(!action.refresh_repo);
        assert!(!action.refresh_prs);
    }

    #[test]
    fn success_has_no_error_detail() {
        let action = plan_dispatch("Op", Ok::<_, AppError>(()), &TOAST);
        assert_eq!(action.error_detail, None);
    }

    #[test]
    fn empty_detail_falls_back_to_label_only() {
        let action = err("Op", AppError::Remote(RemoteError::error("")), &TOAST);
        assert_eq!(action.surface, Surface::Error("Op".into()));
    }

    #[test]
    fn remote_error_redacts_token_url() {
        let r = RemoteError::error("request to https://token@host/api failed");
        assert_eq!(r.message(), "request to https://***@host/api failed");
    }

    #[test]
    fn remote_error_redacts_user_password_url() {
        let r = RemoteError::error("GET https://alice:s3cr3t@host/path");
        assert_eq!(r.message(), "GET https://***@host/path");
    }

    #[test]
    fn remote_error_takes_first_line_only() {
        let r = RemoteError::error("first line\nsecond line with https://x@y");
        assert_eq!(r.message(), "first line");
    }

    #[test]
    fn remote_error_severity_defaults_to_error() {
        assert_eq!(RemoteError::error("x").severity(), Severity::Error);
        assert_eq!(RemoteError::info("x").severity(), Severity::Info);
    }

    #[test]
    fn from_anyhow_is_error_severity_and_redacted() {
        let r = RemoteError::from_anyhow(anyhow::anyhow!("boom https://t@h"));
        assert_eq!(r.severity(), Severity::Error);
        assert_eq!(r.message(), "boom https://***@h");
    }

    #[test]
    fn apperror_from_git_error() {
        let action = err(
            "Op",
            AppError::from(GitError::BranchNotFound { name: "x".into() }),
            &TOAST,
        );
        assert_eq!(
            action.surface,
            Surface::Error("Op: Branch 'x' not found".into())
        );
    }

    #[test]
    fn apperror_from_anyhow_via_question_operator() {
        fn op() -> Result<i32, AppError> {
            Err(anyhow::anyhow!("network down"))?
        }
        let action = plan_dispatch("Op", op(), &TOAST);
        assert_eq!(action.surface, Surface::Error("Op: network down".into()));
    }

    #[test]
    fn git_arm_redacts_credentials_too() {
        // The git arm delegates to GitError::toast_message, which redacts.
        let e = GitError::OperationFailed("fetch https://token@host failed".into());
        let action = err("Fetch", AppError::Git(e), &TOAST);
        assert_eq!(
            action.surface,
            Surface::Error("Fetch: fetch https://***@host failed".into())
        );
    }

    #[test]
    fn question_operator_uses_direct_git_conversion_not_anyhow() {
        // repo.method() returns Result<_, GitError>; ? must use From<GitError>,
        // preserving structured classification (is_info), not flatten via anyhow.
        fn op() -> Result<(), AppError> {
            Err(GitError::EmptyCommit)?;
            Ok(())
        }
        let action = plan_dispatch("Commit", op(), &TOAST);
        assert!(matches!(action.surface, Surface::Info(_)));
    }

    // ── with_repo_blocking ──

    use std::future::Future;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static SCRATCH_UNIQ: AtomicU64 = AtomicU64::new(0);

    /// RAII guard that removes a scratch directory on drop, so leaked temp
    /// repos don't accumulate even when an assertion fails.
    struct Scratch(PathBuf);
    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// Build an empty git repo in a fresh temp dir for lock-dance tests.
    fn scratch_repo() -> (Repository, Scratch) {
        let n = SCRATCH_UNIQ.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("gitforge-dispatch-test-{n}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create scratch dir");
        Repository::init_repo(&dir, false).expect("git init");
        (Repository::open(&dir).expect("open repo"), Scratch(dir))
    }

    /// Run a future on a throwaway multi-thread tokio runtime (the blocking
    /// pool is required for `spawn_blocking`).
    fn block_on<R>(f: impl Future<Output = R>) -> R {
        tokio::runtime::Runtime::new()
            .expect("tokio runtime")
            .block_on(f)
    }

    #[test]
    fn none_repo_yields_operation_failed() {
        let handle: Arc<Mutex<Option<Repository>>> = Arc::new(Mutex::new(None));
        let res = block_on(with_repo_blocking(handle, |_: &Repository| {
            Ok::<_, GitError>(5)
        }));
        assert!(matches!(
            res,
            Err(AppError::Git(GitError::OperationFailed(_)))
        ));
    }

    #[test]
    fn some_repo_runs_op_and_returns_value() {
        let (repo, _scratch) = scratch_repo();
        let handle = Arc::new(Mutex::new(Some(repo)));
        let res = block_on(with_repo_blocking(handle, |_: &Repository| {
            Ok::<_, GitError>(42)
        }));
        assert_eq!(res.unwrap(), 42);
    }

    #[test]
    fn git_error_from_op_becomes_apperror_git() {
        let (repo, _scratch) = scratch_repo();
        let handle = Arc::new(Mutex::new(Some(repo)));
        let res = block_on(with_repo_blocking(handle, |_| {
            Err::<(), _>(GitError::EmptyCommit)
        }));
        assert!(matches!(res, Err(AppError::Git(GitError::EmptyCommit))));
    }

    #[test]
    fn join_error_from_panicking_op_becomes_remote_error() {
        let (repo, _scratch) = scratch_repo();
        let handle = Arc::new(Mutex::new(Some(repo)));
        let res = block_on(with_repo_blocking(
            handle,
            |_: &Repository| -> Result<(), GitError> { panic!("boom") },
        ));
        assert!(matches!(res, Err(AppError::Remote(_))));
    }
}
