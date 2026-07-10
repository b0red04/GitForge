use std::path::{Path, PathBuf};
use std::sync::Arc;

use gitforge_git::{RepoState, Repository};
use parking_lot::Mutex;

use super::app::MainViewMode;
use super::diff_panel::CommitDiffState;
use super::diff_viewer::DiffViewMode;
use super::repo_tabs::RepoTabView;
use super::sidebar::SidebarExpansion;

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
    /// True after the first hosting API fetch for this tab completes.
    pub(crate) pull_requests_loaded: bool,
}

/// Per-tab UI state saved when switching away from a repository tab.
#[derive(Debug, Clone)]
pub(crate) struct TabSnapshot {
    pub selected_commit_id: Option<String>,
    pub graph_was_uncommitted: bool,
    pub diff_state: Option<CommitDiffState>,
    pub diff_view_mode: DiffViewMode,
    pub diff_code_file: Option<String>,
    pub diff_code_content: Option<String>,
    pub commit_message: String,
    pub ai_alternatives: Vec<String>,
    pub view_mode: MainViewMode,
    pub diff_overlay_open: bool,
    pub sidebar_expansion: SidebarExpansion,
}

/// Open repository tabs, active tab selection, drag/reorder state, and
/// recently-closed tab paths. Panel rendering and the Selection Cascade live on
/// [`super::repo_session::RepoSession`].
pub(crate) struct TabSession {
    pub(crate) open_repo_tabs: Vec<OpenRepoTab>,
    pub(crate) active_repo_tab_id: Option<u64>,
    pub(crate) next_repo_tab_id: u64,
    pub(crate) closed_repo_tabs: Vec<PathBuf>,
    /// The id of the repository tab currently being dragged, or `None` when no
    /// drag is in flight.
    pub(crate) tab_drag_source: Option<u64>,
    /// `(target tab id, insert_before)` for the tab bar insertion caret.
    pub(crate) tab_drop_target: Option<(u64, bool)>,
}

impl TabSession {
    pub fn new() -> Self {
        Self {
            open_repo_tabs: Vec::new(),
            active_repo_tab_id: None,
            next_repo_tab_id: 1,
            closed_repo_tabs: Vec::new(),
            tab_drag_source: None,
            tab_drop_target: None,
        }
    }

    pub(crate) fn active_tab(&self) -> Option<&OpenRepoTab> {
        let active_id = self.active_repo_tab_id?;
        self.open_repo_tabs.iter().find(|tab| tab.id == active_id)
    }

    pub(crate) fn active_tab_mut(&mut self) -> Option<&mut OpenRepoTab> {
        let active_id = self.active_repo_tab_id?;
        self.tab_mut(active_id)
    }

    pub(crate) fn tab_mut(&mut self, id: u64) -> Option<&mut OpenRepoTab> {
        self.open_repo_tabs.iter_mut().find(|tab| tab.id == id)
    }

    pub(crate) fn alloc_tab_id(&mut self) -> u64 {
        let id = self.next_repo_tab_id;
        self.next_repo_tab_id += 1;
        id
    }

    /// Create a new loading tab for `path` and append it. Returns the new tab id.
    pub(crate) fn push_loading_tab(&mut self, path: PathBuf) -> u64 {
        let id = self.alloc_tab_id();
        let repo = Arc::new(Mutex::new(None));
        self.open_repo_tabs.push(OpenRepoTab {
            id,
            path: Self::normalize_repo_path(&path),
            repo,
            repo_state: None,
            loading: true,
            last_error: None,
            panel_snapshot: None,
            pull_requests: Vec::new(),
            pull_requests_loading: false,
            pull_requests_loaded: false,
        });
        id
    }

