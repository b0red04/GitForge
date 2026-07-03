use gitforge_git::{GitError, RepoState};
use gpui::*;

use crate::views::app::{AppDialog, GitForgeApp};
use crate::views::ops::pr_ops::PullRequestRefreshMode;
use crate::views::diff_panel::CommitDiffState;
use crate::views::diff_viewer::file_diff_path_or_empty;
use crate::views::graph_panel::GraphSelection;
use crate::views::ops::dispatch::{AppError, OpEffects, Surface, plan_dispatch, with_repo_blocking};
use crate::views::repo_session::{GitOpReadiness, SelectionEffect};
use crate::views::status_panel::StatusFileSection;
use crate::views::toasts::ToastKind;

impl GitForgeApp {
    pub fn select_uncommitted(&mut self, cx: &mut Context<Self>) {
        let effect = self.repo_session.set_selection(GraphSelection::Uncommitted);
        self.apply_selection_effect(effect, cx);
    }

    pub fn select_commit(&mut self, idx: usize, cx: &mut Context<Self>) {
        let effect = self.repo_session.set_selection(GraphSelection::Commit(idx));
        self.apply_selection_effect(effect, cx);
    }

    pub(crate) fn on_graph_selection_changed(&mut self, cx: &mut Context<Self>) {
        let effect = self.repo_session.cascade_current();
        self.apply_selection_effect(effect, cx);
    }

    /// Interpret the [`SelectionEffect`] returned by the Selection Cascade
    /// (ADR-0003). `RepoSession` is GPUI-free and cannot spawn, so it
    /// describes the async work needed and this helper performs it.
    fn apply_selection_effect(&mut self, effect: SelectionEffect, cx: &mut Context<Self>) {
        match effect {
            SelectionEffect::ClearDiff => cx.notify(),
            SelectionEffect::LoadDiffForSelected => {
                cx.notify();
                self.load_diff_for_selected(cx);
            }
        }
    }

    pub fn select_diff_file(&mut self, file_idx: usize, cx: &mut Context<Self>) {
        self.repo_session.diff_panel.select_file(file_idx);
        cx.notify();
    }

    pub fn open_diff_overlay_for_file(&mut self, file_idx: usize, cx: &mut Context<Self>) {
        self.repo_session.diff_panel.select_file(file_idx);
        self.prepare_diff_overlay_state();
        if !self.repo_session.diff_overlay_open {
            self.repo_session.diff_overlay_open = true;
        }
        cx.notify();
    }

    fn prepare_diff_overlay_state(&mut self) {
        self.repo_session.diff_panel.set_diff_mode();
        let (selected_file_idx, file_count) = self
            .repo_session
            .diff_panel
            .diff_state()
            .map(|d| (d.selected_file_idx, d.file_diffs.len()))
            .unwrap_or((None, 0));
        if let Some(file_idx) = normalized_overlay_file_idx(selected_file_idx, file_count) {
            self.repo_session.diff_panel.select_file(file_idx);
        }
    }

    /// Toggle the large diff overlay that renders the selected file's line-level
    /// diff over the sidebar + commit graph. The right-hand file list stays
    /// visible to drive file selection; everything beneath the overlay is
    /// occluded (disabled) while open.
    pub fn toggle_diff_overlay(&mut self, cx: &mut Context<Self>) {
        let opening = !self.repo_session.diff_overlay_open;
        if opening {
            self.prepare_diff_overlay_state();
        }
        self.repo_session.diff_overlay_open = opening;
        cx.notify();
    }

    pub fn view_file_at_commit(&mut self, file_path: String, cx: &mut Context<Self>) {
        let Some(idx) = self.repo_session.graph_panel.selected_idx() else {
            return;
        };
        let Some(commit_id) = self
            .repo_session
            .graph_panel
            .commit_id_at(idx)
            .map(String::from)
        else {
            return;
        };

        let path_for_result = file_path.clone();

        self.run_git_blocking(
            "View file",
            cx,
            super::dispatch::OpEffects::QUIET,
            move |repo| repo.file_at_commit(&commit_id, std::path::Path::new(&file_path)),
            move |this, data, cx| {
                if let Some(data) = data {
                    let content = String::from_utf8_lossy(&data).to_string();
                    this.repo_session
                        .diff_panel
                        .set_code_view(content, path_for_result);
                    cx.notify();
                } else {
                    tracing::info!("File not found at commit");
                    this.repo_session.diff_panel.set_diff_mode();
                    cx.notify();
                }
            },
        );
    }

