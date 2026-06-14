use gpui::*;

use super::super::settings_window::SettingsSection;
use crate::views::app::{AppDialog, GitForgeApp};
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
        self.hosting_repos.clear();
        self.hosting_repos_loading = false;
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
