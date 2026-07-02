use std::path::{Path, PathBuf};
use std::sync::Arc;

use gitforge_git::{RepoState, Repository};
use gitforge_graph::Graph;
use gpui::AppContext;
use parking_lot::Mutex;

use super::app::MainViewMode;
use super::commit_editor::CommitEditor;
use super::diff_panel::{CommitDiffState, DiffPanel};
use super::diff_viewer::DiffViewMode;
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
    /// True after the first hosting API fetch for this tab completes.
    pub(crate) pull_requests_loaded: bool,
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
    pub diff_overlay_open: bool,
    pub sidebar_expansion: super::sidebar::SidebarExpansion,
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
    /// When true, the selected file's line-level diff is rendered as a large
    /// overlay covering the sidebar + commit graph, leaving the right-hand file
    /// list visible to drive file selection. Per-tab (persisted in
    /// [`TabSnapshot`]).
    pub(crate) diff_overlay_open: bool,
    pub(crate) loading: bool,
    pub(crate) last_error: Option<String>,
    /// Transient label (e.g. "Fetching…") shown while a remote op runs; set by
    /// the dispatch shell via `OpEffects::remote_status` and cleared on
    /// completion.
    pub(crate) remote_status: String,
    pub(crate) closed_repo_tabs: Vec<PathBuf>,
    /// The id of the repository tab currently being dragged, or `None` when no
    /// drag is in flight. Set when the drag begins (in the `on_drag`
    /// constructor) and cleared on drop or drag cancel. Used only to dim the
    /// source tab while dragging.
    pub(crate) tab_drag_source: Option<u64>,
    /// `(target tab id, insert_before)` describing where the dragged tab would
    /// land if released right now, or `None` when the cursor is not over a tab.
    /// Updated continuously by `on_drag_move`; read by the renderer to draw the
    /// insertion caret and by `on_drop` to perform the move.
    pub(crate) tab_drop_target: Option<(u64, bool)>,
}

/// The readiness of the active tab to run a git op — the single guard
/// `run_git_blocking` checks before spawning. GPUI-free, so it is unit-testable
/// without a `TestAppContext`.
///
/// Distinct from `active_repo_ready` (which gates on `repo_state.is_some()` for
/// UI that reads the snapshot): the git-op path needs the live repository
/// handle, not the snapshot. The inner `Option<Repository>` of the handle is
/// checked later by `with_repo_blocking`; `Ready` only asserts the active tab
/// exists, is not loading, and holds a handle.
pub(crate) enum GitOpReadiness {
    /// The active tab is loaded and holds a repo handle — the op may run.
    Ready(Arc<Mutex<Option<Repository>>>),
    /// No active tab (no repo open). The caller surfaces a Warning.
    NoRepo,
    /// The active tab is still in discovery (`loading == true`). The caller
    /// skips silently.
    Loading,
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
            diff_overlay_open: false,
            loading: false,
            last_error: None,
            remote_status: String::new(),
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

    /// Whether the active tab has finished discovery and can run git ops.
    pub(crate) fn active_repo_ready(&self) -> bool {
        self.active_tab()
            .is_some_and(|tab| !tab.loading && tab.repo_state.is_some())
    }

    /// The single readiness check for a git op. Returns the repo handle when the
    /// active tab is loaded, or the reason it cannot run. See
    /// [`GitOpReadiness`].
    pub(crate) fn git_op_readiness(&self) -> GitOpReadiness {
        let Some(tab) = self.active_tab() else {
            return GitOpReadiness::NoRepo;
        };
        if tab.loading {
            return GitOpReadiness::Loading;
        }
        GitOpReadiness::Ready(tab.repo.clone())
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
            .set_data(Vec::new(), Vec::new(), Graph::new(), false, None);
        self.diff_panel.clear();
        self.status_panel.clear();
    }

    pub(crate) fn clear_active_repo_view(&mut self) {
        self.clear_repo_panels();
        self.last_error = None;
        self.loading = false;
    }

