use crate::error::{classify_git_failure, GitError, GitResult};
use crate::repository::Repository;
use std::process::Command;

impl Repository {
    /// Spawns a `git` subprocess.
    pub fn fast_forward(&self, branch: &str) -> GitResult<()> {
        self.run_git(&["merge", "--ff-only", branch])?;
        Ok(())
    }

    /// Spawns a `git` subprocess.
    pub fn merge(&self, branch: &str, no_ff: bool) -> GitResult<String> {
        let mut args = vec!["merge"];
        if no_ff {
            args.push("--no-ff");
        }
        args.push(branch);
        let output = self.run_git(&args)?;
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    /// Spawns a `git` subprocess.
    pub fn rebase(&self, branch: &str) -> GitResult<()> {
        self.run_git(&["rebase", branch])?;
        Ok(())
    }

    /// Spawns a `git` subprocess.
    pub fn rebase_abort(&self) -> GitResult<()> {
        self.run_git(&["rebase", "--abort"])?;
        Ok(())
    }

    /// Spawns a `git` subprocess.
    pub fn rebase_continue(&self) -> GitResult<()> {
        let output = Command::new("git")
            .args(["rebase", "--continue"])
            .env("GIT_EDITOR", "true")
            .current_dir(&self.path)
            .output()
            .map_err(|e| {
                GitError::OperationFailed(format!("Failed to run git rebase --continue: {}", e))
            })?;

        if !output.status.success() {
            return Err(classify_git_failure(&["rebase", "--continue"], &output));
        }
        Ok(())
    }

    /// Spawns a `git` subprocess.
    pub fn cherry_pick(&self, sha: &str) -> GitResult<()> {
        self.run_git(&["cherry-pick", sha])?;
        Ok(())
    }

    /// Spawns a `git` subprocess.
    pub fn cherry_pick_abort(&self) -> GitResult<()> {
        self.run_git(&["cherry-pick", "--abort"])?;
        Ok(())
    }

    /// Spawns a `git` subprocess.
    pub fn cherry_pick_continue(&self) -> GitResult<()> {
        let output = Command::new("git")
            .args(["cherry-pick", "--continue"])
            .env("GIT_EDITOR", "true")
            .current_dir(&self.path)
            .output()
            .map_err(|e| {
                GitError::OperationFailed(format!(
                    "Failed to run git cherry-pick --continue: {}",
                    e
                ))
            })?;

        if !output.status.success() {
            return Err(classify_git_failure(&["cherry-pick", "--continue"], &output));
        }
        Ok(())
    }

    /// Spawns a `git` subprocess.
    pub fn revert(&self, sha: &str) -> GitResult<()> {
        self.run_git(&["revert", "--no-edit", sha])?;
        Ok(())
    }

    /// Spawns a `git` subprocess.
    pub fn mixed_reset(&self, reference: &str) -> GitResult<()> {
        self.run_git(&["reset", "--mixed", reference])?;
        Ok(())
    }

    /// Spawns a `git` subprocess.
    pub fn hard_reset(&self, reference: &str) -> GitResult<()> {
        self.run_git(&["reset", "--hard", reference])?;
        Ok(())
    }
}
