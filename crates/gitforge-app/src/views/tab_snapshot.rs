use super::app::MainViewMode;
use super::diff_panel::CommitDiffState;
use super::diff_viewer::DiffViewMode;
use super::graph_panel::{GraphPanel, GraphSelection};
use super::repo_session::{RepoSession, SelectionEffect};
use super::sidebar::SidebarExpansion;

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

impl TabSnapshot {
    /// Capture the active tab's panel state from a [`RepoSession`].
    pub(crate) fn capture(session: &RepoSession) -> Self {
        let selection = session.graph_panel.selection();
        let (commit_message, ai_alternatives) = session.commit_editor.snapshot_data();
        Self {
            selected_commit_id: commit_id_for_graph_selection(&session.graph_panel, selection),
            graph_was_uncommitted: session.graph_panel.is_uncommitted_selected(),
            diff_state: session.diff_panel.diff_state().cloned(),
            diff_view_mode: session.diff_panel.view_mode(),
            diff_code_file: session.diff_panel.code_view_file().map(String::from),
            diff_code_content: session.diff_panel.code_view_content().map(String::from),
            commit_message,
            ai_alternatives,
            view_mode: session.view_mode.clone(),
            diff_overlay_open: session.diff_overlay_open,
            sidebar_expansion: session.sidebar_state.expansion(),
        }
    }

    /// Restore this snapshot into `session`. Returns a [`SelectionEffect`] when
    /// the cascade must re-derive diff from the restored graph selection.
    pub(crate) fn apply_to(self, session: &mut RepoSession) -> Option<SelectionEffect> {
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
        } = self;

        session.view_mode = view_mode;
        session.diff_overlay_open = diff_overlay_open;
        session.sidebar_state.apply_expansion(&sidebar_expansion);

        restore_graph_selection(
            &mut session.graph_panel,
            selected_commit_id.as_deref(),
            graph_was_uncommitted,
        );
        let sel = session.graph_panel.selection();
        let preserved = preserved_tab_diff(
            diff_state.as_ref(),
            sel,
            commit_id_for_graph_selection(&session.graph_panel, sel).as_deref(),
        );

        session
            .commit_editor
            .restore_from_snapshot(commit_message, ai_alternatives);

        if preserved {
            session.sync_status_for_selection(sel);
            session.diff_panel.restore_from_snapshot(
                diff_state,
                diff_view_mode,
                diff_code_file,
                diff_code_content,
            );
            session.sync_overlay_file_selection();
            None
        } else {
            Some(session.cascade(sel))
        }
    }
}

fn commit_id_for_graph_selection(
    graph_panel: &GraphPanel,
    selection: GraphSelection,
) -> Option<String> {
    match selection {
        GraphSelection::Commit(idx) => graph_panel.commit_id_at(idx).map(str::to_string),
        GraphSelection::Uncommitted | GraphSelection::None => None,
    }
}

fn restore_graph_selection(
    graph_panel: &mut GraphPanel,
    selected_commit_id: Option<&str>,
    graph_was_uncommitted: bool,
) {
    if let Some(commit_id) = selected_commit_id
        && let Some(idx) = graph_panel.find_commit_idx(commit_id)
    {
        graph_panel.select_commit(idx);
    } else if graph_was_uncommitted {
        graph_panel.select_uncommitted();
    }
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

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;
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
        assert!(!preserved_tab_diff(
            snap.diff_state.as_ref(),
            GraphSelection::Commit(0),
            Some("def"),
        ));
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

}
