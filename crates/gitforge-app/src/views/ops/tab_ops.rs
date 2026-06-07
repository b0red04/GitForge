use gitforge_git::{RepoState, Repository};use gitforge_graph::Graph;
use gpui::*;

use parking_lot::Mutex;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::views::app::{GitForgeApp, OpenRepoTab, MAX_CLOSED_TABS};
use crate::views::repo_tabs::RepoTabView;
use crate::views::settings_window::SettingsRepoData;

#[allow(dead_code)]
impl GitForgeApp {

    pub(crate) fn active_tab(&self) -> Option<&OpenRepoTab> {
        let active_id = self.active_repo_tab_id?;
        self.open_repo_tabs.iter().find(|tab| tab.id == active_id)
    }

    pub(crate) fn active_tab_mut(&mut self) -> Option<&mut OpenRepoTab> {
        let active_id = self.active_repo_tab_id?;
        self.open_repo_tabs
            .iter_mut()
            .find(|tab| tab.id == active_id)
    }

    pub(crate) fn active_repo_state(&self) -> Option<&RepoState> {
        self.active_tab().and_then(|tab| tab.repo_state.as_ref())
    }

    pub(crate) fn active_repo_handle(&self) -> Option<Arc<Mutex<Option<Repository>>>> {
        self.active_tab().map(|tab| tab.repo.clone())
    }

    pub(crate) fn require_active_repo_handle(&mut self) -> Option<Arc<Mutex<Option<Repository>>>> {
        let handle = self.active_repo_handle();
        if handle.is_none() {
            self.last_error = Some("No repository open".into());
        }
        handle
    }

    pub(crate) fn repo_tab_views(&self) -> Vec<RepoTabView> {
        self.open_repo_tabs
            .iter()
            .map(|tab| RepoTabView {
                id: tab.id,
                name: tab
                    .path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("repository")
                    .to_string(),
                loading: tab.loading,
                has_error: tab.last_error.is_some(),
            })
            .collect()
    }

    pub(crate) fn normalize_repo_path(path: &Path) -> PathBuf {
        std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
    }

    pub(crate) fn find_tab_by_path(&self, path: &Path) -> Option<u64> {
        let normalized = Self::normalize_repo_path(path);
        self.open_repo_tabs
            .iter()
            .find(|tab| Self::normalize_repo_path(&tab.path) == normalized)
            .map(|tab| tab.id)
    }

    pub(crate) fn clear_repo_panels(&mut self) {
        self.graph_panel
            .set_data(Vec::new(), Vec::new(), Graph::new(), false);
        self.diff_panel.clear();
        self.status_panel.clear();
    }

    pub(crate) fn clear_active_repo_view(&mut self) {
        self.clear_repo_panels();
        self.last_error = None;
        self.loading = false;
    }

    pub(crate) fn apply_active_repo_tab_to_view(&mut self) {
        let Some((repo_state, loading, last_error)) = self
            .active_tab()
            .map(|tab| (tab.repo_state.clone(), tab.loading, tab.last_error.clone()))
        else {
            self.clear_active_repo_view();
            return;
        };

        if let Some(repo_state) = repo_state {
            self.loading = false;
            self.last_error = None;
            self.apply_repo_state_to_panels(&repo_state);
        } else {
            self.clear_repo_panels();
            self.loading = loading;
            self.last_error = last_error;
        }
    }

    pub fn restore_open_repo_tabs(&mut self, cx: &mut Context<Self>) {
        let paths = self.settings.open_repo_paths.clone();
        if paths.is_empty() {
            return;
        }

        let active_path = self.settings.active_repo_path.clone();
        let mut restore_ids = Vec::new();

        for path in paths {
            let path_buf = PathBuf::from(path);
            let id = self.next_repo_tab_id;
            self.next_repo_tab_id += 1;
            let repo = Arc::new(Mutex::new(None));
            self.open_repo_tabs.push(OpenRepoTab {
                id,
                path: Self::normalize_repo_path(&path_buf),
                repo,
                repo_state: None,
                loading: true,
                last_error: None,
            });
            restore_ids.push(id);
        }

        self.active_repo_tab_id = active_path
            .as_deref()
            .and_then(|path| self.find_tab_by_path(Path::new(path)))
            .or_else(|| restore_ids.first().copied());
        self.apply_active_repo_tab_to_view();

        for tab_id in restore_ids {
            self.start_loading_repo_tab(tab_id, cx);
        }
    }

