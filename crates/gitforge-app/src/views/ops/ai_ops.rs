use gpui::Context;

use crate::views::app::GitForgeApp;
use crate::views::ops::dispatch::{
    AppError, BusyFlag, OpEffects, RemoteError, spawn_blocking_ok, with_repo_blocking,
};

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

        self.run_op_full(
            "Generate commit message",
            cx,
            OpEffects {
                busy: Some(BusyFlag::AiGenerating),
                ..OpEffects::QUIET
            },
            move || async move {
                let diff = with_repo_blocking(handle, |repo| repo.diff_head_to_index(None)).await?;
                if diff.trim().is_empty() {
                    return Err(AppError::Remote(RemoteError::info(
                        "No staged changes to generate a commit message from",
                    )));
                }
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
                Ok((messages, default_idx))
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
            None,
            None,
        );
    }

    pub fn select_ai_alternative(&mut self, idx: usize, cx: &mut Context<Self>) {
        self.repo_session.commit_editor.accept_ai_suggestion(idx);
        cx.notify();
    }
}
