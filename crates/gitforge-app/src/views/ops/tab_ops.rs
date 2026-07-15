use gitforge_git::RepoState;
use gpui::*;

use std::path::{Path, PathBuf};

use crate::views::app::GitForgeApp;
use crate::views::ops::pr_ops::{PullRequestRefreshMode, pull_request_refresh_mode_for_tab};
use crate::views::repo_session::{RefreshReselectPolicy, RepoSession};
use crate::views::settings_window::SettingsRepoData;

/// Panel handoff when the active repository tab changes.
enum RepoTabTransition {
    /// Switch to an existing tab: defer reselect and restore snapshot (ADR-0006).
    ActivateExisting(u64),
    /// New empty loading tab: reselect immediately; no snapshot to restore.
    OpenNewLoading(u64),
}

/// Side effects to run after the active tab id changes.
enum ActiveTabChangeKind {
    SwitchedExisting,
    OpenedNew { tab_id: u64 },
}

impl GitForgeApp {
    pub fn restore_open_repo_tabs(&mut self, cx: &mut Context<Self>) {
        let paths = self.settings.open_repo_paths.clone();
        if paths.is_empty() {
            return;
        }

        let active_path = self.settings.active_repo_path.clone();
        let mut restore_ids = Vec::new();

        for path in paths {
            let id = self
                .repo_session
                .tabs
                .push_loading_tab(PathBuf::from(path));
            restore_ids.push(id);
        }

        self.repo_session.tabs.active_repo_tab_id = active_path
            .as_deref()
            .and_then(|path| self.repo_session.tabs.find_tab_by_path(Path::new(path)))
            .or_else(|| restore_ids.first().copied());
        self.repo_session
            .apply_active_repo_tab_to_view(RefreshReselectPolicy::Reselect);

        for tab_id in restore_ids {
            self.start_loading_repo_tab(tab_id, cx);
        }
    }

    pub(crate) fn open_or_activate_repo_tab(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        self.repo_session.pending_file_dialog = false;
        let normalized = RepoSession::normalize_repo_path(&path);
        if let Some(tab_id) = self.repo_session.tabs.find_tab_by_path(&normalized) {
            self.activate_repo_tab(tab_id, cx);
            let should_retry = self
                .repo_session
                .tabs
                .open_repo_tabs
                .iter()
                .find(|tab| tab.id == tab_id)
                .is_some_and(|tab| tab.last_error.is_some() && !tab.loading);
            if should_retry {
                self.start_loading_repo_tab(tab_id, cx);
            }
            return;
        }

        let id = self.repo_session.tabs.push_loading_tab(path);
        self.perform_repo_tab_transition(RepoTabTransition::OpenNewLoading(id), cx);
        self.on_active_repo_tab_changed(ActiveTabChangeKind::OpenedNew { tab_id: id }, cx);
    }

    pub(crate) fn start_loading_repo_tab(&mut self, tab_id: u64, cx: &mut Context<Self>) {
        let Some(tab) = self.repo_session.tabs.tab_mut(tab_id) else {
            return;
        };
        let path = tab.path.clone();
        let repo_handle = tab.repo.clone();
        tab.loading = true;
        tab.last_error = None;
        cx.notify();

        let load_options = self.load_options();

        self.run_blocking(
            "Load repository",
            cx,
            super::dispatch::OpEffects::SILENT,
            move || RepoState::discover_with_repo(&path, load_options),
            move |this, (repo, repo_state_data), cx| {
                *repo_handle.lock() = Some(repo);
                this.finish_repo_tab_load(tab_id, Ok(repo_state_data), cx);
            },
            move |this, err_msg, cx| {
                this.finish_repo_tab_load(tab_id, Err(err_msg), cx);
            },
        );
    }

    pub(crate) fn finish_repo_tab_load(
        &mut self,
        tab_id: u64,
        result: Result<RepoState, String>,
        cx: &mut Context<Self>,
    ) {
        let is_active = self.repo_session.tabs.is_active(tab_id);
        match result {
            Ok(repo_state) => {
                let Some(tab) = self.repo_session.tabs.tab_mut(tab_id) else {
                    return;
                };
                tab.path = repo_state.path.clone();
                tab.repo_state = Some(repo_state.clone());
                tab.loading = false;
                tab.last_error = None;

                if is_active {
                    self.repo_session
                        .sidebar_state
                        .seed_expanded_remotes(&repo_state);
                    self.repo_session
                        .apply_active_repo_tab_to_view(RefreshReselectPolicy::Reselect);
                    self.refresh_pull_requests(cx, PullRequestRefreshMode::Initial);
                    self.fetch_on_activate(cx);
                }
                self.record_recent_repo(&repo_state.path);
                self.save_settings();
            }
            Err(error) => {
                let Some(tab) = self.repo_session.tabs.tab_mut(tab_id) else {
                    return;
                };
                tab.loading = false;
                tab.last_error = Some(format!("Failed to load repository: {}", error));

                if is_active {
                    self.repo_session
                        .apply_active_repo_tab_to_view(RefreshReselectPolicy::Reselect);
                }
            }
        }
        self.notify_settings_window(cx);
        self.restart_periodic_fetch(cx);
        cx.notify();
    }

    pub fn activate_repo_tab(&mut self, tab_id: u64, cx: &mut Context<Self>) {
        if self.repo_session.tabs.is_active(tab_id) {
            return;
        }
        self.perform_repo_tab_transition(RepoTabTransition::ActivateExisting(tab_id), cx);
        self.on_active_repo_tab_changed(ActiveTabChangeKind::SwitchedExisting, cx);
    }

