use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum FileStatus {
    Modified,
    Added,
    Deleted,
    Renamed,
    Copied,
    Untracked,
    Conflicted,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct DiffStat {
    pub added: u32,
    pub deleted: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    pub path: String,
    pub old_path: Option<String>,
    pub status: FileStatus,
    pub staged: bool,
    #[serde(default)]
    pub diff_stat: Option<DiffStat>,
}

impl RepoStatus {
    pub fn has_changes(&self) -> bool {
        !self.staged.is_empty() || !self.unstaged.is_empty()
            || !self.untracked.is_empty() || !self.conflicted.is_empty()
    }

    pub fn changed_file_count(&self) -> usize {
        self.staged.len()
            + self.unstaged.len()
            + self.untracked.len()
            + self.conflicted.len()
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
