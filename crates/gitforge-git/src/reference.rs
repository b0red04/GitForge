use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum RefKind {
    Branch,
    RemoteBranch,
    Tag,
    Stash,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefInfo {
    pub name: String,
    pub kind: RefKind,
    pub target_commit_id: String,
    pub is_head: bool,
    pub remote_name: Option<String>,
}