    /// THE Selection Cascade invariant enforcer (ADR-0003). Given a graph
    /// selection, makes `status_panel` (enter/exit graph-staging, gated on
    /// `view_mode`) and `diff_panel` (clear) consistent with it.
    ///
    /// Does **not** write `graph_panel` — the selection is the cascade's
    /// input, not its output. The caller must have set `graph_panel.selection`
    /// before calling this (or in the same atomic update via
    /// [`Self::set_selection`]).
    ///
    /// Returns the async work the caller must spawn (none / just notify /
    /// load the diff). See [`SelectionEffect`].
    fn cascade(&mut self, sel: GraphSelection) -> SelectionEffect {
        match sel {
            GraphSelection::Commit(_) => {
                if self.view_mode == MainViewMode::CommitHistory {
                    self.status_panel.exit_graph_staging();
                }
                self.diff_panel.clear();
                SelectionEffect::LoadDiffForSelected
            }
            GraphSelection::Uncommitted => {
                if self.view_mode == MainViewMode::CommitHistory {
                    self.status_panel.enter_graph_staging();
                }
                self.diff_panel.clear();
                SelectionEffect::ClearDiff
            }
            GraphSelection::None => {
                self.diff_panel.clear();
                SelectionEffect::ClearDiff
            }
        }
    }

    /// Selection entry for clicks and programmatic selection. Writes
    /// `graph_panel` to the given selection, forces `view_mode =
    /// CommitHistory` (explicit navigation), then runs the cascade.
    pub fn set_selection(&mut self, sel: GraphSelection) -> SelectionEffect {
        self.view_mode = MainViewMode::CommitHistory;
        match sel {
            GraphSelection::Commit(idx) => self.graph_panel.select_commit(idx),
            GraphSelection::Uncommitted => self.graph_panel.select_uncommitted(),
            GraphSelection::None => self.graph_panel.clear_selection(),
        }
        self.cascade(sel)
    }

    /// Selection entry for keyboard navigation. The graph panel has already
    /// moved its own selection via `select_prev`/`select_next`, so this reads
    /// `graph_panel.selection()` and runs the cascade without re-writing the
    /// graph. Does not touch `view_mode` (the user is already in history view
    /// when navigating the graph by keyboard).
    pub fn cascade_current(&mut self) -> SelectionEffect {
        let sel = self.graph_panel.selection();
        self.cascade(sel)
    }

    pub(crate) fn apply_repo_state_to_panels(&mut self, repo_state_data: &RepoState) {
        use gitforge_graph::CommitEntry;

        let has_uncommitted = repo_state_data.status.has_changes();

        // Capture the user's selection BEFORE `set_data` wipes it, keyed by
        // stable commit id. A fetch can prepend commits and shift every index,
        // so remembering the (unstable) index would silently land on a
        // different commit — the id lets us re-resolve to the new index below.
        // The borrow from `commit_id_at` ends immediately because we copy the
        // id into an owned `String`, leaving `graph_panel` free for the
        // mutable `set_data` call that follows.
        let prev_selection = match self.graph_panel.selection() {
            GraphSelection::Commit(idx) => self
                .graph_panel
                .commit_id_at(idx)
                .map(|id| PriorSelection::Commit(id.to_string())),
            GraphSelection::Uncommitted => Some(PriorSelection::Uncommitted),
            GraphSelection::None => None,
        };

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
            if repo_state_data.head_branch.is_none() {
                repo_state_data.head_commit.clone()
            } else {
                None
            },
        );
        let in_history = self.view_mode == MainViewMode::CommitHistory;
        let preserve_staging = in_history && self.status_panel.is_graph_staging();
        self.status_panel
            .set_status(repo_state_data.status.clone(), preserve_staging);

