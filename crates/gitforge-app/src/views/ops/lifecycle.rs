//! Busy-flag lifecycle for staged operations. Each variant names which UI
//! spinner field to toggle and, when applicable, the stale-response token
//! captured at spawn time. `run_op_full` sets the flag before spawn and clears
//! it on every outcome when [`BusyFlag::still_relevant`] holds. Callers gate
//! data writes with the same cloned [`BusyFlag`] so guards stay in one place.

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
    CommitPushGeneratingBranch,
    CreatePrRepos(String),
    CreatePrBranches {
        provider: String,
        to_repo: String,
    },
    CreatePrGeneratingAi,
    CreatePrSubmitting,
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
            Self::CommitPushGeneratingBranch => app.commit_push_generating_branch = loading,
            Self::CreatePrRepos(_) => app.create_pr.loading_repos = loading,
            Self::CreatePrBranches { .. } => app.create_pr.loading_branches = loading,
            Self::CreatePrGeneratingAi => app.create_pr.generating_ai = loading,
            Self::CreatePrSubmitting => app.create_pr.submitting = loading,
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
            | Self::CommitPushGeneratingBranch
            | Self::CreatePrGeneratingAi
            | Self::CreatePrSubmitting => true,
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
            BusyFlag::CommitPushGeneratingBranch,
            BusyFlag::CreatePrGeneratingAi,
            BusyFlag::CreatePrSubmitting,
        ] {
            assert!(flag.still_relevant(&app), "{flag:?}");
        }
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
}
