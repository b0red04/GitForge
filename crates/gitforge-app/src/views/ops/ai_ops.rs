use gpui::Context;

use crate::views::app::GitForgeApp;

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
        let provider_setup =
            tokio::task::spawn_blocking(move || {
                gitforge_ai::create_provider(&provider_name, &provider_config)
            });
        let Some(open_repo) = self.repo_session.require_active_repo_handle() else {
            cx.notify();
            return;
        };

        self.ai_generating = true;
        cx.notify();

        cx.spawn(async move |this, cx| {
            let diff_result = tokio::task::spawn_blocking(move || {
                let repo_lock = open_repo.lock();
                let Some(repo) = repo_lock.as_ref() else {
                    return Err::<String, anyhow::Error>(anyhow::anyhow!("No repository open"));
                };
                repo.diff_head_to_index(None)
                    .map_err(|e| anyhow::anyhow!("{}", e))
            })
            .await;

            let diff = match diff_result {
                Ok(Ok(d)) => d,
                Ok(Err(e)) => {
                    tracing::error!("Failed to get staged diff: {}", e);
                    this.update(cx, |this, cx| {
                        this.ai_generating = false;
                        cx.notify();
                    })
                    .ok();
                    return;
                }
                Err(e) => {
                    tracing::error!("Diff task panicked: {}", e);
                    this.update(cx, |this, cx| {
                        this.ai_generating = false;
                        cx.notify();
                    })
                    .ok();
                    return;
                }
            };

            if diff.trim().is_empty() {
                tracing::warn!("No staged changes to generate commit message from");
                this.update(cx, |this, cx| {
                    this.ai_generating = false;
                    cx.notify();
                })
                .ok();
                return;
            }

            let provider = match provider_setup.await {
                Ok(Ok(p)) => p,
                Ok(Err(e)) => {
                    tracing::error!("Failed to create AI provider: {}", e);
                    this.update(cx, |this, cx| {
                        this.ai_generating = false;
                        this.repo_session.last_error = Some(format!("AI provider error: {e}"));
                        cx.notify();
                    })
                    .ok();
                    return;
                }
                Err(e) => {
                    tracing::error!("Provider setup task panicked: {}", e);
                    this.update(cx, |this, cx| {
                        this.ai_generating = false;
                        cx.notify();
                    })
                    .ok();
                    return;
                }
            };

            match provider.generate_commit_messages(&diff, &commit_config).await {
                Ok(messages) => {
                    let default_idx =
                        gitforge_ai::pick_default_message(&messages, &commit_config.default_alternative);
                    this.update(cx, |this, cx| {
                        this.ai_generating = false;
                        if !messages.is_empty() {
                            if let Some(msg) = messages.get(default_idx) {
                                this.repo_session.status_panel.commit_editor.set_message(msg);
                            }
                            this.repo_session.status_panel.commit_editor.set_ai_alternatives(messages);
                        }
                        cx.notify();
                    })
                    .ok();
                }
                Err(e) => {
                    tracing::error!("AI generation failed: {}", e);
                    this.update(cx, |this, cx| {
                        this.ai_generating = false;
                        this.repo_session.last_error = Some(format!("AI generation failed: {e}"));
                        cx.notify();
                    })
                    .ok();
                }
            }
        })
        .detach();
    }

    pub fn select_ai_alternative(&mut self, idx: usize, cx: &mut Context<Self>) {
        self.repo_session.status_panel.commit_editor.accept_ai_suggestion(idx);
        cx.notify();
    }
}
