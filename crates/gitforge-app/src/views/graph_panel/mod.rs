mod model;
mod render;

pub use model::GraphSelection;

use gitforge_git::{CommitInfo, RefInfo};
use gitforge_graph::Graph;
use gpui::UniformListScrollHandle;

use model::GraphPanelModel;

/// Commit graph panel: GPUI-free state in [`GraphPanelModel`], rendering in [`render`].
pub struct GraphPanel {
    model: GraphPanelModel,
    pub(crate) scroll_handle: UniformListScrollHandle,
}

impl GraphPanel {
    pub fn new() -> Self {
        Self {
            model: GraphPanelModel::new(),
            scroll_handle: UniformListScrollHandle::default(),
        }
    }

    pub(crate) fn model(&self) -> &GraphPanelModel {
        &self.model
    }

    pub fn set_data(
        &mut self,
        commits: Vec<CommitInfo>,
        references: Vec<RefInfo>,
        graph: Graph,
        has_uncommitted: bool,
        detached_head_commit: Option<String>,
    ) {
        self.model.set_data(
            commits,
            references,
            graph,
            has_uncommitted,
            detached_head_commit,
        );
    }

    pub fn set_branch_filter(&mut self, branch: Option<String>) {
        self.model.set_branch_filter(branch);
    }

    pub fn selection(&self) -> GraphSelection {
        self.model.selection()
    }

    pub fn is_uncommitted_selected(&self) -> bool {
        self.model.is_uncommitted_selected()
    }

    pub fn selected_commit_idx(&self) -> Option<usize> {
        self.model.selected_commit_idx()
    }

    pub fn selected_idx(&self) -> Option<usize> {
        self.model.selected_commit_idx()
    }

    pub fn clear_selection(&mut self) {
        self.model.clear_selection();
    }

    pub fn select_uncommitted(&mut self) {
        self.model.select_uncommitted();
    }

    pub fn select_commit(&mut self, idx: usize) {
        self.model.select_commit(idx);
    }

    pub fn select_prev(&mut self) -> bool {
        self.model.select_delta(-1)
    }

    pub fn select_next(&mut self) -> bool {
        self.model.select_delta(1)
    }

    pub fn commit_id_at(&self, idx: usize) -> Option<&str> {
        self.model.commit_id_at(idx)
    }

    pub fn find_commit_idx(&self, commit_id: &str) -> Option<usize> {
        self.model.find_commit_idx(commit_id)
    }

    pub(crate) fn start_column_resize(
        &mut self,
        column: model::HistoryColumn,
        start_x: f32,
    ) {
        self.model.start_column_resize(column, start_x);
    }

    pub(crate) fn update_column_resize(&mut self, current_x: f32) -> bool {
        self.model.update_column_resize(current_x)
    }

    pub(crate) fn finish_column_resize(&mut self) -> bool {
        self.model.finish_column_resize()
    }

