use gpui::Context;

use crate::views::app::GitForgeApp;
use crate::views::ops::dispatch::{OpEffects, run_ai_generation};

impl GitForgeApp {
    pub fn generate_commit_message(&mut self, cx: &mut Context<Self>) {
        let provider_name = self.settings.ai.provider.clone();
        if provider_name == "disabled" {
            return;
        }
        let commit_config = self.settings.ai.commit_message_config();
        let mut provider_config = self.settings.ai.provider_config();
        let model = provider_config.model_or_default(&provider_name);
        if model.is_empty() {
            self.repo_session.last_error = Some(format!(
                "No model configured for provider \"{provider_name}\""
            ));
            cx.notify();
            return;
        }
        provider_config.model = model.clone();
        let Some(handle) = self.repo_session.require_active_repo_handle() else {
            cx.notify();
            return;
        };

        self.ai_generating = true;
        cx.notify();

        self.run_op_full(
            "Generate commit message",
            cx,
            OpEffects::QUIET,
            move || {
                run_ai_generation(
                    handle,
                    |repo| repo.diff_head_to_index(None),
                    "No staged changes to generate a commit message from".to_string(),
                    provider_name,
                    provider_config,
                    move |provider, diff| async move {
                        let messages = provider
                            .generate_commit_messages(&diff, &commit_config)
                            .await?;
                        let default_idx = gitforge_ai::pick_default_message(
                            &messages,
                            &commit_config.default_alternative,
                        );
                        Ok((messages, default_idx))
                    },
                )
            },
            move |this, (messages, default_idx), cx| {
                if !messages.is_empty() {
                    if let Some(msg) = messages.get(default_idx) {
                        this.repo_session.commit_editor.set_message(msg);
                    }
                    this.repo_session
                        .commit_editor
                        .set_ai_alternatives(messages);
                }
                cx.notify();
            },
            None,
            Some(Box::new(|this, _cx| {
                this.ai_generating = false;
            })),
        );
    }

    pub fn select_ai_alternative(&mut self, idx: usize, cx: &mut Context<Self>) {
        self.repo_session.commit_editor.accept_ai_suggestion(idx);
        cx.notify();
    }

    pub fn generate_feature_branch_name(&mut self, cx: &mut Context<Self>) {
        let provider_name = self.settings.ai.provider.clone();
        if provider_name == "disabled" {
            return;
        }
        let mut provider_config = self.settings.ai.provider_config();
        let model = provider_config.model_or_default(&provider_name);
        if model.is_empty() {
            self.repo_session.last_error = Some(format!(
                "No model configured for provider \"{provider_name}\""
            ));
            cx.notify();
            return;
        }
        provider_config.model = model;
        let max_diff_chars = self.settings.ai.commit_message_config().max_diff_chars;
        let current_branch = self
            .repo_session
            .active_repo_state()
            .and_then(|state| state.head_branch.clone())
            .unwrap_or_else(|| "HEAD".to_string());
        let Some(handle) = self.repo_session.require_active_repo_handle() else {
            cx.notify();
            return;
        };

        self.commit_push_generating_branch = true;
        cx.notify();

        self.run_op_full(
            "Generate branch name",
            cx,
            OpEffects::QUIET,
            move || {
                run_ai_generation(
                    handle,
                    |repo| repo.diff_head_to_index(None),
                    "No staged changes to generate a branch name from".to_string(),
                    provider_name,
                    provider_config,
                    move |provider, diff| async move {
                        provider
                            .generate_branch_name(&diff, &current_branch, max_diff_chars)
                            .await
                    },
                )
            },
            move |this, name, cx| {
                this.set_dialog_input_text(&name, cx);
            },
            None,
            Some(Box::new(|this, _cx| {
                this.commit_push_generating_branch = false;
            })),
        );
    }
}
