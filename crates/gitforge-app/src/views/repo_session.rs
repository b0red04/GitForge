use std::path::{Path, PathBuf};
use std::sync::Arc;

use gitforge_git::{RepoState, Repository};
use gitforge_graph::Graph;
use gpui::AppContext;
use parking_lot::Mutex;

use super::app::MainViewMode;
use super::commit_editor::CommitEditor;
use super::diff_panel::{CommitDiffState, DiffPanel};
use super::graph_panel::{GraphPanel, GraphSelection};
use super::sidebar::SidebarState;
use super::status_panel::StatusPanel;
pub(crate) use super::tab_session::{OpenRepoTab, TabSnapshot, TabSession, drop_caret_index};

pub(crate) struct RepoSession {
    pub(crate) tabs: TabSession,
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
            tabs: TabSession::new(),
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
        }
    }

    pub(crate) fn active_tab(&self) -> Option<&OpenRepoTab> {
        self.tabs.active_tab()
    }

    pub(crate) fn active_tab_mut(&mut self) -> Option<&mut OpenRepoTab> {
        self.tabs.active_tab_mut()
    }

    pub(crate) fn active_repo_state(&self) -> Option<&RepoState> {
        self.active_tab().and_then(|tab| tab.repo_state.as_ref())
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

    pub(crate) fn repo_tab_views(&self) -> Vec<super::repo_tabs::RepoTabView> {
        self.tabs.repo_tab_views()
    }

    pub(crate) fn normalize_repo_path(path: &Path) -> PathBuf {
        TabSession::normalize_repo_path(path)
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

    /// Align `status_panel` graph-staging mode with `sel`, gated on
    /// [`MainViewMode::CommitHistory`]. Does not touch `diff_panel`.
    fn sync_status_for_selection(&mut self, sel: GraphSelection) {
        match sel {
            GraphSelection::Commit(_) => {
                if self.view_mode == MainViewMode::CommitHistory {
                    self.status_panel.exit_graph_staging();
                }
            }
            GraphSelection::Uncommitted => {
                if self.view_mode == MainViewMode::CommitHistory {
                    self.status_panel.enter_graph_staging();
                }
            }
            GraphSelection::None => {}
        }
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
        self.sync_status_for_selection(sel);
        match sel {
            GraphSelection::Commit(_) => {
                self.diff_panel.clear();
                SelectionEffect::LoadDiffForSelected
            }
            GraphSelection::Uncommitted | GraphSelection::None => {
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

    pub(crate) fn apply_repo_state_to_panels(
        &mut self,
        repo_state_data: &RepoState,
        reselect: RefreshReselectPolicy,
    ) {
        use gitforge_graph::CommitEntry;

        let has_uncommitted = repo_state_data.status.has_changes();

        // Capture the user's selection BEFORE `set_data` wipes it, keyed by
        // stable commit id. A fetch can prepend commits and shift every index,
        // so remembering the (unstable) index would silently land on a
        // different commit — the id lets us re-resolve to the new index below.
        // The borrow from `commit_id_at` ends immediately because we copy the
        // id into an owned `String`, leaving `graph_panel` free for the
        // mutable `set_data` call that follows.
        //
        // Tab switches defer re-selection to [`Self::restore_snapshot_from_tab`]
        // so the outgoing tab's graph selection is not applied to the incoming
        // tab's commit list.
        let prev_selection = match reselect {
            RefreshReselectPolicy::Reselect => match self.graph_panel.selection() {
                GraphSelection::Commit(idx) => self
                    .graph_panel
                    .commit_id_at(idx)
                    .map(|id| PriorSelection::Commit(id.to_string())),
                GraphSelection::Uncommitted => Some(PriorSelection::Uncommitted),
                GraphSelection::None => None,
            },
            RefreshReselectPolicy::DeferToSnapshot => None,
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
        let preserve_staging = match reselect {
            RefreshReselectPolicy::Reselect => {
                self.view_mode == MainViewMode::CommitHistory
                    && self.status_panel.is_graph_staging()
            }
            // Tab switch: the outgoing tab's staging mode must not leak into the
            // incoming tab's status data. Restore re-derives staging from graph
            // selection via [`Self::sync_status_for_selection`].
            RefreshReselectPolicy::DeferToSnapshot => false,
        };
        self.status_panel
            .set_status(repo_state_data.status.clone(), preserve_staging);

        if reselect == RefreshReselectPolicy::DeferToSnapshot {
            return;
        }

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
        self.apply_repo_state_to_panels(&repo_state_data, RefreshReselectPolicy::Reselect);
        if let Some(tab) = self.active_tab_mut() {
            tab.path = repo_state_data.path.clone();
            tab.repo_state = Some(repo_state_data.clone());
            tab.loading = false;
            tab.last_error = None;
        }
    }

    pub(crate) fn apply_active_repo_tab_to_view(&mut self, reselect: RefreshReselectPolicy) {
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
            self.apply_repo_state_to_panels(&repo_state, reselect);
        } else {
            self.clear_repo_panels();
            self.loading = loading;
            self.last_error = last_error;
        }
    }

    /// Tab-switch path: rebuild the incoming tab's graph/status data without
    /// applying the outgoing tab's selection, then restore per-tab UI from
    /// [`TabSnapshot`].
    pub(crate) fn apply_incoming_tab_after_switch(&mut self) -> Option<SelectionEffect> {
        self.apply_active_repo_tab_to_view(RefreshReselectPolicy::DeferToSnapshot);
        self.restore_snapshot_from_tab()
    }

    pub fn take_commit_message(&mut self) -> String {
        let msg = self.commit_editor.take_message();
        self.status_panel.reset_after_commit();
        msg
    }

    pub(crate) fn push_closed_tab(&mut self, path: PathBuf) {
        self.tabs.push_closed_tab(path);
    }

    /// Reset all in-flight tab-drag state. Called by the tab bar's drop
    /// handlers after a drop completes (whether on a tab or the tail).
    pub(crate) fn clear_tab_drag(&mut self) {
        self.tabs.clear_tab_drag();
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
        let (commit_message, ai_alternatives) = self.commit_editor.snapshot_data();
        let view_mode = self.view_mode.clone();
        let diff_overlay_open = self.diff_overlay_open;
        let sidebar_expansion = self.sidebar_state.expansion();

        self.tabs.store_active_panel_snapshot(TabSnapshot {
            selected_commit_id,
            graph_was_uncommitted,
            diff_state,
            diff_view_mode,
            diff_code_file,
            diff_code_content,
            commit_message,
            ai_alternatives,
            view_mode,
            diff_overlay_open,
            sidebar_expansion,
        });
    }

    /// Restore per-tab UI state saved by [`Self::save_snapshot_to_active_tab`].
    ///
    /// When the snapshot's cached diff still matches the restored graph
    /// selection ([`preserved_tab_diff`], the tab-switch analogue of
    /// `PreservedCommit`), diff is restored from the snapshot, graph-staging
    /// mode is derived via [`Self::sync_status_for_selection`], and the
    /// cascade is skipped so ADR-0001's diff cache stays valid. Otherwise the
    /// Selection Cascade re-derives status and diff from the restored graph
    /// selection and returns the async work the caller must spawn.
    pub(crate) fn restore_snapshot_from_tab(&mut self) -> Option<SelectionEffect> {
        let snapshot = self.tabs.active_panel_snapshot();

        let snap = snapshot?;

        let TabSnapshot {
            selected_commit_id,
            graph_was_uncommitted,
            diff_state,
            diff_view_mode,
            diff_code_file,
            diff_code_content,
            commit_message,
            ai_alternatives,
            view_mode,
            diff_overlay_open,
            sidebar_expansion,
        } = snap;

        self.view_mode = view_mode;
        self.diff_overlay_open = diff_overlay_open;
        self.sidebar_state.apply_expansion(&sidebar_expansion);

        self.restore_graph_selection_from_snapshot(
            selected_commit_id.as_deref(),
            graph_was_uncommitted,
        );
        let sel = self.graph_panel.selection();
        let commit_id_at = match sel {
            GraphSelection::Commit(idx) => self.graph_panel.commit_id_at(idx).map(str::to_string),
            GraphSelection::Uncommitted | GraphSelection::None => None,
        };
        let preserved = preserved_tab_diff(
            diff_state.as_ref(),
            sel,
            commit_id_at.as_deref(),
        );

        self.commit_editor
            .restore_from_snapshot(commit_message, ai_alternatives);

        if preserved {
            self.sync_status_for_selection(sel);
            self.diff_panel.restore_from_snapshot(
                diff_state,
                diff_view_mode,
                diff_code_file,
                diff_code_content,
            );
            self.apply_overlay_file_selection();
            None
        } else {
            Some(self.cascade(sel))
        }
    }

    fn restore_graph_selection_from_snapshot(
        &mut self,
        selected_commit_id: Option<&str>,
        graph_was_uncommitted: bool,
    ) {
        if let Some(commit_id) = selected_commit_id {
            if let Some(idx) = self.graph_panel.find_commit_idx(commit_id) {
                self.graph_panel.select_commit(idx);
            }
        } else if graph_was_uncommitted {
            self.graph_panel.select_uncommitted();
        }
    }

    fn apply_overlay_file_selection(&mut self) {
        if !self.diff_overlay_open {
            return;
        }
        self.diff_panel.set_diff_mode();
        let (selected_file_idx, file_count) = self
            .diff_panel
            .diff_state()
            .map(|d| (d.selected_file_idx, d.file_diffs.len()))
            .unwrap_or((None, 0));
        if let Some(file_idx) = normalized_overlay_file_idx(selected_file_idx, file_count) {
            self.diff_panel.select_file(file_idx);
        }
    }
}

/// Whether [`RepoSession::apply_repo_state_to_panels`] should re-select the
/// graph after rebuilding commit data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RefreshReselectPolicy {
    /// Fetch/refresh path: preserve or fall back using the current graph
    /// selection captured before the rebuild.
    Reselect,
    /// Tab-switch path: rebuild graph data only; selection is restored from
    /// [`TabSnapshot`] by [`RepoSession::restore_snapshot_from_tab`].
    DeferToSnapshot,
}

/// Tab-switch analogue of `PreservedCommit` (ADR-0003): the snapshot's cached
/// diff is still valid for the restored graph selection, so restore may skip
/// the cascade and keep ADR-0001's diff cache intact.
pub(crate) fn preserved_tab_diff(
    diff_state: Option<&CommitDiffState>,
    sel: GraphSelection,
    selected_commit_id: Option<&str>,
) -> bool {
    match sel {
        GraphSelection::Commit(_) => diff_state
            .zip(selected_commit_id)
            .is_some_and(|(diff, id)| diff.commit_id == id),
        GraphSelection::Uncommitted | GraphSelection::None => false,
    }
}

/// Normalize the overlay's selected file index against the current file list.
pub(crate) fn normalized_overlay_file_idx(
    selected_file_idx: Option<usize>,
    file_count: usize,
) -> Option<usize> {
    if file_count == 0 {
        return None;
    }
    Some(
        selected_file_idx
            .filter(|idx| *idx < file_count)
            .unwrap_or(0),
    )
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
        session.tabs.open_repo_tabs.push(tab);
        session.tabs.active_repo_tab_id = Some(id);
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
            session.tabs.open_repo_tabs.push(fake_tab(1, false, true));
            session.tabs.active_repo_tab_id = None;
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
            s.apply_repo_state_to_panels(&repo_state, RefreshReselectPolicy::Reselect);

            let diff = s
                .diff_panel
                .diff_state()
                .expect("PreservedCommit must keep the cached diff intact");
            assert_eq!(diff.commit_id, SAMPLE_COMMIT_ID);
            assert_eq!(s.graph_panel.selection(), GraphSelection::Commit(0));
            assert!(!s.status_panel.is_graph_staging());
        });
    }

    #[gpui::test]
    fn tab_restore_preserves_cached_diff_when_preserved(cx: &mut TestAppContext) {
        use std::sync::Arc;
        use parking_lot::Mutex;

        cx.update(|app| {
            let mut s = one_commit_session(app);
            s.view_mode = MainViewMode::CommitHistory;
            s.graph_panel.select_commit(0);
            s.diff_panel
                .set_diff(CommitDiffState::new(SAMPLE_COMMIT_ID.into(), vec![], None));

            let snapshot = TabSnapshot {
                selected_commit_id: Some(SAMPLE_COMMIT_ID.into()),
                graph_was_uncommitted: false,
                diff_state: s.diff_panel.diff_state().cloned(),
                diff_view_mode: s.diff_panel.view_mode(),
                diff_code_file: None,
                diff_code_content: None,
                commit_message: String::new(),
                ai_alternatives: vec![],
                view_mode: MainViewMode::CommitHistory,
                diff_overlay_open: false,
                sidebar_expansion: s.sidebar_state.expansion(),
            };

            // Simulate defer-reselect leaving panels without selection/diff.
            s.diff_panel.clear();
            s.graph_panel.clear_selection();

            s.tabs.open_repo_tabs.push(OpenRepoTab {
                id: 1,
                path: PathBuf::from("/tmp/test-repo"),
                repo: Arc::new(Mutex::new(None)),
                repo_state: None,
                loading: false,
                last_error: None,
                panel_snapshot: Some(snapshot),
                pull_requests: vec![],
                pull_requests_loading: false,
                pull_requests_loaded: false,
            });
            s.tabs.active_repo_tab_id = Some(1);
            s.tabs.next_repo_tab_id = 2;

            let effect = s.restore_snapshot_from_tab();
            assert_eq!(effect, None);
            let diff = s
                .diff_panel
                .diff_state()
                .expect("PreservedTab must keep the cached diff intact");
            assert_eq!(diff.commit_id, SAMPLE_COMMIT_ID);
            assert_eq!(s.graph_panel.selection(), GraphSelection::Commit(0));
            assert!(!s.status_panel.is_graph_staging());
        });
    }

    #[gpui::test]
    fn tab_restore_derives_graph_staging_for_uncommitted(cx: &mut TestAppContext) {
        use std::sync::Arc;
        use parking_lot::Mutex;

        cx.update(|app| {
            let mut s = one_commit_session(app);
            let snapshot = TabSnapshot {
                selected_commit_id: None,
                graph_was_uncommitted: true,
                diff_state: None,
                diff_view_mode: s.diff_panel.view_mode(),
                diff_code_file: None,
                diff_code_content: None,
                commit_message: String::new(),
                ai_alternatives: vec![],
                view_mode: MainViewMode::CommitHistory,
                diff_overlay_open: false,
                sidebar_expansion: s.sidebar_state.expansion(),
            };

            s.tabs.open_repo_tabs.push(OpenRepoTab {
                id: 1,
                path: PathBuf::from("/tmp/test-repo"),
                repo: Arc::new(Mutex::new(None)),
                repo_state: None,
                loading: false,
                last_error: None,
                panel_snapshot: Some(snapshot),
                pull_requests: vec![],
                pull_requests_loading: false,
                pull_requests_loaded: false,
            });
            s.tabs.active_repo_tab_id = Some(1);

            let effect = s.restore_snapshot_from_tab();
            assert_eq!(effect, Some(SelectionEffect::ClearDiff));
            assert_eq!(s.graph_panel.selection(), GraphSelection::Uncommitted);
            assert!(s.status_panel.is_graph_staging());
        });
    }

    #[gpui::test]
    fn defer_reselect_does_not_leak_outgoing_graph_staging(cx: &mut TestAppContext) {
        cx.update(|app| {
            let mut s = one_commit_session(app);
            s.view_mode = MainViewMode::CommitHistory;
            s.status_panel.enter_graph_staging();
            assert!(s.status_panel.is_graph_staging());

            let repo_state = repo_state_with_commits(vec![sample_commit(SAMPLE_COMMIT_ID)]);
            s.apply_repo_state_to_panels(&repo_state, RefreshReselectPolicy::DeferToSnapshot);

            assert_eq!(s.graph_panel.selection(), GraphSelection::None);
            assert!(!s.status_panel.is_graph_staging());
        });
    }

    #[gpui::test]
    fn tab_restore_cascades_when_diff_not_preserved(cx: &mut TestAppContext) {
        use std::sync::Arc;
        use parking_lot::Mutex;

        cx.update(|app| {
            let mut s = one_commit_session(app);
            s.view_mode = MainViewMode::CommitHistory;
            s.graph_panel.select_commit(0);
            s.diff_panel
                .set_diff(CommitDiffState::new(SAMPLE_COMMIT_ID.into(), vec![], None));

            let snapshot = TabSnapshot {
                selected_commit_id: Some(SAMPLE_COMMIT_ID.into()),
                graph_was_uncommitted: false,
                diff_state: None,
                diff_view_mode: s.diff_panel.view_mode(),
                diff_code_file: None,
                diff_code_content: None,
                commit_message: String::new(),
                ai_alternatives: vec![],
                view_mode: MainViewMode::CommitHistory,
                diff_overlay_open: false,
                sidebar_expansion: s.sidebar_state.expansion(),
            };

            s.tabs.open_repo_tabs.push(OpenRepoTab {
                id: 1,
                path: PathBuf::from("/tmp/test-repo"),
                repo: Arc::new(Mutex::new(None)),
                repo_state: None,
                loading: false,
                last_error: None,
                panel_snapshot: Some(snapshot),
                pull_requests: vec![],
                pull_requests_loading: false,
                pull_requests_loaded: false,
            });
            s.tabs.active_repo_tab_id = Some(1);

            let effect = s.restore_snapshot_from_tab();
            assert_eq!(effect, Some(SelectionEffect::LoadDiffForSelected));
            assert!(s.diff_panel.diff_state().is_none());
        });
    }
}

