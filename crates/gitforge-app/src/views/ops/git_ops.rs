use gitforge_git::RepoState;
use gitforge_graph::{CommitEntry, Graph};
use gpui::*;


use crate::views::app::{GitForgeApp, MainViewMode};
use crate::views::diff_panel::CommitDiffState;
use crate::views::status_panel::StatusFileSection;

impl GitForgeApp {

    pub(crate) fn apply_repo_state_to_panels(&mut self, repo_state_data: &RepoState) {
        let has_uncommitted = repo_state_data.status.has_changes();

        let commit_count = repo_state_data.commits.len();
        let start = std::time::Instant::now();

        let commit_entries: Vec<CommitEntry> = repo_state_data
            .commits
            .iter()
            .map(|c| CommitEntry::new(c.id.clone(), c.parent_ids.clone()))
            .collect();
        let built_graph = Graph::build(&commit_entries);

        let elapsed = start.elapsed();
        tracing::info!(
            "Graph::build: {} commits in {:.2}ms ({:.0} commits/ms)",
            commit_count,
            elapsed.as_secs_f64() * 1000.0,
            commit_count as f64 / elapsed.as_secs_f64().max(0.001) / 1000.0,
        );

        self.graph_panel.set_data(
            repo_state_data.commits.clone(),
            repo_state_data.references.clone(),
            built_graph,
            has_uncommitted,
        );
        let in_history = self.view_mode == MainViewMode::CommitHistory;
        let preserve_staging =
            in_history && self.status_panel.is_graph_staging();
        self.status_panel
            .set_status(repo_state_data.status.clone(), preserve_staging);
        self.diff_panel.clear();

        if has_uncommitted {
            self.graph_panel.select_uncommitted();
            if in_history {
                self.status_panel.enter_graph_staging();
            }
        } else {
            self.graph_panel.clear_selection();
        }
    }

    pub(crate) fn apply_repo_state(&mut self, repo_state_data: RepoState) {
        self.apply_repo_state_to_panels(&repo_state_data);
        if let Some(tab) = self.active_tab_mut() {
            tab.path = repo_state_data.path.clone();
            tab.repo_state = Some(repo_state_data.clone());
            tab.loading = false;
            tab.last_error = None;
        }
    }

    pub fn select_uncommitted(&mut self, cx: &mut Context<Self>) {
        self.view_mode = MainViewMode::CommitHistory;
        self.graph_panel.select_uncommitted();
        self.diff_panel.clear();
        self.status_panel.enter_graph_staging();
        cx.notify();
    }

    pub fn select_commit(&mut self, idx: usize, cx: &mut Context<Self>) {
        self.view_mode = MainViewMode::CommitHistory;
        self.graph_panel.select_commit(idx);
        self.status_panel.exit_graph_staging();
        self.load_diff_for_selected(cx);
    }

    pub(crate) fn on_graph_selection_changed(&mut self, cx: &mut Context<Self>) {
        use crate::views::graph_panel::GraphSelection;

        match self.graph_panel.selection() {
            GraphSelection::Uncommitted => {
                self.view_mode = MainViewMode::CommitHistory;
                self.diff_panel.clear();
                self.status_panel.enter_graph_staging();
                cx.notify();
            }
            GraphSelection::Commit(_) => {
                self.status_panel.exit_graph_staging();
                self.load_diff_for_selected(cx);
            }
            GraphSelection::None => {
                self.diff_panel.clear();
                cx.notify();
            }
        }
    }

    pub fn select_diff_file(&mut self, file_idx: usize, cx: &mut Context<Self>) {
        self.diff_panel.select_file(file_idx);
        cx.notify();
    }

