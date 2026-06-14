use gitforge_git::{GitError, Repository};
use gpui::*;

use std::path::PathBuf;

use super::super::settings_window::SettingsSection;
use crate::views::app::{AppDialog, GitForgeApp};

impl GitForgeApp {
    pub fn open_create_branch_dialog(
        &mut self,
        start_point: Option<String>,
        cx: &mut Context<Self>,
    ) {
        self.active_dialog = AppDialog::CreateBranch { start_point };
        self.dialog_input.clear();
        cx.notify();
    }

    pub fn open_rename_branch_dialog(&mut self, old_name: String, cx: &mut Context<Self>) {
        self.active_dialog = AppDialog::RenameBranch { old_name };
        self.dialog_input.clear();
        cx.notify();
    }

    pub fn open_delete_branch_dialog(&mut self, name: String, force: bool, cx: &mut Context<Self>) {
        if self.is_current_branch(&name) {
            self.push_toast(
                crate::views::toasts::ToastKind::Warning,
                format!("Cannot delete the currently checked-out branch '{}'.", name),
                cx,
            );
            return;
        }
        self.dialog_force = force;
        self.active_dialog = AppDialog::DeleteBranch { name };
        self.dialog_input.clear();
        cx.notify();
    }

    pub fn open_stash_push_dialog(&mut self, cx: &mut Context<Self>) {
        self.active_dialog = AppDialog::StashPush;
        self.dialog_input.clear();
        cx.notify();
    }

    pub fn cancel_dialog(&mut self, cx: &mut Context<Self>) {
        self.active_dialog = AppDialog::None;
        self.dialog_input.clear();
        self.dialog_input_2.clear();
        self.dialog_force = false;
        cx.notify();
    }

    pub fn edit_dialog_input(&mut self, typed_char: Option<&str>, cx: &mut Context<Self>) {
        self.dialog_input.edit(typed_char);
        cx.notify();
    }

    pub fn edit_dialog_input_2(&mut self, typed_char: Option<&str>, cx: &mut Context<Self>) {
        self.dialog_input_2.edit(typed_char);
        cx.notify();
    }

    pub fn confirm_dialog(&mut self, cx: &mut Context<Self>) {
        let input = self.dialog_input.text().trim().to_string();
        let input_2 = self.dialog_input_2.text().trim().to_string();
        let dialog_force = self.dialog_force;
        let dialog = self.active_dialog.clone();
        self.active_dialog = AppDialog::None;
        self.dialog_input.clear();
        self.dialog_input_2.clear();
        self.dialog_force = false;

        match dialog {
            AppDialog::CreateBranch { start_point } => {
                if input.is_empty() {
                    return;
                }
                self.create_branch(input, start_point, cx);
            }
            AppDialog::RenameBranch { old_name } => {
                if input.is_empty() {
                    return;
                }
                self.rename_branch(old_name, input, cx);
            }
            AppDialog::DeleteBranch { name } => {
                self.delete_branch(name, dialog_force, cx);
            }
            AppDialog::CreateTag { target } => {
                if input.is_empty() {
                    return;
                }
                self.create_tag(input, target, cx);
            }
            AppDialog::StashPush => {
                self.stash_push(if input.is_empty() { None } else { Some(input) }, cx);
            }
            AppDialog::Push { .. } => {
                let branch = if input.is_empty() {
                    match self.repo_session.active_repo_state() {
                        Some(rs) => rs
                            .references
                            .iter()
                            .find(|r| r.is_head && r.kind == gitforge_git::RefKind::Branch)
                            .map(|r| r.name.clone()),
                        None => None,
                    }
                } else {
                    Some(input)
                };
                let Some(branch_name) = branch else { return };
                self.push_current_branch("origin".into(), branch_name, false, cx);
            }
            AppDialog::Pull { .. } => {
                let remote = if input.is_empty() {
                    "origin".into()
                } else {
                    input
                };
                self.pull_from_remote(remote, false, cx);
            }
            AppDialog::CloneRepo => {
                if input.is_empty() {
                    return;
                }
                let parts: Vec<&str> = input.splitn(2, ' ').collect();
                if parts.len() < 2 {
                    return;
                }
                self.clone_repository(parts[0].to_string(), parts[1].to_string(), cx);
            }
            AppDialog::AddRemote => {
                let parts: Vec<&str> = input.split_whitespace().collect();
                if parts.len() < 2 {
                    return;
                }
                self.add_remote(parts[0].to_string(), parts[1].to_string(), cx);
            }
            AppDialog::SshGenerateKey => {
                let email = if input.is_empty() {
                    "user@example.com".to_string()
                } else {
                    input
                };
                self.generate_ssh_key("ed25519".to_string(), email, cx);
            }
            AppDialog::SshTestConnection => {
                let host = if input.is_empty() {
                    "github.com".to_string()
                } else {
                    input
                };
                self.test_ssh_connection(host, cx);
            }
            AppDialog::CredentialAdd => {
                let parts: Vec<&str> = input.split_whitespace().collect();
                if parts.len() < 2 {
                    return;
                }
                let password = if input_2.is_empty() {
                    return;
                } else {
                    input_2
                };
                self.add_credential(parts[0].to_string(), parts[1].to_string(), password, cx);
            }
            AppDialog::CloneFromHosting { .. } => {}
            AppDialog::SearchHosting { provider } => {
                if input.is_empty() {
                    return;
                }
                self.search_hosting_repos(input, provider, cx);
            }
            AppDialog::ForkRepo {
                owner,
                repo,
                provider,
            } => {
                self.fork_repo(owner, repo, provider, cx);
            }
            AppDialog::CreateWorktree => {
                if input.is_empty() {
                    return;
                }
                let path = input.to_string();
                let refname = if input_2.is_empty() {
                    None
                } else {
                    Some(input_2)
                };
                self.create_worktree(path, refname, None, cx);
            }
            AppDialog::RemoveWorktree { path } => {
                self.remove_worktree(path, true, cx);
            }
            AppDialog::InitRepo { parent } => {
                let name = input.trim().to_string();
                if name.is_empty() {
                    return;
                }
                self.init_repository(parent, name, cx);
            }
            AppDialog::CreatePullRequest => {}
            AppDialog::None => {}
        }
    }
    pub fn open_create_worktree_dialog(&mut self, cx: &mut Context<Self>) {
        self.active_dialog = AppDialog::CreateWorktree;
        self.dialog_input.clear();
        self.dialog_input_2.clear();
        cx.notify();
    }