#[cfg(test)]
mod snapshot_tests {
    use std::collections::HashSet;

    use super::{
        GraphSelection, TabSnapshot, normalized_overlay_file_idx, preserved_tab_diff,
    };
    use super::super::app::MainViewMode;
    use super::super::diff_panel::CommitDiffState;
    use super::super::diff_viewer::DiffViewMode;
    use super::super::sidebar::SidebarExpansion;

    fn empty_snapshot(diff_state: Option<CommitDiffState>) -> TabSnapshot {
        TabSnapshot {
            selected_commit_id: None,
            graph_was_uncommitted: false,
            diff_state,
            diff_view_mode: DiffViewMode::Diff,
            diff_code_file: None,
            diff_code_content: None,
            commit_message: String::new(),
            ai_alternatives: vec![],
            view_mode: MainViewMode::CommitHistory,
            diff_overlay_open: false,
            sidebar_expansion: SidebarExpansion {
                branches: true,
                remotes: true,
                tags: true,
                worktrees: true,
                pull_requests: true,
                expanded_remotes: HashSet::new(),
            },
        }
    }

    #[test]
    fn preserved_tab_diff_true_when_commit_and_diff_match() {
        let snap = empty_snapshot(Some(CommitDiffState::new(
            "abc".into(),
            vec![],
            None,
        )));
        assert!(preserved_tab_diff(
            snap.diff_state.as_ref(),
            GraphSelection::Commit(0),
            Some("abc"),
        ));
    }

