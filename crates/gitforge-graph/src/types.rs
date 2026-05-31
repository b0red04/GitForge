use serde::{Deserialize, Serialize};

pub type CommitId = String;
pub type LaneId = usize;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphNode {
    pub commit_id: CommitId,
    pub row: usize,
    pub lane: LaneId,
    pub is_merge: bool,
}

impl GraphNode {
    pub fn new(commit_id: CommitId, row: usize, lane: LaneId) -> Self {
        Self {
            commit_id,
            row,
            lane,
            is_merge: false,
        }
    }
}