        // Decide what to do with selection after the rebuild, then route
        // through the Selection Cascade (`Self::cascade`) so the
        // status/diff panels stay consistent with the new graph selection.
        // See ADR-0003.
        //
        // `PreservedCommit` bypasses the cascade entirely: the user's prior
        // commit is still present (possibly at a shifted index), commits are
        // immutable so its cached diff is still valid, and skipping the
        // cascade lets `sync_diff_view` hit its cache key (ADR-0001). The
        // other arms clear the diff (via the cascade) because the selection
        // genuinely changed.
        //
        // This method returns `()` (not `SelectionEffect`): the cascade's
        // outcome here is always `ClearDiff` (a "notify"), and the callers
        // (`apply_repo_state` → `refresh_repository`, and
        // `apply_active_repo_tab_to_view` → `tab_ops`) already call
        // `cx.notify()`. The snapshot path never reaches
        // `LoadDiffForSelected`.
        let new_commit_ids: Vec<&str> = repo_state_data
            .commits
            .iter()
            .map(|c| c.id.as_str())
            .collect();
        match reselect_after_refresh(prev_selection, has_uncommitted, &new_commit_ids) {
            RefreshSelection::PreservedCommit(idx) => {
                self.graph_panel.select_commit(idx);
            }
            RefreshSelection::PreservedUncommitted => {
                self.graph_panel.select_uncommitted();
                let _ = self.cascade(GraphSelection::Uncommitted);
            }
            RefreshSelection::Fallback => {
                if has_uncommitted {
                    self.graph_panel.select_uncommitted();
                    let _ = self.cascade(GraphSelection::Uncommitted);
                } else {
                    self.graph_panel.clear_selection();
                    let _ = self.cascade(GraphSelection::None);
                }
            }
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

    /// Reset all in-flight tab-drag state. Called by the tab bar's drop
    /// handlers after a drop completes (whether on a tab or the tail).
    pub(crate) fn clear_tab_drag(&mut self) {
        self.tab_drag_source = None;
        self.tab_drop_target = None;
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
        let diff_overlay_open = self.diff_overlay_open;
        let sidebar_expansion = self.sidebar_state.expansion();

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
            diff_overlay_open,
            sidebar_expansion,
        });
    }

    pub(crate) fn restore_snapshot_from_tab(&mut self) {
        let snapshot = self.active_tab().and_then(|tab| tab.panel_snapshot.clone());

        let Some(snap) = snapshot else { return };

        self.view_mode = snap.view_mode;
        self.diff_overlay_open = snap.diff_overlay_open;
        self.sidebar_state.apply_expansion(&snap.sidebar_expansion);

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
        if self.diff_overlay_open {
            self.diff_panel.set_diff_mode();
            let (selected_file_idx, file_count) = self
                .diff_panel
                .diff_state()
                .map(|d| (d.selected_file_idx, d.file_diffs.len()))
                .unwrap_or((None, 0));
            if let Some(file_idx) = selected_file_idx
                .filter(|idx| *idx < file_count)
                .or_else(|| (file_count > 0).then_some(0))
            {
                self.diff_panel.select_file(file_idx);
            }
        }

        if let Some(ref commit_id) = snap.selected_commit_id {
            if let Some(idx) = self.graph_panel.find_commit_idx(commit_id) {
                self.graph_panel.select_commit(idx);
            }
        } else if snap.graph_was_uncommitted {
            self.graph_panel.select_uncommitted();
        }
    }
}

/// Pure, GPUI-free tab reordering: move the tab `dragged_id` so it sits
/// immediately before (when `before` is true) or after `target_id` in `tabs`.
///
/// No-op if `dragged_id == target_id` or either id is not present. The target's
/// position is recomputed after the dragged tab is removed, so the result is
/// correct regardless of whether the dragged tab was originally to the left or
/// right of the target.
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
/// No-op if it is absent or already last.
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

/// Pure, GPUI-free computation of the insertion-caret index to render in the
/// tab bar while a reorder drag is in flight.
///
/// Returns `None` when no drag is active, when the dragged tab is not present,
/// when the recorded drop target id is not present, or when the computed
/// position would be a no-op move (immediately adjacent to the source). When
/// `drop_target` is `None` the caret sits at the end of the bar.
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