    pub fn back_to_diff_mode(&mut self, cx: &mut Context<Self>) {
        self.repo_session.diff_panel.set_diff_mode();
        cx.notify();
    }
    pub fn toggle_sidebar_branches(&mut self, cx: &mut Context<Self>) {
        self.repo_session.sidebar_state.toggle_branches();
        self.save_settings();
        cx.notify();
    }

    pub fn toggle_sidebar_remotes(&mut self, cx: &mut Context<Self>) {
        self.repo_session.sidebar_state.toggle_remotes();
        self.save_settings();
        cx.notify();
    }

    pub fn toggle_sidebar_tags(&mut self, cx: &mut Context<Self>) {
        self.repo_session.sidebar_state.toggle_tags();
        self.save_settings();
        cx.notify();
    }

    pub fn toggle_sidebar_worktrees(&mut self, cx: &mut Context<Self>) {
        self.repo_session.sidebar_state.toggle_worktrees();
        cx.notify();
    }

    pub fn toggle_sidebar_pull_requests(&mut self, cx: &mut Context<Self>) {
        self.repo_session.sidebar_state.toggle_pull_requests();
        self.save_settings();
        cx.notify();
    }

    pub fn toggle_sidebar_remote(&mut self, remote: String, cx: &mut Context<Self>) {
        self.repo_session.sidebar_state.toggle_remote(remote);
        cx.notify();
    }

    pub fn select_diff_line(&mut self, line_idx: usize, extend: bool, cx: &mut Context<Self>) {
        self.repo_session.diff_panel.select_line(line_idx, extend);
        cx.notify();
    }

    pub fn update_sidebar_filter(&mut self, typed_char: Option<&str>, cx: &mut Context<Self>) {
        self.repo_session.sidebar_state.update_filter(typed_char);
        cx.notify();
    }

    pub fn clear_sidebar_filter(&mut self, cx: &mut Context<Self>) {
        self.repo_session.sidebar_state.clear_filter();
        cx.notify();
    }

    pub fn navigate_to_ref(&mut self, commit_id: String, cx: &mut Context<Self>) {
        if let Some(idx) = self.repo_session.graph_panel.find_commit_idx(&commit_id) {
            self.select_commit(idx, cx);
        }
    }

    pub fn set_branch_filter(&mut self, branch: Option<String>, cx: &mut Context<Self>) {
        self.repo_session.graph_panel.set_branch_filter(branch);
        cx.notify();
    }

    pub fn select_status_file(
        &mut self,
        section: StatusFileSection,
        idx: usize,
        cx: &mut Context<Self>,
    ) {
        self.repo_session.status_panel.select_file(section, idx);
        cx.notify();
    }

    pub fn open_status_diff(
        &mut self,
        section: StatusFileSection,
        idx: usize,
        path: String,
        cx: &mut Context<Self>,
    ) {
        self.repo_session.status_panel.open_file_diff(section, idx);

        let is_staged = section == StatusFileSection::Staged;

        self.run_git_blocking(
            "Load status diff",
            cx,
            super::dispatch::OpEffects::QUIET,
            move |repo| {
                let diff_text = if is_staged {
                    repo.diff_head_to_index(Some(std::path::Path::new(&path)))?
                } else {
                    repo.diff_index_to_worktree(Some(std::path::Path::new(&path)))?
                };
                Ok(diff_text)
            },
            move |this, diff_text, cx| {
                let file_diffs = gitforge_diff::parser::parse_unified_diff(&diff_text);
                if let Some(diff) = file_diffs.into_iter().next() {
                    this.repo_session.status_panel.set_diff(diff);
                }
                cx.notify();
            },
        );
    }

    pub fn show_commit_dialog(&mut self, cx: &mut Context<Self>) {
        self.repo_session.status_panel.show_commit();
        cx.notify();
    }

    pub fn cancel_commit_dialog(&mut self, cx: &mut Context<Self>) {
        self.repo_session.status_panel.cancel_commit();
        cx.notify();
    }

    pub fn edit_commit_message(&mut self, typed_char: Option<&str>, cx: &mut Context<Self>) {
        match typed_char {
            Some(ch) => self.repo_session.commit_editor.type_char(ch),
            None => self.repo_session.commit_editor.backspace(),
        }
        cx.notify();
    }

