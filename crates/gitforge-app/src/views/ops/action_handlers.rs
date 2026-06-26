use gpui::*;

use crate::views::app::{
    AddRemote, AppDialog, BackToDiff, CheckForUpdates, CloneFromGithub, CloneFromGitlab, CloneRepo, CloseDialog,
    CloseTab, CreateBranch, CreatePullRequest, CreateWorktree, FetchAll, GitForgeApp, InitRepo,
    MainViewMode, ManageAccounts, NewTab, OpenAiSettings, OpenInBrowser, OpenInEditor,
    OpenInFileManager, OpenInTerminal, OpenRepoManagement, OpenRepository, OpenSshKey, Preferences,
    PullCurrent, PushCurrent, QuitApp, RefreshRepository, ReopenClosedTab, SelectNextCommit,
    SelectPrevCommit, ShowCommandPalette, ShowHistory, ShowStatusPanel, SoftReset, StashPop,
    StashPush, ToggleTheme, ViewFileAtCommit,
};
use crate::views::commands::{CommandAction, TitlebarMenu};
use crate::views::settings_window::SettingsSection;
use crate::views::sidebar::ContextMenuAction;

impl GitForgeApp {
    pub fn toggle_local_branch_dropdown(&mut self, cx: &mut Context<Self>) {
        self.local_branch_dropdown_open = !self.local_branch_dropdown_open;
        if self.local_branch_dropdown_open {
            self.titlebar_menus_visible = false;
            self.active_titlebar_menu = None;
        }
        cx.notify();
    }

    pub fn close_local_branch_dropdown(&mut self, cx: &mut Context<Self>) {
        if self.local_branch_dropdown_open {
            self.local_branch_dropdown_open = false;
            cx.notify();
        }
    }

    pub fn close_floating_menus(&mut self, cx: &mut Context<Self>) {
        let changed = self.local_branch_dropdown_open
            || self.titlebar_menus_visible
            || self.active_titlebar_menu.is_some()
            || self.repo_session.sidebar_state.context_menu != ContextMenuAction::None;

        self.local_branch_dropdown_open = false;
        self.titlebar_menus_visible = false;
        self.active_titlebar_menu = None;
        self.repo_session.sidebar_state.dismiss_context_menu();

        if changed {
            cx.notify();
        }
    }

    pub fn toggle_titlebar_menus(&mut self, cx: &mut Context<Self>) {
        self.titlebar_menus_visible = !self.titlebar_menus_visible;
        if self.titlebar_menus_visible {
            self.local_branch_dropdown_open = false;
        }
        if !self.titlebar_menus_visible {
            self.active_titlebar_menu = None;
        }
        cx.notify();
    }

    pub fn open_titlebar_menu(&mut self, menu: TitlebarMenu, cx: &mut Context<Self>) {
        if self.titlebar_menus_visible && self.active_titlebar_menu == Some(menu) {
            return;
        }
        self.titlebar_menus_visible = true;
        self.active_titlebar_menu = Some(menu);
        self.local_branch_dropdown_open = false;
        cx.notify();
    }

    pub fn close_titlebar_menu(&mut self, cx: &mut Context<Self>) {
        if self.active_titlebar_menu.is_some() {
            self.active_titlebar_menu = None;
            cx.notify();
        }
    }

    fn hide_titlebar_menus(&mut self, cx: &mut Context<Self>) {
        if self.titlebar_menus_visible || self.active_titlebar_menu.is_some() {
            self.titlebar_menus_visible = false;
            self.active_titlebar_menu = None;
            cx.notify();
        }
    }

    pub fn execute_command_palette_action(
        &mut self,
        action: CommandAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.command_palette.hide(cx);
        window.dispatch_action(action.boxed_action(), cx);
    }

    pub fn execute_command_palette_selection(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(action) = self.command_palette.selected_action() {
            self.execute_command_palette_action(action, window, cx);
        }
    }