    pub fn close_repo_tab(&mut self, tab_id: u64, cx: &mut Context<Self>) {
        let Some(index) = self
            .repo_session
            .tabs
            .open_repo_tabs
            .iter()
            .position(|tab| tab.id == tab_id)
        else {
            return;
        };
        let closed_path = self.repo_session.tabs.open_repo_tabs[index].path.clone();
        let was_active = self.repo_session.tabs.is_active(tab_id);

        if was_active {
            self.repo_session.save_snapshot_to_active_tab();
        }

        self.repo_session.tabs.open_repo_tabs.remove(index);
        self.repo_session.push_closed_tab(closed_path);

        if was_active {
            self.restore_neighbor_after_close_active_tab(index, cx);
        }

        self.persist_active_tab_ui(cx);
    }

    fn perform_repo_tab_transition(
        &mut self,
        transition: RepoTabTransition,
        cx: &mut Context<Self>,
    ) {
        let tab_id = match transition {
            RepoTabTransition::ActivateExisting(id) | RepoTabTransition::OpenNewLoading(id) => id,
        };
        self.repo_session.handoff_to_tab(tab_id);
        match transition {
            RepoTabTransition::ActivateExisting(_) => {
                if let Some(effect) = self.repo_session.apply_incoming_tab_after_switch() {
                    self.apply_selection_effect(effect, cx);
                }
            }
            RepoTabTransition::OpenNewLoading(_) => {
                self.repo_session
                    .apply_active_repo_tab_to_view(RefreshReselectPolicy::Reselect);
            }
        }
    }

    /// After removing the active tab at `removed_index`, select a neighbor and
    /// restore its panel snapshot.
    fn restore_neighbor_after_close_active_tab(
        &mut self,
        removed_index: usize,
        cx: &mut Context<Self>,
    ) {
        self.repo_session.tabs.select_neighbor_after_close(removed_index);
        if let Some(effect) = self.repo_session.apply_incoming_tab_after_switch() {
            self.apply_selection_effect(effect, cx);
        }
    }

    fn on_active_repo_tab_changed(&mut self, kind: ActiveTabChangeKind, cx: &mut Context<Self>) {
        match kind {
            ActiveTabChangeKind::SwitchedExisting => {
                if let Some(repo_state) = self
                    .repo_session
                    .active_tab()
                    .and_then(|tab| tab.repo_state.clone())
                {
                    self.repo_session
                        .sidebar_state
                        .seed_expanded_remotes(&repo_state);
                }
                self.fetch_on_activate(cx);
                let pr_mode = self
                    .repo_session
                    .active_tab()
                    .map(pull_request_refresh_mode_for_tab)
                    .unwrap_or(PullRequestRefreshMode::Initial);
                self.refresh_pull_requests(cx, pr_mode);
            }
            ActiveTabChangeKind::OpenedNew { tab_id } => {
                self.start_loading_repo_tab(tab_id, cx);
            }
        }
        self.persist_active_tab_ui(cx);
    }

    fn persist_active_tab_ui(&mut self, cx: &mut Context<Self>) {
        self.save_settings();
        cx.notify();
        self.restart_periodic_fetch(cx);
    }

    pub(crate) fn record_recent_repo(&mut self, path: &Path) {
        let path_str = path.to_string_lossy().to_string();
        self.settings.recent_repo_paths.retain(|p| p != &path_str);
        self.settings.recent_repo_paths.insert(0, path_str);
        while self.settings.recent_repo_paths.len() > 20 {
            self.settings.recent_repo_paths.pop();
        }
    }
    pub(crate) fn reopen_closed_tab(&mut self, cx: &mut Context<Self>) {
        let Some(path) = self.repo_session.tabs.closed_repo_tabs.pop() else {
            return;
        };
        self.open_or_activate_repo_tab(path, cx);
    }

    pub fn open_repo_from_settings(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        self.open_or_activate_repo_tab(path, cx);
        self.notify_settings_window(cx);
    }

    pub fn remove_recent_repo_path(&mut self, path: String, cx: &mut Context<Self>) {
        self.settings.recent_repo_paths.retain(|p| p != &path);
        self.settings.save();
        self.notify_settings_window(cx);
        cx.notify();
    }

    pub fn reopen_closed_repo_from_settings(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        self.repo_session.tabs.closed_repo_tabs.retain(|p| p != &path);
        self.open_or_activate_repo_tab(path, cx);
        self.notify_settings_window(cx);
    }

    pub fn activate_repo_tab_from_settings(&mut self, tab_id: u64, cx: &mut Context<Self>) {
        self.activate_repo_tab(tab_id, cx);
        self.notify_settings_window(cx);
    }

    pub fn settings_repo_data(&self) -> SettingsRepoData {
        let active_path = self.repo_session.active_tab().map(|tab| tab.path.clone());
        let active_settings = active_path
            .as_ref()
            .map(|path| self.settings.repo_settings_for_path(path))
            .unwrap_or_default();
        SettingsRepoData {
            open_tabs: self
                .repo_session
                .tabs
                .open_repo_tabs
                .iter()
                .map(|tab| (tab.id, tab.path.clone()))
                .collect(),
            active_path,
            active_settings,
            recent_paths: self.settings.recent_repo_paths.clone(),
            closed_paths: self.repo_session.tabs.closed_repo_tabs.clone(),
        }
    }
}
