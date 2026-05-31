//! Zed-style commit line segments for a single full-height graph canvas.
//!
//! Adapted from Zed's `git_graph` lane state machine (segment list, not per-row arcs).

use crate::types::CommitId;
use smallvec::{smallvec, SmallVec};
use std::collections::HashMap;
use std::ops::Range;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CurveKind {
    Merge,
    Checkout,
}

#[derive(Debug, Clone)]
pub enum CommitLineSegment {
    Straight {
        to_row: usize,
    },
    Curve {
        to_column: usize,
        on_row: usize,
        curve_kind: CurveKind,
    },
}

#[derive(Debug, Clone)]
pub struct CommitLine {
    pub child_column: usize,
    pub full_interval: Range<usize>,
    pub color_lane: usize,
    pub segments: SmallVec<[CommitLineSegment; 2]>,
}

impl CommitLine {
    pub fn first_visible_segment(&self, first_visible_row: usize) -> Option<(usize, usize)> {
        if first_visible_row > self.full_interval.end {
            return None;
        }
        if first_visible_row <= self.full_interval.start {
            return Some((0, self.child_column));
        }

        let mut current_column = self.child_column;
        for (idx, segment) in self.segments.iter().enumerate() {
            match segment {
                CommitLineSegment::Straight { to_row } => {
                    if *to_row >= first_visible_row {
                        return Some((idx, current_column));
                    }
                }
                CommitLineSegment::Curve {
                    to_column,
                    on_row,
                    ..
                } => {
                    if *on_row >= first_visible_row {
                        return Some((idx, current_column));
                    }
                    current_column = *to_column;
                }
            }
        }
        None
    }
}

#[derive(Debug, Clone)]
pub struct GraphRow {
    pub commit_id: CommitId,
    pub lane: usize,
    pub color_lane: usize,
    pub is_merge: bool,
}

#[derive(Debug)]
enum LaneState {
    Empty,
    Active {
        starting_col: usize,
        starting_row: usize,
        destination_column: Option<usize>,
        segments: SmallVec<[CommitLineSegment; 2]>,
    },
}

impl LaneState {
    fn finalize(
        &mut self,
        ending_row: usize,
        lane_column: usize,
        parent_column: usize,
    ) -> Option<CommitLine> {
        let state = std::mem::replace(self, LaneState::Empty);

        match state {
            LaneState::Active {
                starting_row,
                starting_col,
                destination_column,
                mut segments,
            } => {
                let final_destination = destination_column.unwrap_or(parent_column);

                match segments.last_mut() {
                    Some(CommitLineSegment::Straight { to_row })
                        if *to_row == usize::MAX =>
                    {
                        if final_destination != lane_column {
                            *to_row = ending_row.saturating_sub(1);
                            let curved = CommitLineSegment::Curve {
                                to_column: final_destination,
                                on_row: ending_row,
                                curve_kind: CurveKind::Checkout,
                            };
                            if *to_row == starting_row {
                                let last = segments.len() - 1;
                                segments[last] = curved;
                            } else {
                                segments.push(curved);
                            }
                        } else {
                            *to_row = ending_row;
                        }
                    }
                    Some(CommitLineSegment::Curve {
                        on_row,
                        to_column,
                        curve_kind,
                    }) if *on_row == usize::MAX => {
                        if *to_column == usize::MAX {
                            *to_column = final_destination;
                        }
                        if matches!(curve_kind, CurveKind::Merge) {
                            *on_row = starting_row + 1;
                            if *on_row < ending_row {
                                if *to_column != final_destination {
                                    segments.push(CommitLineSegment::Straight {
                                        to_row: ending_row - 1,
                                    });
                                    segments.push(CommitLineSegment::Curve {
                                        to_column: final_destination,
                                        on_row: ending_row,
                                        curve_kind: CurveKind::Checkout,
                                    });
                                } else {
                                    segments.push(CommitLineSegment::Straight {
                                        to_row: ending_row,
                                    });
                                }
                            } else if *to_column != final_destination {
                                segments.push(CommitLineSegment::Curve {
                                    to_column: final_destination,
                                    on_row: ending_row,
                                    curve_kind: CurveKind::Checkout,
                                });
                            }
                        } else {
                            *on_row = ending_row;
                            if *to_column != final_destination {
                                segments.push(CommitLineSegment::Straight {
                                    to_row: ending_row,
                                });
                                segments.push(CommitLineSegment::Curve {
                                    to_column: final_destination,
                                    on_row: ending_row,
                                    curve_kind: CurveKind::Checkout,
                                });
                            }
                        }
                    }
                    Some(CommitLineSegment::Curve { on_row, to_column, .. }) => {
                        if *on_row < ending_row {
                            if *to_column != final_destination {
                                segments.push(CommitLineSegment::Straight {
                                    to_row: ending_row - 1,
                                });
                                segments.push(CommitLineSegment::Curve {
                                    to_column: final_destination,
                                    on_row: ending_row,
                                    curve_kind: CurveKind::Checkout,
                                });
                            } else {
                                segments.push(CommitLineSegment::Straight {
                                    to_row: ending_row,
                                });
                            }
                        } else if *to_column != final_destination {
                            segments.push(CommitLineSegment::Curve {
                                to_column: final_destination,
                                on_row: ending_row,
                                curve_kind: CurveKind::Checkout,
                            });
                        }
                    }
                    _ => {}
                }

                Some(CommitLine {
                    child_column: starting_col,
                    full_interval: starting_row..ending_row,
                    color_lane: lane_column,
                    segments,
                })
            }
            LaneState::Empty => None,
        }
    }

