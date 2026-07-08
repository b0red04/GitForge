use gitforge_git::{GitError, RebaseAction, RebasePlan, RebasePlanEntry};
use gpui::Context;

use crate::views::app::{AppDialog, GitForgeApp};
use crate::views::dialogs::squash_wizard::{SquashWizardEntry, SquashWizardState, SquashWizardStep};
use crate::views::ops::dispatch::{
    AppError, BusyFlag, OpEffects, RemoteError, Surface, plan_dispatch, spawn_blocking_ok,
    with_repo_blocking,
};
use crate::views::toasts::ToastKind;

impl GitForgeApp {
    pub fn open_squash_wizard(&mut self, cx: &mut Context<Self>) {
        if self
            .squash_wizard
            .as_ref()
            .is_some_and(|w| w.submitting)
        {
            return;
        }
        if self
            .repo_session
            .active_repo_state()
            .is_some_and(|s| s.rebase_in_progress)
        {
            self.push_toast(
                ToastKind::Warning,
                "Finish or abort the current rebase first".to_string(),
                cx,
            );
            return;
        }

        self.run_git_blocking(
            "Load squash commits",
            cx,
            OpEffects::QUIET,
            |repo| {
                if repo.is_rebase_in_progress() {
                    return Err(GitError::OperationFailed(
                        "A rebase is already in progress".into(),
                    ));
                }
                let branch = repo
                    .head_branch()?
                    .ok_or_else(|| GitError::OperationFailed("Detached HEAD".into()))?;
                let onto_ref = repo.squash_onto_ref(&branch)?;
                let onto = repo.merge_base(&onto_ref, "HEAD")?;
                let commits = repo.commits_in_range(&onto, "HEAD")?;
                if commits.len() < 2 {
                    return Err(GitError::OperationFailed(format!(
                        "Need at least 2 commits since {onto_ref} to squash (found {})",
                        commits.len()
                    )));
                }
                let needs_force_push = repo.remote_branch_exists("origin", &branch)?;
                Ok((branch, onto, commits, needs_force_push))
            },
            |this, (branch, onto, commits, needs_force_push), cx| {
                let entries = commits
                    .iter()
                    .map(|c| SquashWizardEntry {
                        sha: c.id.clone(),
                        short_id: c.short_id.clone(),
                        summary: c.summary.clone(),
                        action: RebaseAction::Pick,
                        message: None,
                    })
                    .collect();

                this.squash_wizard = Some(SquashWizardState::new(
                    branch.clone(),
                    onto,
                    entries,
                    cx,
                ));
                if let Some(wizard) = this.squash_wizard.as_mut() {
                    wizard.needs_force_push = needs_force_push;
                }
                this.active_dialog = AppDialog::SquashWizard;
                cx.notify();
            },
        );
    }

    pub fn squash_wizard_squash_all(&mut self, cx: &mut Context<Self>) {
        let Some(wizard) = self.squash_wizard.as_mut() else {
            return;
        };
        if wizard.entries.is_empty() {
            return;
        }
        for (i, entry) in wizard.entries.iter_mut().enumerate() {
            entry.action = if i == 0 {
                RebaseAction::Pick
            } else {
                RebaseAction::Squash
            };
        }
        wizard.open_action_dropdown = None;
        wizard.open_action_bounds = None;
        let msg = wizard
            .entries
            .last()
            .map(|e| e.summary.clone())
            .unwrap_or_default();
        wizard.combined_message = msg.clone();
        wizard.message_input.set_text(&msg);
        cx.notify();
    }

    pub fn close_squash_action_dropdown(&mut self, cx: &mut Context<Self>) {
        if let Some(wizard) = self.squash_wizard.as_mut()
            && wizard.open_action_dropdown.is_some()
        {
            wizard.open_action_dropdown = None;
            wizard.open_action_bounds = None;
            cx.notify();
        }
    }

    pub fn toggle_squash_action_dropdown(&mut self, idx: usize, cx: &mut Context<Self>) {
        let Some(wizard) = self.squash_wizard.as_mut() else {
            return;
        };
        if wizard.open_action_dropdown == Some(idx) {
            wizard.open_action_dropdown = None;
            wizard.open_action_bounds = None;
        } else {
            wizard.open_action_dropdown = Some(idx);
            wizard.open_action_bounds = wizard
                .action_trigger_bounds
                .get(idx)
                .and_then(|b| *b);
        }
        cx.notify();
    }

    pub fn select_squash_action(
        &mut self,
        idx: usize,
        action: RebaseAction,
        cx: &mut Context<Self>,
    ) {
        if idx == 0 && matches!(action, RebaseAction::Squash | RebaseAction::Fixup) {
            return;
        }
        if let Some(wizard) = self.squash_wizard.as_mut()
            && let Some(entry) = wizard.entries.get_mut(idx)
        {
            entry.action = action;
            wizard.open_action_dropdown = None;
            wizard.open_action_bounds = None;
            cx.notify();
        }
    }

