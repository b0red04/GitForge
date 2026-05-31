use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum FileChangeKind {
    Added,
    Deleted,
    Modified,
    Renamed,
    Copied,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileChange {
    pub kind: FileChangeKind,
    pub path: String,
    pub old_path: Option<String>,
    pub old_id: Option<String>,
    pub new_id: Option<String>,
}