    pub(crate) fn is_active(&self, tab_id: u64) -> bool {
        self.active_repo_tab_id == Some(tab_id)
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

    pub(crate) fn push_closed_tab(&mut self, path: PathBuf) {
        let normalized = Self::normalize_repo_path(&path);
        self.closed_repo_tabs.retain(|p| p != &normalized);
        self.closed_repo_tabs.push(normalized);
        while self.closed_repo_tabs.len() > MAX_CLOSED_TABS {
            self.closed_repo_tabs.remove(0);
        }
    }

    pub(crate) fn clear_tab_drag(&mut self) {
        self.tab_drag_source = None;
        self.tab_drop_target = None;
    }

    pub(crate) fn store_active_panel_snapshot(&mut self, snapshot: TabSnapshot) {
        let Some(tab) = self.active_tab_mut() else {
            return;
        };
        tab.panel_snapshot = Some(snapshot);
    }

    pub(crate) fn active_panel_snapshot(&self) -> Option<TabSnapshot> {
        self.active_tab()?.panel_snapshot.clone()
    }

    /// After removing the tab at `removed_index`, select the tab to the left, or
    /// the new first tab, or none if the list is empty.
    pub(crate) fn select_neighbor_after_close(&mut self, removed_index: usize) {
        self.active_repo_tab_id = if self.open_repo_tabs.is_empty() {
            None
        } else if removed_index > 0 {
            Some(self.open_repo_tabs[removed_index - 1].id)
        } else {
            Some(self.open_repo_tabs[0].id)
        };
    }
}

/// Pure, GPUI-free tab reordering: move the tab `dragged_id` so it sits
/// immediately before (when `before` is true) or after `target_id` in `tabs`.
pub(crate) fn reorder_repo_tab(
    tabs: &mut Vec<OpenRepoTab>,
    dragged_id: u64,
    target_id: u64,
    before: bool,
) {
    if dragged_id == target_id {
        return;
    }
    let Some(from) = tabs.iter().position(|t| t.id == dragged_id) else {
        return;
    };
    if !tabs.iter().any(|t| t.id == target_id) {
        return;
    }
    let tab = tabs.remove(from);
    let target_after = tabs
        .iter()
        .position(|t| t.id == target_id)
        .expect("target id present (checked above)");
    let dest = if before {
        target_after
    } else {
        target_after + 1
    };
    tabs.insert(dest, tab);
}

/// Pure, GPUI-free tab reordering: move `dragged_id` to the very end of `tabs`.
pub(crate) fn move_repo_tab_to_end(tabs: &mut Vec<OpenRepoTab>, dragged_id: u64) {
    let Some(from) = tabs.iter().position(|t| t.id == dragged_id) else {
        return;
    };
    if from == tabs.len() - 1 {
        return;
    }
    let tab = tabs.remove(from);
    tabs.push(tab);
}

/// Pure, GPUI-free computation of the insertion-caret index in the tab bar.
pub(crate) fn drop_caret_index(
    tabs: &[OpenRepoTab],
    drag_source: Option<u64>,
    drop_target: Option<(u64, bool)>,
) -> Option<usize> {
    let source = drag_source?;
    let src_idx = tabs.iter().position(|t| t.id == source)?;
    let caret = match drop_target {
        Some((tid, before)) => {
            let idx = tabs.iter().position(|t| t.id == tid)?;
            if before { idx } else { idx + 1 }
        }
        None => tabs.len(),
    };
    if caret == src_idx || caret == src_idx + 1 {
        None
    } else {
        Some(caret)
    }
}

#[cfg(test)]
mod reorder_tests {
    use super::*;

    fn fake_tab(id: u64) -> OpenRepoTab {
        OpenRepoTab {
            id,
            path: PathBuf::from(format!("/repo/{id}")),
            repo: Arc::new(Mutex::new(None)),
            repo_state: None,
            loading: false,
            last_error: None,
            panel_snapshot: None,
            pull_requests: Vec::new(),
            pull_requests_loading: false,
            pull_requests_loaded: false,
        }
    }

    fn ids(tabs: &[OpenRepoTab]) -> Vec<u64> {
        tabs.iter().map(|t| t.id).collect()
    }

    fn tabs(ids: &[u64]) -> Vec<OpenRepoTab> {
        ids.iter().map(|id| fake_tab(*id)).collect()
    }

    #[test]
    fn move_last_to_front() {
        let mut t = tabs(&[10, 20, 30]);
        reorder_repo_tab(&mut t, 30, 10, true);
        assert_eq!(ids(&t), vec![30, 10, 20]);
    }

    #[test]
    fn move_first_to_end_after_last() {
        let mut t = tabs(&[10, 20, 30]);
        reorder_repo_tab(&mut t, 10, 30, false);
        assert_eq!(ids(&t), vec![20, 30, 10]);
    }

    #[test]
    fn move_first_after_second() {
        let mut t = tabs(&[10, 20, 30]);
        reorder_repo_tab(&mut t, 10, 20, false);
        assert_eq!(ids(&t), vec![20, 10, 30]);
    }

    #[test]
    fn move_second_before_first() {
        let mut t = tabs(&[10, 20, 30]);
        reorder_repo_tab(&mut t, 20, 10, true);
        assert_eq!(ids(&t), vec![20, 10, 30]);
    }

