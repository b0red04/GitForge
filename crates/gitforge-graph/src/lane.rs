use crate::types::{LaneId, CommitId};

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct ActiveLane {
    id: LaneId,
    commit_id: Option<CommitId>,
    parent_lane: Option<LaneId>,
}

pub struct LaneAssigner {
    lanes: Vec<ActiveLane>,
    next_lane_id: LaneId,
    free_lanes: Vec<LaneId>,
}

impl LaneAssigner {
    pub fn new() -> Self {
        Self {
            lanes: Vec::new(),
            next_lane_id: 0,
            free_lanes: Vec::new(),
        }
    }

    fn allocate_lane(&mut self, commit_id: Option<CommitId>, parent_lane: Option<LaneId>) -> LaneId {
        if let Some(free) = self.free_lanes.pop() {
            self.lanes.push(ActiveLane { id: free, commit_id, parent_lane });
            return free;
        }
        let id = self.next_lane_id;
        self.next_lane_id += 1;
        self.lanes.push(ActiveLane { id, commit_id, parent_lane });
        id
    }

    fn find_lane_by_commit(&self, commit_id: &str) -> Option<usize> {
        self.lanes.iter().position(|l| l.commit_id.as_deref() == Some(commit_id))
    }

    fn find_lane_by_id(&self, lane_id: LaneId) -> Option<usize> {
        self.lanes.iter().position(|l| l.id == lane_id)
    }

    pub fn assign_lane(&mut self, commit_id: &str, parent_ids: &[String]) -> (LaneId, Vec<LaneId>) {
        let commit_id = commit_id.to_string();

        let lane = if let Some(pos) = self.find_lane_by_commit(&commit_id) {
            let lane_id = self.lanes[pos].id;
            self.lanes[pos].commit_id = None;
            lane_id
        } else {
            self.allocate_lane(None, None)
        };

        let mut parent_lanes = Vec::with_capacity(parent_ids.len());

        for (i, pid) in parent_ids.iter().enumerate() {
            if i == 0 {
                if let Some(pos) = self.find_lane_by_id(lane) {
                    self.lanes[pos].commit_id = Some(pid.clone());
                }
                parent_lanes.push(lane);
            } else {
                let new_lane = self.allocate_lane(Some(pid.clone()), Some(lane));
                parent_lanes.push(new_lane);
            }
        }

        if parent_ids.is_empty() {
            self.free_lanes.push(lane);
            self.lanes.retain(|l| l.id != lane);
        }

        (lane, parent_lanes)
    }

    pub fn total_lanes(&self) -> usize {
        self.next_lane_id
    }

    pub fn lane_columns(&self) -> Vec<LaneId> {
        let mut cols: Vec<LaneId> = self.lanes.iter().map(|l| l.id).collect();
        cols.sort();
        cols
    }
}
