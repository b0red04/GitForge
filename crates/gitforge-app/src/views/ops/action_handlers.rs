use gpui::*;

use crate::views::app::{
    GitForgeApp, AppDialog, MainViewMode,
    OpenRepository, CloseDialog, SelectPrevCommit, SelectNextCommit, BackToDiff, ShowStatusPanel, RefreshRepository,
    SoftReset, CreateBranch, StashPush, StashPop, FetchAll, PushCurrent,
    PullCurrent, ToggleTheme, ShowCommandPalette, NewTab, CloseTab,
    ReopenClosedTab, InitRepo, OpenRepoManagement, OpenInEditor,
    OpenInTerminal, OpenInFileManager, Preferences, QuitApp,
};
use crate::views::commands::TitlebarMenu;
use crate::views::settings_window::SettingsSection;

impl GitForgeApp {

    pub fn toggle_toolbar_more(&mut self, cx: &mut Context<Self>) {
        self.toolbar_more_open = !self.toolbar_more_open;
        cx.notify();
    }

    pub fn close_toolbar_more(&mut self, cx: &mut Context<Self>) {
        if self.toolbar_more_open {
            self.toolbar_more_open = false;
            cx.notify();
        }
    }

    pub fn toggle_titlebar_menus(&mut self, cx: &mut Context<Self>) {
        self.titlebar_menus_visible = !self.titlebar_menus_visible;
        if !self.titlebar_menus_visible {
            self.active_titlebar_menu = None;
        }
        cx.notify();
    }

    pub fn open_titlebar_menu(&mut self, menu: TitlebarMenu, cx: &mut Context<Self>) {
        self.titlebar_menus_visible = true;
        self.active_titlebar_menu = Some(menu);
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
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.hide_titlebar_menus(cx);
        self.command_palette.show(cx);
    }
    pub fn execute_command_palette_action(&mut self, action: &str, cx: &mut Context<Self>) {
        self.command_palette.hide(cx);
        self.execute_app_command(action, cx);
    }

