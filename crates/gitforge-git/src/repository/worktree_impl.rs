use crate::error::GitResult;
use crate::repository::Repository;
use crate::worktree::WorktreeInfo;
use std::path::Path;

impl Repository {
    /// Spawns a `git` subprocess.
    pub fn worktree_list(&self) -> GitResult<Vec<WorktreeInfo>> {
        let output = self.run_git(&["worktree", "list", "--porcelain"])?;
        let text = String::from_utf8_lossy(&output.stdout);
        let mut worktrees = Vec::new();
        let mut current_wt: Option<WorktreeInfo> = None;

        for line in text.lines() {
            if line.is_empty() {
                if let Some(wt) = current_wt.take() {
                    worktrees.push(wt);
                }
                continue;
            }

            if let Some(path_str) = line.strip_prefix("worktree ") {
                if let Some(ref mut wt) = current_wt {
                    wt.path = path_str.into();
                } else {
                    current_wt = Some(WorktreeInfo {
                        path: path_str.into(),
                        head_commit: None,
                        branch: None,
                        is_current: false,
                        is_detached: false,
                        is_bare: false,
                        is_prunable: false,
                    });
                }
            } else if let Some(head) = line.strip_prefix("HEAD ") {
                if let Some(ref mut wt) = current_wt {
                    wt.head_commit = Some(head.to_string());
                }
            } else if let Some(branch) = line.strip_prefix("branch refs/heads/") {
                if let Some(ref mut wt) = current_wt {
                    wt.branch = Some(branch.to_string());
                }
            } else if line == "detached" {
                if let Some(ref mut wt) = current_wt {
                    wt.is_detached = true;
                }
            } else if line == "bare" {
                if let Some(ref mut wt) = current_wt {
                    wt.is_bare = true;
                }
            }
        }

        if let Some(wt) = current_wt.take() {
            worktrees.push(wt);
        }

        let current_path = self.path.canonicalize().unwrap_or_else(|_| self.path.clone());
        for wt in &mut worktrees {
            let wt_canonical = wt.path.canonicalize().unwrap_or_else(|_| wt.path.clone());
            wt.is_current = wt_canonical == current_path;
        }

        Ok(worktrees)
    }

    /// Spawns a `git` subprocess.
    pub fn worktree_add(&self, target_path: &Path, refname: Option<&str>, create_branch: Option<&str>) -> GitResult<()> {
        let mut args = vec!["worktree", "add"];
        if let Some(branch_name) = create_branch {
            args.push("-b");
            args.push(branch_name);
        }
        args.push(target_path.to_str().unwrap_or(""));
        if let Some(rf) = refname {
            args.push(rf);
        }
        self.run_git(&args)?;
        Ok(())
    }

    /// Spawns a `git` subprocess.
    pub fn worktree_remove(&self, path: &Path, force: bool) -> GitResult<()> {
        let mut args = vec!["worktree", "remove"];
        if force {
            args.push("--force");
        }
        args.push(path.to_str().unwrap_or(""));
        self.run_git(&args)?;
        Ok(())
    }

    /// Spawns a `git` subprocess.
    pub fn worktree_prune(&self) -> GitResult<()> {
        self.run_git(&["worktree", "prune"])?;
        Ok(())
    }
}
