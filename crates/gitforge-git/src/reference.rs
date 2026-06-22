use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
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
    pub commits_ahead: u32,
    pub commits_behind: u32,
}