    pub fn perform_commit(&mut self, amend: bool, cx: &mut Context<Self>) {
        if self.repo_session.commit_editor.message().trim().is_empty() {
            return;
        }
        let message = self.repo_session.take_commit_message();
        let behavior = self.active_repo_behavior_settings();
        let branch_to_push = self
            .repo_session
            .active_repo_state()
            .and_then(|state| state.head_branch.clone());

        self.run_git_blocking(
            "Commit",
            cx,
            super::dispatch::OpEffects::QUIET,
            move |repo| {
                if amend {
                    repo.commit_amend(&message)?;
                } else {
                    repo.commit(&message)?;
                }
                Ok(())
            },
            move |this, _value: (), cx| {
                this.refresh_repository(cx);
                if behavior.auto_push_on_commit {
                    if let Some(branch) = branch_to_push.clone() {
                        this.push_current_branch("origin".into(), branch, false, cx);
                    } else {
                        this.push_toast(
                            crate::views::toasts::ToastKind::Warning,
                            "Commit succeeded; skipped auto-push because HEAD is detached."
                                .to_string(),
                            cx,
                        );
                    }
                }
            },
        );
    }
    pub fn load_status(&mut self, cx: &mut Context<Self>) {
        self.run_git_blocking(
            "Load status",
            cx,
            super::dispatch::OpEffects::QUIET,
            move |repo| repo.status(),
            move |this, status, cx| {
                this.repo_session.status_panel.set_status(status, false);
                cx.notify();
            },
        );
    }

    /// Called when the GitForge window (re)gains focus from outside the app.
    /// Reloads the working-tree status cheaply, and — if periodic fetch is
    /// enabled and enough time has passed since the last (periodic or
    /// focus-triggered) fetch — kicks off a debounced remote fetch.
    pub(crate) fn on_window_focused(&mut self, cx: &mut Context<Self>) {
        if !self.repo_session.active_repo_ready() {
            return;
        }
        self.load_status(cx);

        let behavior = self.active_repo_behavior_settings();
        if !behavior.periodic_fetch_enabled {
            return;
        }
        let cooldown = std::time::Duration::from_secs(
            behavior.fetch_interval_minutes.max(1).saturating_mul(60),
        );
        if should_focus_fetch(self.last_auto_fetch_at, std::time::Instant::now(), cooldown) {
            self.last_auto_fetch_at = Some(std::time::Instant::now());
            self.fetch_all(cx);
        }
    }

    pub fn stage_file(&mut self, path: String, cx: &mut Context<Self>) {
        self.run_git_op("Stage file", cx, move |repo| {
            let p = std::path::PathBuf::from(&path);
            repo.stage_paths(&[p.as_path()])
        });
    }

    pub fn unstage_file(&mut self, path: String, cx: &mut Context<Self>) {
        self.run_git_op("Unstage file", cx, move |repo| {
            let p = std::path::PathBuf::from(&path);
            repo.unstage_paths(&[p.as_path()])
        });
    }

    pub fn discard_file(&mut self, path: String, cx: &mut Context<Self>) {
        self.run_git_op("Discard file", cx, move |repo| {
            let p = std::path::PathBuf::from(&path);
            repo.discard_worktree_changes(&[p.as_path()])
        });
    }

    pub fn remove_untracked_file(&mut self, path: String, cx: &mut Context<Self>) {
        self.run_git_op("Remove untracked", cx, move |repo| {
            let p = std::path::PathBuf::from(&path);
            repo.remove_untracked(&[p.as_path()])
        });
    }

    pub fn stage_all(&mut self, cx: &mut Context<Self>) {
        self.run_git_op("Stage all", cx, move |repo| repo.stage_all());
    }

    pub fn unstage_all(&mut self, cx: &mut Context<Self>) {
        self.run_git_op("Unstage all", cx, move |repo| repo.unstage_all());
    }

    pub fn stage_selected_lines(&mut self, cx: &mut Context<Self>) {
        self.apply_selected_lines_patch("Stage lines", false, cx);
    }

    pub fn unstage_selected_lines(&mut self, cx: &mut Context<Self>) {
        self.apply_selected_lines_patch("Unstage lines", true, cx);
    }