/// What the caller (`GitForgeApp`) must do asynchronously after a
/// selection-driven cascade. Returned by [`RepoSession::cascade`],
/// [`RepoSession::set_selection`], and [`RepoSession::cascade_current`].
///
/// `RepoSession` stays GPUI-free, so it cannot spawn the diff load itself;
/// instead it tells the caller what async work (if any) is needed, and the
/// caller interprets the effect. See ADR-0003.
///
/// The snapshot path (`apply_repo_state_to_panels`) returns `()`, not this
/// type: it only ever reaches `ClearDiff` outcomes (or bypasses the cascade
/// entirely for `PreservedCommit`), and its callers already call
/// `cx.notify()`, which is all `ClearDiff` asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[must_use]
pub(crate) enum SelectionEffect {
    /// The cascade already cleared `diff_panel`. The caller just notifies.
    ClearDiff,
    /// The caller should call `load_diff_for_selected(cx)` to fetch and
    /// install the diff for the newly-selected commit.
    LoadDiffForSelected,
}

/// The user's graph selection captured *before* a refresh rebuilds the commit
/// list. Used by [`reselect_after_refresh`] to decide what to re-select
/// afterwards. Stored by stable commit id rather than (unstable) index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PriorSelection {
    /// A real commit was selected; we remember its stable id.
    Commit(String),
    /// The working-tree / uncommitted node was selected.
    Uncommitted,
}

/// What [`RepoSession::apply_repo_state_to_panels`] should do with the graph
/// selection after a refresh, given the user's prior selection and the
/// post-refresh commit list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RefreshSelection {
    /// Re-select this index in the new commit list. The prior commit is still
    /// present (possibly at a shifted index because the fetch prepended commits).
    PreservedCommit(usize),
    /// Keep the working-tree node selected.
    PreservedUncommitted,
    /// The prior selection is no longer valid (commit pruned, working tree
    /// newly clean, or nothing was selected). The caller falls back to its
    /// default: auto-pick uncommitted if there are changes, else clear.
    Fallback,
}

/// Pure decision: given the user's prior selection, the post-refresh working
/// tree state, and the post-refresh commit list (in display order), what should
/// `apply_repo_state_to_panels` re-select?
///
/// `new_commit_ids` is the post-refresh commit list in display order; a
/// returned [`RefreshSelection::PreservedCommit`] index refers to a position in
/// that slice. Keeping this free of GPUI/`GraphPanel` types makes the
/// index-stability edge cases (commit pruned, working tree changed, nothing
/// selected) exhaustively unit-testable.
pub(crate) fn reselect_after_refresh(
    prev: Option<PriorSelection>,
    has_uncommitted: bool,
    new_commit_ids: &[&str],
) -> RefreshSelection {
    match prev {
        Some(PriorSelection::Commit(id)) => match new_commit_ids.iter().position(|c| *c == id) {
            Some(idx) => RefreshSelection::PreservedCommit(idx),
            None => RefreshSelection::Fallback,
        },
        Some(PriorSelection::Uncommitted) => {
            if has_uncommitted {
                RefreshSelection::PreservedUncommitted
            } else {
                RefreshSelection::Fallback
            }
        }
        None => RefreshSelection::Fallback,
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
        // Dragging tab 10 (idx 0) to the tail lands at index 3.
        assert_eq!(drop_caret_index(&t, Some(10), None), Some(3));
    }

    #[test]
    fn drop_caret_tail_is_noop_when_source_already_last() {
        let t = tabs(&[10, 20, 30]);
        // Tab 30 is already at the end; tail caret (3) == src_idx (2) + 1.
        assert_eq!(drop_caret_index(&t, Some(30), None), None);
    }

    #[test]
    fn drop_caret_before_target() {
        let t = tabs(&[10, 20, 30]);
        // Drag 10 before 30 (idx 2) -> caret 2.
        assert_eq!(drop_caret_index(&t, Some(10), Some((30, true))), Some(2));
    }

    #[test]
    fn drop_caret_after_target() {
        let t = tabs(&[10, 20, 30]);
        // Drag 10 after 30 (idx 2) -> caret 3.
        assert_eq!(drop_caret_index(&t, Some(10), Some((30, false))), Some(3));
    }

    #[test]
    fn drop_caret_skips_positions_adjacent_to_source() {
        let t = tabs(&[10, 20, 30]);
        // Source 20 (idx 1). Before-self adjacent: before 20 -> caret 1 == src.
        assert_eq!(drop_caret_index(&t, Some(20), Some((20, true))), None);
        // After-self adjacent: after 20 -> caret 2 == src + 1.
        assert_eq!(drop_caret_index(&t, Some(20), Some((20, false))), None);
        // After the tab just before source: after 10 -> caret 1 == src.
        assert_eq!(drop_caret_index(&t, Some(20), Some((10, false))), None);
        // Before the tab just after source: before 30 -> caret 2 == src + 1.
        assert_eq!(drop_caret_index(&t, Some(20), Some((30, true))), None);
    }
}

