use crate::error::{GitError, GitResult};
use crate::repository::Repository;
use crate::diff::{FileChange, FileChangeKind};
use crate::diff_stat::parse_numstat;
use crate::status::DiffStat;
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

fn map_change(change: gix::object::tree::diff::ChangeDetached) -> FileChange {
    match change {
        gix::object::tree::diff::ChangeDetached::Addition { location, id, .. } => FileChange {
            kind: FileChangeKind::Added,
            path: location.to_string(),
            old_path: None,
            old_id: None,
            new_id: Some(id.to_hex().to_string()),
        },
        gix::object::tree::diff::ChangeDetached::Deletion { location, id, .. } => FileChange {
            kind: FileChangeKind::Deleted,
            path: location.to_string(),
            old_path: None,
            old_id: Some(id.to_hex().to_string()),
            new_id: None,
        },
        gix::object::tree::diff::ChangeDetached::Modification { location, previous_id, id, .. } => FileChange {
            kind: FileChangeKind::Modified,
            path: location.to_string(),
            old_path: None,
            old_id: Some(previous_id.to_hex().to_string()),
            new_id: Some(id.to_hex().to_string()),
        },
        gix::object::tree::diff::ChangeDetached::Rewrite { source_location, source_id, location, id, copy, .. } => FileChange {
            kind: if copy { FileChangeKind::Copied } else { FileChangeKind::Renamed },
            path: location.to_string(),
            old_path: Some(source_location.to_string()),
            old_id: Some(source_id.to_hex().to_string()),
            new_id: Some(id.to_hex().to_string()),
        },
    }
}

impl Repository {
    pub fn diff_between_commits(&self, old_commit_id: &str, new_commit_id: &str) -> GitResult<Vec<FileChange>> {
        let old_tree = self.find_commit_tree(old_commit_id)?;
        let new_tree = self.find_commit_tree(new_commit_id)?;

        let changes = self.repo.diff_tree_to_tree(Some(&old_tree), Some(&new_tree), None)
            .map_err(|e| GitError::OperationFailed(e.to_string()))?;

        Ok(changes.into_iter().map(map_change).collect())
    }

    pub fn diff_commit_against_parent(&self, commit_id: &str) -> GitResult<Vec<FileChange>> {
        let tree = self.find_commit_tree(commit_id)?;

        let id = self.parse_object_id(commit_id, "commit ID")?;
        let commit = self.repo.find_commit(id)
            .map_err(|e| GitError::OperationFailed(e.to_string()))?;

        let parent_ids: Vec<gix::ObjectId> = commit.parent_ids()
            .map(|p| p.detach())
            .collect();

        let old_tree = match parent_ids.first() {
            Some(pid) => {
                let parent = self.repo.find_commit(*pid)
                    .map_err(|e| GitError::OperationFailed(e.to_string()))?;
                parent.tree().map_err(|e| GitError::OperationFailed(e.to_string()))?
            }
            None => self.repo.empty_tree(),
        };

        let changes = self.repo.diff_tree_to_tree(Some(&old_tree), Some(&tree), None)
            .map_err(|e| GitError::OperationFailed(e.to_string()))?;

        Ok(changes.into_iter().map(map_change).collect())
    }

    /// Spawns a `git` subprocess.
    pub fn unified_diff_for_commit(&self, commit_id: &str) -> GitResult<String> {
        let output = Command::new("git")
            .args(["diff-tree", "-p", "--no-color", commit_id])
            .current_dir(&self.path)
            .output()
            .map_err(|e| GitError::OperationFailed(format!("Failed to run git diff-tree: {}", e)))?;

        if !output.status.success() {
            return Err(GitError::OperationFailed(
                String::from_utf8_lossy(&output.stderr).to_string(),
            ));
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    /// Spawns a `git` subprocess.
    pub fn diff_index_to_worktree(&self, path: Option<&Path>) -> GitResult<String> {
        let mut args = vec!["diff", "--no-color"];
        if let Some(p) = path {
            args.push("--");
            args.push(p.to_str().unwrap_or(""));
        }
        let output = self.run_git(&args)?;
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    /// Spawns a `git` subprocess.
    pub fn diff_head_to_index(&self, path: Option<&Path>) -> GitResult<String> {
        let mut args = vec!["diff", "--cached", "--no-color"];
        if let Some(p) = path {
            args.push("--");
            args.push(p.to_str().unwrap_or(""));
        }
        let output = self.run_git(&args)?;
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    /// Spawns a `git` subprocess.
    pub fn diff_numstat_vs_head(&self) -> GitResult<HashMap<String, DiffStat>> {
        let output = self.run_git(&["diff", "--numstat", "--no-renames", "HEAD"])?;
        Ok(parse_numstat(&String::from_utf8_lossy(&output.stdout)))
    }
}
