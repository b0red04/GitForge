use crate::lines::{CommitLine, GraphRow, LineGraphBuilder};
use crate::types::*;
use std::collections::HashMap;
use std::ops::Range;

const VISIBLE_LINE_TILE_ROWS: usize = 64;

#[derive(Debug, Clone)]
pub struct Graph {
    nodes: Vec<GraphNode>,
    lines: Vec<CommitLine>,
    rows: Vec<GraphRow>,
    commit_to_row: HashMap<CommitId, usize>,
    total_lanes: usize,
    visible_line_tiles: Vec<Vec<usize>>,
}

impl Graph {
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            lines: Vec::new(),
            rows: Vec::new(),
            commit_to_row: HashMap::new(),
            total_lanes: 0,
            visible_line_tiles: Vec::new(),
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
        let num_rows = self.nodes.len();
        let tile_count = num_rows.div_ceil(VISIBLE_LINE_TILE_ROWS);
        self.visible_line_tiles = vec![Vec::new(); tile_count];

        if num_rows == 0 {
            return;
        }

        let max_row = num_rows - 1;
        for (line_idx, line) in self.lines.iter().enumerate() {
            let start = line.full_interval.start.min(max_row);
            let end = line.full_interval.end.min(max_row);
            if start > end {
                continue;
            }

            let start_tile = start / VISIBLE_LINE_TILE_ROWS;
            let end_tile = end / VISIBLE_LINE_TILE_ROWS;
            for tile in start_tile..=end_tile {
                self.visible_line_tiles[tile].push(line_idx);
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
        if rows.start >= rows.end || self.visible_line_tiles.is_empty() {
            return Vec::new();
        }

        let start = rows.start.min(self.nodes.len());
        let end = rows.end.min(self.nodes.len());
        if start >= end {
            return Vec::new();
        }

        let mut line_indices = Vec::new();
        let start_tile = start / VISIBLE_LINE_TILE_ROWS;
        let end_tile = (end - 1) / VISIBLE_LINE_TILE_ROWS;
        for tile_lines in &self.visible_line_tiles[start_tile..=end_tile] {
            line_indices.extend(tile_lines.iter().copied());
        }
        line_indices.sort_unstable();
        line_indices.dedup();
        line_indices.retain(|idx| {
            let line = &self.lines[*idx];
            line.full_interval.start < end && line.full_interval.end >= start
        });
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

    fn linear_entries(count: usize) -> Vec<CommitEntry> {
        (0..count)
            .map(|idx| {
                let id = format!("c{idx}");
                let parents = if idx + 1 < count {
                    vec![format!("c{}", idx + 1)]
                } else {
                    Vec::new()
                };
                CommitEntry::new(id, parents)
            })
            .collect()
    }

    fn branchy_entries(count: usize) -> Vec<CommitEntry> {
        let branch_interval = 10;
        (0..count)
            .map(|idx| {
                let id = format!("c{idx}");
                let parents = if idx % branch_interval == 0
                    && idx + branch_interval < count
                    && idx + 1 < count
                {
                    vec![
                        format!("c{}", idx + 1),
                        format!("c{}", idx + branch_interval),
                    ]
                } else if idx + 1 < count {
                    vec![format!("c{}", idx + 1)]
                } else {
                    Vec::new()
                };
                CommitEntry::new(id, parents)
            })
            .collect()
    }

    fn merge_heavy_entries(count: usize) -> Vec<CommitEntry> {
        (0..count)
            .map(|idx| {
                let id = format!("c{idx}");
                let parents = if idx + 3 < count && idx % 3 == 0 {
                    vec![format!("c{}", idx + 1), format!("c{}", idx + 3)]
                } else if idx + 1 < count {
                    vec![format!("c{}", idx + 1)]
                } else {
                    Vec::new()
                };
                CommitEntry::new(id, parents)
            })
            .collect()
    }

    fn naive_visible_line_indices(graph: &Graph, rows: Range<usize>) -> Vec<usize> {
        if rows.start >= rows.end {
            return Vec::new();
        }

        let start = rows.start.min(graph.len());
        let end = rows.end.min(graph.len());
        if start >= end {
            return Vec::new();
        }

        graph
            .lines()
            .iter()
            .enumerate()
            .filter(|(_, line)| line.full_interval.start < end && line.full_interval.end >= start)
            .map(|(idx, _)| idx)
            .collect()
    }

    fn assert_visible_matches_naive(entries: Vec<CommitEntry>, ranges: &[Range<usize>]) {
        let graph = Graph::build(&entries);
        for rows in ranges {
            assert_eq!(
                graph.visible_line_indices(rows.clone()),
                naive_visible_line_indices(&graph, rows.clone()),
                "visible lines diverged for range {rows:?}",
            );
        }
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

    #[test]
    fn visible_line_indices_matches_naive_for_linear_history() {
        assert_visible_matches_naive(
            linear_entries(150),
            &[0..1, 1..3, 40..80, 63..65, 120..150, 149..190],
        );
    }

    #[test]
    fn visible_line_indices_matches_naive_for_branchy_history() {
        assert_visible_matches_naive(
            branchy_entries(150),
            &[0..40, 32..96, 63..65, 64..128, 100..140],
        );
    }

    #[test]
    fn visible_line_indices_matches_naive_for_merge_heavy_history() {
        assert_visible_matches_naive(
            merge_heavy_entries(150),
            &[0..40, 40..80, 63..65, 80..130, 128..150],
        );
    }

    #[test]
    fn visible_line_indices_matches_naive_for_out_of_bounds_ranges() {
        assert_visible_matches_naive(
            branchy_entries(90),
            &[0..usize::MAX, usize::MAX - 1..usize::MAX, 90..120],
        );
    }
}