    pub fn view_file_at_commit(&mut self, file_path: String, cx: &mut Context<Self>) {
        let Some(idx) = self.graph_panel.selected_idx() else {
            return;
        };
        let Some(commit_id) = self.graph_panel.commit_id_at(idx).map(String::from) else {
            return;
        };

        let Some(open_repo) = self.require_active_repo_handle() else {
            cx.notify();
            return;
        };
        let path_for_result = file_path.clone();

        cx.spawn(async move |this, cx| {
            let fp = file_path;
            let result = tokio::task::spawn_blocking(move || {
                let repo_lock = open_repo.lock();
                let Some(repo) = repo_lock.as_ref() else {
                    return Err(gitforge_git::GitError::OperationFailed(
                        "No repository open".into(),
                    ));
                };
                let data = repo.file_at_commit(&commit_id, std::path::Path::new(&fp))?;
                Ok(data)
            })
            .await;

            match result {
                Ok(Ok(Some(data))) => {
                    let content = String::from_utf8_lossy(&data).to_string();
                    let fp = path_for_result;
                    this.update(cx, |this, cx| {
                        this.diff_panel.set_code_view(content, fp);
                        cx.notify();
                    })
                    .ok();
                }
                Ok(Ok(None)) => {
                    tracing::info!("File not found at commit");
                }
                Ok(Err(e)) => {
                    tracing::warn!("Failed to load file: {}", e);
                }
                Err(e) => {
                    tracing::warn!("File load task panicked: {}", e);
                }
            }
        })
        .detach();
    }

    pub fn back_to_diff_mode(&mut self, cx: &mut Context<Self>) {
        self.diff_panel.set_diff_mode();
        cx.notify();
    }
    pub fn toggle_sidebar_branches(&mut self, cx: &mut Context<Self>) {
        self.sidebar_state.branches_expanded = !self.sidebar_state.branches_expanded;
        self.save_settings();
        cx.notify();
    }

    pub fn toggle_sidebar_remotes(&mut self, cx: &mut Context<Self>) {
        self.sidebar_state.remotes_expanded = !self.sidebar_state.remotes_expanded;
        self.save_settings();
        cx.notify();
    }

    pub fn toggle_sidebar_tags(&mut self, cx: &mut Context<Self>) {
        self.sidebar_state.tags_expanded = !self.sidebar_state.tags_expanded;
        self.save_settings();
        cx.notify();
    }

    pub fn toggle_sidebar_remote(&mut self, remote: String, cx: &mut Context<Self>) {
        if self.sidebar_state.expanded_remotes.contains(&remote) {
            self.sidebar_state.expanded_remotes.remove(&remote);
        } else {
            self.sidebar_state.expanded_remotes.insert(remote);
        }
        cx.notify();
    }

    pub fn select_diff_line(&mut self, line_idx: usize, extend: bool, cx: &mut Context<Self>) {
        self.diff_panel.select_line(line_idx, extend);
        cx.notify();
    }

    pub fn update_sidebar_filter(&mut self, typed_char: Option<&str>, cx: &mut Context<Self>) {
        match typed_char {
            Some(ch) => {
                self.sidebar_state.search_filter.push_str(ch);
            }
            None => {
                self.sidebar_state.search_filter.pop();
            }
        }
        cx.notify();
    }

    pub fn clear_sidebar_filter(&mut self, cx: &mut Context<Self>) {
        self.sidebar_state.search_filter.clear();
        cx.notify();
    }

    pub fn navigate_to_ref(&mut self, commit_id: String, cx: &mut Context<Self>) {
        if let Some(idx) = self.graph_panel.find_commit_idx(&commit_id) {
            self.select_commit(idx, cx);
        }
    }

    pub fn set_branch_filter(&mut self, branch: Option<String>, cx: &mut Context<Self>) {
        self.graph_panel.set_branch_filter(branch);
        cx.notify();
    }

    pub fn toggle_checkpoint_refs(&mut self, cx: &mut Context<Self>) {
        self.settings.show_checkpoint_refs = !self.settings.show_checkpoint_refs;
        self.refresh_repository(cx);
    }

