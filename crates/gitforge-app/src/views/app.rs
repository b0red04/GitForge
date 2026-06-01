use gitforge_git::{GitError, RepoState, Repository};
use gitforge_graph::{CommitEntry, Graph};
use gitforge_ui::{AppColors, Theme, ThemeEntry, rgba_to_hsla};
use gpui::*;

use parking_lot::Mutex;
use std::sync::Arc;

use super::command_palette::CommandPalette;
use super::commands::TitlebarMenu;
use super::diff_panel::CommitDiffState;
use super::diff_panel::DiffPanel;
use super::graph_panel::GraphPanel;
use super::settings::{AppSettings, CustomCommand};
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
    ManageAccounts,
    SearchHosting {
        provider: String,
    },
    ForkRepo {
        owner: String,
        repo: String,
        provider: String,
    },
    AiSettings,
    SetAiApiKey {
        provider: String,
    },
    CreateWorktree,
    RemoveWorktree {
        path: String,
    },
}

pub struct GitForgeApp {
    colors: AppColors,
    open_repo: Arc<Mutex<Option<Repository>>>,
    repo_state: Option<RepoState>,
    graph_panel: GraphPanel,
    diff_panel: DiffPanel,
    pub status_panel: StatusPanel,
    pub sidebar_state: SidebarState,
    pub view_mode: MainViewMode,
    active_dialog: AppDialog,
    dialog_input: String,
    dialog_input_2: String,
    dialog_input_focus: FocusHandle,
    pub remote_status: String,
    loading: bool,
    focus_handle: FocusHandle,
    settings: AppSettings,
    ssh_keys: Vec<gitforge_git::SshKey>,
    ssh_agent_status: Option<gitforge_git::SshAgentStatus>,
    hosting_accounts: Vec<gitforge_hosting::HostingAccount>,
    hosting_repos: Vec<gitforge_hosting::RemoteRepo>,
    hosting_repos_loading: bool,
    ai_generating: bool,
    last_error: Option<String>,
    toolbar_more_open: bool,
    titlebar_menus_visible: bool,
    active_titlebar_menu: Option<TitlebarMenu>,
    pub command_palette: CommandPalette,
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
            open_repo: Arc::new(Mutex::new(None)),
            repo_state: None,
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
        };
        app.sidebar_state.branches_expanded = app.settings.sidebar_branches_expanded;
        app.sidebar_state.remotes_expanded = app.settings.sidebar_remotes_expanded;
        app.sidebar_state.tags_expanded = app.settings.sidebar_tags_expanded;
        app.load_ssh_state();
        app.load_hosting_accounts();
        app
    }

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

    pub fn set_theme(&mut self, name: &str, cx: &mut Context<Self>) {
        match Theme::load_by_name(name) {
            Ok(theme) => {
                self.colors = AppColors::from_theme(&theme);
                self.settings.theme = name.to_string();
                self.settings.save();
                cx.notify();
            }
            Err(e) => {
                tracing::warn!("Failed to load theme '{}': {}", name, e);
            }
        }
    }

    fn handle_toggle_theme(
        &mut self,
        _action: &ToggleTheme,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let new_theme = if self.settings.theme == "default-dark" {
            "default-light"
        } else {
            "default-dark"
        };
        self.set_theme(new_theme, cx);
    }

    fn handle_show_command_palette(
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
                    self.load_diff_for_selected(cx);
                }
            }
            "select_next" => {
                if self.graph_panel.select_next() {
                    self.load_diff_for_selected(cx);
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
            "toggle_theme" => self.set_theme(
                if self.settings.theme == "default-dark" {
                    "default-light"
                } else {
                    "default-dark"
                },
                cx,
            ),
            "clone" => self.open_clone_dialog(cx),
            "clone_github" => self.open_clone_from_hosting_dialog("github".to_string(), cx),
            "clone_gitlab" => self.open_clone_from_hosting_dialog("gitlab".to_string(), cx),
            "add_remote" => self.open_add_remote_dialog(cx),
            "ssh_key" => self.open_ssh_generate_key_dialog(cx),
            "accounts" => self.open_manage_accounts_dialog(cx),
            "ai_settings" => self.open_ai_settings_dialog(cx),
            "open_browser" => self.open_repo_in_browser(cx),
            "worktree" => self.open_create_worktree_dialog(cx),
            "show_status" => {
                self.view_mode = MainViewMode::Status;
                self.load_status(cx);
            }
            "soft_reset" => self.soft_reset(cx),
            "open_editor" => {
                if let Some(path) = self.repo_state.as_ref().map(|r| r.path.clone()) {
                    self.open_in_editor(path, cx);
                }
            }
            "open_terminal" => {
                if let Some(path) = self.repo_state.as_ref().map(|r| r.path.clone()) {
                    self.open_in_terminal(path, cx);
                }
            }
            _ => {}
        }
    }

    fn spawn_open_dialog(&mut self, cx: &mut Context<Self>) {
        let open_repo_arc = self.open_repo.clone();
        let include_custom_refs = self.settings.show_checkpoint_refs;

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
            let log_options = gitforge_git::CommitLogOptions {
                include_custom_refs,
            };

            let result = tokio::task::spawn_blocking(
                move || -> Result<(Repository, RepoState), GitError> {
                    let repo = Repository::discover(&path_buf)?;
                    let repo_state = RepoState::from_repository_with_options(&repo, log_options)?;
                    Ok((repo, repo_state))
                },
            )
            .await;

            match result {
                Ok(Ok((repo, repo_state_data))) => {
                    *open_repo_arc.lock() = Some(repo);
                    this.update(cx, |this, cx| {
                        this.last_error = None;
                        this.apply_repo_state(repo_state_data);
                        this.loading = false;
                        cx.notify();
                    })
                    .ok();
                }
                Ok(Err(e)) => {
                    this.update(cx, |this, cx| {
                        this.last_error = Some(format!("Failed to load repository: {}", e));
                        this.loading = false;
                        cx.notify();
                    })
                    .ok();
                }
                Err(e) => {
                    this.update(cx, |this, cx| {
                        this.last_error = Some(format!("Task panicked: {}", e));
                        this.loading = false;
                        cx.notify();
                    })
                    .ok();
                }
            }
        })
        .detach();
    }

    pub fn execute_command_palette_selection(&mut self, cx: &mut Context<Self>) {
        if let Some(action) = self.command_palette.selected_action().map(String::from) {
            self.execute_command_palette_action(&action, cx);
        }
    }

    pub fn available_themes() -> Vec<ThemeEntry> {
        Theme::discover_themes()
    }

    fn handle_open_repository(
        &mut self,
        _action: &OpenRepository,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        tracing::info!("OpenRepository action fired");
        self.loading = true;
        cx.notify();

        let open_repo_arc = self.open_repo.clone();
        let include_custom_refs = self.settings.show_checkpoint_refs;

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
            let log_options = gitforge_git::CommitLogOptions {
                include_custom_refs,
            };

            let result = tokio::task::spawn_blocking(
                move || -> Result<(Repository, RepoState), GitError> {
                    let repo = Repository::discover(&path_buf)?;
                    let repo_state = RepoState::from_repository_with_options(&repo, log_options)?;
                    Ok((repo, repo_state))
                },
            )
            .await;

            match result {
                Ok(Ok((repo, repo_state_data))) => {
                    *open_repo_arc.lock() = Some(repo);

                    this.update(cx, |this, cx| {
                        this.last_error = None;
                        this.apply_repo_state(repo_state_data);
                        this.loading = false;
                        cx.notify();
                    })
                    .ok();
                }
                Ok(Err(e)) => {
                    tracing::error!("Failed to load repository: {}", e);
                    this.update(cx, |this, cx| {
                        this.last_error = Some(format!("Failed to load repository: {}", e));
                        this.loading = false;
                        cx.notify();
                    })
                    .ok();
                }
                Err(e) => {
                    tracing::error!("Task panicked: {}", e);
                    this.update(cx, |this, cx| {
                        this.last_error = Some(format!("Task panicked: {}", e));
                        this.loading = false;
                        cx.notify();
                    })
                    .ok();
                }
            }
        })
        .detach();
    }

    fn handle_close_dialog(
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

    fn handle_select_prev(
        &mut self,
        _action: &SelectPrevCommit,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.graph_panel.select_prev() {
            self.load_diff_for_selected(cx);
        }
    }

    fn handle_select_next(
        &mut self,
        _action: &SelectNextCommit,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.graph_panel.select_next() {
            self.load_diff_for_selected(cx);
        }
    }

    fn handle_show_status(
        &mut self,
        _action: &ShowStatusPanel,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.view_mode = MainViewMode::Status;
        self.load_status(cx);
    }

    fn handle_refresh(
        &mut self,
        _action: &RefreshRepository,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.refresh_repository(cx);
    }

    fn handle_soft_reset(
        &mut self,
        _action: &SoftReset,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.soft_reset(cx);
    }

    fn handle_create_branch(
        &mut self,
        _action: &CreateBranch,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_create_branch_dialog(None, cx);
    }

    fn handle_stash_push(
        &mut self,
        _action: &StashPush,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.active_dialog = AppDialog::StashPush;
        self.dialog_input.clear();
        cx.notify();
    }

    fn handle_stash_pop(
        &mut self,
        _action: &StashPop,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.stash_pop(cx);
    }

    fn handle_fetch_all(
        &mut self,
        _action: &FetchAll,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.fetch_all(cx);
    }

    fn handle_push_current(
        &mut self,
        _action: &PushCurrent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_push_dialog(cx);
    }

    fn handle_pull_current(
        &mut self,
        _action: &PullCurrent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_pull_dialog(cx);
    }

    fn apply_repo_state(&mut self, repo_state_data: RepoState) {
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
        self.status_panel.set_status(repo_state_data.status.clone());
        self.diff_panel.clear();
        self.repo_state = Some(repo_state_data);
    }

    pub fn select_commit(&mut self, idx: usize, cx: &mut Context<Self>) {
        self.view_mode = MainViewMode::CommitHistory;
        self.graph_panel.select(idx);
        self.load_diff_for_selected(cx);
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

        let open_repo = self.open_repo.clone();
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

    pub fn back_to_diff(
        &mut self,
        _action: &BackToDiff,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.diff_panel.set_diff_mode();
        cx.notify();
    }

    pub fn back_to_diff_mode(&mut self, cx: &mut Context<Self>) {
        self.diff_panel.set_diff_mode();
        cx.notify();
    }

    pub fn toggle_sidebar_branches(&mut self, cx: &mut Context<Self>) {
        self.sidebar_state.branches_expanded = !self.sidebar_state.branches_expanded;
        cx.notify();
    }

    pub fn toggle_sidebar_remotes(&mut self, cx: &mut Context<Self>) {
        self.sidebar_state.remotes_expanded = !self.sidebar_state.remotes_expanded;
        cx.notify();
    }

    pub fn toggle_sidebar_tags(&mut self, cx: &mut Context<Self>) {
        self.sidebar_state.tags_expanded = !self.sidebar_state.tags_expanded;
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
        section: super::status_panel::StatusFileSection,
        idx: usize,
        path: String,
        cx: &mut Context<Self>,
    ) {
        self.status_panel.select_file(section, idx);

        let open_repo = self.open_repo.clone();
        let is_staged = section == super::status_panel::StatusFileSection::Staged;

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

        let open_repo = self.open_repo.clone();

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

    pub fn generate_commit_message(&mut self, cx: &mut Context<Self>) {
        let provider_name = self.settings.ai.provider.clone();
        if provider_name == "disabled" {
            return;
        }
        let model = if self.settings.ai.model.is_empty() {
            match provider_name.as_str() {
                "ollama" => "codellama".to_string(),
                "openai" => "gpt-4o-mini".to_string(),
                "anthropic" => "claude-sonnet-4-20250514".to_string(),
                _ => return,
            }
        } else {
            self.settings.ai.model.clone()
        };
        let conventional = self.settings.ai.conventional_commits;
        let open_repo = self.open_repo.clone();

        self.ai_generating = true;
        cx.notify();

        cx.spawn(async move |this, cx| {
            let diff_result = tokio::task::spawn_blocking(move || {
                let repo_lock = open_repo.lock();
                let Some(repo) = repo_lock.as_ref() else {
                    return Err::<String, anyhow::Error>(anyhow::anyhow!("No repository open"));
                };
                repo.diff_head_to_index(None)
                    .map_err(|e| anyhow::anyhow!("{}", e))
            })
            .await;

            let diff = match diff_result {
                Ok(Ok(d)) => d,
                Ok(Err(e)) => {
                    tracing::error!("Failed to get staged diff: {}", e);
                    this.update(cx, |this, cx| {
                        this.ai_generating = false;
                        cx.notify();
                    })
                    .ok();
                    return;
                }
                Err(e) => {
                    tracing::error!("Diff task panicked: {}", e);
                    this.update(cx, |this, cx| {
                        this.ai_generating = false;
                        cx.notify();
                    })
                    .ok();
                    return;
                }
            };

            if diff.trim().is_empty() {
                tracing::warn!("No staged changes to generate commit message from");
                this.update(cx, |this, cx| {
                    this.ai_generating = false;
                    cx.notify();
                })
                .ok();
                return;
            }

            let provider_result = gitforge_ai::create_provider(&provider_name, &model);
            let provider = match provider_result {
                Ok(p) => p,
                Err(e) => {
                    tracing::error!("Failed to create AI provider: {}", e);
                    this.update(cx, |this, cx| {
                        this.ai_generating = false;
                        cx.notify();
                    })
                    .ok();
                    return;
                }
            };

            match provider
                .generate_commit_messages(&diff, conventional, 3)
                .await
            {
                Ok(messages) => {
                    this.update(cx, |this, cx| {
                        this.ai_generating = false;
                        if !messages.is_empty() {
                            this.status_panel.commit_message_mut().clear();
                            this.status_panel
                                .commit_message_mut()
                                .push_str(&messages[0]);
                            this.status_panel.set_ai_alternatives(messages);
                        }
                        cx.notify();
                    })
                    .ok();
                }
                Err(e) => {
                    tracing::error!("AI generation failed: {}", e);
                    this.update(cx, |this, cx| {
                        this.ai_generating = false;
                        cx.notify();
                    })
                    .ok();
                }
            }
        })
        .detach();
    }

    pub fn open_ai_settings_dialog(&mut self, cx: &mut Context<Self>) {
        self.active_dialog = AppDialog::AiSettings;
        self.dialog_input = self.settings.ai.provider.clone();
        self.dialog_input_2 = self.settings.ai.model.clone();
        cx.notify();
    }

    pub fn select_ai_alternative(&mut self, idx: usize, cx: &mut Context<Self>) {
        let alts = self.status_panel.ai_alternatives().to_vec();
        if let Some(msg) = alts.get(idx) {
            self.status_panel.commit_message_mut().clear();
            self.status_panel.commit_message_mut().push_str(msg);
        }
        cx.notify();
    }

    pub fn summarize_file_diff(&mut self, path: String, cx: &mut Context<Self>) {
        let provider_name = self.settings.ai.provider.clone();
        if provider_name == "disabled" {
            return;
        }
        let model = if self.settings.ai.model.is_empty() {
            match provider_name.as_str() {
                "ollama" => "codellama".to_string(),
                "openai" => "gpt-4o-mini".to_string(),
                "anthropic" => "claude-sonnet-4-20250514".to_string(),
                _ => return,
            }
        } else {
            self.settings.ai.model.clone()
        };
        let open_repo = self.open_repo.clone();
        let path_for_result = path.clone();

        cx.spawn(async move |this, cx| {
            let p = path_for_result.clone();
            let diff_result = tokio::task::spawn_blocking(move || {
                let repo_lock = open_repo.lock();
                let Some(repo) = repo_lock.as_ref() else {
                    return Err::<String, anyhow::Error>(anyhow::anyhow!("No repository open"));
                };
                let path = std::path::Path::new(&p);
                repo.diff_index_to_worktree(Some(path))
                    .or_else(|_| repo.diff_head_to_index(Some(path)))
                    .map_err(|e| anyhow::anyhow!("{}", e))
            })
            .await;

            let diff = match diff_result {
                Ok(Ok(d)) => d,
                Ok(Err(e)) => {
                    tracing::error!("Failed to get diff for {}: {}", path_for_result, e);
                    return;
                }
                Err(e) => {
                    tracing::error!("Diff task panicked: {}", e);
                    return;
                }
            };

            if diff.trim().is_empty() {
                return;
            }

            let provider = match gitforge_ai::create_provider(&provider_name, &model) {
                Ok(p) => p,
                Err(e) => {
                    tracing::error!("Failed to create AI provider: {}", e);
                    return;
                }
            };

            match provider.summarize_diff(&diff).await {
                Ok(summary) => {
                    let p = path_for_result.clone();
                    this.update(cx, |this, cx| {
                        this.status_panel.set_file_summary(p.clone(), summary);
                        this.status_panel.show_file_summary(Some(p));
                        cx.notify();
                    })
                    .ok();
                }
                Err(e) => {
                    tracing::error!("AI summarization failed: {}", e);
                }
            }
        })
        .detach();
    }

    pub fn dismiss_file_summary(&mut self, cx: &mut Context<Self>) {
        self.status_panel.show_file_summary(None);
        cx.notify();
    }

    fn open_ai_api_key_dialog(&mut self, provider: String, cx: &mut Context<Self>) {
        self.active_dialog = AppDialog::SetAiApiKey { provider };
        self.dialog_input.clear();
        cx.notify();
    }

    pub fn load_status(&mut self, cx: &mut Context<Self>) {
        let open_repo = self.open_repo.clone();

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
                        this.status_panel.set_status(status);
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

    pub fn open_stash_push_dialog(&mut self, cx: &mut Context<Self>) {
        self.active_dialog = AppDialog::StashPush;
        self.dialog_input.clear();
        cx.notify();
    }

    pub fn cancel_dialog(&mut self, cx: &mut Context<Self>) {
        self.active_dialog = AppDialog::None;
        self.dialog_input.clear();
        self.dialog_input_2.clear();
        cx.notify();
    }

    pub fn edit_dialog_input(&mut self, typed_char: Option<&str>, cx: &mut Context<Self>) {
        match typed_char {
            Some(ch) => self.dialog_input.push_str(ch),
            None => {
                self.dialog_input.pop();
            }
        }
        cx.notify();
    }

    pub fn confirm_dialog(&mut self, cx: &mut Context<Self>) {
        let input = self.dialog_input.trim().to_string();
        let input_2 = self.dialog_input_2.trim().to_string();
        let dialog = self.active_dialog.clone();
        self.active_dialog = AppDialog::None;
        self.dialog_input.clear();
        self.dialog_input_2.clear();

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
                    match self.repo_state.as_ref() {
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
            AppDialog::AddAccount { provider } => {
                if input.is_empty() {
                    return;
                }
                self.add_hosting_account(provider, input, cx);
            }
            AppDialog::ManageAccounts => {}
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
            AppDialog::AiSettings => {
                let provider = input.to_string();
                let model = input_2.to_string();
                if !provider.is_empty() && provider != "disabled" {
                    self.settings.ai.provider = provider;
                    self.settings.ai.model = model;
                    self.save_settings();
                }
            }
            AppDialog::SetAiApiKey { provider } => {
                if input.is_empty() {
                    return;
                }
                if let Err(e) = gitforge_ai::store_api_key(&provider, &input) {
                    tracing::error!("Failed to store API key: {}", e);
                }
                self.open_ai_settings_dialog(cx);
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
            AppDialog::None => {}
        }
    }

    fn run_git_op<F, R>(&mut self, label: &str, cx: &mut Context<Self>, op: F)
    where
        F: FnOnce(&gitforge_git::Repository) -> Result<R, gitforge_git::GitError> + Send + 'static,
        R: Send + 'static,
    {
        let open_repo = self.open_repo.clone();
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

    fn run_git_op_with_status<F, R>(
        &mut self,
        label: &str,
        status: &str,
        cx: &mut Context<Self>,
        op: F,
    ) where
        F: FnOnce(&gitforge_git::Repository) -> Result<R, gitforge_git::GitError> + Send + 'static,
        R: Send + 'static,
    {
        let open_repo = self.open_repo.clone();
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
        self.sidebar_state.worktrees_expanded = !self.sidebar_state.worktrees_expanded;
        cx.notify();
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

    fn open_repo_from_path(&mut self, path: std::path::PathBuf, cx: &mut Context<Self>) {
        let open_repo_arc = self.open_repo.clone();
        self.loading = true;
        cx.notify();

        let log_options = gitforge_git::CommitLogOptions {
            include_custom_refs: self.settings.show_checkpoint_refs,
        };

        cx.spawn(async move |this, cx| {
            let result = tokio::task::spawn_blocking(move || -> Result<(gitforge_git::Repository, gitforge_git::RepoState), gitforge_git::GitError> {
                let repo = gitforge_git::Repository::discover(&path)?;
                let repo_state = gitforge_git::RepoState::from_repository_with_options(&repo, log_options)?;
                Ok((repo, repo_state))
            }).await;

            match result {
                Ok(Ok((repo, repo_state_data))) => {
                    *open_repo_arc.lock() = Some(repo);
                    this.update(cx, |this, cx| {
                        this.apply_repo_state(repo_state_data);
                        this.loading = false;
                        cx.notify();
                    }).ok();
                }
                Ok(Err(e)) => {
                    tracing::error!("Failed to open cloned repo: {}", e);
                    this.update(cx, |this, cx| {
                        this.loading = false;
                        cx.notify();
                    }).ok();
                }
                Err(e) => {
                    tracing::error!("Open repo task panicked: {}", e);
                    this.update(cx, |this, cx| {
                        this.loading = false;
                        cx.notify();
                    }).ok();
                }
            }
        }).detach();
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

    pub fn load_ssh_state(&mut self) {
        self.ssh_keys = gitforge_git::ssh::list_ssh_keys().unwrap_or_default();
        self.ssh_agent_status = Some(gitforge_git::ssh::check_ssh_agent());
    }

    pub fn open_ssh_generate_key_dialog(&mut self, cx: &mut Context<Self>) {
        self.active_dialog = AppDialog::SshGenerateKey;
        self.dialog_input.clear();
        self.dialog_input_2.clear();
        cx.notify();
    }

    pub fn open_ssh_test_dialog(&mut self, cx: &mut Context<Self>) {
        self.active_dialog = AppDialog::SshTestConnection;
        self.dialog_input.clear();
        cx.notify();
    }

    pub fn generate_ssh_key(&mut self, key_type: String, email: String, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let result = tokio::task::spawn_blocking(move || {
                gitforge_git::ssh::generate_ssh_key(&key_type, &email, None, None)
            })
            .await;

            match result {
                Ok(Ok(_path)) => {
                    this.update(cx, |this, cx| {
                        this.load_ssh_state();
                        this.remote_status = "SSH key generated successfully".to_string();
                        cx.notify();
                    })
                    .ok();
                }
                Ok(Err(e)) => {
                    this.update(cx, |this, cx| {
                        this.remote_status = format!("SSH key generation failed: {}", e);
                        cx.notify();
                    })
                    .ok();
                }
                Err(e) => {
                    this.update(cx, |this, cx| {
                        this.remote_status = format!("SSH key generation error: {}", e);
                        cx.notify();
                    })
                    .ok();
                }
            }
        })
        .detach();
    }

    pub fn delete_ssh_key(&mut self, key_name: String, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let result =
                tokio::task::spawn_blocking(move || gitforge_git::ssh::delete_ssh_key(&key_name))
                    .await;

            match result {
                Ok(Ok(())) => {
                    this.update(cx, |this, cx| {
                        this.load_ssh_state();
                        cx.notify();
                    })
                    .ok();
                }
                Ok(Err(e)) => {
                    tracing::error!("Failed to delete SSH key: {}", e);
                }
                Err(e) => {
                    tracing::error!("SSH key delete task panicked: {}", e);
                }
            }
        })
        .detach();
    }

    pub fn add_key_to_agent(&mut self, key_name: String, cx: &mut Context<Self>) {
        let key_name_display = key_name.clone();
        cx.spawn(async move |this, cx| {
            let kn = key_name;
            let kn_display = key_name_display;
            let result =
                tokio::task::spawn_blocking(move || gitforge_git::ssh::add_key_to_agent(&kn)).await;

            match result {
                Ok(Ok(())) => {
                    this.update(cx, |this, cx| {
                        this.load_ssh_state();
                        this.remote_status = format!("Key {} added to ssh-agent", kn_display);
                        cx.notify();
                    })
                    .ok();
                }
                Ok(Err(e)) => {
                    this.update(cx, |this, cx| {
                        this.remote_status = format!("Failed to add key to agent: {}", e);
                        cx.notify();
                    })
                    .ok();
                }
                Err(e) => {
                    this.update(cx, |this, cx| {
                        this.remote_status = format!("ssh-add error: {}", e);
                        cx.notify();
                    })
                    .ok();
                }
            }
        })
        .detach();
    }

    pub fn remove_key_from_agent(&mut self, key_name: String, cx: &mut Context<Self>) {
        let key_name_display = key_name.clone();
        cx.spawn(async move |this, cx| {
            let kn = key_name;
            let kn_display = key_name_display;
            let result =
                tokio::task::spawn_blocking(move || gitforge_git::ssh::remove_key_from_agent(&kn))
                    .await;

            match result {
                Ok(Ok(())) => {
                    this.update(cx, |this, cx| {
                        this.load_ssh_state();
                        this.remote_status = format!("Key {} removed from ssh-agent", kn_display);
                        cx.notify();
                    })
                    .ok();
                }
                Ok(Err(e)) => {
                    this.update(cx, |this, cx| {
                        this.remote_status = format!("Failed to remove key from agent: {}", e);
                        cx.notify();
                    })
                    .ok();
                }
                Err(e) => {
                    this.update(cx, |this, cx| {
                        this.remote_status = format!("ssh-add error: {}", e);
                        cx.notify();
                    })
                    .ok();
                }
            }
        })
        .detach();
    }

    pub fn test_ssh_connection(&mut self, host: String, cx: &mut Context<Self>) {
        let host_display = host.clone();
        self.remote_status = format!("Testing SSH connection to {}...", host);
        cx.notify();

        cx.spawn(async move |this, cx| {
            let h = host;
            let result =
                tokio::task::spawn_blocking(move || gitforge_git::ssh::test_ssh_connection(&h))
                    .await;

            match result {
                Ok(Ok(msg)) => {
                    let display = host_display;
                    this.update(cx, |this, cx| {
                        this.remote_status = format!("SSH test {}: {}", display, msg);
                        cx.notify();
                    })
                    .ok();
                }
                Ok(Err(e)) => {
                    this.update(cx, |this, cx| {
                        this.remote_status = format!("SSH test failed: {}", e);
                        cx.notify();
                    })
                    .ok();
                }
                Err(e) => {
                    this.update(cx, |this, cx| {
                        this.remote_status = format!("SSH test error: {}", e);
                        cx.notify();
                    })
                    .ok();
                }
            }
        })
        .detach();
    }

    pub fn open_credential_add_dialog(&mut self, cx: &mut Context<Self>) {
        self.active_dialog = AppDialog::CredentialAdd;
        self.dialog_input.clear();
        self.dialog_input_2.clear();
        cx.notify();
    }

    pub fn add_credential(
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
                        this.remote_status = "Credential stored in keyring".to_string();
                        cx.notify();
                    })
                    .ok();
                }
                Ok(Err(e)) => {
                    this.update(cx, |this, cx| {
                        this.remote_status = format!("Failed to store credential: {}", e);
                        cx.notify();
                    })
                    .ok();
                }
                Err(e) => {
                    this.update(cx, |this, cx| {
                        this.remote_status = format!("Credential storage error: {}", e);
                        cx.notify();
                    })
                    .ok();
                }
            }
        })
        .detach();
    }

    pub fn delete_credential(&mut self, host: String, username: String, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let result = tokio::task::spawn_blocking(move || {
                gitforge_git::credential::delete_credential(&host, &username)
            })
            .await;

            match result {
                Ok(Ok(())) => {
                    this.update(cx, |this, cx| {
                        this.remote_status = "Credential deleted".to_string();
                        cx.notify();
                    })
                    .ok();
                }
                Ok(Err(e)) => {
                    this.update(cx, |this, cx| {
                        this.remote_status = format!("Failed to delete credential: {}", e);
                        cx.notify();
                    })
                    .ok();
                }
                Err(e) => {
                    this.update(cx, |this, cx| {
                        this.remote_status = format!("Credential delete error: {}", e);
                        cx.notify();
                    })
                    .ok();
                }
            }
        })
        .detach();
    }

    fn find_hosting_account(&self, provider: &str) -> Option<gitforge_hosting::HostingAccount> {
        self.hosting_accounts
            .iter()
            .find(|a| a.provider == provider)
            .cloned()
    }

    fn load_hosting_accounts(&mut self) {
        let path = dirs::config_dir()
            .unwrap_or_default()
            .join("gitforge")
            .join("hosting_accounts.json");

        if path.exists() {
            if let Ok(data) = std::fs::read_to_string(&path) {
                if let Ok(accounts) = serde_json::from_str(&data) {
                    self.hosting_accounts = accounts;
                }
            }
        }
    }

    fn save_hosting_accounts(&self) {
        let path = dirs::config_dir()
            .unwrap_or_default()
            .join("gitforge")
            .join("hosting_accounts.json");

        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        if let Ok(data) = serde_json::to_string_pretty(&self.hosting_accounts) {
            let _ = std::fs::write(&path, data);
        }
    }

    pub fn add_hosting_account(&mut self, provider: String, token: String, cx: &mut Context<Self>) {
        self.remote_status = format!("Authenticating with {}...", provider);
        cx.notify();

        let provider_name = provider.clone();
        let token_for_auth = token.clone();
        cx.spawn(async move |this, cx| {
            let Some(p) = gitforge_hosting::get_provider(&provider_name) else {
                this.update(cx, |this, cx| {
                    this.remote_status = format!("Unknown provider: {}", provider_name);
                    cx.notify();
                })
                .ok();
                return;
            };

            let result = p.authenticate(&token_for_auth).await;

            match result {
                Ok(account) => {
                    this.update(cx, |this, cx| {
                        this.hosting_accounts.push(account);
                        this.save_hosting_accounts();
                        this.remote_status = "Account authenticated successfully".to_string();
                        cx.notify();
                    })
                    .ok();
                }
                Err(e) => {
                    this.update(cx, |this, cx| {
                        this.remote_status = format!("Authentication failed: {}", e);
                        cx.notify();
                    })
                    .ok();
                }
            }
        })
        .detach();
    }

    pub fn remove_hosting_account(
        &mut self,
        username: String,
        provider: String,
        cx: &mut Context<Self>,
    ) {
        if let Some(account) = self
            .hosting_accounts
            .iter()
            .find(|a| a.username == username && a.provider == provider)
        {
            let _ = gitforge_hosting::HostingAccount::delete_token(&account.token_key);
        }
        self.hosting_accounts
            .retain(|a| !(a.username == username && a.provider == provider));
        self.save_hosting_accounts();
        cx.notify();
    }

    pub fn open_clone_from_hosting_dialog(&mut self, provider: String, cx: &mut Context<Self>) {
        self.active_dialog = AppDialog::CloneFromHosting {
            provider: provider.clone(),
        };
        self.hosting_repos.clear();
        self.hosting_repos_loading = true;
        cx.notify();

        let account = self.find_hosting_account(&provider);

        let provider_name = provider.clone();
        cx.spawn(async move |this, cx| {
            let Some(account) = account else {
                this.update(cx, |this, cx| {
                    this.hosting_repos_loading = false;
                    this.remote_status =
                        format!("No {} account configured. Add one first.", provider_name);
                    cx.notify();
                })
                .ok();
                return;
            };

            let provider_name = account.provider.clone();
            let Some(p) = gitforge_hosting::get_provider(&provider_name) else {
                this.update(cx, |this, cx| {
                    this.hosting_repos_loading = false;
                    this.remote_status = "Unknown provider".to_string();
                    cx.notify();
                })
                .ok();
                return;
            };

            let result = p.list_repos(&account).await;

            match result {
                Ok(repos) => {
                    this.update(cx, |this, cx| {
                        this.hosting_repos = repos;
                        this.hosting_repos_loading = false;
                        cx.notify();
                    })
                    .ok();
                }
                Err(e) => {
                    this.update(cx, |this, cx| {
                        this.hosting_repos_loading = false;
                        this.remote_status = format!("Failed to list repos: {}", e);
                        cx.notify();
                    })
                    .ok();
                }
            }
        })
        .detach();
    }

    pub fn clone_hosting_repo(
        &mut self,
        clone_url: String,
        repo_name: String,
        cx: &mut Context<Self>,
    ) {
        self.active_dialog = AppDialog::None;
        self.remote_status = format!("Cloning {}...", repo_name);
        cx.notify();

        cx.spawn(async move |this, cx| {
            let path = dirs::home_dir()
                .unwrap_or_default()
                .join("Projects")
                .join(&repo_name);
            let path_display = path.display().to_string();
            let url = clone_url;

            let result = tokio::task::spawn_blocking(move || {
                gitforge_git::Repository::clone_repo(&url, &path, false, None)
            })
            .await;

            match result {
                Ok(Ok(_)) => {
                    let p = std::path::PathBuf::from(path_display);
                    this.update(cx, |this, cx| {
                        this.remote_status.clear();
                        this.open_repo_from_path(p, cx);
                    })
                    .ok();
                }
                Ok(Err(e)) => {
                    this.update(cx, |this, cx| {
                        this.remote_status = format!("Clone failed: {}", e);
                        cx.notify();
                    })
                    .ok();
                }
                Err(e) => {
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

    pub fn open_add_account_dialog(&mut self, provider: String, cx: &mut Context<Self>) {
        self.active_dialog = AppDialog::AddAccount { provider };
        self.dialog_input.clear();
        cx.notify();
    }

    pub fn open_manage_accounts_dialog(&mut self, cx: &mut Context<Self>) {
        self.active_dialog = AppDialog::ManageAccounts;
        cx.notify();
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

    pub fn search_hosting_repos(
        &mut self,
        query: String,
        provider: String,
        cx: &mut Context<Self>,
    ) {
        let account = self.find_hosting_account(&provider);

        self.hosting_repos.clear();
        self.hosting_repos_loading = true;
        cx.notify();

        let provider_name = provider.clone();
        cx.spawn(async move |this, cx| {
            let Some(account) = account else {
                this.update(cx, |this, cx| {
                    this.hosting_repos_loading = false;
                    this.remote_status = format!("No {} account configured.", provider_name);
                    cx.notify();
                })
                .ok();
                return;
            };

            let provider_name = account.provider.clone();
            let Some(p) = gitforge_hosting::get_provider(&provider_name) else {
                this.update(cx, |this, cx| {
                    this.hosting_repos_loading = false;
                    cx.notify();
                })
                .ok();
                return;
            };

            let result = p.search_repos(&account, &query).await;

            match result {
                Ok(repos) => {
                    this.update(cx, |this, cx| {
                        this.hosting_repos = repos;
                        this.hosting_repos_loading = false;
                        cx.notify();
                    })
                    .ok();
                }
                Err(e) => {
                    this.update(cx, |this, cx| {
                        this.hosting_repos_loading = false;
                        this.remote_status = format!("Search failed: {}", e);
                        cx.notify();
                    })
                    .ok();
                }
            }
        })
        .detach();
    }

    pub fn fork_repo(
        &mut self,
        owner: String,
        repo: String,
        provider: String,
        cx: &mut Context<Self>,
    ) {
        self.active_dialog = AppDialog::None;
        self.remote_status = format!("Forking {}/{}...", owner, repo);
        cx.notify();

        let account = self.find_hosting_account(&provider);

        cx.spawn(async move |this, cx| {
            let Some(account) = account else {
                this.update(cx, |this, cx| {
                    this.remote_status = "No account configured for fork".to_string();
                    cx.notify();
                })
                .ok();
                return;
            };

            let provider_name = account.provider.clone();
            let Some(p) = gitforge_hosting::get_provider(&provider_name) else {
                this.update(cx, |this, cx| {
                    this.remote_status = "Unknown provider for fork".to_string();
                    cx.notify();
                })
                .ok();
                return;
            };

            let result = p.create_fork(&account, &owner, &repo).await;

            match result {
                Ok(forked) => {
                    this.update(cx, |this, cx| {
                        this.remote_status = format!("Forked to {}", forked.full_name);
                        cx.notify();
                    })
                    .ok();
                }
                Err(e) => {
                    this.update(cx, |this, cx| {
                        this.remote_status = format!("Fork failed: {}", e);
                        cx.notify();
                    })
                    .ok();
                }
            }
        })
        .detach();
    }

    pub fn open_in_browser(&mut self, url: String) {
        let _ = std::process::Command::new("xdg-open").arg(&url).spawn();
    }

    pub fn open_in_editor(&mut self, path: std::path::PathBuf, _cx: &mut Context<Self>) {
        let cmd = &self.settings.tools.editor_command;
        let _ = std::process::Command::new(cmd).arg(&path).spawn();
    }

    pub fn open_in_terminal(&mut self, path: std::path::PathBuf, _cx: &mut Context<Self>) {
        let cmd = &self.settings.tools.terminal_command;
        let _ = std::process::Command::new(cmd)
            .arg("--working-directory")
            .arg(&path)
            .spawn()
            .or_else(|_| {
                std::process::Command::new(cmd)
                    .arg("--dir")
                    .arg(&path)
                    .spawn()
            })
            .or_else(|_| std::process::Command::new(cmd).current_dir(&path).spawn())
            .ok();
    }

    pub fn open_file_in_editor(
        &mut self,
        file_path: String,
        line: Option<usize>,
        _cx: &mut Context<Self>,
    ) {
        let cmd = &self.settings.tools.editor_command;
        let path = std::path::PathBuf::from(&file_path);
        let formatted = match line {
            Some(l) => format!("{}:{}", file_path, l),
            None => file_path.clone(),
        };
        let _ = std::process::Command::new(cmd)
            .arg(&formatted)
            .spawn()
            .or_else(|_| {
                std::process::Command::new(cmd)
                    .arg("+")
                    .arg(line.unwrap_or(1).to_string())
                    .arg(&file_path)
                    .spawn()
            })
            .or_else(|_| std::process::Command::new(cmd).arg(&path).spawn())
            .ok();
    }

    pub fn open_diff_tool(&mut self, old_path: &str, new_path: &str, _cx: &mut Context<Self>) {
        let tool = &self.settings.tools.diff_tool;
        if tool.is_empty() {
            return;
        }
        let _ = std::process::Command::new(tool)
            .arg(old_path)
            .arg(new_path)
            .spawn();
    }

    pub fn open_merge_tool(&mut self, file_path: &str, _cx: &mut Context<Self>) {
        let tool = &self.settings.tools.merge_tool;
        if tool.is_empty() {
            let _ = std::process::Command::new("git")
                .args(["mergetool", file_path])
                .spawn();
            return;
        }
        let _ = std::process::Command::new(tool).arg(file_path).spawn();
    }

    pub fn run_custom_command(
        &mut self,
        command: &CustomCommand,
        repo_path: &std::path::Path,
        file: Option<&str>,
        line: Option<usize>,
        commit: Option<&str>,
    ) {
        let mut cmd_str = command.command.clone();
        if let Some(f) = file {
            cmd_str = cmd_str.replace("{file}", f);
        }
        if let Some(l) = line {
            cmd_str = cmd_str.replace("{line}", &l.to_string());
        }
        if let Some(c) = commit {
            cmd_str = cmd_str.replace("{commit}", c);
        }
        cmd_str = cmd_str.replace("{repo}", &repo_path.to_string_lossy());
        let _ = std::process::Command::new("sh")
            .args(["-c", &cmd_str])
            .current_dir(repo_path)
            .spawn();
    }

    pub fn open_repo_in_browser(&mut self, _cx: &mut Context<Self>) {
        let Some(rs) = &self.repo_state else { return };

        let remotes: Vec<_> = rs
            .references
            .iter()
            .filter(|r| r.kind == gitforge_git::RefKind::RemoteBranch)
            .filter_map(|r| r.name.split('/').next().map(|s| s.to_string()))
            .collect();

        let remote_name = if remotes.contains(&"origin".to_string()) {
            "origin"
        } else {
            match remotes.first() {
                Some(r) => r.as_str(),
                None => return,
            }
        };

        let head_branch = rs
            .references
            .iter()
            .find(|r| r.is_head && r.kind == gitforge_git::RefKind::Branch)
            .map(|r| r.name.clone());

        let remote_branch = head_branch.as_ref().and_then(|b| {
            rs.references.iter().find(|r| {
                r.kind == gitforge_git::RefKind::RemoteBranch
                    && r.name == format!("{}/{}", remote_name, b)
            })
        });

        let remote_url = rs
            .references
            .iter()
            .find(|r| {
                r.kind == gitforge_git::RefKind::RemoteBranch
                    && r.name.starts_with(&format!("{}/", remote_name))
            })
            .and_then(|_| self.get_remote_url(remote_name));

        let Some(url) = remote_url else { return };
        let clean_url = gitforge_hosting::urls::normalize_remote_url(&url);

        let sha = remote_branch
            .map(|r| r.target_commit_id.clone())
            .or_else(|| rs.commits.first().map(|c| c.id.clone()));

        let provider = gitforge_hosting::urls::detect_provider(&clean_url);
        let full_name = gitforge_hosting::urls::extract_repo_full_name(&clean_url);

        let browser_url = match (&provider, &sha) {
            (Some(p), Some(_s)) => p.repo_url(&full_name),
            (Some(p), None) => p.repo_url(&full_name),
            (None, _) => clean_url.clone(),
        };

        self.open_in_browser(browser_url);
    }

    pub fn open_commit_in_browser(&mut self, commit_id: String) {
        let _rs = &self.repo_state;
        let remote_url = self.get_first_remote_url();
        let Some(url) = remote_url else { return };

        let clean_url = gitforge_hosting::urls::normalize_remote_url(&url);
        let provider = gitforge_hosting::urls::detect_provider(&clean_url);
        let full_name = gitforge_hosting::urls::extract_repo_full_name(&clean_url);

        let browser_url = match provider {
            Some(p) => p.commit_url(&full_name, &commit_id),
            None => clean_url,
        };

        self.open_in_browser(browser_url);
    }

    pub fn open_file_at_line_in_browser(&mut self, path: String, line: Option<u32>) {
        let Some(rs) = &self.repo_state else { return };

        let sha = rs.commits.first().map(|c| c.id.clone());
        let Some(sha) = sha else { return };

        let remote_url = self.get_first_remote_url();
        let Some(url) = remote_url else { return };

        let clean_url = gitforge_hosting::urls::normalize_remote_url(&url);
        let provider = gitforge_hosting::urls::detect_provider(&clean_url);
        let full_name = gitforge_hosting::urls::extract_repo_full_name(&clean_url);

        let browser_url = match provider {
            Some(p) => p.file_url(&full_name, &sha, &path, line),
            None => return,
        };

        self.open_in_browser(browser_url);
    }

    fn get_remote_url(&self, remote_name: &str) -> Option<String> {
        let repo_lock = self.open_repo.lock();
        let repo = repo_lock.as_ref()?;
        let remotes = repo.remote_list().ok()?;
        remotes
            .iter()
            .find(|(name, _)| name == remote_name)
            .map(|(_, url)| url.clone())
    }

    fn get_first_remote_url(&self) -> Option<String> {
        let repo_lock = self.open_repo.lock();
        let repo = repo_lock.as_ref()?;
        let remotes = repo.remote_list().ok()?;
        remotes.first().map(|(_, url)| url.clone())
    }

    pub fn open_fork_dialog(
        &mut self,
        owner: String,
        repo: String,
        provider: String,
        cx: &mut Context<Self>,
    ) {
        self.active_dialog = AppDialog::ForkRepo {
            owner,
            repo,
            provider,
        };
        cx.notify();
    }

    fn save_settings(&mut self) {
        self.settings.sidebar_branches_expanded = self.sidebar_state.branches_expanded;
        self.settings.sidebar_remotes_expanded = self.sidebar_state.remotes_expanded;
        self.settings.sidebar_tags_expanded = self.sidebar_state.tags_expanded;
        if let Some(rs) = &self.repo_state {
            self.settings.last_repo_path = Some(rs.path.to_string_lossy().to_string());
        }
        self.settings.save();
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
        let open_repo = self.open_repo.clone();
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

    fn refresh_repository(&mut self, cx: &mut Context<Self>) {
        self.save_settings();
        let open_repo = self.open_repo.clone();
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

    fn load_diff_for_selected(&mut self, cx: &mut Context<Self>) {
        let Some(idx) = self.graph_panel.selected_idx() else {
            return;
        };
        let Some(commit_id) = self.graph_panel.commit_id_at(idx).map(String::from) else {
            return;
        };

        let open_repo = self.open_repo.clone();
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

impl Render for GitForgeApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let bg = rgba_to_hsla(self.colors.background);
        let text = rgba_to_hsla(self.colors.text);
        let entity = cx.entity().downgrade();

        let sidebar = super::sidebar::render_sidebar(
            self.repo_state.as_ref(),
            &self.colors,
            self.loading,
            &self.sidebar_state,
            entity.clone(),
            window,
            &self.hosting_accounts,
        );

        let toolbar = super::toolbar::render_toolbar(
            self.repo_state.as_ref(),
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
            MainViewMode::CommitHistory => self.diff_panel.render(
                self.repo_state.as_ref(),
                self.graph_panel.selected_idx(),
                &self.colors,
                entity.clone(),
                self.loading,
            ),
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
            self.repo_state.as_ref(),
            &self.colors,
            window,
            entity.clone(),
            self.titlebar_menus_visible,
            self.active_titlebar_menu,
        );

        let mut inner = div()
            .id("app-content")
            .flex()
            .flex_col()
            .size_full()
            .overflow_hidden()
            .text_color(text)
            .child(titlebar);

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
            inner = inner.child(render_dialog_overlay(
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

        if let Some(palette) = self.command_palette.render(&self.colors, entity, window) {
            inner = inner.child(palette);
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
            .child(super::window_chrome::render_window_chrome(
                inner,
                &self.colors,
                window,
            ))
    }
}

fn render_dialog_overlay(
    dialog: &AppDialog,
    input_value: &str,
    input_value_2: &str,
    input_focus: &FocusHandle,
    colors: &AppColors,
    entity: WeakEntity<GitForgeApp>,
    window: &mut Window,
    hosting_repos: &[gitforge_hosting::RemoteRepo],
    hosting_repos_loading: bool,
    hosting_accounts_from_render: &[gitforge_hosting::HostingAccount],
) -> Stateful<Div> {
    let overlay_bg = rgba_to_hsla(colors.background).opacity(0.7);
    let surface = rgba_to_hsla(colors.surface);
    let border = rgba_to_hsla(colors.border);
    let text_color = rgba_to_hsla(colors.text);
    let accent = rgba_to_hsla(colors.accent);
    let muted = rgba_to_hsla(colors.text_muted);
    let warning = rgba_to_hsla(colors.warning);

    let title = match dialog {
        AppDialog::CreateBranch { .. } => "Create Branch",
        AppDialog::RenameBranch { .. } => "Rename Branch",
        AppDialog::CreateTag { .. } => "Create Tag",
        AppDialog::StashPush => "Stash Changes",
        AppDialog::Push { .. } => "Push",
        AppDialog::Pull { .. } => "Pull",
        AppDialog::CloneRepo => "Clone Repository",
        AppDialog::AddRemote => "Add Remote",
        AppDialog::SshGenerateKey => "Generate SSH Key",
        AppDialog::SshTestConnection => "Test SSH Connection",
        AppDialog::CredentialAdd => "Add Credential",
        AppDialog::CloneFromHosting { .. } => "Clone from Hosting",
        AppDialog::AddAccount { .. } => "Add Account",
        AppDialog::ManageAccounts => "Manage Accounts",
        AppDialog::SearchHosting { .. } => "Search Repositories",
        AppDialog::ForkRepo { .. } => "Fork Repository",
        AppDialog::AiSettings => "AI Settings",
        AppDialog::SetAiApiKey { .. } => "Set API Key",
        AppDialog::CreateWorktree => "Create Worktree",
        AppDialog::RemoveWorktree { .. } => "Remove Worktree",
        AppDialog::None => "",
    };

    let placeholder = match dialog {
        AppDialog::CreateBranch { .. } => "Branch name",
        AppDialog::RenameBranch { .. } => "New branch name",
        AppDialog::CreateTag { .. } => "Tag name",
        AppDialog::StashPush => "Stash message (optional)",
        AppDialog::Push { .. } => "Branch name (empty = current)",
        AppDialog::Pull { .. } => "Remote name (empty = origin)",
        AppDialog::CloneRepo => "URL destination-path",
        AppDialog::AddRemote => "name url",
        AppDialog::SshGenerateKey => "Email address",
        AppDialog::SshTestConnection => "Host (e.g. github.com)",
        AppDialog::CredentialAdd => "host username",
        AppDialog::CloneFromHosting { .. } => "Search repos...",
        AppDialog::AddAccount { .. } => "Personal Access Token",
        AppDialog::ManageAccounts => "",
        AppDialog::SearchHosting { .. } => "Search query...",
        AppDialog::ForkRepo { owner, repo, .. } => {
            return render_fork_confirm_overlay(owner, repo, colors, entity);
        }
        AppDialog::AiSettings => {
            return render_ai_settings_overlay(
                input_value,
                input_value_2,
                input_focus,
                colors,
                entity,
                window,
            );
        }
        AppDialog::CreateWorktree => {
            return render_create_worktree_overlay(
                input_value,
                input_value_2,
                input_focus,
                colors,
                entity,
                window,
            );
        }
        AppDialog::SetAiApiKey { .. } => "sk-... (API key)",
        AppDialog::RemoveWorktree { path } => {
            return render_remove_worktree_overlay(path, colors, entity);
        }
        AppDialog::None => "",
    };

    if matches!(dialog, AppDialog::CloneFromHosting { .. }) {
        return render_hosting_repos_overlay(
            dialog,
            input_value,
            input_focus,
            colors,
            entity,
            window,
            hosting_repos,
            hosting_repos_loading,
        );
    }

    if matches!(dialog, AppDialog::ManageAccounts) {
        return render_manage_accounts_overlay(colors, entity, &hosting_accounts_from_render);
    }

    if matches!(dialog, AppDialog::SearchHosting { .. }) {
        return render_hosting_repos_overlay(
            dialog,
            input_value,
            input_focus,
            colors,
            entity,
            window,
            hosting_repos,
            hosting_repos_loading,
        );
    }

    let is_focused = input_focus.is_focused(window);
    let display_text = if input_value.is_empty() && !is_focused {
        placeholder.to_string()
    } else {
        let mut t = input_value.to_string();
        if is_focused {
            t.push('\u{2502}');
        }
        t
    };
    let display_color = if input_value.is_empty() && !is_focused {
        muted
    } else {
        text_color
    };
    let border_focus_color = if is_focused { accent } else { border };

    let ent_cancel = entity.clone();
    let ent_cancel2 = entity.clone();
    let ent_confirm = entity.clone();
    let ent_confirm2 = entity.clone();
    let ent_input = entity.clone();
    let fh = input_focus.clone();

    let mut dialog_box = div()
        .id("dialog-box")
        .w(px(360.0))
        .bg(surface)
        .border_1()
        .border_color(border)
        .rounded(px(6.0))
        .p_4()
        .flex()
        .flex_col()
        .gap_3()
        .child(
            div()
                .text_sm()
                .font_weight(FontWeight::BOLD)
                .text_color(text_color)
                .child(title.to_string()),
        );

    dialog_box = dialog_box
        .child(
            div()
                .id(ElementId::Name("dialog-input".into()))
                .track_focus(&fh)
                .px_2()
                .py_1()
                .border_1()
                .border_color(border_focus_color)
                .rounded(px(3.0))
                .bg(rgba_to_hsla(colors.background))
                .cursor_pointer()
                .on_click(move |_ev, window, _cx| {
                    window.focus(&fh);
                })
                .on_key_down(move |ev: &KeyDownEvent, _window, cx| {
                    let key = &ev.keystroke.key;
                    match key.as_str() {
                        "enter" => {
                            if let Some(e) = ent_confirm.upgrade() {
                                e.update(cx, |this, cx| {
                                    this.confirm_dialog(cx);
                                });
                            }
                        }
                        "escape" => {
                            if let Some(e) = ent_cancel.upgrade() {
                                e.update(cx, |this, cx| {
                                    this.cancel_dialog(cx);
                                });
                            }
                        }
                        "backspace" => {
                            if let Some(e) = ent_input.upgrade() {
                                e.update(cx, |this, cx| {
                                    this.edit_dialog_input(None, cx);
                                });
                            }
                        }
                        _ => {
                            if let Some(ch) = ev.keystroke.key_char.clone() {
                                if !ev.keystroke.modifiers.platform {
                                    if let Some(e) = ent_input.upgrade() {
                                        let c = ch;
                                        e.update(cx, |this, cx| {
                                            this.edit_dialog_input(Some(&c), cx);
                                        });
                                    }
                                }
                            }
                        }
                    }
                })
                .child(
                    div()
                        .text_sm()
                        .text_color(display_color)
                        .child(display_text),
                ),
        )
        .child(
            div()
                .flex()
                .gap_2()
                .justify_end()
                .child(
                    div()
                        .id("dialog-cancel")
                        .px_3()
                        .py_1()
                        .border_1()
                        .border_color(border)
                        .rounded(px(3.0))
                        .cursor_pointer()
                        .text_xs()
                        .text_color(muted)
                        .child("Cancel")
                        .on_click(move |_ev, _window, cx| {
                            if let Some(e) = ent_cancel2.upgrade() {
                                e.update(cx, |this, cx| {
                                    this.cancel_dialog(cx);
                                });
                            }
                        }),
                )
                .child(
                    div()
                        .id("dialog-confirm")
                        .px_3()
                        .py_1()
                        .border_1()
                        .border_color(warning)
                        .rounded(px(3.0))
                        .cursor_pointer()
                        .text_xs()
                        .text_color(warning)
                        .child("Confirm")
                        .on_click(move |_ev, _window, cx| {
                            if let Some(e) = ent_confirm2.upgrade() {
                                e.update(cx, |this, cx| {
                                    this.confirm_dialog(cx);
                                });
                            }
                        }),
                ),
        );

    div()
        .id("dialog-overlay")
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .bg(overlay_bg)
        .flex()
        .items_center()
        .justify_center()
        .child(dialog_box)
}

fn render_hosting_repos_overlay(
    dialog: &AppDialog,
    _input_value: &str,
    _input_focus: &FocusHandle,
    colors: &AppColors,
    entity: WeakEntity<GitForgeApp>,
    _window: &mut Window,
    hosting_repos: &[gitforge_hosting::RemoteRepo],
    hosting_repos_loading: bool,
) -> Stateful<Div> {
    let overlay_bg = rgba_to_hsla(colors.background).opacity(0.7);
    let surface = rgba_to_hsla(colors.surface);
    let border = rgba_to_hsla(colors.border);
    let text_color = rgba_to_hsla(colors.text);
    let muted = rgba_to_hsla(colors.text_muted);
    let accent = rgba_to_hsla(colors.accent);

    let (_provider_name, dialog_title) = match dialog {
        AppDialog::CloneFromHosting { provider } => {
            (provider.clone(), format!("Clone from {}", provider))
        }
        AppDialog::SearchHosting { provider } => {
            (provider.clone(), format!("Search on {}", provider))
        }
        _ => (String::new(), "Browse Repositories".to_string()),
    };

    let ent_cancel = entity.clone();

    let mut content = div()
        .id("dialog-box")
        .w(px(500.0))
        .max_h(px(500.0))
        .bg(surface)
        .border_1()
        .border_color(border)
        .rounded(px(6.0))
        .p_4()
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::BOLD)
                        .text_color(text_color)
                        .child(dialog_title),
                )
                .child(
                    div()
                        .id("hosting-cancel")
                        .px_2()
                        .py_0()
                        .border_1()
                        .border_color(border)
                        .rounded(px(3.0))
                        .cursor_pointer()
                        .text_xs()
                        .text_color(muted)
                        .child("Close")
                        .on_click(move |_ev, _window, cx| {
                            if let Some(e) = ent_cancel.upgrade() {
                                e.update(cx, |this, cx| {
                                    this.cancel_dialog(cx);
                                });
                            }
                        }),
                ),
        );

    if hosting_repos_loading {
        content = content.child(
            div()
                .text_xs()
                .text_color(muted)
                .child("Loading repositories..."),
        );
    } else if hosting_repos.is_empty() {
        content = content.child(
            div()
                .text_xs()
                .text_color(muted)
                .child("No repositories found"),
        );
    } else {
        let mut list = div().flex().flex_col().gap_1();
        for (i, repo) in hosting_repos.iter().enumerate() {
            let ent_clone = entity.clone();
            let clone_url = repo.clone_url.clone();
            let repo_name = repo.name.clone();
            let vis = if repo.is_private { "private" } else { "public" };
            let stars = repo.stars;
            let desc = repo.description.as_deref().unwrap_or("");

            list = list.child(
                div()
                    .id(ElementId::NamedInteger("hosting-repo".into(), i as u64))
                    .px_2()
                    .py_1()
                    .border_1()
                    .border_color(border)
                    .rounded(px(3.0))
                    .cursor_pointer()
                    .hover(|s| s.bg(rgba_to_hsla(colors.surface_high)))
                    .on_click(move |_ev, _window, cx| {
                        if let Some(e) = ent_clone.upgrade() {
                            let url = clone_url.clone();
                            let name = repo_name.clone();
                            e.update(cx, |this, cx| {
                                this.clone_hosting_repo(url, name, cx);
                            });
                        }
                    })
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_0()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(text_color)
                                            .child(repo.name.clone()),
                                    )
                                    .child(div().text_xs().text_color(muted).child(vis))
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(accent)
                                            .child(format!("*{}", stars)),
                                    ),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(muted)
                                    .overflow_hidden()
                                    .child(desc.to_string()),
                            ),
                    ),
            );
        }
        content = content.child(list);
    }

    div()
        .id("dialog-overlay")
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .bg(overlay_bg)
        .flex()
        .items_center()
        .justify_center()
        .child(content)
}

fn render_manage_accounts_overlay(
    colors: &AppColors,
    entity: WeakEntity<GitForgeApp>,
    accounts: &[gitforge_hosting::HostingAccount],
) -> Stateful<Div> {
    let overlay_bg = rgba_to_hsla(colors.background).opacity(0.7);
    let surface = rgba_to_hsla(colors.surface);
    let border = rgba_to_hsla(colors.border);
    let text_color = rgba_to_hsla(colors.text);
    let muted = rgba_to_hsla(colors.text_muted);
    let accent = rgba_to_hsla(colors.accent);
    let warning = rgba_to_hsla(colors.warning);

    let ent_cancel = entity.clone();
    let ent_github = entity.clone();
    let ent_gitlab = entity.clone();
    let ent_codeberg = entity.clone();

    let mut content = div()
        .id("dialog-box")
        .w(px(420.0))
        .max_h(px(500.0))
        .bg(surface)
        .border_1()
        .border_color(border)
        .rounded(px(6.0))
        .p_4()
        .flex()
        .flex_col()
        .gap_3()
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::BOLD)
                        .text_color(text_color)
                        .child("Hosting Accounts"),
                )
                .child(
                    div()
                        .id("accounts-cancel")
                        .px_2()
                        .py_0()
                        .border_1()
                        .border_color(border)
                        .rounded(px(3.0))
                        .cursor_pointer()
                        .text_xs()
                        .text_color(muted)
                        .child("Close")
                        .on_click(move |_ev, _window, cx| {
                            if let Some(e) = ent_cancel.upgrade() {
                                e.update(cx, |this, cx| {
                                    this.cancel_dialog(cx);
                                });
                            }
                        }),
                ),
        )
        .child(
            div()
                .text_xs()
                .text_color(muted)
                .child("Add an account by entering a Personal Access Token (PAT)."),
        )
        .child(
            div()
                .flex()
                .gap_2()
                .child(render_add_provider_button(
                    "GitHub", accent, border, ent_github, "github",
                ))
                .child(render_add_provider_button(
                    "GitLab", accent, border, ent_gitlab, "gitlab",
                ))
                .child(render_add_provider_button(
                    "Codeberg",
                    accent,
                    border,
                    ent_codeberg,
                    "codeberg",
                )),
        );

    if accounts.is_empty() {
        content = content.child(
            div()
                .text_xs()
                .text_color(muted)
                .child("No accounts configured."),
        );
    } else {
        let mut list = div().flex().flex_col().gap_1();
        for (i, account) in accounts.iter().enumerate() {
            let ent_remove = entity.clone();
            let username = account.username.clone();
            let provider = account.provider.clone();
            let display = account.display_name.clone();
            let prov_lower = account.provider.clone();

            let provider_color = match account.provider.as_str() {
                "github" => accent,
                "gitlab" => rgba_to_hsla(colors.accent_secondary),
                "codeberg" => rgba_to_hsla(colors.success),
                _ => muted,
            };

            list = list.child(
                div()
                    .id(ElementId::NamedInteger("account-row".into(), i as u64))
                    .px_2()
                    .py_1()
                    .border_1()
                    .border_color(border)
                    .rounded(px(3.0))
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(provider_color)
                                    .child(prov_lower),
                            )
                            .child(div().text_sm().text_color(text_color).child(display))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(muted)
                                    .child(format!("@{}", username)),
                            ),
                    )
                    .child(
                        div()
                            .id(ElementId::NamedInteger("remove-account".into(), i as u64))
                            .px_2()
                            .py_0()
                            .border_1()
                            .border_color(warning)
                            .rounded(px(3.0))
                            .cursor_pointer()
                            .text_xs()
                            .text_color(warning)
                            .child("Remove")
                            .on_click(move |_ev, _window, cx| {
                                if let Some(e) = ent_remove.upgrade() {
                                    let u = username.clone();
                                    let p = provider.clone();
                                    e.update(cx, |this, cx| {
                                        this.remove_hosting_account(u, p, cx);
                                    });
                                }
                            }),
                    ),
            );
        }
        content = content.child(list);
    }

    div()
        .id("dialog-overlay")
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .bg(overlay_bg)
        .flex()
        .items_center()
        .justify_center()
        .child(content)
}

