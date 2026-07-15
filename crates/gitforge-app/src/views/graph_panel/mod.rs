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

impl Default for GraphPanel {
    fn default() -> Self {
        Self::new()
    }
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

    pub(crate) fn clear_selection(&mut self) {
        self.model.clear_selection();
    }

    pub(crate) fn select_uncommitted(&mut self) {
        self.model.select_uncommitted();
    }

    pub(crate) fn select_commit(&mut self, idx: usize) {
        self.model.select_commit(idx);
    }

    pub fn propose_delta(&self, delta: isize) -> Option<GraphSelection> {
        self.model.propose_delta(delta)
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