    fn apply_selected_lines_patch(
        &mut self,
        label: &'static str,
        reverse: bool,
        cx: &mut Context<Self>,
    ) {
        let Some(diff) = self.repo_session.status_panel.current_diff().cloned() else {
            return;
        };
        let indices = self.repo_session.status_panel.diff_selected_indices();
        if indices.is_empty() {
            return;
        }

        let path = file_diff_path_or_empty(&diff).to_string();

        let hunks = gitforge_diff::extract_patch_from_selection(&diff.lines, &indices);
        if hunks.is_empty() {
            return;
        }

        let patch = format!("--- a/{}\n+++ b/{}\n{}", path, path, hunks);

        self.run_git_op(label, cx, move |repo| {
            repo.apply_patch(&patch, true, reverse)
        });
    }

    pub fn select_status_diff_line(
        &mut self,
        line_idx: usize,
        extend: bool,
        cx: &mut Context<Self>,
    ) {
        self.repo_session
            .status_panel
            .select_diff_line(line_idx, extend);
        cx.notify();
    }

    pub fn soft_reset(&mut self, cx: &mut Context<Self>) {
        self.run_git_op("Soft reset", cx, move |repo| repo.soft_reset_head(1));
    }
    /// Fire-and-refresh git op: runs `op`, then refreshes the repository on
    /// success. Thin adapter over `run_git_blocking` with `OpEffects::GIT`.
    pub(crate) fn run_git_op<F, R>(&mut self, label: &str, cx: &mut Context<Self>, op: F)
    where
        F: FnOnce(&gitforge_git::Repository) -> Result<R, gitforge_git::GitError> + Send + 'static,
        R: Send + 'static,
    {
        self.run_git_blocking(label, cx, super::dispatch::OpEffects::GIT, op, |_, _, _| {});
    }

    pub fn create_branch(
        &mut self,
        name: String,
        start_point: Option<String>,
        cx: &mut Context<Self>,
    ) {
        self.run_git_op("Create branch", cx, move |repo| {
            repo.create_and_checkout_branch(&name, start_point.as_deref())
        });
    }

    pub fn delete_branch(&mut self, name: String, force: bool, cx: &mut Context<Self>) {
        if self.is_current_branch(&name) {
            self.push_toast(
                crate::views::toasts::ToastKind::Warning,
                format!("Cannot delete the currently checked-out branch '{}'.", name),
                cx,
            );
            return;
        }
        self.run_git_op("Delete branch", cx, move |repo| {
            repo.delete_branch(&name, force)
        });
    }

    pub(crate) fn is_current_branch(&self, name: &str) -> bool {
        let active_state = self.repo_session.active_repo_state();
        active_state.and_then(|state| state.head_branch.as_deref()) == Some(name)
            || active_state.is_some_and(|state| {
                state.references.iter().any(|reference| {
                    reference.kind == gitforge_git::RefKind::Branch
                        && reference.is_head
                        && reference.name == name
                })
            })
    }

    pub fn rename_branch(&mut self, old: String, new: String, cx: &mut Context<Self>) {
        self.run_git_op("Rename branch", cx, move |repo| {
            repo.rename_branch(&old, &new)
        });
    }

    pub fn checkout_branch(&mut self, name: String, cx: &mut Context<Self>) {
        self.local_branch_dropdown_open = false;
        self.run_git_blocking(
            "Checkout",
            cx,
            super::dispatch::OpEffects::GIT,
            move |repo| repo.checkout_branch(&name),
            |_, _, _| {},
        );
    }

    pub fn checkout_remote_branch(&mut self, name: String, cx: &mut Context<Self>) {
        self.local_branch_dropdown_open = false;
        self.run_git_blocking(
            "Checkout",
            cx,
            super::dispatch::OpEffects::GIT,
            move |repo| repo.checkout_remote_branch(&name),
            |_, _, _| {},
        );
    }

    pub fn merge_branch(&mut self, branch: String, no_ff: bool, cx: &mut Context<Self>) {
        self.run_git_op("Merge", cx, move |repo| repo.merge(&branch, no_ff));
    }

    pub fn cherry_pick(&mut self, sha: String, cx: &mut Context<Self>) {
        self.run_git_op("Cherry-pick", cx, move |repo| repo.cherry_pick(&sha));
    }

