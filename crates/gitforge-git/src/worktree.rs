use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorktreeInfo {
    pub path: PathBuf,
    pub head_commit: Option<String>,
    pub branch: Option<String>,
    pub is_current: bool,
    pub is_detached: bool,
    pub is_bare: bool,
    pub is_prunable: bool,
}