    pub fn select_status_file(
        &mut self,
        section: StatusFileSection,
        idx: usize,
        path: String,
        cx: &mut Context<Self>,
    ) {
        self.status_panel.select_file(section, idx);

        let Some(open_repo) = self.require_active_repo_handle() else {
            cx.notify();
            return;
        };
        let is_staged = section == StatusFileSection::Staged;

        cx.spawn(async move |this, cx| {
            let result = tokio::task::spawn_blocking(move || {
                let repo_lock = open_repo.lock();
                let Some(repo) = repo_lock.as_ref() else {
                    return Err(gitforge_git::GitError::OperationFailed(
                        "No repository open".into(),
                    ));
                };
                let diff_text = if is_staged {
                    repo.diff_head_to_index(Some(std::path::Path::new(&path)))?
                } else {
                    repo.diff_index_to_worktree(Some(std::path::Path::new(&path)))?
                };
                Ok(diff_text)
            })
            .await;

            match result {
                Ok(Ok(diff_text)) => {
                    let file_diffs = gitforge_diff::parser::parse_unified_diff(&diff_text);
                    this.update(cx, |this, cx| {
                        if let Some(diff) = file_diffs.into_iter().next() {
                            this.status_panel.set_diff(diff);
                        }
                        cx.notify();
                    })
                    .ok();
                }
                Ok(Err(e)) => {
                    tracing::warn!("Failed to load status diff: {}", e);
                }
                Err(e) => {
                    tracing::warn!("Status diff task panicked: {}", e);
                }
            }
        })
        .detach();
    }

    pub fn show_commit_dialog(&mut self, cx: &mut Context<Self>) {
        self.status_panel.show_commit();
        cx.notify();
    }

    pub fn cancel_commit_dialog(&mut self, cx: &mut Context<Self>) {
        self.status_panel.cancel_commit();
        cx.notify();
    }

    pub fn edit_commit_message(&mut self, typed_char: Option<&str>, cx: &mut Context<Self>) {
        let msg = self.status_panel.commit_message().to_string();
        let mut new_msg = msg;
        match typed_char {
            Some(ch) => new_msg.push_str(ch),
            None => {
                new_msg.pop();
            }
        }
        self.status_panel.commit_message_mut().clear();
        self.status_panel.commit_message_mut().push_str(&new_msg);
        cx.notify();
    }

    pub fn perform_commit(&mut self, amend: bool, cx: &mut Context<Self>) {
        let message = self.status_panel.take_commit_message();
        if message.trim().is_empty() {
            return;
        }

        let Some(open_repo) = self.require_active_repo_handle() else {
            cx.notify();
            return;
        };

        cx.spawn(async move |this, cx| {
            let result = tokio::task::spawn_blocking(move || {
                let repo_lock = open_repo.lock();
                let Some(repo) = repo_lock.as_ref() else {
                    return Err(gitforge_git::GitError::OperationFailed(
                        "No repository open".into(),
                    ));
                };
                if amend {
                    repo.commit_amend(&message)?;
                } else {
                    repo.commit(&message)?;
                }
                Ok(())
            })
            .await;

            match result {
                Ok(Ok(())) => {
                    this.update(cx, |this, cx| {
                        this.refresh_repository(cx);
                    })
                    .ok();
                }
                Ok(Err(e)) => {
                    tracing::error!("Commit failed: {}", e);
                }
                Err(e) => {
                    tracing::error!("Commit task panicked: {}", e);
                }
            }
        })
        .detach();
    }
    pub fn load_status(&mut self, cx: &mut Context<Self>) {
        let Some(open_repo) = self.require_active_repo_handle() else {
            cx.notify();
            return;
        };

        cx.spawn(async move |this, cx| {
            let result = tokio::task::spawn_blocking(move || {
                let repo_lock = open_repo.lock();
                let Some(repo) = repo_lock.as_ref() else {
                    return Err(gitforge_git::GitError::OperationFailed(
                        "No repository open".into(),
                    ));
                };
                repo.status()
            })
            .await;

            match result {
                Ok(Ok(status)) => {
                    this.update(cx, |this, cx| {
                        this.status_panel.set_status(status, false);
                        cx.notify();
                    })
                    .ok();
                }
                Ok(Err(e)) => {
                    tracing::warn!("Failed to load status: {}", e);
                }
                Err(e) => {
                    tracing::warn!("Status task panicked: {}", e);
                }
            }
        })
        .detach();
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
        let Some(diff) = self.status_panel.current_diff().cloned() else {
            return;
        };
        let indices = self.status_panel.diff_selected_indices();
        if indices.is_empty() {
            return;
        }

        let path = diff
            .new_path
            .as_deref()
            .or(diff.old_path.as_deref())
            .unwrap_or("")
            .to_string();

        let hunks = gitforge_diff::extract_patch_from_selection(&diff.lines, &indices);
        if hunks.is_empty() {
            return;
        }

        let patch = format!("--- a/{}\n+++ b/{}\n{}", path, path, hunks);

        self.run_git_op("Stage lines", cx, move |repo| {
            repo.apply_patch(&patch, true, false)
        });
    }