    fn is_empty(&self) -> bool {
        matches!(self, LaneState::Empty)
    }
}

pub struct LineGraphBuilder {
    lane_states: SmallVec<[LaneState; 8]>,
    parent_to_lanes: HashMap<CommitId, SmallVec<[usize; 2]>>,
    pub rows: Vec<GraphRow>,
    pub lines: Vec<CommitLine>,
    max_lanes: usize,
}

impl LineGraphBuilder {
    pub fn new() -> Self {
        Self {
            lane_states: SmallVec::new(),
            parent_to_lanes: HashMap::new(),
            rows: Vec::new(),
            lines: Vec::new(),
            max_lanes: 0,
        }
    }

    pub fn build(&mut self, commits: &[super::graph::CommitEntry]) {
        self.rows.clear();
        self.lines.clear();
        self.lane_states.clear();
        self.parent_to_lanes.clear();
        self.max_lanes = 0;

        self.rows.reserve(commits.len());
        self.lines.reserve(commits.len() / 2);

        for commit in commits {
            let commit_row = self.rows.len();
            let is_merge = commit.parent_ids.len() > 1;

            let commit_lane = self
                .parent_to_lanes
                .get(&commit.id)
                .and_then(|lanes| lanes.iter().copied().min())
                .unwrap_or_else(|| self.first_empty_lane());

            if let Some(lanes) = self.parent_to_lanes.remove(&commit.id) {
                for lane_column in lanes {
                    let state = &mut self.lane_states[lane_column];

                    if let LaneState::Active {
                        starting_row,
                        segments,
                        ..
                    } = state
                    {
                        if let Some(CommitLineSegment::Curve {
                            to_column,
                            curve_kind: CurveKind::Merge,
                            ..
                        }) = segments.first_mut()
                        {
                            let curve_row = *starting_row + 1;
                            let would_overlap = lane_column != commit_lane
                                && curve_row < commit_row
                                && self.rows[curve_row..commit_row]
                                    .iter()
                                    .any(|c| c.lane == commit_lane);
                            if would_overlap {
                                *to_column = lane_column;
                            }
                        }
                    }

                    if let Some(line) =
                        state.finalize(commit_row, lane_column, commit_lane)
                    {
                        self.lines.push(line);
                    }
                }
            }

            for (parent_idx, parent_id) in commit.parent_ids.iter().enumerate() {
                if parent_idx == 0 {
                    self.lane_states[commit_lane] = LaneState::Active {
                        starting_col: commit_lane,
                        starting_row: commit_row,
                        destination_column: None,
                        segments: smallvec![CommitLineSegment::Straight {
                            to_row: usize::MAX
                        }],
                    };
                    self.parent_to_lanes
                        .entry(parent_id.clone())
                        .or_default()
                        .push(commit_lane);
                } else {
                    let new_lane = self.first_empty_lane();
                    self.lane_states[new_lane] = LaneState::Active {
                        starting_col: commit_lane,
                        starting_row: commit_row,
                        destination_column: None,
                        segments: smallvec![CommitLineSegment::Curve {
                            to_column: usize::MAX,
                            on_row: usize::MAX,
                            curve_kind: CurveKind::Merge,
                        }],
                    };
                    self.parent_to_lanes
                        .entry(parent_id.clone())
                        .or_default()
                        .push(new_lane);
                }
            }

            self.max_lanes = self.max_lanes.max(self.lane_states.len());

            self.rows.push(GraphRow {
                commit_id: commit.id.clone(),
                lane: commit_lane,
                color_lane: commit_lane,
                is_merge,
            });
        }
    }

    pub fn total_lanes(&self) -> usize {
        self.max_lanes
    }

    fn first_empty_lane(&mut self) -> usize {
        if let Some(ix) = self.lane_states.iter().position(LaneState::is_empty) {
            return ix;
        }
        self.lane_states.push(LaneState::Empty);
        self.lane_states.len() - 1
    }
}

impl Default for LineGraphBuilder {
    fn default() -> Self {
        Self::new()
    }
}
