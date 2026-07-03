use gpui::Context;

use crate::views::app::{AppDialog, CommitPushMode, GitForgeApp};
use crate::views::ops::dispatch::{
    AppError, OpEffects, RemoteError, Surface, plan_dispatch, spawn_blocking_ok,
    with_repo_blocking,
};
use crate::views::ops::pr_ops::PullRequestRefreshMode;
use crate::views::toasts::ToastKind;

impl GitForgeApp {
    pub fn start_commit_and_push(&mut self, cx: &mut Context<Self>) {
        let Some(state) = self.repo_session.active_repo_state() else {
            self.push_toast(ToastKind::Warning, "No repository open".to_string(), cx);
            return;
        };
        if !state.status.has_changes() {
            self.push_toast(ToastKind::Info, "No changes to commit".to_string(), cx);
            return;
        }

        self.run_git_blocking(
            "Stage all",
            cx,
            super::dispatch::OpEffects::QUIET,
            |repo| repo.stage_all(),
            |this, _, cx| {
                this.refresh_repository(cx);
                this.open_commit_and_push_dialog(cx);
            },
        );
    }

    pub fn open_commit_and_push_dialog(&mut self, cx: &mut Context<Self>) {
        let detached = self
            .repo_session
            .active_repo_state()
            .is_none_or(|state| state.head_branch.is_none());
        let current_branch = self
            .repo_session
            .active_repo_state()
            .and_then(|state| state.head_branch.clone())
            .unwrap_or_default();

        self.commit_push_mode = if detached {
            CommitPushMode::FeatureBranch
        } else {
            CommitPushMode::CurrentBranch
        };
        self.dialog_input.clear();
        self.active_dialog = AppDialog::CommitAndPush {
            current_branch: current_branch.clone(),
            detached,
        };
        cx.notify();
    }

    pub fn set_commit_push_mode(&mut self, mode: CommitPushMode, cx: &mut Context<Self>) {
        self.commit_push_mode = mode;
        cx.notify();
    }

    pub fn confirm_commit_and_push(&mut self, current_branch: String, cx: &mut Context<Self>) {
        let use_feature = self.commit_push_mode == CommitPushMode::FeatureBranch;
        self.commit_push_mode = CommitPushMode::CurrentBranch;
        self.dialog_input.clear();
        self.execute_commit_and_push(use_feature, current_branch, cx);
    }

