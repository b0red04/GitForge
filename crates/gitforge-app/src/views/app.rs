use gitforge_git::{RepoState, Repository, RepoLoadOptions, CommitLogOptions};
use gitforge_ui::{AppColors, Theme, rgba_to_hsla};
use gpui::*;

use parking_lot::Mutex;
use std::path::PathBuf;
use std::sync::Arc;

use super::command_palette::CommandPalette;
use super::commands::TitlebarMenu;
use super::diff_panel::DiffPanel;
use super::graph_panel::GraphPanel;
use super::settings::AppSettings;
use super::settings_window::SettingsWindow;
use super::sidebar::SidebarState;
use super::status_panel::StatusPanel;

actions!(
    gitforge,
    [
        OpenRepository,
        CloseDialog,
        SelectPrevCommit,
        SelectNextCommit,
        ViewFileAtCommit,
        BackToDiff,
        ShowStatusPanel,
        ShowHistory,
        RefreshRepository,
        SoftReset,
        CreateBranch,
        StashPush,
        StashPop,
        FetchAll,
        PushCurrent,
        PullCurrent,
        ToggleTheme,
        ShowCommandPalette,
        NewTab,
        CloseTab,
        ReopenClosedTab,
        InitRepo,
        OpenRepoManagement,
        OpenInEditor,
        OpenInTerminal,
        OpenInFileManager,
        OpenInBrowser,
        Preferences,
        QuitApp,
        CloneRepo,
        CloneFromGithub,
        CloneFromGitlab,
        AddRemote,
        CreateWorktree,
        OpenSshKey,
        ManageAccounts,
        OpenAiSettings,
    ]
);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MainViewMode {
    CommitHistory,
    Status,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub enum AppDialog {
    None,
    CreateBranch {
        start_point: Option<String>,
    },
    RenameBranch {
        old_name: String,
    },
    CreateTag {
        target: Option<String>,
    },
    StashPush,
    Push {
        branch: Option<String>,
        remote: Option<String>,
    },
    Pull {
        remote: Option<String>,
    },
    CloneRepo,
    AddRemote,
    SshGenerateKey,
    SshTestConnection,
    CredentialAdd,
    CloneFromHosting {
        provider: String,
    },
    AddAccount {
        provider: String,
    },
    SearchHosting {
        provider: String,
    },
    ForkRepo {
        owner: String,
        repo: String,
        provider: String,
    },
    CreateWorktree,
    RemoveWorktree {
        path: String,
    },
    InitRepo {
        parent: PathBuf,
    },
}

pub(crate) const MAX_CLOSED_TABS: usize = 20;

pub(crate) struct OpenRepoTab {
    pub(crate) id: u64,
    pub(crate) path: PathBuf,
    pub(crate) repo: Arc<Mutex<Option<Repository>>>,
    pub(crate) repo_state: Option<RepoState>,
    pub(crate) loading: bool,
    pub(crate) last_error: Option<String>,
}

pub struct GitForgeApp {
    pub(crate) colors: AppColors,
    pub(crate) open_repo_tabs: Vec<OpenRepoTab>,
    pub(crate) active_repo_tab_id: Option<u64>,
    pub(crate) next_repo_tab_id: u64,
    pub(crate) graph_panel: GraphPanel,
    pub(crate) diff_panel: DiffPanel,
    pub status_panel: StatusPanel,
    pub sidebar_state: SidebarState,
    pub view_mode: MainViewMode,
    pub(crate) active_dialog: AppDialog,
    pub(crate) dialog_input: String,
    pub(crate) dialog_input_2: String,
    pub(crate) dialog_input_focus: FocusHandle,
    pub remote_status: String,
    pub(crate) loading: bool,
    pub(crate) focus_handle: FocusHandle,
    pub(crate) settings: AppSettings,
    pub(crate) ssh_keys: Vec<gitforge_git::SshKey>,
    pub(crate) ssh_agent_status: Option<gitforge_git::SshAgentStatus>,
    pub(crate) hosting_accounts: Vec<gitforge_hosting::HostingAccount>,
    pub(crate) hosting_repos: Vec<gitforge_hosting::RemoteRepo>,
    pub(crate) hosting_repos_loading: bool,
    pub(crate) ai_generating: bool,
    pub(crate) last_error: Option<String>,
    pub(crate) toolbar_more_open: bool,
    pub(crate) titlebar_menus_visible: bool,
    pub(crate) active_titlebar_menu: Option<TitlebarMenu>,
    pub command_palette: CommandPalette,
    pub(crate) closed_repo_tabs: Vec<PathBuf>,
    pub(crate) settings_window: Option<WindowHandle<SettingsWindow>>,
    pub(crate) quit_requested: bool,
}

impl Focusable for GitForgeApp {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

#[allow(dead_code)]
impl GitForgeApp {
    pub fn new(cx: &mut App) -> Self {
        let settings = AppSettings::load();
        let theme = Theme::load_by_name(&settings.theme).unwrap_or_else(|_| Theme::default_dark());
        let colors = AppColors::from_theme(&theme);
        let sidebar_state = SidebarState::new(cx);
        let mut app = Self {
            colors,
            open_repo_tabs: Vec::new(),
            active_repo_tab_id: None,
            next_repo_tab_id: 1,
            graph_panel: GraphPanel::new(),
            diff_panel: DiffPanel::new(),
            status_panel: StatusPanel::new(cx),
            sidebar_state,
            view_mode: MainViewMode::CommitHistory,
            active_dialog: AppDialog::None,
            dialog_input: String::new(),
            dialog_input_2: String::new(),
            dialog_input_focus: cx.focus_handle(),
            remote_status: String::new(),
            loading: false,
            focus_handle: cx.focus_handle(),
            settings,
            ssh_keys: Vec::new(),
            ssh_agent_status: None,
            hosting_accounts: Vec::new(),
            hosting_repos: Vec::new(),
            hosting_repos_loading: false,
            ai_generating: false,
            last_error: None,
            toolbar_more_open: false,
            titlebar_menus_visible: false,
            active_titlebar_menu: None,
            command_palette: CommandPalette::new(cx),
            closed_repo_tabs: Vec::new(),
            settings_window: None,
            quit_requested: false,
        };
        app.sidebar_state.branches_expanded = app.settings.sidebar_branches_expanded;
        app.sidebar_state.remotes_expanded = app.settings.sidebar_remotes_expanded;
        app.sidebar_state.tags_expanded = app.settings.sidebar_tags_expanded;
        app.load_ssh_state();
        app.load_hosting_accounts();
        app
    }
    pub(crate) fn open_repo_from_path(&mut self, path: std::path::PathBuf, cx: &mut Context<Self>) {
        self.open_or_activate_repo_tab(path, cx);
    }
    pub(crate) fn save_settings(&mut self) {
        self.settings.sidebar_branches_expanded = self.sidebar_state.branches_expanded;
        self.settings.sidebar_remotes_expanded = self.sidebar_state.remotes_expanded;
        self.settings.sidebar_tags_expanded = self.sidebar_state.tags_expanded;
        self.settings.open_repo_paths = self
            .open_repo_tabs
            .iter()
            .map(|tab| tab.path.to_string_lossy().to_string())
            .collect();
        self.settings.active_repo_path = self
            .active_tab()
            .map(|tab| tab.path.to_string_lossy().to_string());
        self.settings.last_repo_path = self.settings.active_repo_path.clone();
        self.settings.save();
    }

    pub(crate) fn load_options(&self) -> RepoLoadOptions {
        RepoLoadOptions {
            commit_limit: self.settings.commit_limit,
            log_options: CommitLogOptions {
                include_custom_refs: self.settings.show_checkpoint_refs,
            },
        }
    }
}

impl Render for GitForgeApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.quit_requested {
            self.quit_requested = false;
            if let Some(handle) = self.settings_window.take() {
                let _ = handle.update(cx, |_, w, _| w.remove_window());
            }
            window.remove_window();
        }

        let bg = rgba_to_hsla(self.colors.background);
        let text = rgba_to_hsla(self.colors.text);
        let entity = cx.entity().downgrade();
        let active_repo_state = self.active_repo_state();

        let sidebar = super::sidebar::render_sidebar(
            active_repo_state,
            &self.colors,
            self.loading,
            &self.sidebar_state,
            entity.clone(),
            window,
            &self.hosting_accounts,
        );

        let toolbar = super::toolbar::render_toolbar(
            active_repo_state,
            &self.colors,
            self.view_mode == MainViewMode::Status,
            self.toolbar_more_open,
            entity.clone(),
        );

        let graph_area = super::layout::grow_center(div()).child(self.graph_panel.render(
            &self.colors,
            self.settings.show_checkpoint_refs,
            entity.clone(),
        ));

        let right_content = match self.view_mode {
            MainViewMode::CommitHistory => {
                if self.graph_panel.is_uncommitted_selected() {
                    self.status_panel.render_graph_staging(
                        active_repo_state,
                        &self.colors,
                        entity.clone(),
                        window,
                        self.ai_generating,
                    )
                } else {
                    self.diff_panel.render(
                        active_repo_state,
                        self.graph_panel.selected_commit_idx(),
                        &self.colors,
                        entity.clone(),
                        self.loading,
                    )
                }
            }
            MainViewMode::Status => {
                self.status_panel
                    .render(&self.colors, entity.clone(), window, self.ai_generating)
            }
        };

        let right_panel = super::layout::grow_right(div()).child(right_content);

        let status_bar =
            super::toolbar::render_status_bar(&self.remote_status, &self.colors, window);

        let error_banner = self.last_error.as_ref().map(|err| {
            let _error_color = rgba_to_hsla(self.colors.error);
            div()
                .w_full()
                .px_3()
                .py_2()
                .bg(gpui::hsla(0.0, 0.8, 0.3, 0.9))
                .text_color(gpui::hsla(0.0, 0.0, 1.0, 1.0))
                .text_sm()
                .child(err.clone())
        });

        let titlebar = super::titlebar::render_titlebar(
            active_repo_state,
            &self.colors,
            window,
            entity.clone(),
            self.titlebar_menus_visible,
            self.active_titlebar_menu,
        );
        let titlebar_divider = super::titlebar::render_titlebar_divider(&self.colors);

        let mut inner = div()
            .id("app-content")
            .relative()
            .flex()
            .flex_col()
            .size_full()
            .overflow_hidden()
            .text_color(text)
            .child(titlebar)
            .child(titlebar_divider);

        let repo_tab_views = self.repo_tab_views();
        if !repo_tab_views.is_empty() {
            inner = inner.child(super::repo_tabs::render_repo_tab_bar(
                &repo_tab_views,
                self.active_repo_tab_id,
                &self.colors,
                entity.clone(),
            ));
        }

        if let Some(banner) = error_banner {
            inner = inner.child(banner);
        }

        inner = inner.child(
            div()
                .flex_1()
                .h_full()
                .flex()
                .flex_row()
                .overflow_hidden()
                .child(sidebar)
                .child(
                    div()
                        .flex_1()
                        .h_full()
                        .flex()
                        .flex_col()
                        .bg(bg)
                        .overflow_hidden()
                        .child(toolbar)
                        .child(
                            div()
                                .flex_1()
                                .min_h(px(0.0))
                                .flex()
                                .flex_row()
                                .overflow_hidden()
                                .child(graph_area)
                                .child(right_panel),
                        )
                        .child(status_bar),
                ),
        );

        if self.active_dialog != AppDialog::None {
            inner = inner.child(super::ops::dialog_render::render_dialog_overlay(
                &self.active_dialog,
                &self.dialog_input,
                &self.dialog_input_2,
                &self.dialog_input_focus,
                &self.colors,
                entity.clone(),
                window,
                &self.hosting_repos,
                self.hosting_repos_loading,
                &self.hosting_accounts,
            ));
        }

        if let Some(palette) = self.command_palette.render(&self.colors, entity.clone(), window) {
            inner = inner.child(palette);
        }

        if let Some(menu) = self.active_titlebar_menu {
            inner = inner.child(super::titlebar::render_titlebar_menu_dropdown(
                menu,
                &self.colors,
                entity.clone(),
            ));
        }

        div()
            .id("app-root")
            .size_full()
            .bg(gpui::transparent_black())
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::handle_open_repository))
            .on_action(cx.listener(Self::handle_close_dialog))
            .on_action(cx.listener(Self::handle_select_prev))
            .on_action(cx.listener(Self::handle_select_next))
            .on_action(cx.listener(Self::back_to_diff))
            .on_action(cx.listener(Self::handle_show_status))
            .on_action(cx.listener(Self::handle_show_history))
            .on_action(cx.listener(Self::handle_refresh))
            .on_action(cx.listener(Self::handle_soft_reset))
            .on_action(cx.listener(Self::handle_create_branch))
            .on_action(cx.listener(Self::handle_stash_push))
            .on_action(cx.listener(Self::handle_stash_pop))
            .on_action(cx.listener(Self::handle_fetch_all))
            .on_action(cx.listener(Self::handle_push_current))
            .on_action(cx.listener(Self::handle_pull_current))
            .on_action(cx.listener(Self::handle_toggle_theme))
            .on_action(cx.listener(Self::handle_show_command_palette))
            .on_action(cx.listener(Self::handle_new_tab))
            .on_action(cx.listener(Self::handle_close_tab))
            .on_action(cx.listener(Self::handle_reopen_closed_tab))
            .on_action(cx.listener(Self::handle_init_repo))
            .on_action(cx.listener(Self::handle_open_repo_management))
            .on_action(cx.listener(Self::handle_open_in_editor))
            .on_action(cx.listener(Self::handle_open_in_terminal))
            .on_action(cx.listener(Self::handle_open_in_file_manager))
            .on_action(cx.listener(Self::handle_open_in_browser))
            .on_action(cx.listener(Self::handle_preferences))
            .on_action(cx.listener(Self::handle_quit))
            .on_action(cx.listener(Self::handle_clone))
            .on_action(cx.listener(Self::handle_clone_github))
            .on_action(cx.listener(Self::handle_clone_gitlab))
            .on_action(cx.listener(Self::handle_add_remote))
            .on_action(cx.listener(Self::handle_create_worktree))
            .on_action(cx.listener(Self::handle_open_ssh_key))
            .on_action(cx.listener(Self::handle_manage_accounts))
            .on_action(cx.listener(Self::handle_open_ai_settings))
            .on_action(cx.listener(Self::handle_view_file))
            .child(super::window_chrome::render_window_chrome(
                inner,
                &self.colors,
                window,
            ))
    }
}

