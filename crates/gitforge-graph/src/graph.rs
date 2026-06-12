use crate::lines::{CommitLine, GraphRow, LineGraphBuilder};
use crate::types::*;
use std::collections::HashMap;
use std::ops::Range;

#[derive(Debug, Clone)]
pub struct Graph {
    nodes: Vec<GraphNode>,
    lines: Vec<CommitLine>,
    rows: Vec<GraphRow>,
    commit_to_row: HashMap<CommitId, usize>,
    total_lanes: usize,
    visible_lines_by_row: Vec<Vec<usize>>,
}

impl Graph {
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            lines: Vec::new(),
            rows: Vec::new(),
            commit_to_row: HashMap::new(),
            total_lanes: 0,
            visible_lines_by_row: Vec::new(),
        }
    }

    pub fn build(commits: &[CommitEntry]) -> Self {
        let mut builder = LineGraphBuilder::new();
        builder.build(commits);

        let mut graph = Self::new();
        graph.total_lanes = builder.total_lanes();
        graph.lines = builder.lines;
        graph.rows = builder.rows;

        for (row, row_data) in graph.rows.iter().enumerate() {
            let mut node = GraphNode::new(row_data.commit_id.clone(), row, row_data.lane);
            node.is_merge = row_data.is_merge;
            graph.commit_to_row.insert(row_data.commit_id.clone(), row);
            graph.nodes.push(node);
        }

        graph.rebuild_visible_line_index();

        graph
    }

    fn rebuild_visible_line_index(&mut self) {
        self.visible_lines_by_row = vec![Vec::new(); self.nodes.len()];

        if self.visible_lines_by_row.is_empty() {
            return;
        }

        let max_row = self.visible_lines_by_row.len() - 1;
        for (line_idx, line) in self.lines.iter().enumerate() {
            let start = line.full_interval.start.min(max_row);
            let end = line.full_interval.end.min(max_row);
            if start > end {
                continue;
            }

            for row in start..=end {
                self.visible_lines_by_row[row].push(line_idx);
            }
        }
    }

    pub fn nodes(&self) -> &[GraphNode] {
        &self.nodes
    }

    pub fn lines(&self) -> &[CommitLine] {
        &self.lines
    }

    pub fn line_at(&self, index: usize) -> Option<&CommitLine> {
        self.lines.get(index)
    }

    pub fn visible_line_indices(&self, rows: Range<usize>) -> Vec<usize> {
        if rows.start >= rows.end || self.visible_lines_by_row.is_empty() {
            return Vec::new();
        }

        let start = rows.start.min(self.visible_lines_by_row.len());
        let end = rows.end.min(self.visible_lines_by_row.len());
        if start >= end {
            return Vec::new();
        }

        let mut line_indices = Vec::new();
        for row_lines in &self.visible_lines_by_row[start..end] {
            line_indices.extend(row_lines.iter().copied());
        }
        line_indices.sort_unstable();
        line_indices.dedup();
        line_indices
    }

    pub fn row_for_commit(&self, commit_id: &str) -> Option<usize> {
        self.commit_to_row.get(commit_id).copied()
    }

    pub fn total_lanes(&self) -> usize {
        self.total_lanes
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
}

#[derive(Debug, Clone)]
pub struct CommitEntry {
    pub id: CommitId,
    pub parent_ids: Vec<CommitId>,
}

impl CommitEntry {
    pub fn new(id: CommitId, parent_ids: Vec<CommitId>) -> Self {
        Self { id, parent_ids }
    }
}

impl Default for Graph {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cid(s: &str) -> CommitId {
        s.to_string()
    }

    fn entry(id: &str, parents: &[&str]) -> CommitEntry {
        CommitEntry::new(cid(id), parents.iter().map(|s| cid(s)).collect())
    }

    #[test]
    fn empty_input() {
        let g = Graph::build(&[]);
        assert!(g.is_empty());
        assert_eq!(g.len(), 0);
        assert_eq!(g.total_lanes(), 0);
        assert!(g.nodes().is_empty());
        assert!(g.lines().is_empty());
    }

    #[test]
    fn single_root_commit() {
        let g = Graph::build(&[entry("a1", &[])]);
        assert_eq!(g.len(), 1);
        let node = &g.nodes()[0];
        assert_eq!(node.commit_id, "a1");
        assert_eq!(node.row, 0);
        assert_eq!(node.lane, 0);
        assert!(!node.is_merge);
    }

