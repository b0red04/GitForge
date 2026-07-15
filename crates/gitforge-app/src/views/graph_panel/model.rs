use gitforge_git::{CommitInfo, RefInfo};
use gitforge_graph::Graph;
use std::collections::HashMap;
use std::sync::Arc;

use crate::views::layout::{self, AUTHOR_COL, HASH_COL, TIME_COL};

pub const GRAPH_COL_MIN: f32 = 80.0;
pub const GRAPH_COL_MAX: f32 = 1200.0;
pub const HASH_COL_MIN: f32 = 48.0;
pub const HASH_COL_MAX: f32 = 140.0;
pub const TIME_COL_MIN: f32 = 70.0;
pub const TIME_COL_MAX: f32 = 160.0;
pub const AUTHOR_COL_MIN: f32 = 60.0;
pub const AUTHOR_COL_MAX: f32 = 200.0;
pub(crate) const LEFT_PADDING: f32 = 12.0;
pub(crate) const LANE_WIDTH: f32 = 16.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphSelection {
    None,
    Uncommitted,
    Commit(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HistoryColumn {
    Graph,
    Sha,
    Time,
    Author,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct HistoryColumnResize {
    pub column: HistoryColumn,
    pub start_x: f32,
    pub start_width: f32,
}

#[derive(Clone)]
pub(crate) struct CommitRowRenderData {
    pub summary: String,
    pub short_id: String,
    pub author_name: String,
    pub relative_time: String,
}

/// GPUI-free graph panel state: commits, selection, branch filter, column widths.
pub struct GraphPanelModel {
    pub(crate) commits: Arc<[CommitInfo]>,
    pub(crate) row_render_data: Arc<[CommitRowRenderData]>,
    pub(crate) references: Arc<[RefInfo]>,
    pub(crate) graph: Arc<Graph>,
    selection: GraphSelection,
    pub(crate) has_uncommitted: bool,
    branch_filter: Option<String>,
    /// Indices into `commits` shown in the list when a branch filter is active.
    pub(crate) visible_indices: Vec<usize>,
    pub(crate) use_filtered: bool,
    commit_index: HashMap<String, usize>,
    pub(crate) refs_by_commit: Arc<HashMap<String, Arc<[RefInfo]>>>,
    pub(crate) detached_head_commit: Option<String>,
    pub(crate) graph_col_width: f32,
    graph_col_user_resized: bool,
    pub(crate) hash_col_width: f32,
    pub(crate) time_col_width: f32,
    pub(crate) author_col_width: f32,
    active_resize: Option<HistoryColumnResize>,
}

impl Default for GraphPanelModel {
    fn default() -> Self {
        Self::new()
    }
}

impl GraphPanelModel {
    pub fn new() -> Self {
        Self {
            commits: Arc::from([]),
            row_render_data: Arc::from([]),
            references: Arc::from([]),
            graph: Arc::new(Graph::new()),
            selection: GraphSelection::None,
            has_uncommitted: false,
            branch_filter: None,
            visible_indices: Vec::new(),
            use_filtered: false,
            commit_index: HashMap::new(),
            refs_by_commit: Arc::new(HashMap::new()),
            detached_head_commit: None,
            graph_col_width: layout::GRAPH_LANE_WIDTH,
            graph_col_user_resized: false,
            hash_col_width: HASH_COL,
            time_col_width: TIME_COL,
            author_col_width: AUTHOR_COL,
            active_resize: None,
        }
    }

    pub fn set_data(
        &mut self,
        commits: Vec<CommitInfo>,
        references: Vec<RefInfo>,
        graph: Graph,
        has_uncommitted: bool,
        detached_head_commit: Option<String>,
    ) {
        self.commit_index.clear();
        for (i, c) in commits.iter().enumerate() {
            self.commit_index.insert(c.id.clone(), i);
            self.commit_index.insert(c.short_id.clone(), i);
        }
        self.row_render_data = commits
            .iter()
            .map(|commit| CommitRowRenderData {
                summary: commit.summary.clone(),
                short_id: commit.short_id.clone(),
                author_name: commit.author_name.clone(),
                relative_time: format_relative_time(&commit.author_date),
            })
            .collect::<Vec<_>>()
            .into();
        self.refs_by_commit = Arc::new(build_refs_by_commit(&references));
        if !self.graph_col_user_resized {
            self.graph_col_width = auto_graph_col_width(&graph);
        }

        self.commits = commits.into();
        self.references = references.into();
        self.graph = Arc::new(graph);
        self.has_uncommitted = has_uncommitted;
        self.detached_head_commit = detached_head_commit;
        self.selection = GraphSelection::None;
        self.refresh_visible_indices();
    }

    fn refresh_visible_indices(&mut self) {
        self.visible_indices.clear();
        self.use_filtered = false;

        let Some(ref branch_name) = self.branch_filter else {
            return;
        };

        let target_ref = self.references.iter().find(|r| r.name == *branch_name);
        let Some(target) = target_ref else {
            return;
        };

        let target_id = &target.target_commit_id;
        let mut reachable = std::collections::HashSet::new();
        let mut queue = vec![target_id.clone()];

        while let Some(id) = queue.pop() {
            if reachable.contains(&id) {
                continue;
            }
            reachable.insert(id.clone());
            if let Some(&idx) = self.commit_index.get(&id)
                && let Some(commit) = self.commits.get(idx) {
                    for pid in &commit.parent_ids {
                        queue.push(pid.clone());
                    }
                }
        }

        for (idx, commit) in self.commits.iter().enumerate() {
            if reachable.contains(&commit.id) {
                self.visible_indices.push(idx);
            }
        }

        self.use_filtered = !self.visible_indices.is_empty();
    }

    pub fn set_branch_filter(&mut self, branch: Option<String>) {
        self.branch_filter = branch;
        self.clear_selection();
        self.refresh_visible_indices();
    }

    pub fn selection(&self) -> GraphSelection {
        self.selection
    }

    pub fn is_uncommitted_selected(&self) -> bool {
        self.selection == GraphSelection::Uncommitted
    }

    pub fn selected_commit_idx(&self) -> Option<usize> {
        match self.selection {
            GraphSelection::Commit(idx) => Some(idx),
            _ => None,
        }
    }

    pub fn clear_selection(&mut self) {
        self.selection = GraphSelection::None;
    }

    pub fn select_uncommitted(&mut self) {
        if self.has_uncommitted {
            self.selection = GraphSelection::Uncommitted;
        }
    }

    pub fn select_commit(&mut self, idx: usize) {
        if idx < self.commits.len() {
            self.selection = GraphSelection::Commit(idx);
        }
    }

    /// Compute the selection after `delta` without mutating state.
    pub fn propose_delta(&self, delta: isize) -> Option<GraphSelection> {
        if self.use_filtered {
            selection_after_delta_filtered(
                self.selection,
                delta,
                self.has_uncommitted,
                &self.visible_indices,
            )
        } else {
            selection_after_delta_contiguous(
                self.selection,
                delta,
                self.has_uncommitted,
                self.commits.len(),
            )
        }
    }

    pub fn commit_id_at(&self, idx: usize) -> Option<&str> {
        self.commits.get(idx).map(|c| c.id.as_str())
    }

    pub fn find_commit_idx(&self, commit_id: &str) -> Option<usize> {
        self.commits.iter().position(|c| c.id == commit_id)
    }

    /// Number of commit rows shown in the virtual list (excludes uncommitted row).
    pub fn visible_commit_count(&self) -> usize {
        if self.use_filtered {
            self.visible_indices.len()
        } else {
            self.commits.len()
        }
    }

    /// Total virtual-list items including the optional uncommitted row.
    pub fn total_list_items(&self) -> usize {
        self.visible_commit_count() + usize::from(self.has_uncommitted)
    }

    pub(crate) fn start_column_resize(&mut self, column: HistoryColumn, start_x: f32) {
        let start_width = match column {
            HistoryColumn::Graph => self.graph_col_width,
            HistoryColumn::Sha => self.hash_col_width,
            HistoryColumn::Time => self.time_col_width,
            HistoryColumn::Author => self.author_col_width,
        };
        self.active_resize = Some(HistoryColumnResize {
            column,
            start_x,
            start_width,
        });
    }

    pub(crate) fn update_column_resize(&mut self, current_x: f32) -> bool {
        let Some(active_resize) = self.active_resize else {
            return false;
        };

        let delta = current_x - active_resize.start_x;
        let (target, min, max) = match active_resize.column {
            HistoryColumn::Graph => (
                &mut self.graph_col_width,
                GRAPH_COL_MIN,
                GRAPH_COL_MAX.max(active_resize.start_width),
            ),
            HistoryColumn::Sha => (&mut self.hash_col_width, HASH_COL_MIN, HASH_COL_MAX),
            HistoryColumn::Time => (&mut self.time_col_width, TIME_COL_MIN, TIME_COL_MAX),
            HistoryColumn::Author => (&mut self.author_col_width, AUTHOR_COL_MIN, AUTHOR_COL_MAX),
        };
        let signed_delta = match active_resize.column {
            HistoryColumn::Time | HistoryColumn::Author => -delta,
            HistoryColumn::Graph | HistoryColumn::Sha => delta,
        };
        let next_width = (active_resize.start_width + signed_delta).clamp(min, max);

        if (*target - next_width).abs() < f32::EPSILON {
            return false;
        }

        if active_resize.column == HistoryColumn::Graph {
            self.graph_col_user_resized = true;
        }

        *target = next_width;
        true
    }

    pub(crate) fn finish_column_resize(&mut self) -> bool {
        self.active_resize.take().is_some()
    }
}

/// Map a virtual-list row index to a commit index in the full commit list.
pub(crate) fn commit_idx_for_list_row_with(
    has_uncommitted: bool,
    use_filtered: bool,
    visible_indices: &[usize],
    commit_count: usize,
    list_row: usize,
) -> Option<usize> {
    if has_uncommitted && list_row == 0 {
        return None;
    }
    let display_idx = list_row.saturating_sub(usize::from(has_uncommitted));
    if use_filtered {
        visible_indices.get(display_idx).copied()
    } else if display_idx < commit_count {
        Some(display_idx)
    } else {
        None
    }
}

/// Pure selection navigation within a branch-filtered visible set.
pub fn selection_after_delta_filtered(
    selection: GraphSelection,
    delta: isize,
    has_uncommitted: bool,
    filtered_indices: &[usize],
) -> Option<GraphSelection> {
    if filtered_indices.is_empty() && !has_uncommitted {
        return None;
    }

    match (selection, delta) {
        (GraphSelection::None, 1) => {
            if has_uncommitted {
                Some(GraphSelection::Uncommitted)
            } else if !filtered_indices.is_empty() {
                Some(GraphSelection::Commit(filtered_indices[0]))
            } else {
                None
            }
        }
        (GraphSelection::None, -1) => None,
        (GraphSelection::Uncommitted, 1) => {
            if filtered_indices.is_empty() {
                None
            } else {
                Some(GraphSelection::Commit(filtered_indices[0]))
            }
        }
        (GraphSelection::Uncommitted, -1) => None,
        (GraphSelection::Commit(idx), -1) if filtered_indices.first() == Some(&idx) => {
            if has_uncommitted {
                Some(GraphSelection::Uncommitted)
            } else {
                None
            }
        }
        (GraphSelection::Commit(idx), d) => {
            let pos = filtered_indices.iter().position(|&i| i == idx)?;
            let candidate = pos as isize + d;
            if candidate < 0 || candidate as usize >= filtered_indices.len() {
                None
            } else {
                Some(GraphSelection::Commit(filtered_indices[candidate as usize]))
            }
        }
        _ => None,
    }
}

/// Pure selection navigation over contiguous commit indices (no branch filter).
pub fn selection_after_delta_contiguous(
    selection: GraphSelection,
    delta: isize,
    has_uncommitted: bool,
    commit_count: usize,
) -> Option<GraphSelection> {
    if commit_count == 0 && !has_uncommitted {
        return None;
    }

    match (selection, delta) {
        (GraphSelection::None, 1) => {
            if has_uncommitted {
                Some(GraphSelection::Uncommitted)
            } else if commit_count > 0 {
                Some(GraphSelection::Commit(0))
            } else {
                None
            }
        }
        (GraphSelection::None, -1) => None,
        (GraphSelection::Uncommitted, 1) => {
            if commit_count == 0 {
                None
            } else {
                Some(GraphSelection::Commit(0))
            }
        }
        (GraphSelection::Uncommitted, -1) => None,
        (GraphSelection::Commit(0), -1) => {
            if has_uncommitted {
                Some(GraphSelection::Uncommitted)
            } else {
                None
            }
        }
        (GraphSelection::Commit(idx), d) => {
            let candidate = idx as isize + d;
            if candidate < 0 || candidate as usize >= commit_count {
                None
            } else {
                Some(GraphSelection::Commit(candidate as usize))
            }
        }
        _ => None,
    }
}

pub(crate) fn build_refs_by_commit(references: &[RefInfo]) -> HashMap<String, Arc<[RefInfo]>> {
    let mut grouped: HashMap<String, Vec<RefInfo>> = HashMap::new();
    for rf in references {
        grouped
            .entry(rf.target_commit_id.clone())
            .or_default()
            .push(rf.clone());
    }

    grouped
        .into_iter()
        .map(|(commit_id, refs)| (commit_id, Arc::from(refs)))
        .collect()
}

pub(crate) fn auto_graph_col_width(graph: &Graph) -> f32 {
    use gitforge_graph::CommitLineSegment;

    let max_node_lane = graph.nodes().iter().map(|node| node.lane).max();
    let max_line_lane = graph.lines().iter().fold(None, |max_lane, line| {
        let line_max = line.segments.iter().fold(
            line.child_column.max(line.color_lane),
            |segment_max, segment| match segment {
                CommitLineSegment::Straight { .. } => segment_max,
                CommitLineSegment::Curve { to_column, .. } => segment_max.max(*to_column),
            },
        );

        Some(max_lane.map_or(line_max, |lane: usize| lane.max(line_max)))
    });

    let max_lane = max_node_lane
        .into_iter()
        .chain(max_line_lane)
        .max()
        .unwrap_or(0);
    let required_width = LEFT_PADDING + (max_lane as f32 + 1.0) * LANE_WIDTH + LEFT_PADDING;

    required_width
        .max(layout::GRAPH_LANE_WIDTH)
        .clamp(GRAPH_COL_MIN, GRAPH_COL_MAX)
}

fn format_relative_time(dt: &chrono::DateTime<chrono::Utc>) -> String {
    let now = chrono::Utc::now();
    let diff = now.signed_duration_since(*dt);
    if diff.num_seconds() < 60 {
        "just now".into()
    } else if diff.num_minutes() < 60 {
        format!("{}m ago", diff.num_minutes())
    } else if diff.num_hours() < 24 {
        format!("{}h ago", diff.num_hours())
    } else if diff.num_days() < 30 {
        format!("{}d ago", diff.num_days())
    } else if diff.num_days() < 365 {
        format!("{}mo ago", diff.num_days() / 30)
    } else {
        format!("{}y ago", diff.num_days() / 365)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gitforge_git::{CommitInfo, RefInfo, RefKind};
    use gitforge_graph::{CommitEntry, Graph};

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
    fn select_delta_down_from_none_with_uncommitted() {
        assert_eq!(
            selection_after_delta_contiguous(GraphSelection::None, 1, true, 2),
            Some(GraphSelection::Uncommitted)
        );
    }

    #[test]
    fn select_delta_uncommitted_to_first_commit() {
        assert_eq!(
            selection_after_delta_contiguous(GraphSelection::Uncommitted, 1, true, 2),
            Some(GraphSelection::Commit(0))
        );
    }

    #[test]
    fn select_delta_filtered_skips_hidden_commits() {
        // visible: commits 0 and 2 only
        let filtered = vec![0, 2];
        assert_eq!(
            selection_after_delta_filtered(GraphSelection::Commit(0), 1, false, &filtered),
            Some(GraphSelection::Commit(2))
        );
        assert_eq!(
            selection_after_delta_filtered(GraphSelection::Commit(2), -1, false, &filtered),
            Some(GraphSelection::Commit(0))
        );
    }

    #[test]
    fn branch_filter_limits_visible_rows() {
        let c0 = sample_commit("aaa", vec![]);
        let c1 = sample_commit("bbb", vec!["aaa"]);
        let c2 = sample_commit("ccc", vec!["bbb"]);
        let commits = vec![c0, c1, c2];
        let graph = build_graph(&commits);
        // Branch tip at bbb — ccc is not reachable, so filtering must shrink the set.
        let refs = vec![RefInfo {
            name: "main".into(),
            kind: RefKind::Branch,
            target_commit_id: "bbb".into(),
            is_head: true,
            remote_name: None,
            commits_ahead: 0,
            commits_behind: 0,
        }];

        let mut model = GraphPanelModel::new();
        model.set_data(commits, refs, graph, false, None);
        assert_eq!(model.visible_commit_count(), 3);

        model.set_branch_filter(Some("main".into()));
        assert_eq!(model.visible_commit_count(), 2);
        assert_eq!(model.visible_indices, vec![0, 1]);
        assert_eq!(
            commit_idx_for_list_row_with(false, model.use_filtered, &model.visible_indices, model.commits.len(), 0),
            Some(0)
        );
        assert_eq!(
            commit_idx_for_list_row_with(false, model.use_filtered, &model.visible_indices, model.commits.len(), 1),
            Some(1)
        );
        assert_eq!(
            commit_idx_for_list_row_with(false, model.use_filtered, &model.visible_indices, model.commits.len(), 2),
            None
        );

        model.set_branch_filter(Some("missing".into()));
        // Unknown branch → filter inactive, all commits visible
        assert_eq!(model.visible_commit_count(), 3);
    }

    #[test]
    fn model_propose_delta_respects_branch_filter() {
        let c0 = sample_commit("aaa", vec![]);
        let c1 = sample_commit("bbb", vec![]);
        let commits = vec![c0, c1];
        let graph = build_graph(&commits);
        let refs = vec![RefInfo {
            name: "only-a".into(),
            kind: RefKind::Branch,
            target_commit_id: "aaa".into(),
            is_head: false,
            remote_name: None,
            commits_ahead: 0,
            commits_behind: 0,
        }];

        let mut model = GraphPanelModel::new();
        model.set_data(commits, refs, graph, false, None);
        model.set_branch_filter(Some("only-a".into()));
        assert_eq!(model.visible_commit_count(), 1);

        model.select_commit(0);
        assert!(model.propose_delta(1).is_none());
        assert!(model.propose_delta(-1).is_none());
    }
}