fn render_add_provider_button(
    label: &str,
    color: Hsla,
    border_color: Hsla,
    entity: WeakEntity<GitForgeApp>,
    provider: &str,
) -> Stateful<Div> {
    let provider = provider.to_string();
    div()
        .id(ElementId::Name(format!("add-{}", provider).into()))
        .px_2()
        .py_0()
        .border_1()
        .border_color(border_color)
        .rounded(px(3.0))
        .cursor_pointer()
        .text_xs()
        .text_color(color)
        .child(format!("+ {}", label))
        .on_click(move |_ev, _window, cx| {
            if let Some(e) = entity.upgrade() {
                let p = provider.clone();
                e.update(cx, |this, cx| {
                    this.open_add_account_dialog(p, cx);
                });
            }
        })
}

fn render_fork_confirm_overlay(
    owner: &str,
    repo: &str,
    colors: &AppColors,
    entity: WeakEntity<GitForgeApp>,
) -> Stateful<Div> {
    let overlay_bg = rgba_to_hsla(colors.background).opacity(0.7);
    let surface = rgba_to_hsla(colors.surface);
    let border = rgba_to_hsla(colors.border);
    let text_color = rgba_to_hsla(colors.text);
    let muted = rgba_to_hsla(colors.text_muted);
    let warning = rgba_to_hsla(colors.warning);

    let ent_cancel = entity.clone();
    let ent_confirm = entity.clone();
    let owner_owned = owner.to_string();
    let repo_owned = repo.to_string();

    div()
        .id("dialog-overlay")
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .bg(overlay_bg)
        .flex()
        .items_center()
        .justify_center()
        .child(
            div()
                .id("dialog-box")
                .w(px(360.0))
                .bg(surface)
                .border_1()
                .border_color(border)
                .rounded(px(6.0))
                .p_4()
                .flex()
                .flex_col()
                .gap_3()
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::BOLD)
                        .text_color(text_color)
                        .child("Fork Repository"),
                )
                .child(
                    div()
                        .text_sm()
                        .text_color(text_color)
                        .child(format!("Fork {}/{} to your account?", owner, repo)),
                )
                .child(
                    div()
                        .flex()
                        .gap_2()
                        .justify_end()
                        .child(
                            div()
                                .id("fork-cancel")
                                .px_3()
                                .py_1()
                                .border_1()
                                .border_color(border)
                                .rounded(px(3.0))
                                .cursor_pointer()
                                .text_xs()
                                .text_color(muted)
                                .child("Cancel")
                                .on_click(move |_ev, _window, cx| {
                                    if let Some(e) = ent_cancel.upgrade() {
                                        e.update(cx, |this, cx| {
                                            this.cancel_dialog(cx);
                                        });
                                    }
                                }),
                        )
                        .child(
                            div()
                                .id("fork-confirm")
                                .px_3()
                                .py_1()
                                .border_1()
                                .border_color(warning)
                                .rounded(px(3.0))
                                .cursor_pointer()
                                .text_xs()
                                .text_color(warning)
                                .child("Fork")
                                .on_click(move |_ev, _window, cx| {
                                    if let Some(e) = ent_confirm.upgrade() {
                                        let o = owner_owned.clone();
                                        let r = repo_owned.clone();
                                        e.update(cx, |this, cx| {
                                            let provider = this
                                                .hosting_accounts
                                                .first()
                                                .map(|a| a.provider.clone())
                                                .unwrap_or_default();
                                            this.fork_repo(o, r, provider, cx);
                                        });
                                    }
                                }),
                        ),
                ),
        )
}

