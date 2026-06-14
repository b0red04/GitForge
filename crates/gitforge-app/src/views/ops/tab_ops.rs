use gitforge_git::RepoState;
use gpui::*;

use parking_lot::Mutex;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::views::app::GitForgeApp;
use crate::views::repo_session::{OpenRepoTab, RepoSession};
use crate::views::settings_window::SettingsRepoData;

impl GitForgeApp {
    pub fn restore_open_repo_tabs(&mut self, cx: &mut Context<Self>) {
        let paths = self.settings.open_repo_paths.clone();
        if paths.is_empty() {
            return;
        }

        let active_path = self.settings.active_repo_path.clone();
        let mut restore_ids = Vec::new();

        for path in paths {
            let path_buf = PathBuf::from(path);
            let id = self.repo_session.next_repo_tab_id;
            self.repo_session.next_repo_tab_id += 1;
            let repo = Arc::new(Mutex::new(None));
            self.repo_session.open_repo_tabs.push(OpenRepoTab {
                id,
                path: RepoSession::normalize_repo_path(&path_buf),
                repo,
                repo_state: None,
                loading: true,
                last_error: None,
                panel_snapshot: None,
                pull_requests: Vec::new(),
                pull_requests_loading: false,
            });
            restore_ids.push(id);
        }

        self.repo_session.active_repo_tab_id = active_path
            .as_deref()
            .and_then(|path| self.repo_session.find_tab_by_path(Path::new(path)))
            .or_else(|| restore_ids.first().copied());
        self.repo_session.apply_active_repo_tab_to_view();

        for tab_id in restore_ids {
            self.start_loading_repo_tab(tab_id, cx);
        }
    }

    pub(crate) fn open_or_activate_repo_tab(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        let normalized = RepoSession::normalize_repo_path(&path);
        if let Some(tab_id) = self.repo_session.find_tab_by_path(&normalized) {
            self.activate_repo_tab(tab_id, cx);
            let should_retry = self
                .repo_session
                .open_repo_tabs
                .iter()
                .find(|tab| tab.id == tab_id)
                .is_some_and(|tab| tab.last_error.is_some() && !tab.loading);
            if should_retry {
                self.start_loading_repo_tab(tab_id, cx);
            }
            return;
        }

        let id = self.repo_session.next_repo_tab_id;
        self.repo_session.next_repo_tab_id += 1;
        let repo = Arc::new(Mutex::new(None));
        self.repo_session.open_repo_tabs.push(OpenRepoTab {
            id,
            path: normalized,
            repo,
            repo_state: None,
            loading: true,
            last_error: None,
            panel_snapshot: None,
            pull_requests: Vec::new(),
            pull_requests_loading: false,
        });
        self.repo_session.active_repo_tab_id = Some(id);
        self.repo_session.apply_active_repo_tab_to_view();
        self.save_settings();
        cx.notify();
        self.start_loading_repo_tab(id, cx);
        self.restart_periodic_fetch(cx);
    }

    pub(crate) fn start_loading_repo_tab(&mut self, tab_id: u64, cx: &mut Context<Self>) {
        let Some(tab) = self
            .repo_session
            .open_repo_tabs
            .iter_mut()
            .find(|tab| tab.id == tab_id)
        else {
            return;
        };
        let path = tab.path.clone();
        let repo_handle = tab.repo.clone();
        tab.loading = true;
        tab.last_error = None;
        if self.repo_session.active_repo_tab_id == Some(tab_id) {
            self.repo_session.loading = true;
            self.repo_session.last_error = None;
        }
        cx.notify();

        let load_options = self.load_options();

        cx.spawn(async move |this, cx| {
            let result = tokio::task::spawn_blocking(move || {
                RepoState::discover_with_repo(&path, load_options)
            })
            .await;

            match result {
                Ok(Ok((repo, repo_state_data))) => {
                    *repo_handle.lock() = Some(repo);
                    this.update(cx, |this, cx| {
                        this.finish_repo_tab_load(tab_id, Ok(repo_state_data), cx);
                    })
                    .ok();
                }
                Ok(Err(e)) => {
                    this.update(cx, |this, cx| {
                        this.finish_repo_tab_load(tab_id, Err(format!("{}", e)), cx);
                    })
                    .ok();
                }
                Err(e) => {
                    this.update(cx, |this, cx| {
                        this.finish_repo_tab_load(tab_id, Err(format!("Task panicked: {}", e)), cx);
                    })
                    .ok();
                }
            }
        })
        .detach();
    }

