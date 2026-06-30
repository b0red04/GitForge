use gpui::Context;

use crate::views::app::{AppDialog, CommitPushMode, GitForgeApp};
use crate::views::ops::dispatch::{
    AppError, OpEffects, RemoteError, spawn_blocking_ok, with_repo_blocking,
};
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
        self.commit_push_generating_branch = false;
        self.dialog_input.clear();
        self.active_dialog = AppDialog::CommitAndPush {
            current_branch: current_branch.clone(),
            detached,
        };
        cx.notify();

        if detached {
            self.generate_feature_branch_name(cx);
        }
    }

    pub fn set_commit_push_mode(&mut self, mode: CommitPushMode, cx: &mut Context<Self>) {
        self.commit_push_mode = mode;
        if mode == CommitPushMode::FeatureBranch && self.dialog_input.text().trim().is_empty() {
            self.generate_feature_branch_name(cx);
        }
        cx.notify();
    }

    pub fn set_dialog_input_text(&mut self, text: &str, cx: &mut Context<Self>) {
        self.dialog_input.set_text(text);
        cx.notify();
    }

    pub fn confirm_commit_and_push(&mut self, current_branch: String, cx: &mut Context<Self>) {
        let use_feature = self.commit_push_mode == CommitPushMode::FeatureBranch;
        let branch_name = if use_feature {
            gitforge_ai::sanitize_branch_name(self.dialog_input.text().trim())
        } else {
            current_branch
        };
        self.commit_push_mode = CommitPushMode::CurrentBranch;
        self.commit_push_generating_branch = false;
        self.dialog_input.clear();
        self.execute_commit_and_push(use_feature, branch_name, cx);
    }

    pub fn execute_commit_and_push(
        &mut self,
        use_feature_branch: bool,
        branch_name: String,
        cx: &mut Context<Self>,
    ) {
        let existing_message = self.repo_session.commit_editor.message().trim().to_string();
        let needs_ai_message = existing_message.is_empty();

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
        if needs_ai_message {
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

        let Some(handle) = self.repo_session.require_active_repo_handle() else {
            cx.notify();
            return;
        };

        let fx = OpEffects {
            refresh_repo: true,
            refresh_prs: false,
            remote_status: Some("Committing and pushing...".to_string()),
            error_channel: super::dispatch::ErrorChannel::Toast,
        };

        let pushed_branch = branch_name.clone();
        self.run_op_full(
            "Commit & Push",
            cx,
            fx,
            move || async move {
                let diff = with_repo_blocking(handle.clone(), |repo| repo.diff_head_to_index(None))
                    .await?;
                if diff.trim().is_empty() {
                    return Err(AppError::Remote(RemoteError::info(
                        "No staged changes to commit",
                    )));
                }

                if use_feature_branch {
                    let create_name = branch_name.clone();
                    with_repo_blocking(handle.clone(), move |repo| {
                        repo.create_and_checkout_branch(&create_name, None)
                    })
                    .await?;
                }

                let message = if !existing_message.is_empty() {
                    existing_message
                } else {
                    let provider = spawn_blocking_ok(move || {
                        gitforge_ai::create_provider(&provider_name, &provider_config)
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

                with_repo_blocking(handle, move |repo| {
                    repo.commit(&message)?;
                    repo.push("origin", Some(&pushed_branch), false, true)?;
                    Ok(pushed_branch)
                })
                .await
            },
            move |this, pushed_branch, cx| {
                this.repo_session.take_commit_message();
                this.push_toast(
                    ToastKind::Success,
                    format!("Committed and pushed to origin/{pushed_branch}"),
                    cx,
                );
            },
            None,
            None,
        );
    }
}