    #[test]
    fn linear_history() {
        let commits = vec![entry("c3", &["c2"]), entry("c2", &["c1"]), entry("c1", &[])];
        let g = Graph::build(&commits);
        assert_eq!(g.len(), 3);

        assert_eq!(g.nodes()[0].lane, 0);
        assert_eq!(g.nodes()[1].lane, 0);
        assert_eq!(g.nodes()[2].lane, 0);

        assert!(!g.lines().is_empty());

        assert_eq!(g.row_for_commit("c3"), Some(0));
        assert_eq!(g.row_for_commit("c2"), Some(1));
        assert_eq!(g.row_for_commit("c1"), Some(2));
        assert_eq!(g.row_for_commit("zz"), None);
    }

    #[test]
    fn branch_and_merge() {
        let commits = vec![
            entry("m", &["b1", "b2"]),
            entry("b2", &["base"]),
            entry("b1", &["base"]),
            entry("base", &[]),
        ];
        let g = Graph::build(&commits);
        assert_eq!(g.len(), 4);

        let merge_node = &g.nodes()[0];
        assert!(merge_node.is_merge);
        assert!(g.lines().iter().any(|l| l.segments.len() > 1));
    }

    #[test]
    fn merge_creates_two_lanes() {
        let commits = vec![
            entry("m", &["left", "right"]),
            entry("right", &["root"]),
            entry("left", &["root"]),
            entry("root", &[]),
        ];
        let g = Graph::build(&commits);
        assert!(g.total_lanes() >= 2);

        let left_lane = g.nodes()[2].lane;
        let right_lane = g.nodes()[1].lane;
        assert_ne!(left_lane, right_lane);
    }

    #[test]
    fn octopus_merge() {
        let commits = vec![
            entry("oct", &["p1", "p2", "p3"]),
            entry("p3", &["root"]),
            entry("p2", &["root"]),
            entry("p1", &["root"]),
            entry("root", &[]),
        ];
        let g = Graph::build(&commits);
        let oct_node = &g.nodes()[0];
        assert!(oct_node.is_merge);
        assert!(g.lines().len() >= 3);
    }

    #[test]
    fn lane_reuse_after_lane_freed() {
        let commits = vec![
            entry("m", &["b1", "b2"]),
            entry("b2", &["root"]),
            entry("b1", &["root"]),
            entry("root", &[]),
        ];
        let g = Graph::build(&commits);
        assert!(g.total_lanes() <= 3);
    }

    #[test]
    fn multiple_roots() {
        let commits = vec![entry("r1", &[]), entry("r2", &[])];
        let g = Graph::build(&commits);
        assert_eq!(g.len(), 2);
    }

    #[test]
    fn default_trait() {
        let g = Graph::default();
        assert!(g.is_empty());
    }

    #[test]
    fn visible_line_indices_empty_graph_returns_empty() {
        let g = Graph::build(&[]);
        assert!(g.visible_line_indices(0..10).is_empty());
    }

    #[test]
    fn visible_line_indices_linear_history_returns_only_intersecting_lines() {
        let commits = vec![
            entry("c5", &["c4"]),
            entry("c4", &["c3"]),
            entry("c3", &["c2"]),
            entry("c2", &["c1"]),
            entry("c1", &[]),
        ];
        let g = Graph::build(&commits);

        let visible = g.visible_line_indices(1..3);
        assert!(!visible.is_empty());
        assert!(visible.iter().all(|idx| {
            let line = g.line_at(*idx).unwrap();
            line.full_interval.start < 3 && line.full_interval.end >= 1
        }));

        let offscreen = g
            .lines()
            .iter()
            .enumerate()
            .filter(|(_, line)| line.full_interval.end < 1 || line.full_interval.start >= 3)
            .map(|(idx, _)| idx)
            .collect::<Vec<_>>();
        assert!(offscreen.iter().all(|idx| !visible.contains(idx)));
    }

    #[test]
    fn visible_line_indices_merge_history_deduplicates_lines() {
        let commits = vec![
            entry("m", &["left", "right"]),
            entry("right", &["root"]),
            entry("left", &["root"]),
            entry("root", &[]),
        ];
        let g = Graph::build(&commits);

        let visible = g.visible_line_indices(0..g.len());
        let mut sorted = visible.clone();
        sorted.sort_unstable();
        sorted.dedup();

        assert_eq!(visible, sorted);
        assert_eq!(visible.len(), g.lines().len());
    }

    #[test]
    fn visible_line_indices_clamps_out_of_bounds_ranges() {
        let commits = vec![entry("c2", &["c1"]), entry("c1", &[])];
        let g = Graph::build(&commits);

        assert_eq!(g.visible_line_indices(0..usize::MAX), vec![0]);
        assert!(
            g.visible_line_indices(usize::MAX - 1..usize::MAX)
                .is_empty()
        );
    }
}