    #[test]
    fn move_middle_before_last() {
        let mut t = tabs(&[10, 20, 30]);
        reorder_repo_tab(&mut t, 20, 30, true);
        assert_eq!(ids(&t), vec![10, 20, 30]);
    }

    #[test]
    fn drop_on_self_is_noop() {
        let mut t = tabs(&[10, 20, 30]);
        reorder_repo_tab(&mut t, 20, 20, true);
        reorder_repo_tab(&mut t, 20, 20, false);
        assert_eq!(ids(&t), vec![10, 20, 30]);
    }

    #[test]
    fn missing_dragged_or_target_is_noop() {
        let mut t = tabs(&[10, 20, 30]);
        reorder_repo_tab(&mut t, 99, 10, true);
        reorder_repo_tab(&mut t, 10, 99, false);
        assert_eq!(ids(&t), vec![10, 20, 30]);
    }

    #[test]
    fn single_tab_before_itself_is_noop() {
        let mut t = tabs(&[10]);
        reorder_repo_tab(&mut t, 10, 10, true);
        assert_eq!(ids(&t), vec![10]);
    }

    #[test]
    fn move_to_end_from_front() {
        let mut t = tabs(&[10, 20, 30]);
        move_repo_tab_to_end(&mut t, 10);
        assert_eq!(ids(&t), vec![20, 30, 10]);
    }

    #[test]
    fn move_to_end_from_middle() {
        let mut t = tabs(&[10, 20, 30]);
        move_repo_tab_to_end(&mut t, 20);
        assert_eq!(ids(&t), vec![10, 30, 20]);
    }

    #[test]
    fn move_to_end_already_last_is_noop() {
        let mut t = tabs(&[10, 20, 30]);
        move_repo_tab_to_end(&mut t, 30);
        assert_eq!(ids(&t), vec![10, 20, 30]);
    }

    #[test]
    fn move_to_end_missing_is_noop() {
        let mut t = tabs(&[10, 20, 30]);
        move_repo_tab_to_end(&mut t, 99);
        assert_eq!(ids(&t), vec![10, 20, 30]);
    }

    #[test]
    fn drop_caret_none_when_no_source() {
        let t = tabs(&[10, 20, 30]);
        assert_eq!(drop_caret_index(&t, None, None), None);
        assert_eq!(drop_caret_index(&t, None, Some((30, true))), None);
    }

    #[test]
    fn drop_caret_none_when_source_absent() {
        let t = tabs(&[10, 20, 30]);
        assert_eq!(drop_caret_index(&t, Some(99), None), None);
    }

    #[test]
    fn drop_caret_none_when_target_absent() {
        let t = tabs(&[10, 20, 30]);
        assert_eq!(drop_caret_index(&t, Some(10), Some((99, true))), None);
    }

    #[test]
    fn drop_caret_at_tail() {
        let t = tabs(&[10, 20, 30]);
        assert_eq!(drop_caret_index(&t, Some(10), None), Some(3));
    }

    #[test]
    fn drop_caret_tail_is_noop_when_source_already_last() {
        let t = tabs(&[10, 20, 30]);
        assert_eq!(drop_caret_index(&t, Some(30), None), None);
    }

    #[test]
    fn drop_caret_before_target() {
        let t = tabs(&[10, 20, 30]);
        assert_eq!(drop_caret_index(&t, Some(10), Some((30, true))), Some(2));
    }

    #[test]
    fn drop_caret_after_target() {
        let t = tabs(&[10, 20, 30]);
        assert_eq!(drop_caret_index(&t, Some(10), Some((30, false))), Some(3));
    }

    #[test]
    fn drop_caret_skips_positions_adjacent_to_source() {
        let t = tabs(&[10, 20, 30]);
        assert_eq!(drop_caret_index(&t, Some(20), Some((20, true))), None);
        assert_eq!(drop_caret_index(&t, Some(20), Some((20, false))), None);
        assert_eq!(drop_caret_index(&t, Some(20), Some((10, false))), None);
        assert_eq!(drop_caret_index(&t, Some(20), Some((30, true))), None);
    }

    #[test]
    fn push_loading_tab_creates_loading_entry() {
        let mut session = TabSession::new();
        let id = session.push_loading_tab(PathBuf::from("/tmp/my-repo"));
        assert_eq!(session.open_repo_tabs.len(), 1);
        let tab = &session.open_repo_tabs[0];
        assert_eq!(tab.id, id);
        assert!(tab.loading);
        assert!(tab.repo_state.is_none());
        assert!(tab.panel_snapshot.is_none());
    }
}