    pub fn cycle_squash_entry_action(&mut self, idx: usize, cx: &mut Context<Self>) {
        let Some(wizard) = self.squash_wizard.as_mut() else {
            return;
        };
        let Some(entry) = wizard.entries.get_mut(idx) else {
            return;
        };
        let actions = RebaseAction::available_for_entry(idx);
        let current = actions
            .iter()
            .position(|a| *a == entry.action)
            .unwrap_or(0);
        entry.action = actions[(current + 1) % actions.len()];
        cx.notify();
    }

    pub fn set_squash_entry_action(
        &mut self,
        idx: usize,
        action: RebaseAction,
        cx: &mut Context<Self>,
    ) {
        if let Some(wizard) = self.squash_wizard.as_mut()
            && let Some(entry) = wizard.entries.get_mut(idx)
        {
            entry.action = action;
            cx.notify();
        }
    }

    pub fn move_squash_entry(&mut self, idx: usize, up: bool, cx: &mut Context<Self>) {
        let Some(wizard) = self.squash_wizard.as_mut() else {
            return;
        };
        let target = if up { idx.wrapping_sub(1) } else { idx + 1 };
        if target >= wizard.entries.len() || (up && idx == 0) {
            return;
        }
        wizard.entries.swap(idx, target);
        wizard.open_action_dropdown = None;
        wizard.open_action_bounds = None;
        cx.notify();
    }

    pub fn squash_wizard_next(&mut self, cx: &mut Context<Self>) {
        let Some(wizard) = self.squash_wizard.as_mut() else {
            return;
        };
        if wizard.step != SquashWizardStep::EditPlan {
            return;
        }
        if wizard.has_squash_action() {
            wizard.combined_message = wizard.message_input.text().to_string();
        } else {
            wizard.combined_message = wizard
                .entries
                .last()
                .map(|e| e.summary.clone())
                .unwrap_or_default();
            wizard.message_input.set_text(&wizard.combined_message);
        }
        wizard.step = SquashWizardStep::ReviewMessages;
        cx.notify();
    }

    pub fn squash_wizard_back(&mut self, cx: &mut Context<Self>) {
        if let Some(wizard) = self.squash_wizard.as_mut() {
            wizard.step = SquashWizardStep::EditPlan;
            cx.notify();
        }
    }

    pub fn edit_squash_message(&mut self, typed_char: Option<&str>, cx: &mut Context<Self>) {
        if let Some(wizard) = self.squash_wizard.as_mut() {
            wizard.message_input.edit(typed_char);
            wizard.combined_message = wizard.message_input.text().to_string();
            cx.notify();
        }
    }