    pub fn revert_commit(&mut self, sha: String, cx: &mut Context<Self>) {
        self.run_git_op("Revert", cx, move |repo| repo.revert(&sha));
    }

    pub fn create_tag(&mut self, name: String, target: Option<String>, cx: &mut Context<Self>) {
        self.run_git_op("Create tag", cx, move |repo| {
            repo.create_tag(&name, None, target.as_deref())
        });
    }

    pub fn delete_tag(&mut self, name: String, cx: &mut Context<Self>) {
        self.run_git_op("Delete tag", cx, move |repo| repo.delete_tag(&name));
    }

    pub fn stash_push(&mut self, message: Option<String>, cx: &mut Context<Self>) {
        self.run_git_op("Stash push", cx, move |repo| {
            repo.stash_push(message.as_deref())
        });
    }

    pub fn stash_pop(&mut self, cx: &mut Context<Self>) {
        self.run_git_op("Stash pop", cx, move |repo| repo.stash_pop());
    }

    pub fn create_worktree(
        &mut self,
        path: String,
        refname: Option<String>,
        new_branch: Option<String>,
        cx: &mut Context<Self>,
    ) {
        self.run_git_op("Create worktree", cx, move |repo| {
            let p = std::path::PathBuf::from(&path);
            repo.worktree_add(&p, refname.as_deref(), new_branch.as_deref())
        });
    }

    pub fn remove_worktree(&mut self, path: String, force: bool, cx: &mut Context<Self>) {
        self.run_git_op("Remove worktree", cx, move |repo| {
            let p = std::path::PathBuf::from(&path);
            repo.worktree_remove(&p, force)
        });
    }

    pub fn prune_worktrees(&mut self, cx: &mut Context<Self>) {
        self.run_git_op("Prune worktrees", cx, move |repo| repo.worktree_prune());
    }

    pub fn switch_worktree(&mut self, path: std::path::PathBuf, cx: &mut Context<Self>) {
        self.open_repo_from_path(path, cx);
    }

    pub fn fetch_all(&mut self, cx: &mut Context<Self>) {
        self.run_git_blocking(
            "Fetch",
            cx,
            super::dispatch::OpEffects::git_with_status("Fetching all remotes..."),
            move |repo| repo.fetch_all(true),
            |_, _, _| {},
        );
    }

