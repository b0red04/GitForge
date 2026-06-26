use crate::error::{GitError, GitResult, classify_git_failure};
use std::path::Path;
use std::process::Command;

pub mod blame_impl;
pub mod branch_impl;
pub mod diff_impl;
pub mod log_impl;
pub mod merge_impl;
pub mod network_impl;
pub mod objects_impl;
pub mod staging_impl;
pub mod stash_impl;
pub mod status_impl;
pub mod submodule_impl;
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
        id.parse()
            .map_err(|e| GitError::OperationFailed(format!("Invalid {} '{}': {}", label, id, e)))
    }

    fn find_commit_tree(&self, commit_id: &str) -> GitResult<gix::Tree<'_>> {
        let id = self.parse_object_id(commit_id, "commit ID")?;
        let commit = self
            .repo
            .find_commit(id)
            .map_err(|e| GitError::OperationFailed(e.to_string()))?;
        commit
            .tree()
            .map_err(|e| GitError::OperationFailed(e.to_string()))
    }

    pub(crate) fn run_git(&self, args: &[&str]) -> GitResult<std::process::Output> {
        let label = args.first().ok_or_else(|| {
            GitError::OperationFailed("git command requires at least one argument".into())
        })?;
        let output = Command::new("git")
            .args(args)
            .current_dir(&self.path)
            .output()
            .map_err(|e| {
                GitError::OperationFailed(format!("Failed to run git {}: {}", label, e))
            })?;

        if !output.status.success() {
            return Err(classify_git_failure(args, &output));
        }
        Ok(output)
    }

    pub(crate) fn run_git_raw(&self, args: &[&str]) -> GitResult<std::process::Output> {
        let label = args.first().ok_or_else(|| {
            GitError::OperationFailed("git command requires at least one argument".into())
        })?;
        Command::new("git")
            .args(args)
            .current_dir(&self.path)
            .output()
            .map_err(|e| GitError::OperationFailed(format!("Failed to run git {}: {}", label, e)))
    }

    pub(crate) fn run_git_with_combined_error(&self, args: &[&str]) -> GitResult<String> {
        let label = args.first().ok_or_else(|| {
            GitError::OperationFailed("git command requires at least one argument".into())
        })?;
        let output = Command::new("git")
            .args(args)
            .current_dir(&self.path)
            .output()
            .map_err(|e| {
                GitError::OperationFailed(format!("Failed to run git {}: {}", label, e))
            })?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let combined = format!("{}{}", stdout, stderr);

        if !output.status.success() {
            return Err(classify_git_failure(args, &output));
        }
        Ok(combined)
    }
}
