use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use gitforge_git::{RepoState, Repository};
use gitforge_graph::Graph;
use gpui::AppContext;
use parking_lot::Mutex;

use super::app::MainViewMode;
use super::commit_editor::CommitEditor;
use super::diff_panel::{CommitDiffState, DiffPanel, DiffViewMode};
use super::graph_panel::{GraphPanel, GraphSelection};
use super::repo_tabs::RepoTabView;
use super::sidebar::SidebarState;
use super::status_panel::{StatusPanel, StatusSelection, StatusViewMode};

pub(crate) const MAX_CLOSED_TABS: usize = 20;

pub(crate) struct OpenRepoTab {
    pub(crate) id: u64,
    pub(crate) path: PathBuf,
    pub(crate) repo: Arc<Mutex<Option<Repository>>>,
    pub(crate) repo_state: Option<RepoState>,
    pub(crate) loading: bool,
    pub(crate) last_error: Option<String>,
    pub(crate) panel_snapshot: Option<TabSnapshot>,
    pub(crate) pull_requests: Vec<gitforge_hosting::PullRequest>,
    pub(crate) pull_requests_loading: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct TabSnapshot {
    pub selected_commit_id: Option<String>,
    pub graph_was_uncommitted: bool,
    pub diff_state: Option<CommitDiffState>,
    pub diff_view_mode: DiffViewMode,
    pub diff_code_file: Option<String>,
    pub diff_code_content: Option<String>,
    pub status_selection: Option<StatusSelection>,
    pub status_view_mode: StatusViewMode,
    pub commit_message: String,
    pub ai_alternatives: Vec<String>,
    pub view_mode: MainViewMode,
    pub sidebar_branches_expanded: bool,
    pub sidebar_remotes_expanded: bool,
    pub sidebar_tags_expanded: bool,
    pub sidebar_worktrees_expanded: bool,
    pub sidebar_expanded_remotes: HashSet<String>,
}

pub(crate) struct RepoSession {
    pub(crate) open_repo_tabs: Vec<OpenRepoTab>,
    pub(crate) active_repo_tab_id: Option<u64>,
    pub(crate) next_repo_tab_id: u64,
    pub(crate) graph_panel: GraphPanel,
    pub(crate) diff_panel: DiffPanel,
    /// Cached, render-only mirror of `diff_panel`. It is embedded with
    /// `.cached(...)` so that scrolling the commit history (which does not
    /// change the diff) recycles its paint instead of re-rendering it.
    pub(crate) diff_view: gpui::Entity<super::diff_panel::DiffViewMirror>,
    pub status_panel: StatusPanel,
    pub commit_editor: CommitEditor,
    pub sidebar_state: SidebarState,
    pub view_mode: MainViewMode,
    pub remote_status: String,
    pub(crate) loading: bool,
    pub(crate) last_error: Option<String>,
    pub(crate) closed_repo_tabs: Vec<PathBuf>,
}

impl RepoSession {
    pub fn new(cx: &mut gpui::App) -> Self {
        Self {
            open_repo_tabs: Vec::new(),
            active_repo_tab_id: None,
            next_repo_tab_id: 1,
            graph_panel: GraphPanel::new(),
            diff_panel: DiffPanel::new(),
            diff_view: cx.new(|_| super::diff_panel::DiffViewMirror::new()),
            status_panel: StatusPanel::new(),
            commit_editor: CommitEditor::new(cx),
            sidebar_state: SidebarState::new(cx),
            view_mode: MainViewMode::CommitHistory,
            remote_status: String::new(),
            loading: false,
            last_error: None,
            closed_repo_tabs: Vec::new(),
        }
    }

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

    pub(crate) fn apply_repo_state_to_panels(&mut self, repo_state_data: &RepoState) {
        use gitforge_graph::CommitEntry;

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
        let in_history = self.view_mode == MainViewMode::CommitHistory;
        let preserve_staging = in_history && self.status_panel.is_graph_staging();
        self.status_panel
            .set_status(repo_state_data.status.clone(), preserve_staging);
        self.diff_panel.clear();

        if has_uncommitted {
            self.graph_panel.select_uncommitted();
            if in_history {
                self.status_panel.enter_graph_staging();
            }
        } else {
            self.graph_panel.clear_selection();
        }
    }

