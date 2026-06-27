use gpui::*;

use super::super::settings_window::SettingsSection;
use crate::views::app::{AppDialog, CommitPushMode, GitForgeApp};
use crate::views::dialogs::{self, init_repo};

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

    pub fn open_create_tag_dialog(&mut self, target: Option<String>, cx: &mut Context<Self>) {
        self.active_dialog = AppDialog::CreateTag { target };
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

    pub fn open_delete_remote_branch_dialog(&mut self, full_name: String, cx: &mut Context<Self>) {
        let Some((remote, branch)) = full_name.split_once('/') else {
            self.push_toast(
                crate::views::toasts::ToastKind::Warning,
                format!("'{full_name}' is not a valid remote branch."),
                cx,
            );
            return;
        };
        let (remote, branch) = (remote.to_string(), branch.to_string());
        // Cheap synchronous UI-layer guard for the common default branch names.
        // The git layer performs the authoritative check via main_branch_name().
        if branch == "main" || branch == "master" {
            self.push_toast(
                crate::views::toasts::ToastKind::Warning,
                format!("Refusing to delete the default branch '{branch}'."),
                cx,
            );
            return;
        }
        self.active_dialog = AppDialog::DeleteRemoteBranch { remote, branch };
        cx.notify();
    }

    pub fn cancel_dialog(&mut self, cx: &mut Context<Self>) {
        self.active_dialog = AppDialog::None;
        self.dialog_input.clear();
        self.dialog_input_2.clear();
        self.dialog_force = false;
        self.commit_push_mode = CommitPushMode::default();
        self.commit_push_generating_branch = false;
        self.hosting_repos.clear();
        self.hosting_repos_loading = false;
        self.add_repo_tab = crate::views::dialogs::AddRepoTab::Local;
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
        if let AppDialog::SearchHosting { provider } = self.active_dialog.clone() {
            let input = self.dialog_input.text().trim().to_string();
            if input.is_empty() {
                return;
            }
            self.dialog_input.clear();
            self.search_hosting_repos(input, provider, cx);
            return;
        }

        let input = self.dialog_input.text().trim().to_string();
        let input_2 = self.dialog_input_2.text().trim().to_string();
        let dialog_force = self.dialog_force;
        let dialog = self.active_dialog.clone();
        self.active_dialog = AppDialog::None;
        self.dialog_input.clear();
        self.dialog_input_2.clear();
        self.dialog_force = false;

        dialogs::confirm(self, dialog, &input, &input_2, dialog_force, cx);
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
        init_repo::spawn_init_repo_picker(self, cx);
    }

    pub fn open_in_file_manager(&mut self, path: std::path::PathBuf, _cx: &mut Context<Self>) {
        let _ = std::process::Command::new("xdg-open").arg(&path).spawn();
    }
}