fn render_ai_settings_overlay(
    provider_input: &str,
    model_input: &str,
    input_focus: &FocusHandle,
    colors: &AppColors,
    entity: WeakEntity<GitForgeApp>,
    window: &mut Window,
) -> Stateful<Div> {
    let overlay_bg = rgba_to_hsla(colors.background).opacity(0.7);
    let surface = rgba_to_hsla(colors.surface);
    let border = rgba_to_hsla(colors.border);
    let text_color = rgba_to_hsla(colors.text);
    let accent = rgba_to_hsla(colors.accent);
    let muted = rgba_to_hsla(colors.text_muted);

    let providers = ["disabled", "ollama", "openai", "anthropic"];
    let current_provider = if providers.contains(&provider_input) {
        provider_input
    } else {
        "disabled"
    };
    let show_model = current_provider != "disabled";
    let show_api_key = current_provider == "openai" || current_provider == "anthropic";

    let ent_cancel = entity.clone();
    let ent_confirm = entity.clone();

    let mut provider_buttons = Vec::new();
    for p in providers {
        let is_active = p == current_provider;
        let bg = if is_active { accent } else { surface };
        let tc = if is_active {
            rgba_to_hsla(colors.background)
        } else {
            text_color
        };
        let bc = if is_active { accent } else { border };
        let ent = entity.clone();
        let p_owned = p.to_string();
        provider_buttons.push(
            div()
                .id(ElementId::Name(format!("ai-provider-{}", p).into()))
                .px_3()
                .py_1()
                .border_1()
                .border_color(bc)
                .rounded(px(3.0))
                .bg(bg)
                .cursor_pointer()
                .text_xs()
                .text_color(tc)
                .font_weight(if is_active {
                    FontWeight::SEMIBOLD
                } else {
                    FontWeight::NORMAL
                })
                .child(p.to_string())
                .on_click(move |_ev, _window, cx| {
                    if let Some(e) = ent.upgrade() {
                        let val = p_owned.clone();
                        e.update(cx, |this, cx| {
                            this.dialog_input = val;
                            match this.dialog_input.as_str() {
                                "ollama" => {
                                    if this.dialog_input_2.is_empty() {
                                        this.dialog_input_2 = "codellama".to_string();
                                    }
                                }
                                "openai" => {
                                    if this.dialog_input_2.is_empty() {
                                        this.dialog_input_2 = "gpt-4o-mini".to_string();
                                    }
                                }
                                "anthropic" => {
                                    if this.dialog_input_2.is_empty() {
                                        this.dialog_input_2 =
                                            "claude-sonnet-4-20250514".to_string();
                                    }
                                }
                                _ => {}
                            }
                            cx.notify();
                        });
                    }
                }),
        );
    }

    let is_focused = input_focus.is_focused(window);
    let model_display = if model_input.is_empty() && !is_focused {
        "Model name".to_string()
    } else {
        let mut t = model_input.to_string();
        if is_focused {
            t.push('\u{2502}');
        }
        t
    };
    let model_color = if model_input.is_empty() && !is_focused {
        muted
    } else {
        text_color
    };

    let ent_model = entity.clone();
    let fh = input_focus.clone();

    let mut dialog_box = div()
        .id("dialog-box")
        .w(px(420.0))
        .bg(surface)
        .border_1()
        .border_color(border)
        .rounded(px(6.0))
        .p_4()
        .flex()
        .flex_col()
        .gap_3()
        .child(
            div()
                .text_sm()
                .font_weight(FontWeight::BOLD)
                .text_color(text_color)
                .child("AI Settings"),
        )
        .child(div().text_xs().text_color(muted).child("Provider"))
        .child(div().flex().gap_2().children(provider_buttons));

    if show_model {
        let ent_m = ent_model.clone();
        let fh2 = fh.clone();
        dialog_box = dialog_box
            .child(div().text_xs().text_color(muted).child("Model"))
            .child(
                div()
                    .id(ElementId::Name("ai-model-input".into()))
                    .track_focus(&fh2)
                    .px_2()
                    .py_1()
                    .border_1()
                    .border_color(border)
                    .rounded(px(3.0))
                    .bg(rgba_to_hsla(colors.background))
                    .cursor_pointer()
                    .on_click(move |_ev, window, _cx| {
                        window.focus(&fh2);
                    })
                    .on_key_down(move |ev: &KeyDownEvent, _window, cx| {
                        match ev.keystroke.key.as_str() {
                            "backspace" => {
                                if let Some(e) = ent_m.upgrade() {
                                    e.update(cx, |this, cx| {
                                        this.dialog_input_2.pop();
                                        cx.notify();
                                    });
                                }
                            }
                            "enter" => {
                                if let Some(e) = ent_m.upgrade() {
                                    e.update(cx, |this, cx| {
                                        this.confirm_dialog(cx);
                                    });
                                }
                            }
                            "escape" => {
                                if let Some(e) = ent_m.upgrade() {
                                    e.update(cx, |this, cx| {
                                        this.cancel_dialog(cx);
                                    });
                                }
                            }
                            _ => {
                                if let Some(ch) = ev.keystroke.key_char.clone() {
                                    if !ev.keystroke.modifiers.platform {
                                        if let Some(e) = ent_m.upgrade() {
                                            let c = ch;
                                            e.update(cx, |this, cx| {
                                                this.dialog_input_2.push_str(&c);
                                                cx.notify();
                                            });
                                        }
                                    }
                                }
                            }
                        }
                    })
                    .child(div().text_sm().text_color(model_color).child(model_display)),
            );
    }

    if show_api_key {
        let ent_key = entity.clone();
        let provider_name = current_provider.to_string();
        dialog_box = dialog_box.child(
            div()
                .id("ai-set-key-btn")
                .px_3()
                .py_1()
                .border_1()
                .border_color(accent)
                .rounded(px(3.0))
                .cursor_pointer()
                .text_xs()
                .text_color(accent)
                .child("Set API Key (stored in keyring)")
                .on_click(move |_ev, _window, cx| {
                    if let Some(e) = ent_key.upgrade() {
                        let p = provider_name.clone();
                        e.update(cx, |this, cx| {
                            this.open_ai_api_key_dialog(p, cx);
                        });
                    }
                }),
        );
    }

    dialog_box = dialog_box.child(
        div()
            .flex()
            .gap_2()
            .justify_end()
            .child(
                div()
                    .id("ai-cancel")
                    .px_3()
                    .py_1()
                    .border_1()
                    .border_color(border)
                    .rounded(px(3.0))
                    .cursor_pointer()
                    .text_xs()
                    .text_color(muted)
                    .child("Cancel")
                    .on_click(move |_ev, _window, cx| {
                        if let Some(e) = ent_cancel.upgrade() {
                            e.update(cx, |this, cx| {
                                this.cancel_dialog(cx);
                            });
                        }
                    }),
            )
            .child(
                div()
                    .id("ai-confirm")
                    .px_3()
                    .py_1()
                    .border_1()
                    .border_color(accent)
                    .rounded(px(3.0))
                    .cursor_pointer()
                    .text_xs()
                    .text_color(accent)
                    .child("Save")
                    .on_click(move |_ev, _window, cx| {
                        if let Some(e) = ent_confirm.upgrade() {
                            e.update(cx, |this, cx| {
                                this.confirm_dialog(cx);
                            });
                        }
                    }),
            ),
    );

    div()
        .id("dialog-overlay")
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .bg(overlay_bg)
        .flex()
        .items_center()
        .justify_center()
        .child(dialog_box)
}