    pub(crate) fn open_or_activate_repo_tab(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        let normalized = Self::normalize_repo_path(&path);
        if let Some(tab_id) = self.find_tab_by_path(&normalized) {
            self.activate_repo_tab(tab_id, cx);
            let should_retry = self
                .open_repo_tabs
                .iter()
                .find(|tab| tab.id == tab_id)
                .is_some_and(|tab| tab.last_error.is_some() && !tab.loading);
            if should_retry {
                self.start_loading_repo_tab(tab_id, cx);
            }
            return;
        }

        let id = self.next_repo_tab_id;
        self.next_repo_tab_id += 1;
        let repo = Arc::new(Mutex::new(None));
        self.open_repo_tabs.push(OpenRepoTab {
            id,
            path: normalized,
            repo,
            repo_state: None,
            loading: true,
            last_error: None,
        });
        self.active_repo_tab_id = Some(id);
        self.apply_active_repo_tab_to_view();
        self.save_settings();
        cx.notify();
        self.start_loading_repo_tab(id, cx);
    }

    pub(crate) fn start_loading_repo_tab(&mut self, tab_id: u64, cx: &mut Context<Self>) {
        let Some(tab) = self.open_repo_tabs.iter_mut().find(|tab| tab.id == tab_id) else {
            return;
        };
        let path = tab.path.clone();
        let repo_handle = tab.repo.clone();
        tab.loading = true;
        tab.last_error = None;
        if self.active_repo_tab_id == Some(tab_id) {
            self.loading = true;
            self.last_error = None;
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
        let is_active = self.active_repo_tab_id == Some(tab_id);
        match result {
            Ok(repo_state) => {
                if let Some(tab) = self.open_repo_tabs.iter_mut().find(|tab| tab.id == tab_id) {
                    tab.path = repo_state.path.clone();
                    tab.repo_state = Some(repo_state.clone());
                    tab.loading = false;
                    tab.last_error = None;
                } else {
                    return;
                }

                if is_active {
                    self.apply_active_repo_tab_to_view();
                }
                self.record_recent_repo(&repo_state.path);
                self.save_settings();
            }
            Err(error) => {
                if let Some(tab) = self.open_repo_tabs.iter_mut().find(|tab| tab.id == tab_id) {
                    tab.loading = false;
                    tab.last_error = Some(format!("Failed to load repository: {}", error));
                } else {
                    return;
                }

                if is_active {
                    self.apply_active_repo_tab_to_view();
                }
            }
        }
        cx.notify();
    }

    pub fn activate_repo_tab(&mut self, tab_id: u64, cx: &mut Context<Self>) {
        if self.active_repo_tab_id == Some(tab_id) {
            return;
        }
        self.active_repo_tab_id = Some(tab_id);
        self.apply_active_repo_tab_to_view();

        self.save_settings();
        cx.notify();
    }

    pub fn close_repo_tab(&mut self, tab_id: u64, cx: &mut Context<Self>) {
        let Some(index) = self.open_repo_tabs.iter().position(|tab| tab.id == tab_id) else {
            return;
        };
        let closed_path = self.open_repo_tabs[index].path.clone();
        let was_active = self.active_repo_tab_id == Some(tab_id);
        self.open_repo_tabs.remove(index);
        self.push_closed_tab(closed_path);

        if was_active {
            self.active_repo_tab_id = if self.open_repo_tabs.is_empty() {
                None
            } else if index > 0 {
                Some(self.open_repo_tabs[index - 1].id)
            } else {
                Some(self.open_repo_tabs[0].id)
            };
            self.apply_active_repo_tab_to_view();
        }

        self.save_settings();
        cx.notify();
    }

    pub(crate) fn push_closed_tab(&mut self, path: PathBuf) {
        let normalized = Self::normalize_repo_path(&path);
        self.closed_repo_tabs.retain(|p| p != &normalized);
        self.closed_repo_tabs.push(normalized);
        while self.closed_repo_tabs.len() > MAX_CLOSED_TABS {
            self.closed_repo_tabs.remove(0);
        }
    }

    pub(crate) fn record_recent_repo(&mut self, path: &Path) {
        let path_str = path.to_string_lossy().to_string();
        self.settings
            .recent_repo_paths
            .retain(|p| p != &path_str);
        self.settings.recent_repo_paths.insert(0, path_str);
        while self.settings.recent_repo_paths.len() > 20 {
            self.settings.recent_repo_paths.pop();
        }
    }
    pub(crate) fn reopen_closed_tab(&mut self, cx: &mut Context<Self>) {
        let Some(path) = self.closed_repo_tabs.pop() else {
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
        self.closed_repo_tabs.retain(|p| p != &path);
        self.open_or_activate_repo_tab(path, cx);
        self.notify_settings_window(cx);
    }

    pub fn activate_repo_tab_from_settings(&mut self, tab_id: u64, cx: &mut Context<Self>) {
        self.activate_repo_tab(tab_id, cx);
        self.notify_settings_window(cx);
    }

    pub fn settings_repo_data(&self) -> SettingsRepoData {
        SettingsRepoData {
            open_tabs: self
                .open_repo_tabs
                .iter()
                .map(|tab| (tab.id, tab.path.clone()))
                .collect(),
            recent_paths: self.settings.recent_repo_paths.clone(),
            closed_paths: self.closed_repo_tabs.clone(),
        }
    }
}
