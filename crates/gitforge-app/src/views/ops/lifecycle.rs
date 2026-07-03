//! Busy-flag lifecycle for staged operations. Each variant names which UI
//! spinner field to toggle and, when applicable, the stale-response token
//! captured at spawn time. `run_op_full` sets the flag before spawn and clears
//! it on every outcome when [`BusyFlag::should_clear_on_complete`] holds.
//! Callers gate data writes with [`BusyFlag::still_relevant`] on the same
//! cloned [`BusyFlag`], so the result-relevance predicate and the cleanup
//! predicate cannot drift. Per-owner flags (e.g. [`BusyFlag::PullRequests`])
//! clear unconditionally — their `set` targets the captured owner, not a shared
//! field — so navigating away mid-request can never strand a spinner.

use std::path::PathBuf;

use crate::views::app::GitForgeApp;

/// Which busy flag an operation owns, plus any stale-response guard token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BusyFlag {
    /// `hosting_repos_loading`. When `expect_provider` is `Some`, discard when
    /// the active hosting-repo view moved to another provider; `None` is
    /// unconditional (e.g. repo search).
    HostingRepos {
        expect_provider: Option<String>,
    },
    AiGenerating,
    CreatePrRepos(String),
    CreatePrBranches {
        provider: String,
        to_repo: String,
    },
    CreatePrGeneratingAi,
    CreatePrSubmitting,
    SquashWizardGeneratingAi { token: u64 },
    PullRequests {
        tab_id: u64,
        tab_path: PathBuf,
    },
}

impl BusyFlag {
    pub(crate) fn set(&self, app: &mut GitForgeApp, loading: bool) {
        match self {
            Self::HostingRepos { .. } => app.hosting_repos_loading = loading,
            Self::AiGenerating => app.ai_generating = loading,
            Self::CreatePrRepos(_) => app.create_pr.loading_repos = loading,
            Self::CreatePrBranches { .. } => app.create_pr.loading_branches = loading,
            Self::CreatePrGeneratingAi => app.create_pr.generating_ai = loading,
            Self::CreatePrSubmitting => app.create_pr.submitting = loading,
            Self::SquashWizardGeneratingAi { token } => {
                if let Some(wizard) = app.squash_wizard.as_mut()
                    && wizard.generation_token == *token
                {
                    wizard.generating_ai_message = loading;
                }
            }
            Self::PullRequests { tab_id, .. } => {
                if let Some(tab) = app
                    .repo_session
                    .open_repo_tabs
                    .iter_mut()
                    .find(|t| t.id == *tab_id)
                {
                    tab.pull_requests_loading = loading;
                }
            }
        }
    }

    pub(crate) fn still_relevant(&self, app: &GitForgeApp) -> bool {
        match self {
            Self::HostingRepos { expect_provider } => match expect_provider.as_deref() {
                None => true,
                Some(provider) => app.active_hosting_repo_provider() == Some(provider),
            },
            Self::AiGenerating
            | Self::CreatePrGeneratingAi
            | Self::CreatePrSubmitting => true,
            Self::SquashWizardGeneratingAi { token } => app
                .squash_wizard
                .as_ref()
                .is_some_and(|w| w.generation_token == *token),
            Self::CreatePrRepos(provider) => app.create_pr.provider == *provider,
            Self::CreatePrBranches { provider, to_repo } => {
                app.create_pr.provider == *provider && app.create_pr.to_repo == *to_repo
            }
            Self::PullRequests { tab_id, tab_path } => {
                app.repo_session.active_repo_tab_id == Some(*tab_id)
                    && app
                        .repo_session
                        .active_tab()
                        .is_some_and(|t| t.path == *tab_path)
            }
        }
    }