fn render_create_worktree_overlay(
    input_value: &str,
    input_value_2: &str,
    input_focus: &FocusHandle,
    colors: &AppColors,
    entity: WeakEntity<GitForgeApp>,
    window: &mut Window,
) -> Stateful<Div> {
    let overlay_bg = rgba_to_hsla(colors.background).opacity(0.7);
    let surface = rgba_to_hsla(colors.surface);
    let border = rgba_to_hsla(colors.border);
    let text_color = rgba_to_hsla(colors.text);
    let accent = rgba_to_hsla(colors.accent);
    let muted = rgba_to_hsla(colors.text_muted);
    let warning = rgba_to_hsla(colors.warning);

    let fh = input_focus.clone();
    let is_focused = input_focus.is_focused(window);
    let display_text = if input_value.is_empty() && !is_focused {
        "Directory path (relative or absolute)".to_string()
    } else {
        let mut t = input_value.to_string();
        if is_focused {
            t.push('\u{2502}');
        }
        t
    };
    let display_color = if input_value.is_empty() && !is_focused {
        muted
    } else {
        text_color
    };
    let border_focus = if is_focused { accent } else { border };

    let display_text_2 = if input_value_2.is_empty() {
        "Branch/tag/commit (optional)".to_string()
    } else {
        input_value_2.to_string()
    };
    let display_color_2 = if input_value_2.is_empty() {
        muted
    } else {
        text_color
    };

    let ent_cancel = entity.clone();
    let ent_cancel2 = entity.clone();
    let ent_cancel3 = entity.clone();
    let ent_confirm = entity.clone();
    let ent_confirm2 = entity.clone();
    let ent_confirm3 = entity.clone();
    let ent_input = entity.clone();
    let ent_input2 = entity.clone();
    let fh1 = fh.clone();
    let fh2 = fh.clone();

    let dialog_box = div()
        .id("dialog-box")
        .w(px(420.0))
        .bg(surface)
        .border_1()
        .border_color(border)
        .rounded(px(6.0))
        .p_4()
        .flex()
        .flex_col()
        .gap_3()
        .child(
            div()
                .text_sm()
                .font_weight(FontWeight::BOLD)
                .text_color(text_color)
                .child("Create Worktree"),
        )
        .child(div().text_xs().text_color(muted).child("Target directory:"))
        .child(
            div()
                .id(ElementId::Name("dialog-input".into()))
                .track_focus(&fh1)
                .px_2()
                .py_1()
                .border_1()
                .border_color(border_focus)
                .rounded(px(3.0))
                .bg(rgba_to_hsla(colors.background))
                .cursor_pointer()
                .on_click(move |_ev, window, _cx| {
                    window.focus(&fh1);
                })
                .on_key_down(move |ev: &KeyDownEvent, _window, cx| {
                    match ev.keystroke.key.as_str() {
                        "enter" => {
                            if let Some(e) = ent_confirm.upgrade() {
                                e.update(cx, |this, cx| {
                                    this.confirm_dialog(cx);
                                });
                            }
                        }
                        "escape" => {
                            if let Some(e) = ent_cancel.upgrade() {
                                e.update(cx, |this, cx| {
                                    this.cancel_dialog(cx);
                                });
                            }
                        }
                        "backspace" => {
                            if let Some(e) = ent_input.upgrade() {
                                e.update(cx, |this, cx| {
                                    this.edit_dialog_input(None, cx);
                                });
                            }
                        }
                        _ => {
                            if let Some(ch) = ev.keystroke.key_char.clone() {
                                if !ev.keystroke.modifiers.platform {
                                    if let Some(e) = ent_input.upgrade() {
                                        let c = ch;
                                        e.update(cx, |this, cx| {
                                            this.edit_dialog_input(Some(&c), cx);
                                        });
                                    }
                                }
                            }
                        }
                    }
                })
                .child(
                    div()
                        .text_sm()
                        .text_color(display_color)
                        .child(display_text),
                ),
        )
        .child(
            div()
                .text_xs()
                .text_color(muted)
                .child("Checkout ref (branch, tag, or commit):"),
        )
        .child(
            div()
                .id(ElementId::Name("dialog-input-2".into()))
                .track_focus(&fh2)
                .px_2()
                .py_1()
                .border_1()
                .border_color(border)
                .rounded(px(3.0))
                .bg(rgba_to_hsla(colors.background))
                .cursor_pointer()
                .on_click(move |_ev, window, _cx| {
                    window.focus(&fh2);
                })
                .on_key_down(move |ev: &KeyDownEvent, _window, cx| {
                    match ev.keystroke.key.as_str() {
                        "enter" => {
                            if let Some(e) = ent_confirm2.upgrade() {
                                e.update(cx, |this, cx| {
                                    this.confirm_dialog(cx);
                                });
                            }
                        }
                        "escape" => {
                            if let Some(e) = ent_cancel2.upgrade() {
                                e.update(cx, |this, cx| {
                                    this.cancel_dialog(cx);
                                });
                            }
                        }
                        "backspace" => {
                            if let Some(e) = ent_input2.upgrade() {
                                e.update(cx, |this, cx| {
                                    this.dialog_input_2.pop();
                                    cx.notify();
                                });
                            }
                        }
                        _ => {
                            if let Some(ch) = ev.keystroke.key_char.clone() {
                                if !ev.keystroke.modifiers.platform {
                                    if let Some(e) = ent_input2.upgrade() {
                                        let c = ch;
                                        e.update(cx, |this, cx| {
                                            this.dialog_input_2.push_str(&c);
                                            cx.notify();
                                        });
                                    }
                                }
                            }
                        }
                    }
                })
                .child(
                    div()
                        .text_sm()
                        .text_color(display_color_2)
                        .child(display_text_2),
                ),
        )
        .child(
            div()
                .flex()
                .gap_2()
                .justify_end()
                .child(
                    div()
                        .id("dialog-cancel")
                        .px_3()
                        .py_1()
                        .border_1()
                        .border_color(border)
                        .rounded(px(3.0))
                        .cursor_pointer()
                        .text_xs()
                        .text_color(muted)
                        .child("Cancel")
                        .on_click(move |_ev, _window, cx| {
                            if let Some(e) = ent_cancel3.upgrade() {
                                e.update(cx, |this, cx| {
                                    this.cancel_dialog(cx);
                                });
                            }
                        }),
                )
                .child(
                    div()
                        .id("dialog-confirm")
                        .px_3()
                        .py_1()
                        .border_1()
                        .border_color(warning)
                        .rounded(px(3.0))
                        .cursor_pointer()
                        .text_xs()
                        .text_color(warning)
                        .child("Create")
                        .on_click(move |_ev, _window, cx| {
                            if let Some(e) = ent_confirm3.upgrade() {
                                e.update(cx, |this, cx| {
                                    this.confirm_dialog(cx);
                                });
                            }
                        }),
                ),
        );

    div()
        .id("dialog-overlay")
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .bg(overlay_bg)
        .flex()
        .items_center()
        .justify_center()
        .child(dialog_box)
}