    #[test]
    fn preserved_tab_diff_false_when_diff_missing_or_mismatched() {
        let snap = empty_snapshot(Some(CommitDiffState::new(
            "abc".into(),
            vec![],
            None,
        )));
        assert!(!preserved_tab_diff(snap.diff_state.as_ref(), GraphSelection::Commit(0), Some("def")));
        assert!(!preserved_tab_diff(
            empty_snapshot(None).diff_state.as_ref(),
            GraphSelection::Commit(0),
            Some("abc"),
        ));
        assert!(!preserved_tab_diff(
            snap.diff_state.as_ref(),
            GraphSelection::Uncommitted,
            None,
        ));
    }

    #[test]
    fn overlay_file_idx_none_when_commit_has_no_files() {
        assert_eq!(normalized_overlay_file_idx(None, 0), None);
        assert_eq!(normalized_overlay_file_idx(Some(3), 0), None);
    }

    #[test]
    fn overlay_file_idx_keeps_valid_selection() {
        assert_eq!(normalized_overlay_file_idx(Some(2), 3), Some(2));
    }

    #[test]
    fn overlay_file_idx_falls_back_to_first_file() {
        assert_eq!(normalized_overlay_file_idx(None, 3), Some(0));
        assert_eq!(normalized_overlay_file_idx(Some(9), 3), Some(0));
    }
}