    /// Whether the shell should clear this flag when the owning request
    /// completes.
    ///
    /// This separates **lifecycle cleanup** (clear the spinner) from **result
    /// relevance** ([`still_relevant`] — apply the data?). Per-owner flags
    /// (e.g. [`PullRequests`], keyed by `tab_id`) target a specific owner in
    /// [`set`], so clearing is always safe: the original tab's spinner must not
    /// get stuck just because the user navigated away mid-request. Shared-field
    /// flags (everything else) only clear when [`still_relevant`] holds, so a
    /// stale request can't prematurely hide a newer request's loading state on
    /// the same field.
    pub(crate) fn should_clear_on_complete(&self, app: &GitForgeApp) -> bool {
        match self {
            // `set` targets the captured `tab_id` directly; the flag is per-tab
            // and cannot collide with a newer request on a different tab.
            Self::PullRequests { .. } => true,
            _ => self.still_relevant(app),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use gpui::TestAppContext;

    use super::BusyFlag;
    use crate::views::app::{AppDialog, GitForgeApp};
    use crate::views::dialogs::AddRepoTab;

    #[gpui::test]
    fn unconditional_variants_always_relevant(cx: &mut TestAppContext) {
        let app = cx.update(GitForgeApp::new);
        for flag in [
            BusyFlag::HostingRepos {
                expect_provider: None,
            },
            BusyFlag::AiGenerating,
            BusyFlag::CreatePrGeneratingAi,
            BusyFlag::CreatePrSubmitting,
        ] {
            assert!(flag.still_relevant(&app), "{flag:?}");
        }
    }

    #[gpui::test]
    fn squash_ai_token_matches_current_wizard(cx: &mut TestAppContext) {
        cx.update(|cx| {
            let mut git_app = GitForgeApp::new(cx);
            // No wizard present → stale result must be discarded.
            assert!(!BusyFlag::SquashWizardGeneratingAi { token: 999 }.still_relevant(&git_app));

            // Build a wizard with a known token and verify a matching flag is
            // still relevant, while a mismatched (older) token is not.
            let mut wizard = crate::views::dialogs::squash_wizard::SquashWizardState::new(
                "feature".into(),
                "main".into(),
                Vec::new(),
                cx,
            );
            wizard.generation_token = 7;
            git_app.squash_wizard = Some(wizard);
            assert!(BusyFlag::SquashWizardGeneratingAi { token: 7 }.still_relevant(&git_app));
            assert!(
                !BusyFlag::SquashWizardGeneratingAi { token: 1 }.still_relevant(&git_app),
                "stale token must be irrelevant"
            );
        });
    }

    #[gpui::test]
    fn hosting_repos_matches_active_dialog_when_provider_expected(cx: &mut TestAppContext) {
        cx.update(|app| {
            let mut git_app = GitForgeApp::new(app);
            git_app.active_dialog = AppDialog::AddRepo;
            git_app.add_repo_tab = AddRepoTab::Account("github".into());

            let flag = BusyFlag::HostingRepos {
                expect_provider: Some("github".into()),
            };
            assert!(flag.still_relevant(&git_app));

            git_app.add_repo_tab = AddRepoTab::Account("gitlab".into());
            assert!(!flag.still_relevant(&git_app));
        });
    }

    #[gpui::test]
    fn create_pr_repos_matches_provider(cx: &mut TestAppContext) {
        cx.update(|app| {
            let mut git_app = GitForgeApp::new(app);
            git_app.create_pr.provider = "github".into();

            let flag = BusyFlag::CreatePrRepos("github".into());
            assert!(flag.still_relevant(&git_app));

            git_app.create_pr.provider = "gitlab".into();
            assert!(!flag.still_relevant(&git_app));
        });
    }

    #[gpui::test]
    fn create_pr_branches_matches_provider_and_repo(cx: &mut TestAppContext) {
        cx.update(|app| {
            let mut git_app = GitForgeApp::new(app);
            git_app.create_pr.provider = "github".into();
            git_app.create_pr.to_repo = "octo/repo".into();

            let flag = BusyFlag::CreatePrBranches {
                provider: "github".into(),
                to_repo: "octo/repo".into(),
            };
            assert!(flag.still_relevant(&git_app));

            git_app.create_pr.to_repo = "other/repo".into();
            assert!(!flag.still_relevant(&git_app));
        });
    }

    #[gpui::test]
    fn pull_requests_matches_tab_id_and_path(cx: &mut TestAppContext) {
        cx.update(|app| {
            let mut git_app = GitForgeApp::new(app);
            let path = PathBuf::from("/tmp/repo");
            git_app.repo_session.active_repo_tab_id = Some(42);
            git_app.repo_session.open_repo_tabs.push(crate::views::repo_session::OpenRepoTab {
                id: 42,
                path: path.clone(),
                repo: std::sync::Arc::new(parking_lot::Mutex::new(None)),
                repo_state: None,
                loading: false,
                last_error: None,
                panel_snapshot: None,
                pull_requests: Vec::new(),
                pull_requests_loading: false,
                pull_requests_loaded: false,
            });

            let flag = BusyFlag::PullRequests {
                tab_id: 42,
                tab_path: path,
            };
            assert!(flag.still_relevant(&git_app));

            git_app.repo_session.active_repo_tab_id = Some(99);
            assert!(!flag.still_relevant(&git_app));
        });
    }

    #[gpui::test]
    fn pull_requests_cleanup_always_clears_even_after_tab_switch(cx: &mut TestAppContext) {
        cx.update(|app| {
            let mut git_app = GitForgeApp::new(app);
            let path = PathBuf::from("/tmp/repo");
            git_app.repo_session.active_repo_tab_id = Some(42);
            git_app.repo_session.open_repo_tabs.push(crate::views::repo_session::OpenRepoTab {
                id: 42,
                path: path.clone(),
                repo: std::sync::Arc::new(parking_lot::Mutex::new(None)),
                repo_state: None,
                loading: false,
                last_error: None,
                panel_snapshot: None,
                pull_requests: Vec::new(),
                pull_requests_loading: true, // spinner on for tab 42
                pull_requests_loaded: false,
            });

            let flag = BusyFlag::PullRequests {
                tab_id: 42,
                tab_path: path,
            };

            // Still on tab 42 → relevant, clear.
            assert!(flag.should_clear_on_complete(&git_app));

            // User switches to tab 99 → result no longer relevant, but cleanup
            // must still clear tab 42's spinner (it targets the captured tab).
            git_app.repo_session.active_repo_tab_id = Some(99);
            assert!(!flag.still_relevant(&git_app));
            assert!(flag.should_clear_on_complete(&git_app));

            // Actually clearing targets tab 42, not the active tab 99.
            flag.set(&mut git_app, false);
            let tab42 = git_app
                .repo_session
                .open_repo_tabs
                .iter()
                .find(|t| t.id == 42)
                .unwrap();
            assert!(!tab42.pull_requests_loading);
        });
    }

    #[gpui::test]
    fn shared_field_cleanup_follows_still_relevant(cx: &mut TestAppContext) {
        cx.update(|app| {
            let mut git_app = GitForgeApp::new(app);
            git_app.active_dialog = AppDialog::AddRepo;
            git_app.add_repo_tab = AddRepoTab::Account("github".into());

            let flag = BusyFlag::HostingRepos {
                expect_provider: Some("github".into()),
            };
            // Provider matches → cleanup proceeds.
            assert!(flag.should_clear_on_complete(&git_app));

            // User switched providers → stale request must NOT clear the shared
            // field (the newer request's loading state is still in flight).
            git_app.add_repo_tab = AddRepoTab::Account("gitlab".into());
            assert!(!flag.should_clear_on_complete(&git_app));
        });
    }
}