    pub fn unstage_selected_lines(&mut self, cx: &mut Context<Self>) {
        let Some(diff) = self.status_panel.current_diff().cloned() else {
            return;
        };
        let indices = self.status_panel.diff_selected_indices();
        if indices.is_empty() {
            return;
        }

        let path = diff
            .new_path
            .as_deref()
            .or(diff.old_path.as_deref())
            .unwrap_or("")
            .to_string();

        let hunks = gitforge_diff::extract_patch_from_selection(&diff.lines, &indices);
        if hunks.is_empty() {
            return;
        }

        let patch = format!("--- a/{}\n+++ b/{}\n{}", path, path, hunks);

        self.run_git_op("Unstage lines", cx, move |repo| {
            repo.apply_patch(&patch, true, true)
        });
    }

    pub fn select_status_diff_line(
        &mut self,
        line_idx: usize,
        extend: bool,
        cx: &mut Context<Self>,
    ) {
        self.status_panel.select_diff_line(line_idx, extend);
        cx.notify();
    }

    pub fn soft_reset(&mut self, cx: &mut Context<Self>) {
        self.run_git_op("Soft reset", cx, move |repo| repo.soft_reset_head(1));
    }
    pub(crate) fn run_git_op<F, R>(&mut self, label: &str, cx: &mut Context<Self>, op: F)
    where
        F: FnOnce(&gitforge_git::Repository) -> Result<R, gitforge_git::GitError> + Send + 'static,
        R: Send + 'static,
    {
        let Some(open_repo) = self.require_active_repo_handle() else {
            cx.notify();
            return;
        };
        let label_owned = label.to_string();
        cx.spawn(async move |this, cx| {
            let result = tokio::task::spawn_blocking(move || {
                let repo_lock = open_repo.lock();
                let Some(repo) = repo_lock.as_ref() else {
                    return Err(gitforge_git::GitError::OperationFailed(
                        "No repository open".into(),
                    ));
                };
                op(repo)
            })
            .await;
            match result {
                Ok(Ok(_)) => {
                    this.update(cx, |this, cx| {
                        this.refresh_repository(cx);
                    })
                    .ok();
                }
                Ok(Err(e)) => tracing::error!("{} failed: {}", label_owned, e),
                Err(e) => tracing::error!("{} task panicked: {}", label_owned, e),
            }
        })
        .detach();
    }

    pub(crate) fn run_git_op_with_status<F, R>(
        &mut self,
        label: &str,
        status: &str,
        cx: &mut Context<Self>,
        op: F,
    ) where
        F: FnOnce(&gitforge_git::Repository) -> Result<R, gitforge_git::GitError> + Send + 'static,
        R: Send + 'static,
    {
        let Some(open_repo) = self.require_active_repo_handle() else {
            cx.notify();
            return;
        };
        self.remote_status = status.to_string();
        cx.notify();
        let label_owned = label.to_string();
        cx.spawn(async move |this, cx| {
            let result = tokio::task::spawn_blocking(move || {
                let repo_lock = open_repo.lock();
                let Some(repo) = repo_lock.as_ref() else {
                    return Err(gitforge_git::GitError::OperationFailed(
                        "No repository open".into(),
                    ));
                };
                op(repo)
            })
            .await;
            match result {
                Ok(Ok(_)) => {
                    this.update(cx, |this, cx| {
                        this.remote_status.clear();
                        this.refresh_repository(cx);
                    })
                    .ok();
                }
                Ok(Err(e)) => {
                    tracing::error!("{} failed: {}", label_owned, e);
                    this.update(cx, |this, cx| {
                        this.remote_status = format!("{} failed: {}", label_owned, e);
                        cx.notify();
                    })
                    .ok();
                }
                Err(e) => {
                    tracing::error!("{} error: {}", label_owned, e);
                    this.update(cx, |this, cx| {
                        this.remote_status = format!("{} error: {}", label_owned, e);
                        cx.notify();
                    })
                    .ok();
                }
            }
        })
        .detach();
    }