    pub(crate) fn spawn_open_dialog(&mut self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let path =
                cx.update(|_cx| rfd::AsyncFileDialog::new().set_title("Open Git Repository"));
            let folder = match path {
                Ok(dialog) => {
                    let result = dialog.pick_folder().await;
                    result
                }
                Err(_) => None,
            };

            let Some(folder) = folder else {
                this.update(cx, |this, cx| {
                    this.repo_session.loading = false;
                    cx.notify();
                })
                .ok();
                return;
            };

            let path_buf = std::path::PathBuf::from(folder.path());
            this.update(cx, |this, cx| {
                this.open_or_activate_repo_tab(path_buf, cx);
            })
            .ok();
        })
        .detach();
    }

    pub(crate) fn handle_open_repository(
        &mut self,
        _action: &OpenRepository,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        tracing::info!("OpenRepository action fired");
        self.repo_session.loading = true;
        cx.notify();

        cx.spawn(async move |this, cx| {
            let path =
                cx.update(|_cx| rfd::AsyncFileDialog::new().set_title("Open Git Repository"));
            let folder = match path {
                Ok(dialog) => {
                    tracing::info!("Showing file dialog");
                    let result = dialog.pick_folder().await;
                    tracing::info!(
                        "File dialog returned: {:?}",
                        result.as_ref().map(|f| f.path())
                    );
                    result
                }
                Err(e) => {
                    tracing::warn!("Failed to create file dialog: {:?}", e);
                    None
                }
            };

            let Some(folder) = folder else {
                this.update(cx, |this, cx| {
                    this.repo_session.loading = false;
                    cx.notify();
                })
                .ok();
                return;
            };

            let path_buf = std::path::PathBuf::from(folder.path());
            this.update(cx, |this, cx| {
                this.open_or_activate_repo_tab(path_buf, cx);
            })
            .ok();
        })
        .detach();
    }

    pub(crate) fn handle_close_dialog(
        &mut self,
        _action: &CloseDialog,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if matches!(self.active_dialog, AppDialog::CreatePullRequest) {
            self.cancel_create_pr_dialog(cx);
        } else if self.active_dialog != AppDialog::None {
            self.active_dialog = AppDialog::None;
            cx.notify();
        } else if self.command_palette.is_visible() {
            self.command_palette.hide(cx);
        } else if self.repo_session.diff_overlay_open {
            self.repo_session.diff_overlay_open = false;
            cx.notify();
        } else if self.local_branch_dropdown_open {
            self.close_local_branch_dropdown(cx);
        } else if self.titlebar_menus_visible || self.active_titlebar_menu.is_some() {
            self.hide_titlebar_menus(cx);
        }
    }

    pub(crate) fn handle_select_prev(
        &mut self,
        _action: &SelectPrevCommit,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.repo_session.graph_panel.select_prev() {
            self.on_graph_selection_changed(cx);
        }
    }

    pub(crate) fn handle_select_next(
        &mut self,
        _action: &SelectNextCommit,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.repo_session.graph_panel.select_next() {
            self.on_graph_selection_changed(cx);
        }
    }

    pub(crate) fn handle_view_file(
        &mut self,
        _action: &ViewFileAtCommit,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(path) = self.repo_session.diff_panel.selected_file_path() {
            self.view_file_at_commit(path, cx);
        }
    }

    pub(crate) fn handle_show_status(
        &mut self,
        _action: &ShowStatusPanel,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.repo_session.view_mode = MainViewMode::Status;
        self.load_status(cx);
    }

    pub(crate) fn handle_show_history(
        &mut self,
        _action: &ShowHistory,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.repo_session.view_mode = MainViewMode::CommitHistory;
        cx.notify();
    }

    pub(crate) fn handle_refresh(
        &mut self,
        _action: &RefreshRepository,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.refresh_repository(cx);
    }

    pub(crate) fn handle_soft_reset(
        &mut self,
        _action: &SoftReset,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.soft_reset(cx);
    }

    pub(crate) fn handle_create_branch(
        &mut self,
        _action: &CreateBranch,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_create_branch_dialog(None, cx);
    }

    pub(crate) fn handle_stash_push(
        &mut self,
        _action: &StashPush,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.active_dialog = AppDialog::StashPush;
        self.dialog_input.clear();
        cx.notify();
    }

    pub(crate) fn handle_stash_pop(
        &mut self,
        _action: &StashPop,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.stash_pop(cx);
    }

    pub(crate) fn handle_fetch_all(
        &mut self,
        _action: &FetchAll,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.fetch_all(cx);
    }

    pub(crate) fn handle_push_current(
        &mut self,
        _action: &PushCurrent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.push_current(cx);
    }

    pub(crate) fn handle_pull_current(
        &mut self,
        _action: &PullCurrent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.pull_current(cx);
    }

    pub(crate) fn handle_create_pull_request(
        &mut self,
        _action: &CreatePullRequest,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_create_pr_dialog(cx);
    }

    pub fn back_to_diff(
        &mut self,
        _action: &BackToDiff,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.repo_session.diff_panel.set_diff_mode();
        cx.notify();
    }

    pub(crate) fn handle_toggle_theme(
        &mut self,
        _action: &ToggleTheme,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.cycle_theme(cx);
    }

    pub(crate) fn handle_show_command_palette(
        &mut self,
        _action: &ShowCommandPalette,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.hide_titlebar_menus(cx);
        self.command_palette.show(window, cx);
    }

    pub(crate) fn handle_new_tab(
        &mut self,
        _action: &NewTab,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.repo_session.loading = true;
        cx.notify();
        self.spawn_open_dialog(cx);
    }

    pub(crate) fn handle_close_tab(
        &mut self,
        _action: &CloseTab,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(tab_id) = self.repo_session.active_repo_tab_id {
            self.close_repo_tab(tab_id, cx);
        }
    }

    pub(crate) fn handle_reopen_closed_tab(
        &mut self,
        _action: &ReopenClosedTab,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.reopen_closed_tab(cx);
    }

    pub(crate) fn handle_init_repo(
        &mut self,
        _action: &InitRepo,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.spawn_init_repo_picker(cx);
    }

    pub(crate) fn handle_open_repo_management(
        &mut self,
        _action: &OpenRepoManagement,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_settings_window(Some(SettingsSection::Repositories), cx);
    }

    pub(crate) fn handle_open_in_editor(
        &mut self,
        _action: &OpenInEditor,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(path) = self
            .repo_session
            .active_repo_state()
            .map(|r| r.path.clone())
        {
            self.open_in_editor(path, cx);
        }
    }

    pub(crate) fn handle_open_in_terminal(
        &mut self,
        _action: &OpenInTerminal,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(path) = self
            .repo_session
            .active_repo_state()
            .map(|r| r.path.clone())
        {
            self.open_in_terminal(path, cx);
        }
    }

    pub(crate) fn handle_open_in_file_manager(
        &mut self,
        _action: &OpenInFileManager,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(path) = self
            .repo_session
            .active_repo_state()
            .map(|r| r.path.clone())
        {
            self.open_in_file_manager(path, cx);
        }
    }

    pub(crate) fn handle_open_in_browser(
        &mut self,
        _action: &OpenInBrowser,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_repo_in_browser(cx);
    }

    pub(crate) fn handle_preferences(
        &mut self,
        _action: &Preferences,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_settings_window(None, cx);
    }

    pub(crate) fn handle_check_for_updates(
        &mut self,
        _action: &CheckForUpdates,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        gitforge_update::check(&gitforge_update::Check, window, cx);
    }

    pub(crate) fn handle_quit(
        &mut self,
        _action: &QuitApp,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(handle) = self.settings_window.take() {
            let _ = handle.update(cx, |_, w, _| w.remove_window());
        }
        window.remove_window();
    }

    pub(crate) fn handle_clone(
        &mut self,
        _action: &CloneRepo,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_clone_dialog(cx);
    }

    pub(crate) fn handle_clone_github(
        &mut self,
        _action: &CloneFromGithub,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_clone_from_hosting_dialog("github".to_string(), cx);
    }

    pub(crate) fn handle_clone_gitlab(
        &mut self,
        _action: &CloneFromGitlab,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_clone_from_hosting_dialog("gitlab".to_string(), cx);
    }

    pub(crate) fn handle_add_remote(
        &mut self,
        _action: &AddRemote,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_add_remote_dialog(cx);
    }

    pub(crate) fn handle_create_worktree(
        &mut self,
        _action: &CreateWorktree,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_create_worktree_dialog(cx);
    }

    pub(crate) fn handle_open_ssh_key(
        &mut self,
        _action: &OpenSshKey,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_ssh_generate_key_dialog(cx);
    }

    pub(crate) fn handle_manage_accounts(
        &mut self,
        _action: &ManageAccounts,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_settings_window(Some(SettingsSection::Accounts), cx);
    }

    pub(crate) fn handle_open_ai_settings(
        &mut self,
        _action: &OpenAiSettings,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_settings_window(Some(SettingsSection::Ai), cx);
    }
}