    pub fn execute_app_command(&mut self, action: &str, cx: &mut Context<Self>) {
        self.hide_titlebar_menus(cx);
        match action {
            "open_repository" => {
                self.loading = true;
                cx.notify();
                self.spawn_open_dialog(cx);
            }
            "refresh" => self.refresh_repository(cx),
            "close_dialog" => self.cancel_dialog(cx),
            "select_prev" => {
                if self.graph_panel.select_prev() {
                    self.on_graph_selection_changed(cx);
                }
            }
            "select_next" => {
                if self.graph_panel.select_next() {
                    self.on_graph_selection_changed(cx);
                }
            }
            "view_file" => {
                if let Some(path) = self.diff_panel.selected_file_path() {
                    self.view_file_at_commit(path, cx);
                }
            }
            "back_to_diff" => self.back_to_diff_mode(cx),
            "show_history" => {
                self.view_mode = MainViewMode::CommitHistory;
                cx.notify();
            }
            "command_palette" => self.command_palette.show(cx),
            "create_branch" => self.open_create_branch_dialog(None, cx),
            "stash_push" => self.open_stash_push_dialog(cx),
            "stash_pop" => self.stash_pop(cx),
            "fetch_all" => self.fetch_all(cx),
            "pull" => self.open_pull_dialog(cx),
            "push" => self.open_push_dialog(cx),
            "toggle_theme" => self.cycle_theme(cx),
            "clone" => self.open_clone_dialog(cx),
            "clone_github" => self.open_clone_from_hosting_dialog("github".to_string(), cx),
            "clone_gitlab" => self.open_clone_from_hosting_dialog("gitlab".to_string(), cx),
            "add_remote" => self.open_add_remote_dialog(cx),
            "ssh_key" => self.open_ssh_generate_key_dialog(cx),
            "accounts" => self.open_settings_window(Some(SettingsSection::Accounts), cx),
            "ai_settings" => self.open_settings_window(Some(SettingsSection::Ai), cx),
            "open_browser" => self.open_repo_in_browser(cx),
            "worktree" => self.open_create_worktree_dialog(cx),
            "show_status" => {
                self.view_mode = MainViewMode::Status;
                self.load_status(cx);
            }
            "soft_reset" => self.soft_reset(cx),
            "open_editor" => {
                if let Some(path) = self.active_repo_state().map(|r| r.path.clone()) {
                    self.open_in_editor(path, cx);
                }
            }
            "open_terminal" => {
                if let Some(path) = self.active_repo_state().map(|r| r.path.clone()) {
                    self.open_in_terminal(path, cx);
                }
            }
            "new_tab" => {
                self.loading = true;
                cx.notify();
                self.spawn_open_dialog(cx);
            }
            "close_tab" => {
                if let Some(tab_id) = self.active_repo_tab_id {
                    self.close_repo_tab(tab_id, cx);
                }
            }
            "reopen_closed_tab" => self.reopen_closed_tab(cx),
            "init_repo" => self.spawn_init_repo_picker(cx),
            "repo_management" => {
                self.open_settings_window(Some(SettingsSection::Repositories), cx)
            }
            "open_file_manager" => {
                if let Some(path) = self.active_repo_state().map(|r| r.path.clone()) {
                    self.open_in_file_manager(path, cx);
                }
            }
            "preferences" => self.open_settings_window(None, cx),
            "quit" => {
                self.quit_requested = true;
                cx.notify();
            }
            _ => {}
        }
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

    pub(crate) fn handle_new_tab(
        &mut self,
        _action: &NewTab,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.execute_app_command("new_tab", cx);
    }

    pub(crate) fn handle_close_tab(
        &mut self,
        _action: &CloseTab,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.execute_app_command("close_tab", cx);
    }

    pub(crate) fn handle_reopen_closed_tab(
        &mut self,
        _action: &ReopenClosedTab,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.execute_app_command("reopen_closed_tab", cx);
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
        if let Some(path) = self.active_repo_state().map(|r| r.path.clone()) {
            self.open_in_editor(path, cx);
        }
    }

    pub(crate) fn handle_open_in_terminal(
        &mut self,
        _action: &OpenInTerminal,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(path) = self.active_repo_state().map(|r| r.path.clone()) {
            self.open_in_terminal(path, cx);
        }
    }

    pub(crate) fn handle_open_in_file_manager(
        &mut self,
        _action: &OpenInFileManager,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(path) = self.active_repo_state().map(|r| r.path.clone()) {
            self.open_in_file_manager(path, cx);
        }
    }

    pub(crate) fn handle_preferences(
        &mut self,
        _action: &Preferences,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_settings_window(None, cx);
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
                    this.loading = false;
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

    pub fn execute_command_palette_selection(&mut self, cx: &mut Context<Self>) {
        if let Some(action) = self.command_palette.selected_action().map(String::from) {
            self.execute_command_palette_action(&action, cx);
        }
    }
    pub(crate) fn handle_open_repository(
        &mut self,
        _action: &OpenRepository,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        tracing::info!("OpenRepository action fired");
        self.loading = true;
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
                    this.loading = false;
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
        self.close_toolbar_more(cx);
        if self.active_dialog != AppDialog::None {
            self.active_dialog = AppDialog::None;
            cx.notify();
        } else if self.command_palette.is_visible() {
            self.command_palette.hide(cx);
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
        if self.graph_panel.select_prev() {
            self.on_graph_selection_changed(cx);
        }
    }

    pub(crate) fn handle_select_next(
        &mut self,
        _action: &SelectNextCommit,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.graph_panel.select_next() {
            self.on_graph_selection_changed(cx);
        }
    }

    pub(crate) fn handle_show_status(
        &mut self,
        _action: &ShowStatusPanel,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.view_mode = MainViewMode::Status;
        self.load_status(cx);
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
        self.open_push_dialog(cx);
    }

    pub(crate) fn handle_pull_current(
        &mut self,
        _action: &PullCurrent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_pull_dialog(cx);
    }

    pub fn back_to_diff(
        &mut self,
        _action: &BackToDiff,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.diff_panel.set_diff_mode();
        cx.notify();
    }
}