    pub fn create_branch(
        &mut self,
        name: String,
        start_point: Option<String>,
        cx: &mut Context<Self>,
    ) {
        self.run_git_op("Create branch", cx, move |repo| {
            repo.create_branch(&name, start_point.as_deref())
        });
    }

    pub fn delete_branch(&mut self, name: String, force: bool, cx: &mut Context<Self>) {
        self.run_git_op("Delete branch", cx, move |repo| {
            repo.delete_branch(&name, force)
        });
    }

    pub fn rename_branch(&mut self, old: String, new: String, cx: &mut Context<Self>) {
        self.run_git_op("Rename branch", cx, move |repo| {
            repo.rename_branch(&old, &new)
        });
    }

    pub fn checkout_branch(&mut self, name: String, cx: &mut Context<Self>) {
        self.run_git_op("Checkout", cx, move |repo| repo.checkout_branch(&name));
    }

    pub fn checkout_commit(&mut self, sha: String, cx: &mut Context<Self>) {
        self.run_git_op("Checkout commit", cx, move |repo| {
            repo.checkout_commit(&sha)
        });
    }

    pub fn merge_branch(&mut self, branch: String, no_ff: bool, cx: &mut Context<Self>) {
        self.run_git_op("Merge", cx, move |repo| repo.merge(&branch, no_ff));
    }

    pub fn rebase_branch(&mut self, branch: String, cx: &mut Context<Self>) {
        self.run_git_op("Rebase", cx, move |repo| repo.rebase(&branch));
    }

    pub fn mixed_reset(&mut self, reference: String, cx: &mut Context<Self>) {
        self.run_git_op("Mixed reset", cx, move |repo| repo.mixed_reset(&reference));
    }

    pub fn hard_reset(&mut self, reference: String, cx: &mut Context<Self>) {
        self.run_git_op("Hard reset", cx, move |repo| repo.hard_reset(&reference));
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
        self.run_git_op_with_status("Fetch", "Fetching all remotes...", cx, move |repo| {
            repo.fetch_all(true)
        });
    }

    pub fn fetch_remote(&mut self, remote: String, cx: &mut Context<Self>) {
        let status = format!("Fetching {}...", remote);
        self.run_git_op_with_status("Fetch", &status, cx, move |repo| {
            repo.fetch(Some(&remote), true)
        });
    }

    pub fn push_current_branch(
        &mut self,
        remote: String,
        branch: String,
        force: bool,
        cx: &mut Context<Self>,
    ) {
        let status = format!("Pushing {} to {}...", branch, remote);
        self.run_git_op_with_status("Push", &status, cx, move |repo| {
            repo.push(&remote, Some(&branch), force, true)
        });
    }

    pub fn pull_from_remote(&mut self, remote: String, rebase: bool, cx: &mut Context<Self>) {
        let status = format!("Pulling from {}...", remote);
        self.run_git_op_with_status("Pull", &status, cx, move |repo| {
            repo.pull(Some(&remote), rebase)
        });
    }

    pub fn clone_repository(&mut self, url: String, path: String, cx: &mut Context<Self>) {
        self.remote_status = format!("Cloning {}...", url);
        cx.notify();

        cx.spawn(async move |this, cx| {
            let path_buf = std::path::PathBuf::from(&path);
            let result = tokio::task::spawn_blocking(move || {
                gitforge_git::Repository::clone_repo(&url, &path_buf, false, None)
            })
            .await;

            match result {
                Ok(Ok(_output)) => {
                    this.update(cx, |this, cx| {
                        this.remote_status.clear();
                        this.open_repo_from_path(std::path::PathBuf::from(path), cx);
                    })
                    .ok();
                }
                Ok(Err(e)) => {
                    tracing::error!("Clone failed: {}", e);
                    this.update(cx, |this, cx| {
                        this.remote_status = format!("Clone failed: {}", e);
                        cx.notify();
                    })
                    .ok();
                }
                Err(e) => {
                    tracing::error!("Clone task panicked: {}", e);
                    this.update(cx, |this, cx| {
                        this.remote_status = format!("Clone error: {}", e);
                        cx.notify();
                    })
                    .ok();
                }
            }
        })
        .detach();
    }