    pub(crate) fn finish_repo_tab_load(
        &mut self,
        tab_id: u64,
        result: Result<RepoState, String>,
        cx: &mut Context<Self>,
    ) {
        let is_active = self.repo_session.active_repo_tab_id == Some(tab_id);
        match result {
            Ok(repo_state) => {
                if let Some(tab) = self
                    .repo_session
                    .open_repo_tabs
                    .iter_mut()
                    .find(|tab| tab.id == tab_id)
                {
                    tab.path = repo_state.path.clone();
                    tab.repo_state = Some(repo_state.clone());
                    tab.loading = false;
                    tab.last_error = None;
                } else {
                    return;
                }

                if is_active {
                    self.repo_session
                        .sidebar_state
                        .seed_expanded_remotes(&repo_state);
                    self.repo_session.apply_active_repo_tab_to_view();
                    self.refresh_pull_requests(cx);
                }
                self.record_recent_repo(&repo_state.path);
                self.save_settings();
            }
            Err(error) => {
                if let Some(tab) = self
                    .repo_session
                    .open_repo_tabs
                    .iter_mut()
                    .find(|tab| tab.id == tab_id)
                {
                    tab.loading = false;
                    tab.last_error = Some(format!("Failed to load repository: {}", error));
                } else {
                    return;
                }

                if is_active {
                    self.repo_session.apply_active_repo_tab_to_view();
                }
            }
        }
        self.notify_settings_window(cx);
        self.restart_periodic_fetch(cx);
        cx.notify();
    }

    pub fn activate_repo_tab(&mut self, tab_id: u64, cx: &mut Context<Self>) {
        if self.repo_session.active_repo_tab_id == Some(tab_id) {
            return;
        }
        self.repo_session.save_snapshot_to_active_tab();
        self.repo_session.active_repo_tab_id = Some(tab_id);
        self.repo_session.apply_active_repo_tab_to_view();
        self.repo_session.restore_snapshot_from_tab();
        {
            let repo_state = self
                .repo_session
                .active_tab()
                .and_then(|tab| tab.repo_state.clone());
            if let Some(ref repo_state) = repo_state {
                self.repo_session
                    .sidebar_state
                    .seed_expanded_remotes(repo_state);
            }
        }

        self.save_settings();
        cx.notify();
        self.restart_periodic_fetch(cx);
        self.refresh_pull_requests(cx);
    }

    pub fn close_repo_tab(&mut self, tab_id: u64, cx: &mut Context<Self>) {
        let Some(index) = self
            .repo_session
            .open_repo_tabs
            .iter()
            .position(|tab| tab.id == tab_id)
        else {
            return;
        };
        let closed_path = self.repo_session.open_repo_tabs[index].path.clone();
        let was_active = self.repo_session.active_repo_tab_id == Some(tab_id);

        if was_active {
            self.repo_session.save_snapshot_to_active_tab();
        }

        self.repo_session.open_repo_tabs.remove(index);
        self.repo_session.push_closed_tab(closed_path);

        if was_active {
            self.repo_session.active_repo_tab_id = if self.repo_session.open_repo_tabs.is_empty() {
                None
            } else if index > 0 {
                Some(self.repo_session.open_repo_tabs[index - 1].id)
            } else {
                Some(self.repo_session.open_repo_tabs[0].id)
            };
            self.repo_session.apply_active_repo_tab_to_view();
            self.repo_session.restore_snapshot_from_tab();
        }

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
        let Some(path) = self.repo_session.closed_repo_tabs.pop() else {
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
        self.repo_session.closed_repo_tabs.retain(|p| p != &path);
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
                .open_repo_tabs
                .iter()
                .map(|tab| (tab.id, tab.path.clone()))
                .collect(),
            active_path,
            active_settings,
            recent_paths: self.settings.recent_repo_paths.clone(),
            closed_paths: self.repo_session.closed_repo_tabs.clone(),
        }
    }
}