#[cfg(test)]
mod refresh_selection_tests {
    use super::*;

    // ---- reselect_after_refresh: real commit preserved ----

    #[test]
    fn preserved_commit_keeps_index_when_unchanged() {
        let ids = ["a", "b", "c"];
        let prev = Some(PriorSelection::Commit("b".into()));
        assert_eq!(
            reselect_after_refresh(prev, true, &ids),
            RefreshSelection::PreservedCommit(1),
        );
    }

    #[test]
    fn preserved_commit_re_resolves_when_fetch_prepended_commits() {
        // The specific bug being fixed: a fetch added "new1" and "new2" at the
        // front, so "b" moved from index 1 to index 3. The old code reset
        // selection entirely; we must land on the *new* index for the same id.
        let ids = ["new1", "new2", "a", "b", "c"];
        let prev = Some(PriorSelection::Commit("b".into()));
        assert_eq!(
            reselect_after_refresh(prev, true, &ids),
            RefreshSelection::PreservedCommit(3),
        );
    }

    #[test]
    fn preserved_commit_survives_even_when_working_tree_has_changes() {
        // Pre-fix behavior was to force-jump to the uncommitted node whenever
        // the repo had changes. The fix keeps the user's commit selected.
        let ids = ["a", "b"];
        let prev = Some(PriorSelection::Commit("a".into()));
        assert_eq!(
            reselect_after_refresh(prev, true, &ids),
            RefreshSelection::PreservedCommit(0),
        );
    }

    // ---- reselect_after_refresh: commit gone (pruned/dropped) ----

    #[test]
    fn pruned_commit_falls_back_when_working_tree_has_changes() {
        let ids = ["a", "c"]; // "b" disappeared (e.g. its remote branch was pruned)
        let prev = Some(PriorSelection::Commit("b".into()));
        assert_eq!(
            reselect_after_refresh(prev, true, &ids),
            RefreshSelection::Fallback,
        );
    }

    #[test]
    fn pruned_commit_falls_back_when_working_tree_clean() {
        let ids = ["a", "c"];
        let prev = Some(PriorSelection::Commit("b".into()));
        assert_eq!(
            reselect_after_refresh(prev, false, &ids),
            RefreshSelection::Fallback,
        );
    }

    #[test]
    fn preserved_commit_falls_back_when_commit_list_emptied() {
        let prev = Some(PriorSelection::Commit("a".into()));
        assert_eq!(
            reselect_after_refresh(prev, false, &[]),
            RefreshSelection::Fallback,
        );
    }

    // ---- reselect_after_refresh: uncommitted node ----

    #[test]
    fn uncommitted_preserved_when_working_tree_still_dirty() {
        let prev = Some(PriorSelection::Uncommitted);
        assert_eq!(
            reselect_after_refresh(prev, true, &["a", "b"]),
            RefreshSelection::PreservedUncommitted,
        );
    }

