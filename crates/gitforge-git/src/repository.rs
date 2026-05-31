use crate::error::{GitError, GitResult};
use std::path::Path;

pub mod log_impl;
pub mod status_impl;
pub mod diff_impl;
pub mod objects_impl;
pub mod write_impl;
pub mod blame_impl;
pub mod worktree_impl;

pub struct Repository {
    pub(crate) repo: gix::Repository,
    pub(crate) path: std::path::PathBuf,
}

impl Repository {
    pub fn open(path: &Path) -> GitResult<Self> {
        let repo = gix::open(path)?;
        Ok(Self {
            repo,
            path: path.to_path_buf(),
        })
    }

    pub fn discover(path: &Path) -> GitResult<Self> {
        let repo = gix::discover(path)?;
        let workdir = repo.workdir().unwrap_or(path).to_path_buf();
        Ok(Self {
            repo,
            path: workdir,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    fn parse_object_id(&self, id: &str, label: &str) -> GitResult<gix::ObjectId> {
        id.parse().map_err(|e| {
            GitError::OperationFailed(format!("Invalid {} '{}': {}", label, id, e))
        })
    }

    fn find_commit_tree(&self, commit_id: &str) -> GitResult<gix::Tree<'_>> {
        let id = self.parse_object_id(commit_id, "commit ID")?;
        let commit = self.repo.find_commit(id)
            .map_err(|e| GitError::OperationFailed(e.to_string()))?;
        commit.tree().map_err(|e| GitError::OperationFailed(e.to_string()))
    }
}
