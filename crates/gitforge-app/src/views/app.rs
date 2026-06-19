use gitforge_git::{CommitLogOptions, RepoLoadOptions};
use gitforge_ui::{AppColors, TextInput, Theme, rgba_to_hsla};
use gpui::*;

use std::path::PathBuf;

use super::command_palette::CommandPalette;
use super::commands::TitlebarMenu;
use super::dialogs::CreatePrState;
use super::repo_session::{RepoSession, drop_caret_index};
use super::settings::AppSettings;
use super::settings_window::SettingsWindow;

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
        CreatePullRequest,
        CheckForUpdates,
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
    DeleteBranch {
        name: String,
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
    CreatePullRequest,
}

pub struct GitForgeApp {
    pub(crate) colors: AppColors,
    pub(crate) repo_session: RepoSession,
    pub(crate) active_dialog: AppDialog,
    pub(crate) dialog_input: TextInput,
    pub(crate) dialog_input_2: TextInput,
    /// Live, toggleable value for the delete-branch dialog's "Force delete"
    /// checkbox. Seeded when the dialog opens; the overlay mutates it.
    pub(crate) dialog_force: bool,
    pub(crate) settings: AppSettings,
    pub(crate) ssh_keys: Vec<gitforge_git::SshKey>,
    pub(crate) ssh_agent_status: Option<gitforge_git::SshAgentStatus>,
    pub(crate) hosting_accounts: Vec<gitforge_hosting::HostingAccount>,
    pub(crate) hosting_repos: Vec<gitforge_hosting::RemoteRepo>,
    pub(crate) hosting_repos_loading: bool,
    pub(crate) ai_generating: bool,
    pub(crate) focus_handle: FocusHandle,
    pub(crate) local_branch_dropdown_open: bool,
    pub(crate) titlebar_menus_visible: bool,
    pub(crate) active_titlebar_menu: Option<TitlebarMenu>,
    pub command_palette: CommandPalette,
    pub(crate) settings_window: Option<WindowHandle<SettingsWindow>>,
    pub(crate) quit_requested: bool,
    pub(crate) periodic_fetch_generation: u64,
    pub(crate) last_auto_fetch_at: Option<std::time::Instant>,
    pub(crate) focus_subscription: Option<gpui::Subscription>,
    pub(crate) toasts: super::toasts::Toasts,
    pub(crate) create_pr: CreatePrState,
    pub(crate) update_indicator: Entity<super::update_indicator::UpdateIndicator>,
    pub(crate) shown_update_notification: bool,
    /// User-controlled width of the left sidebar, in px. Loaded from settings
    /// and persisted on change.
    pub(crate) sidebar_width: f32,
    /// User-controlled width of the right detail/diff pane, in px. Loaded from
    /// settings and persisted on change.
    pub(crate) right_panel_width: f32,
    /// Non-`None` while a side-panel splitter is being dragged.
    pub(crate) active_panel_resize: Option<super::panel_resize::PanelResize>,
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
        let update_indicator =
            cx.new(|cx| super::update_indicator::UpdateIndicator::new(colors.clone(), cx));
        gitforge_update::set_auto_update_enabled(settings.auto_update, cx);
        let mut repo_session = RepoSession::new(cx);
        repo_session
            .sidebar_state
            .apply_persisted_from_settings(&settings);
        let sidebar_width = super::panel_resize::clamp_sidebar_width(settings.sidebar_width);
        let right_panel_width = super::panel_resize::clamp_right_width(settings.right_panel_width);
        let mut app = Self {
            colors,
            repo_session,
            active_dialog: AppDialog::None,
            dialog_input: TextInput::new("", cx),
            dialog_input_2: TextInput::new("", cx),
            dialog_force: false,
            settings,
            ssh_keys: Vec::new(),
            ssh_agent_status: None,
            hosting_accounts: Vec::new(),
            hosting_repos: Vec::new(),
            hosting_repos_loading: false,
            ai_generating: false,
            focus_handle: cx.focus_handle(),
            local_branch_dropdown_open: false,
            titlebar_menus_visible: false,
            active_titlebar_menu: None,
            command_palette: CommandPalette::new(cx),
            settings_window: None,
            quit_requested: false,
            periodic_fetch_generation: 0,
            last_auto_fetch_at: None,
            focus_subscription: None,
            toasts: super::toasts::Toasts::new(),
            create_pr: CreatePrState::new(cx),
            update_indicator,
            shown_update_notification: false,
            sidebar_width,
            right_panel_width,
            active_panel_resize: None,
        };
        app.load_ssh_state();
        app.load_hosting_accounts();
        app
    }
    pub(crate) fn open_repo_from_path(&mut self, path: std::path::PathBuf, cx: &mut Context<Self>) {
        self.open_or_activate_repo_tab(path, cx);
    }
    pub(crate) fn save_settings(&mut self) {
        self.repo_session
            .sidebar_state
            .write_persisted_to_settings(&mut self.settings);
        self.settings.sidebar_width = self.sidebar_width;
        self.settings.right_panel_width = self.right_panel_width;
        self.settings.open_repo_paths = self
            .repo_session
            .open_repo_tabs
            .iter()
            .map(|tab| tab.path.to_string_lossy().to_string())
            .collect();
        self.settings.active_repo_path = self
            .repo_session
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

    pub(crate) fn active_repo_behavior_settings(&self) -> super::settings::RepoBehaviorSettings {
        self.repo_session
            .active_tab()
            .map(|tab| self.settings.repo_settings_for_path(&tab.path))
            .unwrap_or_default()
    }

    /// Refresh the cached diff mirror view when (and only when) the diff's
    /// observable state has changed. This is what keeps commit-history
    /// scrolling cheap: the scroll does not change the diff key, so the mirror
    /// is left clean and GPUI recycles its previous paint instead of
    /// re-rendering the entire diff panel on every scroll frame.
    fn sync_diff_view(
        &self,
        repo_state: Option<&gitforge_git::RepoState>,
        app: WeakEntity<Self>,
        cx: &mut Context<Self>,
    ) {
        let loading = self.repo_session.loading;
        let sel_idx = self.repo_session.graph_panel.selected_commit_idx();
        let selected_commit = repo_state
            .and_then(|rs| sel_idx.and_then(|i| rs.commits.get(i)))
            .cloned();
        let selected_commit_id = selected_commit.as_ref().map(|c| c.id.clone());

        let key = self.repo_session.diff_panel.build_key(
            self.settings.theme.clone(),
            loading,
            selected_commit_id,
        );

        if self.repo_session.diff_view.read(cx).key() == &key {
            return;
        }

        let snapshot = self.repo_session.diff_panel.build_snapshot(
            self.settings.theme.clone(),
            self.colors.clone(),
            loading,
            selected_commit,
            app,
        );
        let diff_view = self.repo_session.diff_view.clone();
        cx.defer(move |cx| {
            diff_view.update(cx, |mirror, cx| {
                mirror.update_snapshot(snapshot);
                cx.notify();
            });
        });
    }

    /// Pushes a transient toast notification and schedules its auto-dismissal.
    pub(crate) fn push_toast(
        &mut self,
        kind: super::toasts::ToastKind,
        message: impl Into<String>,
        cx: &mut Context<Self>,
    ) {
        let secs = kind.auto_dismiss_secs();
        let id = self.toasts.push(kind, message.into());
        cx.notify();
        cx.spawn(async move |this, cx| {
            tokio::time::sleep(std::time::Duration::from_secs(secs)).await;
            this.update(cx, |this, cx| {
                this.toasts.dismiss(id);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub(crate) fn dismiss_toast(&mut self, id: u64, cx: &mut Context<Self>) {
        self.toasts.dismiss(id);
        cx.notify();
    }

    /// Reports a failed git/hosting operation as an error toast, cleaning the
    /// raw error down to its first meaningful line.
    pub(crate) fn report_op_error(&mut self, label: &str, err: &str, cx: &mut Context<Self>) {
        let detail = super::toasts::clean_error_message(err);
        let message = if detail.is_empty() {
            format!("{label} failed")
        } else {
            format!("{label}: {detail}")
        };
        self.push_toast(super::toasts::ToastKind::Error, message, cx);
    }

    /// Reports a `GitError` using its structured `toast_message`, choosing the
    /// toast kind from the variant (e.g. `EmptyCommit` is informational, not an
    /// error). This is the structured replacement for the old string-parsing
    /// toast cleanup path.
    pub(crate) fn report_git_error(
        &mut self,
        label: &str,
        err: &gitforge_git::GitError,
        cx: &mut Context<Self>,
    ) {
        let kind = if err.is_info() {
            super::toasts::ToastKind::Info
        } else {
            super::toasts::ToastKind::Error
        };
        let detail = err.toast_message();
        let message = if detail.is_empty() {
            label.to_string()
        } else {
            format!("{label}: {detail}")
        };
        self.push_toast(kind, message, cx);
    }

    pub(crate) fn toggle_dialog_force(&mut self, cx: &mut Context<Self>) {
        self.dialog_force = !self.dialog_force;
        cx.notify();
    }

    /// Begin dragging a side-panel splitter, recording the start x and the
    /// side's current width so the drag can be computed as a pure delta.
    pub(crate) fn start_panel_resize(
        &mut self,
        side: super::panel_resize::PanelSide,
        start_x: f32,
        cx: &mut Context<Self>,
    ) {
        let start_width = match side {
            super::panel_resize::PanelSide::Sidebar => self.sidebar_width,
            super::panel_resize::PanelSide::Right => self.right_panel_width,
        };
        self.active_panel_resize = Some(super::panel_resize::PanelResize {
            side,
            start_x,
            start_width,
        });
        cx.notify();
    }

    /// Apply a drag delta (in px) to the in-flight resize. Returns true if the
    /// width changed (so the caller can decide whether to notify). Idempotent
    /// when no resize is active.
    pub(crate) fn update_panel_resize(&mut self, current_x: f32) -> bool {
        let Some(active) = self.active_panel_resize else {
            return false;
        };
        let delta = current_x - active.start_x;
        match active.side {
            // Sidebar's right edge: drag right => wider.
            super::panel_resize::PanelSide::Sidebar => {
                let next = super::panel_resize::clamp_sidebar_width(active.start_width + delta);
                if (self.sidebar_width - next).abs() < f32::EPSILON {
                    return false;
                }
                self.sidebar_width = next;
                true
            }
            // Right pane's left edge: drag right => narrower.
            super::panel_resize::PanelSide::Right => {
                let next = super::panel_resize::clamp_right_width(active.start_width - delta);
                if (self.right_panel_width - next).abs() < f32::EPSILON {
                    return false;
                }
                self.right_panel_width = next;
                true
            }
        }
    }

    /// End the in-flight resize (mouse-up). Persists the new width so it
    /// survives restarts. Returns true if a resize was active.
    pub(crate) fn finish_panel_resize(&mut self) -> bool {
        if self.active_panel_resize.take().is_some() {
            self.save_settings();
            true
        } else {
            false
        }
    }

    /// Reset a side to its default width (double-click on its splitter).
    pub(crate) fn reset_panel_width(
        &mut self,
        side: super::panel_resize::PanelSide,
        cx: &mut Context<Self>,
    ) {
        let default = super::panel_resize::default_width(side);
        match side {
            super::panel_resize::PanelSide::Sidebar => self.sidebar_width = default,
            super::panel_resize::PanelSide::Right => self.right_panel_width = default,
        }
        self.save_settings();
        cx.notify();
    }
}

impl Render for GitForgeApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.focus_subscription.is_none() {
            let handle = self.focus_handle.clone();
            let entity = cx.entity().downgrade();
            self.focus_subscription = Some(window.on_focus_in(&handle, cx, move |_w, cx| {
                let _ = entity.update(cx, |this, cx| this.on_window_focused(cx));
            }));
        }

        if self.quit_requested {
            self.quit_requested = false;
            if let Some(handle) = self.settings_window.take() {
                let _ = handle.update(cx, |_, w, _| w.remove_window());
            }
            window.remove_window();
        }

        if !self.shown_update_notification {
            self.shown_update_notification = true;
            let entity = cx.entity().downgrade();
            gitforge_update::notify_if_app_was_updated(cx, |version, cx| {
                if let Some(app) = entity.upgrade() {
                    app.update(cx, |app, cx| {
                        app.push_toast(
                            super::toasts::ToastKind::Success,
                            format!("Updated to GitForge {version}"),
                            cx,
                        );
                    });
                }
            });
        }

        self.update_indicator.update(cx, |indicator, cx| {
            indicator.set_colors(self.colors.clone(), cx);
        });

        let bg = rgba_to_hsla(self.colors.background);
        let text = rgba_to_hsla(self.colors.text);
        let entity = cx.entity().downgrade();
        let active_repo_state = self.repo_session.active_repo_state();
        let (pull_requests, pull_requests_loading) = self
            .repo_session
            .active_tab()
            .map(|tab| (tab.pull_requests.as_slice(), tab.pull_requests_loading))
            .unwrap_or((&[], false));
        let pull_request_hint = self.pull_request_sidebar_hint();

        let sidebar = super::panel_resize::wrap_with_right_edge_resize_handle(
            super::sidebar::render_sidebar(
                active_repo_state,
                &self.colors,
                self.repo_session.loading,
                &self.repo_session.sidebar_state,
                self.sidebar_width,
                entity.clone(),
                window,
                pull_requests,
                pull_requests_loading,
                pull_request_hint,
            ),
            "panel-resize-sidebar",
            super::panel_resize::PanelSide::Sidebar,
            &self.colors,
            entity.clone(),
            true,
        );

        let toolbar =
            super::toolbar::render_toolbar(active_repo_state, &self.colors, entity.clone());

        let graph_panel = super::panel_resize::wrap_with_right_edge_resize_handle(
            self.repo_session.graph_panel.render(
                &self.colors,
                self.settings.graph_show_graph_column,
                self.settings.graph_show_sha_column,
                self.settings.graph_show_time_column,
                self.settings.graph_show_author_column,
                entity.clone(),
            ),
            "panel-resize-right",
            super::panel_resize::PanelSide::Right,
            &self.colors,
            entity.clone(),
            false,
        )
        .h_full()
        .w_full();

        let graph_column = super::layout::grow_center(
            div()
                .flex()
                .flex_col()
                .h_full()
                .overflow_hidden()
                .child(toolbar)
                .child(
                    div()
                        .flex_1()
                        .min_h(px(0.0))
                        .h_full()
                        .w_full()
                        .overflow_hidden()
                        .child(graph_panel),
                ),
        );

        let right_content = match self.repo_session.view_mode {
            MainViewMode::CommitHistory => {
                if self.repo_session.graph_panel.is_uncommitted_selected() {
                    self.repo_session
                        .status_panel
                        .render_graph_staging(
                            active_repo_state,
                            &self.colors,
                            entity.clone(),
                            window,
                            self.ai_generating,
                            &self.repo_session.commit_editor,
                        )
                        .into_any_element()
                } else {
                    // Refresh the cached diff mirror only when the diff's
                    // observable state changed. Scrolling the commit history
                    // leaves it unchanged, so GPUI recycles its paint instead
                    // of re-rendering the whole diff panel every frame.
                    self.sync_diff_view(active_repo_state, entity.clone(), cx);
                    AnyView::from(self.repo_session.diff_view.clone())
                        .cached(super::diff_panel::diff_view_cache_style())
                        .into_any_element()
                }
            }
            MainViewMode::Status => self
                .repo_session
                .status_panel
                .render(
                    &self.colors,
                    entity.clone(),
                    window,
                    self.ai_generating,
                    &self.repo_session.commit_editor,
                )
                .into_any_element(),
        };

        let right_panel =
            super::layout::right_pane_fixed(div(), self.right_panel_width).child(right_content);

        let error_banner = self.repo_session.last_error.as_ref().map(|err| {
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
            &self.hosting_accounts,
            &self.colors,
            window,
            entity.clone(),
            self.titlebar_menus_visible,
            self.active_titlebar_menu,
            self.local_branch_dropdown_open,
            self.update_indicator.clone(),
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

        let repo_tab_views = self.repo_session.repo_tab_views();
        if !repo_tab_views.is_empty() {
            // GPUI clears its active drag on mouse-up whether or not the drop
            // hit a target. If our `tab_drag_source` flag survived (e.g. the
            // drag was cancelled off-target), treat it as inactive here so a
            // tab isn't left dimmed and a caret isn't left drawn. The stale
            // stored value is harmless: it's overwritten on the next drag start
            // (`on_drag`) or drop, and `has_active_drag` gates every read.
            let drag_source = self
                .repo_session
                .tab_drag_source
                .filter(|_| cx.has_active_drag());
            // Derive the insertion-caret index from the live drop target. When
            // the cursor is over the bar's tail (no specific tab target) the
            // caret sits at the end. Positions immediately adjacent to the
            // dragged tab represent a no-op move and are collapsed to `None`.
            let drop_caret = drop_caret_index(
                &self.repo_session.open_repo_tabs,
                drag_source,
                self.repo_session.tab_drop_target,
            );

            inner = inner.child(super::repo_tabs::render_repo_tab_bar(
                &repo_tab_views,
                self.repo_session.active_repo_tab_id,
                &self.colors,
                window,
                entity.clone(),
                drag_source,
                drop_caret,
            ));
        }

        if let Some(banner) = error_banner {
            inner = inner.child(banner);
        }

        let decorations = window.window_decorations();
        let rounding = px(super::layout::WINDOW_CORNER_RADIUS);
        let tiling = match decorations {
            Decorations::Server => Tiling::default(),
            Decorations::Client { tiling } => tiling,
        };

        let resize_listener = super::panel_resize::render_panel_resize_listener(entity.clone());

        let workspace_base = div()
            .relative()
            .flex_1()
            .h_full()
            .flex()
            .flex_row()
            .overflow_hidden()
            .child(sidebar)
            .child(
                div()
                    .flex_1()
                    .min_w(px(super::layout::CENTER_MIN_WIDTH))
                    .h_full()
                    .flex()
                    .flex_row()
                    .bg(bg)
                    .overflow_hidden()
                    .child(graph_column)
                    .child(right_panel),
            )
            .child(resize_listener);

        let workspace_row = if matches!(decorations, Decorations::Client { .. }) {
            super::window_chrome::seal_rounded_corners(
                super::window_chrome::apply_bottom_corner_radius(
                    workspace_base.id("workspace-row"),
                    rounding,
                    tiling,
                ),
                bg,
            )
        } else {
            workspace_base.id("workspace-row")
        };

        inner = inner.child(workspace_row);

        if self.active_dialog != AppDialog::None {
            inner = inner.child(super::ops::dialog_render::render_dialog_overlay(
                &self.active_dialog,
                &self.dialog_input,
                &self.dialog_input_2,
                self.dialog_force,
                &self.colors,
                entity.clone(),
                window,
                &self.hosting_repos,
                self.hosting_repos_loading,
                &self.create_pr,
            ));
        }

        if let Some(palette) = self
            .command_palette
            .render(&self.colors, entity.clone(), window)
        {
            inner = inner.child(palette);
        }

        if let Some(menu) = self.active_titlebar_menu {
            inner = inner.child(super::titlebar::render_titlebar_menu_dropdown(
                menu,
                &self.colors,
                entity.clone(),
            ));
        }

        if self.local_branch_dropdown_open {
            inner = inner.child(super::titlebar::render_local_branch_dropdown(
                active_repo_state,
                &self.colors,
                entity.clone(),
            ));
        }

        if self.repo_session.sidebar_state.context_menu != super::sidebar::ContextMenuAction::None {
            inner = inner.child(super::sidebar::render_context_menu_overlay(
                &self.repo_session.sidebar_state.context_menu,
                self.repo_session.sidebar_state.context_menu_pos,
                &self.colors,
                entity.clone(),
            ));
        }

        if !self.toasts.is_empty() {
            inner = inner.child(super::toasts::render_toasts(
                &self.toasts,
                &self.colors,
                entity.clone(),
            ));
        }

        let root_entity = entity.clone();

        div()
            .id("app-root")
            .size_full()
            .bg(gpui::transparent_black())
            .track_focus(&self.focus_handle)
            .on_click(move |_ev, _window, cx| {
                if let Some(e) = root_entity.upgrade() {
                    e.update(cx, |this, cx| this.close_floating_menus(cx));
                }
            })
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
            .on_action(cx.listener(Self::handle_check_for_updates))
            .on_action(cx.listener(Self::handle_quit))
            .on_action(cx.listener(Self::handle_clone))
            .on_action(cx.listener(Self::handle_clone_github))
            .on_action(cx.listener(Self::handle_clone_gitlab))
            .on_action(cx.listener(Self::handle_add_remote))
            .on_action(cx.listener(Self::handle_create_worktree))
            .on_action(cx.listener(Self::handle_open_ssh_key))
            .on_action(cx.listener(Self::handle_manage_accounts))
            .on_action(cx.listener(Self::handle_open_ai_settings))
            .on_action(cx.listener(Self::handle_create_pull_request))
            .on_action(cx.listener(Self::handle_view_file))
            .child(super::window_chrome::render_window_chrome(
                inner,
                &self.colors,
                window,
            ))
    }
}