    #[test]
    fn uncommitted_falls_back_when_working_tree_becomes_clean() {
        // User had the working tree selected, then committed/stashed everything
        // before the refresh tick — the uncommitted node no longer exists.
        let prev = Some(PriorSelection::Uncommitted);
        assert_eq!(
            reselect_after_refresh(prev, false, &["a", "b"]),
            RefreshSelection::Fallback,
        );
    }

    // ---- reselect_after_refresh: no prior selection ----

    #[test]
    fn no_prior_selection_falls_back_even_with_changes() {
        // First load of a repo: nothing was selected, so we do NOT preserve
        // anything — the caller's fallback auto-selects uncommitted.
        assert_eq!(
            reselect_after_refresh(None, true, &["a", "b"]),
            RefreshSelection::Fallback,
        );
    }

    #[test]
    fn no_prior_selection_falls_back_when_clean() {
        assert_eq!(
            reselect_after_refresh(None, false, &["a", "b"]),
            RefreshSelection::Fallback,
        );
    }
}

#[cfg(test)]
mod active_repo_ready_tests {
    use std::collections::HashSet;

    use super::*;
    use gitforge_git::RepoStatus;
    use gpui::TestAppContext;

    fn minimal_repo_state() -> RepoState {
        RepoState {
            path: PathBuf::from("/tmp/test-repo"),
            head_branch: None,
            head_commit: None,
            commits: vec![],
            references: vec![],
            conflicting_local_branches: HashSet::new(),
            status: RepoStatus::default(),
            worktrees: vec![],
            remotes: vec![],
            rebase_in_progress: false,
        }
    }

    fn fake_tab(id: u64, loading: bool, has_state: bool) -> OpenRepoTab {
        OpenRepoTab {
            id,
            path: PathBuf::from(format!("/repo/{id}")),
            repo: Arc::new(Mutex::new(None)),
            repo_state: has_state.then(minimal_repo_state),
            loading,
            last_error: None,
            panel_snapshot: None,
            pull_requests: Vec::new(),
            pull_requests_loading: false,
            pull_requests_loaded: false,
        }
    }

    fn session_with_tab(cx: &mut gpui::App, tab: OpenRepoTab) -> RepoSession {
        let id = tab.id;
        let mut session = RepoSession::new(cx);
        session.open_repo_tabs.push(tab);
        session.active_repo_tab_id = Some(id);
        session
    }

    #[gpui::test]
    fn active_repo_ready_false_when_no_tab(cx: &mut TestAppContext) {
        cx.update(|app| {
            let session = RepoSession::new(app);
            assert!(!session.active_repo_ready());
        });
    }

    #[gpui::test]
    fn active_repo_ready_false_when_loading(cx: &mut TestAppContext) {
        cx.update(|app| {
            let session = session_with_tab(app, fake_tab(1, true, false));
            assert!(!session.active_repo_ready());
        });
    }

    #[gpui::test]
    fn active_repo_ready_false_when_loaded_but_no_state(cx: &mut TestAppContext) {
        cx.update(|app| {
            let session = session_with_tab(app, fake_tab(1, false, false));
            assert!(!session.active_repo_ready());
        });
    }

    #[gpui::test]
    fn active_repo_ready_true_when_loaded_with_state(cx: &mut TestAppContext) {
        cx.update(|app| {
            let session = session_with_tab(app, fake_tab(1, false, true));
            assert!(session.active_repo_ready());
        });
    }

    #[gpui::test]
    fn git_op_readiness_no_repo_when_no_tab(cx: &mut TestAppContext) {
        cx.update(|app| {
            let session = RepoSession::new(app);
            assert!(matches!(session.git_op_readiness(), GitOpReadiness::NoRepo));
        });
    }