    pub fn open_remove_worktree_dialog(&mut self, path: String, cx: &mut Context<Self>) {
        self.active_dialog = AppDialog::RemoveWorktree { path };
        self.dialog_input.clear();
        cx.notify();
    }

    pub fn toggle_sidebar_worktrees(&mut self, cx: &mut Context<Self>) {
        self.repo_session.sidebar_state.worktrees_expanded =
            !self.repo_session.sidebar_state.worktrees_expanded;
        cx.notify();
    }

    pub fn open_push_dialog(&mut self, cx: &mut Context<Self>) {
        self.active_dialog = AppDialog::Push {
            branch: None,
            remote: None,
        };
        self.dialog_input.clear();
        cx.notify();
    }

    pub fn open_pull_dialog(&mut self, cx: &mut Context<Self>) {
        self.active_dialog = AppDialog::Pull { remote: None };
        self.dialog_input.clear();
        cx.notify();
    }

    pub fn open_clone_dialog(&mut self, cx: &mut Context<Self>) {
        self.active_dialog = AppDialog::CloneRepo;
        self.dialog_input.clear();
        cx.notify();
    }

    pub fn open_add_remote_dialog(&mut self, cx: &mut Context<Self>) {
        self.active_dialog = AppDialog::AddRemote;
        self.dialog_input.clear();
        cx.notify();
    }
    pub fn open_ssh_generate_key_dialog(&mut self, cx: &mut Context<Self>) {
        self.active_dialog = AppDialog::SshGenerateKey;
        self.dialog_input.clear();
        self.dialog_input_2.clear();
        cx.notify();
    }

    fn add_credential(
        &mut self,
        host: String,
        username: String,
        password: String,
        cx: &mut Context<Self>,
    ) {
        cx.spawn(async move |this, cx| {
            let result = tokio::task::spawn_blocking(move || {
                gitforge_git::credential::store_credential(&host, &username, &password, None)
            })
            .await;

            match result {
                Ok(Ok(())) => {
                    this.update(cx, |this, cx| {
                        this.repo_session.remote_status =
                            "Credential stored in keyring".to_string();
                        cx.notify();
                    })
                    .ok();
                }
                Ok(Err(e)) => {
                    this.update(cx, |this, cx| {
                        this.report_op_error("Store credential", &e.to_string(), cx);
                    })
                    .ok();
                }
                Err(e) => {
                    this.update(cx, |this, cx| {
                        this.report_op_error("Store credential", &e.to_string(), cx);
                    })
                    .ok();
                }
            }
        })
        .detach();
    }

    pub fn open_manage_accounts_dialog(&mut self, cx: &mut Context<Self>) {
        self.open_settings_window(Some(SettingsSection::Accounts), cx);
    }

    pub fn open_search_hosting_dialog(&mut self, provider: String, cx: &mut Context<Self>) {
        self.active_dialog = AppDialog::SearchHosting {
            provider: provider.clone(),
        };
        self.hosting_repos.clear();
        self.hosting_repos_loading = false;
        self.dialog_input.clear();
        cx.notify();
    }
    pub fn spawn_init_repo_picker(&mut self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let path =
                cx.update(|_cx| rfd::AsyncFileDialog::new().set_title("Select Parent Directory"));
            let folder = match path {
                Ok(dialog) => dialog.pick_folder().await,
                Err(_) => None,
            };

            let Some(folder) = folder else {
                return;
            };

            let parent = std::path::PathBuf::from(folder.path());
            this.update(cx, |this, cx| {
                this.active_dialog = AppDialog::InitRepo { parent };
                this.dialog_input.set_text("new-repo");
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn init_repository(&mut self, parent: PathBuf, name: String, cx: &mut Context<Self>) {
        let repo_path = parent.join(&name);
        cx.spawn(async move |this, cx| {
            let result = tokio::task::spawn_blocking(move || {
                std::fs::create_dir_all(&repo_path)
                    .map_err(|e| GitError::OperationFailed(e.to_string()))?;
                Repository::init_repo(&repo_path, false)?;
                Ok::<PathBuf, GitError>(repo_path)
            })
            .await;

            match result {
                Ok(Ok(path)) => {
                    this.update(cx, |this, cx| {
                        this.open_or_activate_repo_tab(path, cx);
                    })
                    .ok();
                }
                Ok(Err(e)) => {
                    this.update(cx, |this, cx| {
                        this.repo_session.last_error =
                            Some(format!("Failed to init repository: {}", e));
                        cx.notify();
                    })
                    .ok();
                }
                Err(e) => {
                    this.update(cx, |this, cx| {
                        this.repo_session.last_error = Some(format!("Init task panicked: {}", e));
                        cx.notify();
                    })
                    .ok();
                }
            }
        })
        .detach();
    }
    pub fn open_in_file_manager(&mut self, path: std::path::PathBuf, _cx: &mut Context<Self>) {
        let _ = std::process::Command::new("xdg-open").arg(&path).spawn();
    }
}