    pub fn add_remote(&mut self, name: String, url: String, cx: &mut Context<Self>) {
        self.run_git_op("Add remote", cx, move |repo| repo.remote_add(&name, &url));
    }

    pub fn remove_remote(&mut self, name: String, cx: &mut Context<Self>) {
        self.run_git_op("Remove remote", cx, move |repo| repo.remote_remove(&name));
    }
    pub fn add_to_gitignore(&mut self, path: String, cx: &mut Context<Self>) {
        self.run_git_op("Add to gitignore", cx, move |repo| {
            repo.add_to_gitignore(&path)
        });
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
        let Some(open_repo) = self.require_active_repo_handle() else {
            cx.notify();
            return;
        };
        let path_for_result = file_path.clone();

        cx.spawn(async move |this, cx| {
            let fp = file_path;
            let result = tokio::task::spawn_blocking(move || {
                let repo_lock = open_repo.lock();
                let Some(repo) = repo_lock.as_ref() else {
                    return Err(gitforge_git::GitError::OperationFailed(
                        "No repository open".into(),
                    ));
                };
                repo.blame_file(std::path::Path::new(&fp), None)
            })
            .await;

            match result {
                Ok(Ok(blame_lines)) => {
                    this.update(cx, |this, cx| {
                        this.diff_panel.set_blame(blame_lines, path_for_result);
                        cx.notify();
                    })
                    .ok();
                }
                Ok(Err(e)) => tracing::warn!("Failed to load blame: {}", e),
                Err(e) => tracing::warn!("Blame task panicked: {}", e),
            }
        })
        .detach();
    }

    pub(crate) fn refresh_repository(&mut self, cx: &mut Context<Self>) {
        self.save_settings();
        let Some(open_repo) = self.require_active_repo_handle() else {
            cx.notify();
            return;
        };
        let log_options = gitforge_git::CommitLogOptions {
            include_custom_refs: self.settings.show_checkpoint_refs,
        };

        cx.spawn(async move |this, cx| {
            let result = tokio::task::spawn_blocking(move || {
                let repo_lock = open_repo.lock();
                let Some(repo) = repo_lock.as_ref() else {
                    return Err(gitforge_git::GitError::OperationFailed(
                        "No repository open".into(),
                    ));
                };
                RepoState::from_repository_with_options(repo, log_options)
            })
            .await;

            match result {
                Ok(Ok(repo_state)) => {
                    this.update(cx, |this, cx| {
                        this.apply_repo_state(repo_state);
                        cx.notify();
                    })
                    .ok();
                }
                Ok(Err(e)) => {
                    tracing::error!("Refresh failed: {}", e);
                }
                Err(e) => {
                    tracing::error!("Refresh task panicked: {}", e);
                }
            }
        })
        .detach();
    }

    pub(crate) fn load_diff_for_selected(&mut self, cx: &mut Context<Self>) {
        let Some(idx) = self.graph_panel.selected_idx() else {
            return;
        };
        let Some(commit_id) = self.graph_panel.commit_id_at(idx).map(String::from) else {
            return;
        };

        let Some(open_repo) = self.require_active_repo_handle() else {
            cx.notify();
            return;
        };
        let id_for_state = commit_id.clone();

        cx.spawn(async move |this, cx| {
            let raw_diff = tokio::task::spawn_blocking(move || {
                let repo_lock = open_repo.lock();
                let Some(repo) = repo_lock.as_ref() else {
                    return Err(gitforge_git::GitError::OperationFailed(
                        "No repository open".into(),
                    ));
                };
                repo.unified_diff_for_commit(&commit_id)
            })
            .await;

            match raw_diff {
                Ok(Ok(diff_text)) => {
                    let file_diffs = gitforge_diff::parser::parse_unified_diff(&diff_text);

                    this.update(cx, |this, cx| {
                        this.diff_panel.set_diff(CommitDiffState {
                            commit_id: id_for_state,
                            file_diffs,
                            selected_file_idx: None,
                        });
                        cx.notify();
                    })
                    .ok();
                }
                Ok(Err(e)) => {
                    tracing::warn!("Failed to load diff: {}", e);
                }
                Err(e) => {
                    tracing::warn!("Diff task panicked: {}", e);
                }
            }
        })
        .detach();
    }
}