fn render_remove_worktree_overlay(
    path: &str,
    colors: &AppColors,
    entity: WeakEntity<GitForgeApp>,
) -> Stateful<Div> {
    let overlay_bg = rgba_to_hsla(colors.background).opacity(0.7);
    let surface = rgba_to_hsla(colors.surface);
    let border = rgba_to_hsla(colors.border);
    let text_color = rgba_to_hsla(colors.text);
    let warning = rgba_to_hsla(colors.warning);
    let muted = rgba_to_hsla(colors.text_muted);

    let ent_cancel = entity.clone();
    let ent_confirm = entity.clone();

    let dialog_box = div()
        .id("dialog-box")
        .w(px(400.0))
        .bg(surface)
        .border_1()
        .border_color(border)
        .rounded(px(6.0))
        .p_4()
        .flex()
        .flex_col()
        .gap_3()
        .child(
            div()
                .text_sm()
                .font_weight(FontWeight::BOLD)
                .text_color(text_color)
                .child("Remove Worktree"),
        )
        .child(
            div()
                .text_sm()
                .text_color(text_color)
                .child(format!("Remove worktree at {}?", path)),
        )
        .child(
            div()
                .flex()
                .gap_2()
                .justify_end()
                .child(
                    div()
                        .id("dialog-cancel")
                        .px_3()
                        .py_1()
                        .border_1()
                        .border_color(border)
                        .rounded(px(3.0))
                        .cursor_pointer()
                        .text_xs()
                        .text_color(muted)
                        .child("Cancel")
                        .on_click(move |_ev, _window, cx| {
                            if let Some(e) = ent_cancel.upgrade() {
                                e.update(cx, |this, cx| {
                                    this.cancel_dialog(cx);
                                });
                            }
                        }),
                )
                .child(
                    div()
                        .id("dialog-confirm")
                        .px_3()
                        .py_1()
                        .border_1()
                        .border_color(warning)
                        .rounded(px(3.0))
                        .cursor_pointer()
                        .text_xs()
                        .text_color(warning)
                        .child("Remove")
                        .on_click(move |_ev, _window, cx| {
                            if let Some(e) = ent_confirm.upgrade() {
                                e.update(cx, |this, cx| {
                                    this.confirm_dialog(cx);
                                });
                            }
                        }),
                ),
        );

    div()
        .id("dialog-overlay")
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .bg(overlay_bg)
        .flex()
        .items_center()
        .justify_center()
        .child(dialog_box)
}