    pub fn render(
        &self,
        colors: &gitforge_ui::AppColors,
        show_graph_col: bool,
        show_sha_col: bool,
        show_time_col: bool,
        show_author_col: bool,
        entity: gpui::WeakEntity<crate::views::app::GitForgeApp>,
    ) -> gpui::Div {
        render::render_panel(
            self,
            colors,
            show_graph_col,
            show_sha_col,
            show_time_col,
            show_author_col,
            entity,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gitforge_git::{CommitInfo, RefInfo, RefKind};
    use gitforge_graph::{CommitEntry, Graph};
    use model::HistoryColumn;

    fn sample_commit(id: &str, parents: Vec<&str>) -> CommitInfo {
        CommitInfo {
            id: id.into(),
            short_id: id.get(..7).unwrap_or(id).into(),
            message: "msg".into(),
            summary: "summary".into(),
            author_name: "author".into(),
            author_email: "e".into(),
            author_date: chrono::Utc::now(),
            committer_name: "author".into(),
            committer_email: "e".into(),
            committer_date: chrono::Utc::now(),
            parent_ids: parents.into_iter().map(str::to_string).collect(),
        }
    }

    fn build_graph(commits: &[CommitInfo]) -> Graph {
        let entries: Vec<CommitEntry> = commits
            .iter()
            .map(|c| CommitEntry::new(c.id.clone(), c.parent_ids.clone()))
            .collect();
        Graph::build(&entries)
    }

    #[test]
    fn new_panel_has_no_selection() {
        let panel = GraphPanel::new();
        assert_eq!(panel.selection(), GraphSelection::None);
        assert!(!panel.is_uncommitted_selected());
        assert_eq!(panel.selected_commit_idx(), None);
        assert_eq!(panel.selected_idx(), None);
    }

    #[test]
    fn selected_idx_is_alias_for_selected_commit_idx() {
        let commits = vec![sample_commit("aaa", vec![])];
        let graph = build_graph(&commits);
        let mut panel = GraphPanel::new();
        panel.set_data(commits, vec![], graph, false, None);
        panel.select_commit(0);

        assert_eq!(panel.selected_idx(), panel.selected_commit_idx());
        assert_eq!(panel.selected_idx(), Some(0));
    }

    #[test]
    fn select_next_and_select_prev_delegate_to_model_navigation() {
        let commits = vec![sample_commit("aaa", vec![]), sample_commit("bbb", vec!["aaa"])];
        let graph = build_graph(&commits);
        let mut panel = GraphPanel::new();
        panel.set_data(commits, vec![], graph, false, None);

        assert!(panel.select_next());
        assert_eq!(panel.selection(), GraphSelection::Commit(0));
        assert!(panel.select_next());
        assert_eq!(panel.selection(), GraphSelection::Commit(1));
        assert!(!panel.select_next());

        assert!(panel.select_prev());
        assert_eq!(panel.selection(), GraphSelection::Commit(0));
        assert!(!panel.select_prev());
    }

    #[test]
    fn clear_selection_resets_to_none() {
        let commits = vec![sample_commit("aaa", vec![])];
        let graph = build_graph(&commits);
        let mut panel = GraphPanel::new();
        panel.set_data(commits, vec![], graph, false, None);
        panel.select_commit(0);
        assert_eq!(panel.selection(), GraphSelection::Commit(0));

        panel.clear_selection();
        assert_eq!(panel.selection(), GraphSelection::None);
    }

    #[test]
    fn select_uncommitted_requires_has_uncommitted_flag() {
        let commits = vec![sample_commit("aaa", vec![])];
        let graph = build_graph(&commits);
        let mut panel = GraphPanel::new();
        panel.set_data(commits.clone(), vec![], graph.clone(), false, None);
        panel.select_uncommitted();
        assert!(!panel.is_uncommitted_selected());

        panel.set_data(commits, vec![], graph, true, None);
        panel.select_uncommitted();
        assert!(panel.is_uncommitted_selected());
    }

    #[test]
    fn commit_id_at_and_find_commit_idx_delegate_to_model() {
        let commits = vec![sample_commit("aaa", vec![]), sample_commit("bbb", vec!["aaa"])];
        let graph = build_graph(&commits);
        let mut panel = GraphPanel::new();
        panel.set_data(commits, vec![], graph, false, None);

        assert_eq!(panel.commit_id_at(0), Some("aaa"));
        assert_eq!(panel.commit_id_at(1), Some("bbb"));
        assert_eq!(panel.commit_id_at(2), None);
        assert_eq!(panel.find_commit_idx("bbb"), Some(1));
        assert_eq!(panel.find_commit_idx("nope"), None);
    }

    #[test]
    fn set_branch_filter_clears_current_selection() {
        let c0 = sample_commit("aaa", vec![]);
        let c1 = sample_commit("bbb", vec!["aaa"]);
        let commits = vec![c0, c1];
        let graph = build_graph(&commits);
        let refs = vec![RefInfo {
            name: "main".into(),
            kind: RefKind::Branch,
            target_commit_id: "bbb".into(),
            is_head: true,
            remote_name: None,
            commits_ahead: 0,
            commits_behind: 0,
        }];
        let mut panel = GraphPanel::new();
        panel.set_data(commits, refs, graph, false, None);
        panel.select_commit(1);
        assert_eq!(panel.selection(), GraphSelection::Commit(1));

        panel.set_branch_filter(Some("main".into()));
        assert_eq!(panel.selection(), GraphSelection::None);
    }

    #[test]
    fn column_resize_lifecycle_delegates_to_model() {
        let mut panel = GraphPanel::new();
        assert!(!panel.finish_column_resize());

        panel.start_column_resize(HistoryColumn::Sha, 10.0);
        let changed = panel.update_column_resize(60.0);
        assert!(changed);
        assert!(panel.finish_column_resize());
        assert!(!panel.finish_column_resize());
    }
}