    #[gpui::test]
    fn git_op_readiness_no_repo_when_no_active_tab(cx: &mut TestAppContext) {
        cx.update(|app| {
            let mut session = RepoSession::new(app);
            session.open_repo_tabs.push(fake_tab(1, false, true));
            session.active_repo_tab_id = None;
            assert!(matches!(session.git_op_readiness(), GitOpReadiness::NoRepo));
        });
    }

    #[gpui::test]
    fn git_op_readiness_loading_when_tab_loading(cx: &mut TestAppContext) {
        cx.update(|app| {
            // `has_state = true` here is deliberate: git-op readiness gates on
            // `loading`, not on `repo_state` (unlike `active_repo_ready`).
            let session = session_with_tab(app, fake_tab(1, true, true));
            assert!(matches!(session.git_op_readiness(), GitOpReadiness::Loading));
        });
    }

    #[gpui::test]
    fn git_op_readiness_ready_when_loaded_even_without_state(cx: &mut TestAppContext) {
        cx.update(|app| {
            // The key distinction from `active_repo_ready`: a git op only needs
            // the live handle, so `has_state = false` still yields `Ready`.
            let session = session_with_tab(app, fake_tab(1, false, false));
            match session.git_op_readiness() {
                GitOpReadiness::Ready(_) => {}
                GitOpReadiness::NoRepo => panic!("expected Ready, got NoRepo"),
                GitOpReadiness::Loading => panic!("expected Ready, got Loading"),
            }
        });
    }
}

#[cfg(test)]
mod cascade_tests {
    use super::*;
    use gitforge_git::CommitInfo;
    use gpui::TestAppContext;

    const SAMPLE_COMMIT_ID: &str = "abcdef000000000000000000000000000000001";

    fn sample_commit(id: &str) -> CommitInfo {
        CommitInfo {
            id: id.into(),
            short_id: id.get(..7).unwrap_or(id).into(),
            message: "initial".into(),
            summary: "initial".into(),
            author_name: "n".into(),
            author_email: "e".into(),
            author_date: chrono::Utc::now(),
            committer_name: "n".into(),
            committer_email: "e".into(),
            committer_date: chrono::Utc::now(),
            parent_ids: vec![],
        }
    }

    fn one_commit_session(cx: &mut gpui::App) -> RepoSession {
        let mut session = RepoSession::new(cx);
        let commit = sample_commit(SAMPLE_COMMIT_ID);
        let entries = vec![gitforge_graph::CommitEntry::new(commit.id.clone(), vec![])];
        let graph = Graph::build(&entries);
        session
            .graph_panel
            .set_data(vec![commit], vec![], graph, true, None);
        session
    }

    fn repo_state_with_commits(commits: Vec<CommitInfo>) -> gitforge_git::RepoState {
        use std::collections::HashSet;
        gitforge_git::RepoState {
            path: PathBuf::from("/tmp/test-repo"),
            head_branch: None,
            head_commit: None,
            commits,
            references: vec![],
            conflicting_local_branches: HashSet::new(),
            status: gitforge_git::RepoStatus::default(),
            worktrees: vec![],
            remotes: vec![],
            rebase_in_progress: false,
        }
    }

    #[gpui::test]
    fn commit_in_history_exits_staging_clears_diff_loads(cx: &mut TestAppContext) {
        cx.update(|app| {
            let mut s = one_commit_session(app);
            s.view_mode = MainViewMode::CommitHistory;
            s.status_panel.enter_graph_staging();
            assert!(s.status_panel.is_graph_staging());

            let effect = s.cascade(GraphSelection::Commit(0));

            assert_eq!(effect, SelectionEffect::LoadDiffForSelected);
            assert!(!s.status_panel.is_graph_staging());
            assert!(s.diff_panel.diff_state().is_none());
        });
    }

    #[gpui::test]
    fn uncommitted_in_history_enters_staging_clears_diff(cx: &mut TestAppContext) {
        cx.update(|app| {
            let mut s = one_commit_session(app);
            s.view_mode = MainViewMode::CommitHistory;
            assert!(!s.status_panel.is_graph_staging());

            let effect = s.cascade(GraphSelection::Uncommitted);

            assert_eq!(effect, SelectionEffect::ClearDiff);
            assert!(s.status_panel.is_graph_staging());
            assert!(s.diff_panel.diff_state().is_none());
        });
    }