    pub(crate) fn apply_repo_state(&mut self, repo_state_data: RepoState) {
        self.apply_repo_state_to_panels(&repo_state_data);
        if let Some(tab) = self.active_tab_mut() {
            tab.path = repo_state_data.path.clone();
            tab.repo_state = Some(repo_state_data.clone());
            tab.loading = false;
            tab.last_error = None;
        }
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

    pub fn take_commit_message(&mut self) -> String {
        let msg = self.commit_editor.take_message();
        self.status_panel.reset_after_commit();
        msg
    }

    pub(crate) fn push_closed_tab(&mut self, path: PathBuf) {
        let normalized = Self::normalize_repo_path(&path);
        self.closed_repo_tabs.retain(|p| p != &normalized);
        self.closed_repo_tabs.push(normalized);
        while self.closed_repo_tabs.len() > MAX_CLOSED_TABS {
            self.closed_repo_tabs.remove(0);
        }
    }

    pub(crate) fn save_snapshot_to_active_tab(&mut self) {
        let selected_commit_id = match self.graph_panel.selection() {
            GraphSelection::Commit(idx) => self.graph_panel.commit_id_at(idx).map(String::from),
            _ => None,
        };
        let graph_was_uncommitted = self.graph_panel.is_uncommitted_selected();
        let diff_state = self.diff_panel.diff_state().cloned();
        let diff_view_mode = self.diff_panel.view_mode();
        let diff_code_file = self.diff_panel.code_view_file().map(String::from);
        let diff_code_content = self.diff_panel.code_view_content().map(String::from);
        let status_selection = self.status_panel.status_selection().cloned();
        let status_view_mode = self.status_panel.view_mode();
        let (commit_message, ai_alternatives) = self.commit_editor.snapshot_data();
        let view_mode = self.view_mode.clone();
        let sidebar_branches_expanded = self.sidebar_state.branches_expanded;
        let sidebar_remotes_expanded = self.sidebar_state.remotes_expanded;
        let sidebar_tags_expanded = self.sidebar_state.tags_expanded;
        let sidebar_worktrees_expanded = self.sidebar_state.worktrees_expanded;
        let sidebar_expanded_remotes = self.sidebar_state.expanded_remotes.clone();

        let active_id = match self.active_repo_tab_id {
            Some(id) => id,
            None => return,
        };
        let Some(tab) = self.open_repo_tabs.iter_mut().find(|t| t.id == active_id) else {
            return;
        };
        tab.panel_snapshot = Some(TabSnapshot {
            selected_commit_id,
            graph_was_uncommitted,
            diff_state,
            diff_view_mode,
            diff_code_file,
            diff_code_content,
            status_selection,
            status_view_mode,
            commit_message,
            ai_alternatives,
            view_mode,
            sidebar_branches_expanded,
            sidebar_remotes_expanded,
            sidebar_tags_expanded,
            sidebar_worktrees_expanded,
            sidebar_expanded_remotes,
        });
    }

    pub(crate) fn restore_snapshot_from_tab(&mut self) {
        let snapshot = self.active_tab().and_then(|tab| tab.panel_snapshot.clone());

        let Some(snap) = snapshot else { return };

        self.view_mode = snap.view_mode;
        self.sidebar_state.branches_expanded = snap.sidebar_branches_expanded;
        self.sidebar_state.remotes_expanded = snap.sidebar_remotes_expanded;
        self.sidebar_state.tags_expanded = snap.sidebar_tags_expanded;
        self.sidebar_state.worktrees_expanded = snap.sidebar_worktrees_expanded;
        self.sidebar_state.expanded_remotes = snap.sidebar_expanded_remotes;

        self.commit_editor
            .restore_from_snapshot(snap.commit_message, snap.ai_alternatives);
        self.status_panel
            .restore_from_snapshot(snap.status_selection, snap.status_view_mode);

        self.diff_panel.restore_from_snapshot(
            snap.diff_state,
            snap.diff_view_mode,
            snap.diff_code_file,
            snap.diff_code_content,
        );

        if let Some(ref commit_id) = snap.selected_commit_id {
            if let Some(idx) = self.graph_panel.find_commit_idx(commit_id) {
                self.graph_panel.select_commit(idx);
            }
        } else if snap.graph_was_uncommitted {
            self.graph_panel.select_uncommitted();
        }
    }
}
