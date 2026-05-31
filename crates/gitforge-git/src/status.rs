use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum FileStatus {
    Unmodified,
    Modified,
    Added,
    Deleted,
    Renamed,
    Copied,
    Untracked,
    Ignored,
    Conflicted,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    pub path: String,
    pub old_path: Option<String>,
    pub status: FileStatus,
    pub staged: bool,
}

impl RepoStatus {
    pub fn has_changes(&self) -> bool {
        !self.staged.is_empty() || !self.unstaged.is_empty()
            || !self.untracked.is_empty() || !self.conflicted.is_empty()
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RepoStatus {
    pub staged: Vec<FileEntry>,
    pub unstaged: Vec<FileEntry>,
    pub untracked: Vec<FileEntry>,
    pub conflicted: Vec<FileEntry>,
    pub head_branch: Option<String>,
    pub head_commit: Option<String>,
}