    #[gpui::test]
    fn uncommitted_in_status_view_skips_staging(cx: &mut TestAppContext) {
        cx.update(|app| {
            let mut s = one_commit_session(app);
            s.view_mode = MainViewMode::Status;
            assert!(!s.status_panel.is_graph_staging());

            let effect = s.cascade(GraphSelection::Uncommitted);

            // Gating: view_mode != CommitHistory means the cascade must NOT
            // enter graph staging. The diff is still cleared and the effect
            // still reports ClearDiff.
            assert_eq!(effect, SelectionEffect::ClearDiff);
            assert!(!s.status_panel.is_graph_staging());
            assert!(s.diff_panel.diff_state().is_none());
        });
    }

    #[gpui::test]
    fn none_clears_diff_no_status_change(cx: &mut TestAppContext) {
        cx.update(|app| {
            let mut s = one_commit_session(app);
            s.view_mode = MainViewMode::CommitHistory;
            s.status_panel.enter_graph_staging();
            assert!(s.status_panel.is_graph_staging());

            let effect = s.cascade(GraphSelection::None);

            // The None branch clears the diff but does not touch the status
            // panel (matches pre-ADR-0003 behaviour; see ADR's "Behaviour
            // changes" section).
            assert_eq!(effect, SelectionEffect::ClearDiff);
            assert!(s.status_panel.is_graph_staging());
            assert!(s.diff_panel.diff_state().is_none());
        });
    }

    #[gpui::test]
    fn set_selection_forces_history_view(cx: &mut TestAppContext) {
        cx.update(|app| {
            let mut s = one_commit_session(app);
            s.view_mode = MainViewMode::Status;

            let effect = s.set_selection(GraphSelection::Commit(0));

            assert_eq!(effect, SelectionEffect::LoadDiffForSelected);
            assert_eq!(s.view_mode, MainViewMode::CommitHistory);
            assert_eq!(s.graph_panel.selection(), GraphSelection::Commit(0));
        });
    }

    #[gpui::test]
    fn cascade_current_uses_existing_graph_selection(cx: &mut TestAppContext) {
        cx.update(|app| {
            let mut s = one_commit_session(app);
            s.view_mode = MainViewMode::CommitHistory;
            // Simulate keyboard navigation: the graph panel moved its own
            // selection, the session has not been told yet.
            s.graph_panel.select_commit(0);

            let effect = s.cascade_current();

            // cascade_current reads graph_panel.selection() (Commit) and
            // cascades accordingly, without re-writing the graph.
            assert_eq!(effect, SelectionEffect::LoadDiffForSelected);
            assert_eq!(s.graph_panel.selection(), GraphSelection::Commit(0));
            assert!(!s.status_panel.is_graph_staging());
        });
    }

    #[gpui::test]
    fn preserved_commit_refresh_keeps_cached_diff(cx: &mut TestAppContext) {
        cx.update(|app| {
            let mut s = one_commit_session(app);
            s.view_mode = MainViewMode::CommitHistory;

            s.graph_panel.select_commit(0);
            s.diff_panel
                .set_diff(CommitDiffState::new(SAMPLE_COMMIT_ID.into(), vec![], None));
            assert!(s.diff_panel.diff_state().is_some());

            let repo_state = repo_state_with_commits(vec![sample_commit(SAMPLE_COMMIT_ID)]);
            s.apply_repo_state_to_panels(&repo_state);

            let diff = s
                .diff_panel
                .diff_state()
                .expect("PreservedCommit must keep the cached diff intact");
            assert_eq!(diff.commit_id, SAMPLE_COMMIT_ID);
            assert_eq!(s.graph_panel.selection(), GraphSelection::Commit(0));
            assert!(!s.status_panel.is_graph_staging());
        });
    }
}