    pub fn execute_commit_and_push(
        &mut self,
        use_feature_branch: bool,
        current_branch: String,
        cx: &mut Context<Self>,
    ) {
        let existing_message = self.repo_session.commit_editor.message().trim().to_string();
        let needs_ai_message = existing_message.is_empty();
        let needs_ai = use_feature_branch || needs_ai_message;

        if use_feature_branch && self.settings.ai.provider == "disabled" {
            self.push_toast(
                ToastKind::Error,
                "Enable AI to generate feature branch names.".to_string(),
                cx,
            );
            return;
        }

        if needs_ai_message && self.settings.ai.provider == "disabled" {
            self.push_toast(
                ToastKind::Info,
                "Enter a commit message or enable AI to auto-generate one.".to_string(),
                cx,
            );
            return;
        }

        let provider_name = self.settings.ai.provider.clone();
        let commit_config = self.settings.ai.commit_message_config();
        let mut provider_config = self.settings.ai.provider_config();
        if needs_ai {
            let model = provider_config.model_or_default(&provider_name);
            if model.is_empty() {
                self.repo_session.last_error = Some(format!(
                    "No model configured for provider \"{provider_name}\""
                ));
                cx.notify();
                return;
            }
            provider_config.model = model;
        }

        let max_diff_chars = commit_config.max_diff_chars;
        let branch_context = self
            .repo_session
            .active_repo_state()
            .and_then(|state| state.head_branch.clone())
            .unwrap_or_else(|| "HEAD".to_string());

        let Some(handle) = self.repo_session.require_active_repo_handle() else {
            cx.notify();
            return;
        };

        let fx = OpEffects {
            refresh_repo: true,
            refresh_prs: false,
            remote_status: Some("Committing and pushing...".to_string()),
            error_channel: super::dispatch::ErrorChannel::Toast,
            busy: None,
        };

        if let Some(status) = fx.remote_status.clone() {
            self.repo_session.remote_status = status;
            cx.notify();
        }
        let clear_status = fx.remote_status.is_some();
        let label = "Commit & Push".to_string();

        cx.spawn(async move |this, cx| {
            let progress_id = this
                .update(cx, |app, cx| {
                    let initial = if use_feature_branch {
                        "Generating branch name...".to_string()
                    } else {
                        "Committing and pushing...".to_string()
                    };
                    app.push_progress_toast(initial, cx)
                })
                .ok()
                .unwrap_or(0);

            let mut update_progress = |msg: String| {
                if progress_id == 0 {
                    return;
                }
                this.update(cx, |app, cx| {
                    app.update_progress_toast(progress_id, msg, cx);
                })
                .ok();
            };

            let result = async {
                let diff =
                    with_repo_blocking(handle.clone(), |repo| repo.diff_head_to_index(None))
                        .await?;
                if diff.trim().is_empty() {
                    return Err(AppError::Remote(RemoteError::info(
                        "No staged changes to commit",
                    )));
                }

                let pushed_branch = if use_feature_branch {
                    update_progress("Generating branch name...".to_string());
                    let branch_provider_name = provider_name.clone();
                    let branch_provider_config = provider_config.clone();
                    let provider = spawn_blocking_ok(move || {
                        gitforge_ai::create_provider(&branch_provider_name, &branch_provider_config)
                    })
                    .await?;
                    let raw_name = provider
                        .generate_branch_name(&diff, &branch_context, max_diff_chars)
                        .await?;
                    let branch_name = gitforge_ai::sanitize_branch_name(&raw_name);
                    update_progress(format!("Creating branch {branch_name}..."));
                    let create_name = branch_name.clone();
                    with_repo_blocking(handle.clone(), move |repo| {
                        repo.create_and_checkout_branch(&create_name, None)
                    })
                    .await?;
                    branch_name
                } else {
                    current_branch
                };

                let message = if !existing_message.is_empty() {
                    existing_message
                } else {
                    if use_feature_branch {
                        update_progress("Generating commit message...".to_string());
                    }
                    let message_provider_name = provider_name.clone();
                    let message_provider_config = provider_config.clone();
                    let provider = spawn_blocking_ok(move || {
                        gitforge_ai::create_provider(&message_provider_name, &message_provider_config)
                    })
                    .await?;
                    let messages = provider
                        .generate_commit_messages(&diff, &commit_config)
                        .await?;
                    let default_idx = gitforge_ai::pick_default_message(
                        &messages,
                        &commit_config.default_alternative,
                    );
                    messages.get(default_idx).cloned().ok_or_else(|| {
                        AppError::Remote(RemoteError::error("AI returned no commit message"))
                    })?
                };

                if use_feature_branch {
                    update_progress(format!("Committing and pushing to origin/{pushed_branch}..."));
                }

                with_repo_blocking(handle, move |repo| {
                    repo.commit(&message)?;
                    repo.push("origin", Some(&pushed_branch), false, true)?;
                    Ok(pushed_branch)
                })
                .await
            }
            .await;

            let action = plan_dispatch(&label, result, &fx);
            this.update(cx, |this, cx| {
                if action.refresh_repo {
                    this.refresh_repository(cx);
                }
                if action.refresh_prs {
                    this.refresh_pull_requests(cx, PullRequestRefreshMode::Initial);
                }
                if progress_id != 0 {
                    if let Some(pushed_branch) = action.value {
                        this.repo_session.take_commit_message();
                        this.finish_progress_toast(
                            progress_id,
                            ToastKind::Success,
                            format!("Committed and pushed to origin/{pushed_branch}"),
                            cx,
                        );
                    } else {
                        match action.surface {
                            Surface::Info(msg) => {
                                this.finish_progress_toast(progress_id, ToastKind::Info, msg, cx);
                            }
                            Surface::Error(msg) => {
                                this.finish_progress_toast(progress_id, ToastKind::Error, msg, cx);
                            }
                            Surface::Silent => {
                                this.dismiss_toast(progress_id, cx);
                            }
                        }
                    }
                } else {
                    match action.surface {
                        Surface::Silent => {}
                        Surface::Info(msg) => this.push_toast(ToastKind::Info, msg, cx),
                        Surface::Error(msg) => this.push_toast(ToastKind::Error, msg, cx),
                    }
                    if let Some(pushed_branch) = action.value {
                        this.repo_session.take_commit_message();
                        this.push_toast(
                            ToastKind::Success,
                            format!("Committed and pushed to origin/{pushed_branch}"),
                            cx,
                        );
                    }
                }
                if clear_status {
                    this.repo_session.remote_status.clear();
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
}