    pub(crate) fn restart_periodic_fetch(&mut self, cx: &mut Context<Self>) {
        self.periodic_fetch_generation = self.periodic_fetch_generation.wrapping_add(1);
        let generation = self.periodic_fetch_generation;
        let behavior = self.active_repo_behavior_settings();
        if !behavior.periodic_fetch_enabled || self.repo_session.active_tab().is_none() {
            return;
        }
        let interval_secs = behavior.fetch_interval_minutes.max(1).saturating_mul(60);

        cx.spawn(async move |this, cx| {
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(interval_secs)).await;
                let should_continue = this
                    .update(cx, |this, cx| {
                        if this.periodic_fetch_generation != generation {
                            return false;
                        }
                        let behavior = this.active_repo_behavior_settings();
                        if !behavior.periodic_fetch_enabled
                            || this.repo_session.active_tab().is_none()
                        {
                            return false;
                        }
                        this.last_auto_fetch_at = Some(std::time::Instant::now());
                        this.fetch_all(cx);
                        true
                    })
                    .unwrap_or(false);
                if !should_continue {
                    break;
                }
            }
        })
        .detach();
    }

    /// Debounce window for [`fetch_on_activate`]. Coalesces rapid tab
    /// switches so only the finally-active tab actually fetches.
    const FETCH_ON_ACTIVATE_DEBOUNCE_MS: u64 = 400;

    /// Fetches all remotes shortly after a tab becomes active. Debounced by a
    /// generation counter so rapid tab switches coalesce into a single fetch
    /// for the finally-active tab. Quiet: no status banner; errors toast via
    /// `OpEffects::GIT`; success triggers a full `RepoState` refresh. Runs
    /// unconditionally (not gated on `periodic_fetch_enabled`).
    pub(crate) fn fetch_on_activate(&mut self, cx: &mut Context<Self>) {
        self.fetch_on_activate_generation = self.fetch_on_activate_generation.wrapping_add(1);
        let generation = self.fetch_on_activate_generation;
        if self.repo_session.active_tab().is_none() {
            return;
        }
        cx.spawn(async move |this, cx| {
            tokio::time::sleep(std::time::Duration::from_millis(
                Self::FETCH_ON_ACTIVATE_DEBOUNCE_MS,
            ))
            .await;
            this.update(cx, |this, cx| {
                if this.fetch_on_activate_generation != generation {
                    return;
                }
                if !this.repo_session.active_repo_ready() {
                    return;
                }
                this.last_auto_fetch_at = Some(std::time::Instant::now());
                this.run_git_op("Fetch on activate", cx, move |repo| repo.fetch_all(true));
            })
            .ok();
        })
        .detach();
    }

    pub fn fetch_remote(&mut self, remote: String, cx: &mut Context<Self>) {
        let status = format!("Fetching {}...", remote);
        self.run_git_blocking(
            "Fetch",
            cx,
            super::dispatch::OpEffects::git_with_status(&status),
            move |repo| repo.fetch(Some(&remote), true),
            |_, _, _| {},
        );
    }

    pub fn push_current_branch(
        &mut self,
        remote: String,
        branch: String,
        force: bool,
        cx: &mut Context<Self>,
    ) {
        self.execute_push(remote, branch, force, false, cx);
    }

    pub fn pull_from_remote(&mut self, remote: String, rebase: bool, cx: &mut Context<Self>) {
        let status = format!("Pulling {}...", remote);
        self.run_git_blocking(
            "Pull",
            cx,
            super::dispatch::OpEffects::git_with_status(&status),
            move |repo| repo.pull(Some(&remote), rebase),
            |_, _, _| {},
        );
    }

    pub fn push_current(&mut self, cx: &mut Context<Self>) {
        let Some(state) = self.repo_session.active_repo_state() else {
            self.push_toast(
                crate::views::toasts::ToastKind::Warning,
                "Cannot push: no repository open.".to_string(),
                cx,
            );
            return;
        };
        match state.head_branch.clone() {
            Some(branch) => self.execute_push("origin".into(), branch, false, true, cx),
            None => {
                self.push_toast(
                    crate::views::toasts::ToastKind::Warning,
                    "Cannot push: HEAD is detached (no current branch).".to_string(),
                    cx,
                );
            }
        }
    }

    pub fn pull_current(&mut self, cx: &mut Context<Self>) {
        let Some(state) = self.repo_session.active_repo_state() else {
            self.push_toast(
                crate::views::toasts::ToastKind::Warning,
                "Cannot pull: no repository open.".to_string(),
                cx,
            );
            return;
        };
        let head_branch = state.head_branch.clone();
        if head_branch.is_none() {
            self.push_toast(
                crate::views::toasts::ToastKind::Warning,
                "Cannot pull: HEAD is detached (no current branch).".to_string(),
                cx,
            );
            return;
        }
        if let Some(branch) = head_branch {
            if let Some((ahead, behind)) = self.branch_sync_status(&branch)
                && ahead > 0
                && behind > 0
            {
                self.push_toast(
                    ToastKind::Warning,
                    "Your branch and the remote have different histories. If you combined \
                     commits locally, update the remote instead of pulling.",
                    cx,
                );
                self.active_dialog = AppDialog::UpdateRemoteBranch {
                    remote: "origin".into(),
                    branch,
                };
                cx.notify();
                return;
            }
        }
        // No dirty-tree pre-flight: `git pull` itself performs the authoritative
        // "Your local changes ... would be overwritten" check and aborts only
        // when local edits actually conflict with incoming changes. That abort
        // is classified into `GitError::LocalChangesOverwritten` (with an
        // actionable "commit or stash before pulling" toast), so a broad
        // `has_changes()` pre-flight here would only block pulls that would
        // otherwise merge cleanly.
        self.pull_from_remote("origin".into(), false, cx);
    }

    fn branch_sync_status(&self, branch: &str) -> Option<(u32, u32)> {
        self.repo_session.active_repo_state().and_then(|state| {
            state
                .references
                .iter()
                .find(|r| r.name == branch)
                .map(|r| (r.commits_ahead, r.commits_behind))
        })
    }

    fn execute_push(
        &mut self,
        remote: String,
        branch: String,
        force: bool,
        offer_update_remote_dialog: bool,
        cx: &mut Context<Self>,
    ) {
        let handle = match self.repo_session.git_op_readiness() {
            GitOpReadiness::Ready(handle) => handle,
            GitOpReadiness::NoRepo => {
                self.repo_session.last_error = Some("No repository open".into());
                self.push_toast(ToastKind::Warning, "No repository open", cx);
                return;
            }
            GitOpReadiness::Loading => return,
        };

        let status = format!("Pushing {branch} to {remote}...");
        let fx = OpEffects::git_with_status(&status);
        let remote_for_push = remote.clone();
        let branch_for_push = branch.clone();

        cx.spawn(async move |this, cx| {
            let result = with_repo_blocking(handle, move |repo| {
                repo.push(
                    &remote_for_push,
                    Some(&branch_for_push),
                    force,
                    true,
                )
            })
            .await;

            if offer_update_remote_dialog && !force {
                if let Err(AppError::Git(GitError::NonFastForwardPush {
                    remote,
                    branch,
                    ..
                })) = &result
                {
                    this.update(cx, |this, cx| {
                        this.active_dialog = AppDialog::UpdateRemoteBranch {
                            remote: remote.clone(),
                            branch: branch.clone(),
                        };
                        this.repo_session.remote_status.clear();
                        cx.notify();
                    })
                    .ok();
                    return;
                }
            }

            let action = plan_dispatch("Push", result, &fx);
            this.update(cx, |this, cx| {
                if action.refresh_repo {
                    this.refresh_repository(cx);
                }
                match action.surface {
                    Surface::Silent => {}
                    Surface::Info(msg) => this.push_toast(ToastKind::Info, msg, cx),
                    Surface::Error(msg) => this.push_toast(ToastKind::Error, msg, cx),
                }
                this.repo_session.remote_status.clear();
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub fn clone_repository(&mut self, url: String, path: String, cx: &mut Context<Self>) {
        let path_buf = std::path::PathBuf::from(&path);
        self.run_blocking(
            "Clone",
            cx,
            super::dispatch::OpEffects::QUIET,
            move || gitforge_git::Repository::clone_repo(&url, &path_buf, false, None),
            move |this, _output, cx| {
                this.open_repo_from_path(std::path::PathBuf::from(path), cx);
            },
            |_, _, _| {},
        );
    }

    pub fn add_remote(&mut self, name: String, url: String, cx: &mut Context<Self>) {
        self.run_git_op("Add remote", cx, move |repo| repo.remote_add(&name, &url));
    }

    pub fn remove_remote(&mut self, name: String, cx: &mut Context<Self>) {
        self.run_git_op("Remove remote", cx, move |repo| repo.remote_remove(&name));
    }

    // When wiring `remote_rename`/`remote_set_url`: route through `run_git_op`
    // (which sets `OpEffects::GIT` → repo refresh) so `RepoState`'s `remotes`
    // snapshot stays current — else the PR sidebar / "open in browser" read a
    // stale URL until the next manual refresh.

    pub fn delete_remote_branch(&mut self, remote: String, branch: String, cx: &mut Context<Self>) {
        let status = format!("Deleting {remote}/{branch}…");
        self.run_git_blocking(
            "Delete remote branch",
            cx,
            super::dispatch::OpEffects::git_with_status(&status),
            move |repo| repo.delete_remote_branch(&remote, &branch),
            |_, _, _| {},
        );
    }

    pub fn resolve_conflict_ours(&mut self, path: String, cx: &mut Context<Self>) {
        self.run_git_op("Resolve conflict (ours)", cx, move |repo| {
            repo.resolve_conflict_use_ours(std::path::Path::new(&path))
        });
    }

    pub fn resolve_conflict_theirs(&mut self, path: String, cx: &mut Context<Self>) {
        self.run_git_op("Resolve conflict (theirs)", cx, move |repo| {
            repo.resolve_conflict_use_theirs(std::path::Path::new(&path))
        });
    }
    pub fn view_blame(&mut self, file_path: String, cx: &mut Context<Self>) {
        let path_for_result = file_path.clone();

        self.run_git_blocking(
            "Load blame",
            cx,
            super::dispatch::OpEffects::QUIET,
            move |repo| repo.blame_file(std::path::Path::new(&file_path), None),
            move |this, blame_lines, cx| {
                this.repo_session
                    .diff_panel
                    .set_blame(blame_lines, path_for_result);
                cx.notify();
            },
        );
    }

    pub(crate) fn refresh_repository(&mut self, cx: &mut Context<Self>) {
        self.save_settings();
        let load_options = self.load_options();

        self.run_git_blocking(
            "Refresh",
            cx,
            super::dispatch::OpEffects::QUIET,
            move |repo| RepoState::from_repository_with_options(repo, load_options),
            move |this, repo_state, cx| {
                this.repo_session.apply_repo_state(repo_state);
                this.refresh_pull_requests(cx, PullRequestRefreshMode::Background);
                cx.notify();
            },
        );
    }

    pub(crate) fn load_diff_for_selected(&mut self, cx: &mut Context<Self>) {
        let Some(idx) = self.repo_session.graph_panel.selected_idx() else {
            return;
        };
        let Some(commit_id) = self
            .repo_session
            .graph_panel
            .commit_id_at(idx)
            .map(String::from)
        else {
            return;
        };

        let id_for_state = commit_id.clone();

        self.run_git_blocking(
            "Load diff",
            cx,
            super::dispatch::OpEffects::QUIET,
            move |repo| repo.unified_diff_for_commit(&commit_id),
            move |this, diff_text, cx| {
                let file_diffs = gitforge_diff::parser::parse_unified_diff(&diff_text);
                let has_files = !file_diffs.is_empty();
                this.repo_session.diff_panel.set_diff(CommitDiffState::new(
                    id_for_state,
                    file_diffs,
                    None,
                ));
                // Keep the large diff overlay populated while browsing commits
                // via keyboard: auto-select the first file of each newly-loaded
                // commit. With the overlay closed this preserves the existing
                // behaviour (no file pre-selected in the right-hand list).
                if this.repo_session.diff_overlay_open && has_files {
                    this.repo_session.diff_panel.select_file(0);
                }
                cx.notify();
            },
        );
    }
}

/// Pure decision: should a focus-triggered fetch fire, given the last fetch
/// time, the current time, and the cooldown? Extracted from
/// [`GitForgeApp::on_window_focused`] so the debounce logic is unit-testable
/// without a GPUI context.
fn should_focus_fetch(
    last: Option<std::time::Instant>,
    now: std::time::Instant,
    cooldown: std::time::Duration,
) -> bool {
    match last {
        None => true,
        Some(t) => now.saturating_duration_since(t) >= cooldown,
    }
}

fn normalized_overlay_file_idx(
    selected_file_idx: Option<usize>,
    file_count: usize,
) -> Option<usize> {
    if file_count == 0 {
        return None;
    }
    Some(
        selected_file_idx
            .filter(|idx| *idx < file_count)
            .unwrap_or(0),
    )
}

#[cfg(test)]
mod tests {
    use super::{normalized_overlay_file_idx, should_focus_fetch};

    #[test]
    fn focus_fetch_fires_when_never_fetched() {
        assert!(should_focus_fetch(
            None,
            std::time::Instant::now(),
            std::time::Duration::from_secs(60),
        ));
    }

    #[test]
    fn focus_fetch_suppressed_within_cooldown() {
        let now = std::time::Instant::now();
        let last = now - std::time::Duration::from_secs(10);
        assert!(!should_focus_fetch(
            Some(last),
            now,
            std::time::Duration::from_secs(60),
        ));
    }

    #[test]
    fn focus_fetch_fires_at_and_past_cooldown() {
        let now = std::time::Instant::now();
        let cooldown = std::time::Duration::from_secs(60);
        assert!(should_focus_fetch(Some(now - cooldown), now, cooldown));
        assert!(should_focus_fetch(
            Some(now - std::time::Duration::from_secs(120)),
            now,
            cooldown,
        ));
    }

    #[test]
    fn overlay_file_idx_none_when_commit_has_no_files() {
        assert_eq!(normalized_overlay_file_idx(None, 0), None);
        assert_eq!(normalized_overlay_file_idx(Some(3), 0), None);
    }

    #[test]
    fn overlay_file_idx_keeps_valid_selection() {
        assert_eq!(normalized_overlay_file_idx(Some(2), 3), Some(2));
    }

    #[test]
    fn overlay_file_idx_falls_back_to_first_file() {
        assert_eq!(normalized_overlay_file_idx(None, 3), Some(0));
        assert_eq!(normalized_overlay_file_idx(Some(9), 3), Some(0));
    }
}
