//! Diff overlay policy: per-tab open intent, render eligibility, and file-selection sync.

use super::app::MainViewMode;
use super::repo_session::RepoSession;

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

impl RepoSession {
    /// Per-tab user intent: overlay should be open when applicable (persisted in
    /// [`super::tab_snapshot::TabSnapshot`]).
    pub(crate) fn overlay_intent_open(&self) -> bool {
        self.diff_overlay_open
    }

    /// Whether the large diff overlay should render for the current panel state.
    pub(crate) fn overlay_eligible(&self) -> bool {
        self.diff_overlay_open
            && self.view_mode == MainViewMode::CommitHistory
            && !self.graph_panel.is_uncommitted_selected()
    }

    /// Align diff-panel file selection for the overlay (diff mode + normalized idx).
    /// No-op when overlay intent is not open.
    pub(crate) fn sync_overlay_file_selection(&mut self) {
        if !self.diff_overlay_open {
            return;
        }
        self.diff_panel.set_diff_mode();
        if let Some(diff) = self.diff_panel.diff_state()
            && let Some(file_idx) = normalized_overlay_file_idx(
                diff.selected_file_idx,
                diff.file_diffs.len(),
            )
        {
            self.diff_panel.select_file(file_idx);
        }
    }

    pub(crate) fn open_overlay(&mut self) {
        self.diff_overlay_open = true;
        self.sync_overlay_file_selection();
    }

    pub(crate) fn open_overlay_for_file(&mut self, file_idx: usize) {
        self.diff_overlay_open = true;
        self.diff_panel.select_file(file_idx);
        self.sync_overlay_file_selection();
    }

    /// Toggle overlay intent. Returns whether the overlay is now open.
    pub(crate) fn toggle_overlay(&mut self) -> bool {
        if self.diff_overlay_open {
            self.close_overlay();
            false
        } else {
            self.open_overlay();
            true
        }
    }

    pub(crate) fn close_overlay(&mut self) {
        self.diff_overlay_open = false;
    }

    /// After diff load completes, keep overlay file selection valid if intent is open.
    pub(crate) fn sync_overlay_after_diff_load(&mut self) {
        self.sync_overlay_file_selection();
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use gitforge_diff::types::FileDiff;
    use gitforge_git::CommitInfo;
    use gitforge_graph::Graph;
    use gpui::TestAppContext;

    use super::*;
    use super::super::diff_panel::CommitDiffState;

    const SAMPLE_COMMIT_ID: &str = "abcdef000000000000000000000000000000001";

    fn sample_file_diff(path: &str) -> FileDiff {
        FileDiff {
            old_path: None,
            new_path: Some(path.into()),
            lines: Arc::from([]),
            hunks: vec![],
            is_binary: false,
        }
    }

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

    #[gpui::test]
    fn overlay_eligible_requires_history_view_and_committed_selection(cx: &mut TestAppContext) {
        cx.update(|app| {
            let mut s = one_commit_session(app);
            s.diff_overlay_open = true;
            s.view_mode = MainViewMode::CommitHistory;
            s.graph_panel.select_commit(0);
            assert!(s.overlay_eligible());

            s.view_mode = MainViewMode::Status;
            assert!(!s.overlay_eligible());

            s.view_mode = MainViewMode::CommitHistory;
            s.graph_panel.select_uncommitted();
            assert!(!s.overlay_eligible());
        });
    }

    #[gpui::test]
    fn sync_overlay_file_selection_normalizes_to_first_file(cx: &mut TestAppContext) {
        cx.update(|app| {
            let mut s = one_commit_session(app);
            s.view_mode = MainViewMode::CommitHistory;
            s.graph_panel.select_commit(0);
            s.diff_panel.set_diff(CommitDiffState::new(
                SAMPLE_COMMIT_ID.into(),
                vec![sample_file_diff("a.rs"), sample_file_diff("b.rs")],
                None,
            ));
            s.diff_overlay_open = true;

            s.sync_overlay_file_selection();

            assert_eq!(
                s.diff_panel
                    .diff_state()
                    .and_then(|d| d.selected_file_idx),
                Some(0),
            );
        });
    }

    #[gpui::test]
    fn sync_overlay_file_selection_noop_when_overlay_closed(cx: &mut TestAppContext) {
        cx.update(|app| {
            let mut s = one_commit_session(app);
            s.view_mode = MainViewMode::CommitHistory;
            s.graph_panel.select_commit(0);
            s.diff_panel.set_diff(CommitDiffState::new(
                SAMPLE_COMMIT_ID.into(),
                vec![sample_file_diff("a.rs")],
                None,
            ));
            s.diff_overlay_open = false;

            s.sync_overlay_file_selection();

            assert_eq!(
                s.diff_panel
                    .diff_state()
                    .and_then(|d| d.selected_file_idx),
                None,
            );
        });
    }

    #[gpui::test]
    fn open_overlay_for_file_selects_requested_file(cx: &mut TestAppContext) {
        cx.update(|app| {
            let mut s = one_commit_session(app);
            s.view_mode = MainViewMode::CommitHistory;
            s.graph_panel.select_commit(0);
            s.diff_panel.set_diff(CommitDiffState::new(
                SAMPLE_COMMIT_ID.into(),
                vec![sample_file_diff("a.rs"), sample_file_diff("b.rs")],
                None,
            ));

            s.open_overlay_for_file(1);

            assert!(s.overlay_intent_open());
            assert_eq!(
                s.diff_panel
                    .diff_state()
                    .and_then(|d| d.selected_file_idx),
                Some(1),
            );
        });
    }

    #[gpui::test]
    fn toggle_overlay_opens_and_closes_intent(cx: &mut TestAppContext) {
        cx.update(|app| {
            let mut s = one_commit_session(app);
            s.view_mode = MainViewMode::CommitHistory;
            s.graph_panel.select_commit(0);
            s.diff_panel.set_diff(CommitDiffState::new(
                SAMPLE_COMMIT_ID.into(),
                vec![sample_file_diff("a.rs")],
                None,
            ));

            assert!(s.toggle_overlay());
            assert!(s.overlay_intent_open());

            assert!(!s.toggle_overlay());
            assert!(!s.overlay_intent_open());
        });
    }
}