    pub fn generate_squash_commit_message(&mut self, cx: &mut Context<Self>) {
        let Some(wizard) = self.squash_wizard.as_ref() else {
            return;
        };
        if wizard.step != SquashWizardStep::ReviewMessages {
            return;
        }

        let provider_name = self.settings.ai.provider.clone();
        if provider_name == "disabled" {
            self.push_toast(
                ToastKind::Warning,
                "Enable an AI provider in Settings",
                cx,
            );
            return;
        }

        let commit_config = self.settings.ai.commit_message_config();
        let mut provider_config = self.settings.ai.provider_config();
        let model = provider_config.model_or_default(&provider_name);
        if model.is_empty() {
            self.push_toast(
                ToastKind::Warning,
                format!("No model configured for provider \"{provider_name}\""),
                cx,
            );
            return;
        }
        provider_config.model = model;

        let onto = wizard.onto.clone();
        let expect_onto = onto.clone();
        let expect_token = wizard.generation_token;

        let Some(handle) = self.repo_session.require_active_repo_handle() else {
            return;
        };

        self.run_op_full(
            "Generate squash commit message",
            cx,
            OpEffects {
                busy: Some(BusyFlag::SquashWizardGeneratingAi {
                    token: expect_token,
                }),
                ..OpEffects::QUIET
            },
            move || async move {
                let diff =
                    with_repo_blocking(handle, move |repo| repo.unified_diff_between_refs(&onto, "HEAD"))
                        .await?;
                if diff.trim().is_empty() {
                    return Err(AppError::Remote(RemoteError::info(
                        "No changes in the commits being squashed",
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
                messages.get(default_idx).cloned().ok_or_else(|| {
                    AppError::Remote(RemoteError::error("AI returned no commit message"))
                })
            },
            move |this, message, cx| {
                if let Some(wizard) = this.squash_wizard.as_mut()
                    && wizard.onto == expect_onto
                    && wizard.generation_token == expect_token
                {
                    wizard.message_input.set_text(&message);
                    wizard.combined_message = message;
                }
                cx.notify();
            },
            None,
            None,
        );
    }

    pub fn execute_squash_wizard(&mut self, cx: &mut Context<Self>) {
        let Some(wizard_ref) = self.squash_wizard.as_ref() else {
            return;
        };
        if wizard_ref.submitting {
            return;
        }
        let plan = self.build_squash_rebase_plan(wizard_ref);
        if let Err(err) = plan.validate() {
            self.push_toast(ToastKind::Error, err.to_string(), cx);
            return;
        }
        if wizard_ref.has_squash_action()
            && plan
                .combined_message
                .as_deref()
                .is_some_and(|m| m.trim().is_empty())
        {
            self.push_toast(
                ToastKind::Error,
                "Enter a commit message before squashing".to_string(),
                cx,
            );
            return;
        }

        let needs_force_push = wizard_ref.needs_force_push;
        let branch = wizard_ref.branch.clone();
        if let Some(wizard) = self.squash_wizard.as_mut() {
            wizard.submitting = true;
        }
        self.active_dialog = AppDialog::None;
        cx.notify();

        let progress_id = self.push_progress_toast("Squashing commits...", cx);

        let handle = match self.repo_session.git_op_readiness() {
            crate::views::repo_session::GitOpReadiness::Ready(handle) => handle,
            _ => {
                self.dismiss_toast(progress_id, cx);
                if let Some(wizard) = self.squash_wizard.as_mut() {
                    wizard.submitting = false;
                }
                self.active_dialog = AppDialog::SquashWizard;
                return;
            }
        };

        cx.spawn(async move |this, cx| {
            let result = with_repo_blocking(handle, move |repo| repo.rebase_interactive(&plan)).await;
            let action = plan_dispatch("Squash commits", result, &OpEffects::GIT);
            this.update(cx, |this, cx| {
                if action.refresh_repo {
                    this.refresh_repository(cx);
                }
                if action.value.is_some() {
                    this.squash_wizard = None;
                    let success = if needs_force_push {
                        "Commits squashed — updating remote branch..."
                    } else {
                        "Commits squashed successfully"
                    };
                    this.finish_progress_toast(
                        progress_id,
                        ToastKind::Success,
                        success,
                        cx,
                    );
                    if needs_force_push {
                        this.push_current_branch("origin".into(), branch, true, cx);
                    }
                } else {
                    if let Some(wizard) = this.squash_wizard.as_mut() {
                        wizard.submitting = false;
                    }
                    this.active_dialog = AppDialog::SquashWizard;
                    match action.surface {
                        Surface::Error(msg) => {
                            this.finish_progress_toast(progress_id, ToastKind::Error, msg, cx);
                        }
                        Surface::Info(msg) => {
                            this.finish_progress_toast(progress_id, ToastKind::Info, msg, cx);
                        }
                        Surface::Silent => this.dismiss_toast(progress_id, cx),
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn build_squash_rebase_plan(&self, wizard: &SquashWizardState) -> RebasePlan {
        let message = wizard.message_input.text().trim().to_string();
        RebasePlan {
            onto: wizard.onto.clone(),
            entries: wizard
                .entries
                .iter()
                .map(|e| RebasePlanEntry {
                    sha: e.sha.clone(),
                    short_id: e.short_id.clone(),
                    summary: e.summary.clone(),
                    action: e.action,
                    message: e.message.clone(),
                })
                .collect(),
            combined_message: if wizard.has_squash_action() {
                Some(message)
            } else {
                None
            },
        }
    }

    pub fn rebase_continue_op(&mut self, cx: &mut Context<Self>) {
        self.run_git_blocking(
            "Rebase continue",
            cx,
            OpEffects::GIT,
            |repo| repo.rebase_continue(),
            |_, _, _| {},
        );
    }

    pub fn rebase_skip_op(&mut self, cx: &mut Context<Self>) {
        self.run_git_blocking(
            "Rebase skip",
            cx,
            OpEffects::GIT,
            |repo| repo.rebase_skip(),
            |_, _, _| {},
        );
    }

    pub fn rebase_abort_op(&mut self, cx: &mut Context<Self>) {
        self.run_git_blocking(
            "Rebase abort",
            cx,
            OpEffects::GIT,
            |repo| repo.rebase_abort(),
            |_, _, _| {},
        );
    }
}

#[cfg(test)]
mod tests {
    use gitforge_git::{RebaseAction, RebasePlan, RebasePlanEntry};

    #[test]
    fn squash_all_plan_validates() {
        let plan = RebasePlan {
            onto: "base".into(),
            entries: vec![
                entry(0, RebaseAction::Pick),
                entry(1, RebaseAction::Squash),
                entry(2, RebaseAction::Squash),
            ],
            combined_message: Some("squashed".into()),
        };
        assert!(plan.validate().is_ok());
    }

    #[test]
    fn invalid_squash_first_rejected() {
        let plan = RebasePlan {
            onto: "base".into(),
            entries: vec![
                entry(0, RebaseAction::Squash),
                entry(1, RebaseAction::Pick),
            ],
            combined_message: None,
        };
        assert!(plan.validate().is_err());
    }

    fn entry(idx: usize, action: RebaseAction) -> RebasePlanEntry {
        let id = format!("{:040x}", idx);
        RebasePlanEntry {
            sha: id.clone(),
            short_id: id[..7].to_string(),
            summary: format!("commit {idx}"),
            action,
            message: None,
        }
    }
}
