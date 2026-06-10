use crate::lines::{CommitLine, GraphRow, LineGraphBuilder};
use crate::types::*;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Graph {
    nodes: Vec<GraphNode>,
    lines: Vec<CommitLine>,
    rows: Vec<GraphRow>,
    commit_to_row: HashMap<CommitId, usize>,
    total_lanes: usize,
}

impl Graph {
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            lines: Vec::new(),
            rows: Vec::new(),
            commit_to_row: HashMap::new(),
            total_lanes: 0,
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

        graph
    }

    pub fn nodes(&self) -> &[GraphNode] {
        &self.nodes
    }

    pub fn lines(&self) -> &[CommitLine] {
        &self.lines
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
}
