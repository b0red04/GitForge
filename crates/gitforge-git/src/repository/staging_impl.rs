use crate::error::{GitError, GitResult, classify_git_failure};
use crate::repository::Repository;
use std::path::Path;
use std::process::Command;

impl Repository {
    /// Spawns a `git` subprocess.
    pub fn stage_paths(&self, paths: &[&Path]) -> GitResult<()> {
        if paths.is_empty() {
            return Ok(());
        }
        let mut args = vec!["add", "--"];
        args.extend(paths.iter().map(|p| p.to_str().unwrap_or("")));
        self.run_git(&args)?;
        Ok(())
    }

    /// Spawns a `git` subprocess.
    pub fn stage_all(&self) -> GitResult<()> {
        self.run_git(&["add", "--all"])?;
        Ok(())
    }

    /// Spawns a `git` subprocess.
    pub fn unstage_paths(&self, paths: &[&Path]) -> GitResult<()> {
        if paths.is_empty() {
            return Ok(());
        }
        let mut args = vec!["reset", "HEAD", "--"];
        args.extend(paths.iter().map(|p| p.to_str().unwrap_or("")));
        self.run_git(&args)?;
        Ok(())
    }

    /// Spawns a `git` subprocess.
    pub fn unstage_all(&self) -> GitResult<()> {
        self.run_git(&["reset", "HEAD"])?;
        Ok(())
    }

    /// Spawns a `git` subprocess.
    pub fn commit(&self, message: &str) -> GitResult<String> {
        let output = self.run_git(&["commit", "-m", message])?;
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    /// Spawns a `git` subprocess.
    pub fn commit_amend(&self, message: &str) -> GitResult<String> {
        let output = self.run_git(&["commit", "--amend", "-m", message])?;
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    /// Spawns a `git` subprocess.
    pub fn apply_patch(&self, patch: &str, cached: bool, reverse: bool) -> GitResult<()> {
        let mut args = vec!["apply"];
        if cached {
            args.push("--cached");
        }
        if reverse {
            args.push("-R");
        }
        args.push("-");

        let mut child = Command::new("git")
            .args(&args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .current_dir(&self.path)
            .spawn()
            .map_err(|e| GitError::OperationFailed(format!("Failed to run git apply: {}", e)))?;

        if let Some(mut stdin) = child.stdin.take() {
            use std::io::Write;
            stdin
                .write_all(patch.as_bytes())
                .map_err(|e| GitError::OperationFailed(format!("Failed to write patch: {}", e)))?;
        }

        let output = child.wait_with_output().map_err(|e| {
            GitError::OperationFailed(format!("Failed to wait for git apply: {}", e))
        })?;

        if !output.status.success() {
            return Err(classify_git_failure(&["apply"], &output));
        }
        Ok(())
    }

    /// Spawns a `git` subprocess.
    pub fn discard_worktree_changes(&self, paths: &[&Path]) -> GitResult<()> {
        if paths.is_empty() {
            return Ok(());
        }
        let mut args = vec!["checkout", "--"];
        args.extend(paths.iter().map(|p| p.to_str().unwrap_or("")));
        self.run_git(&args)?;
        Ok(())
    }

    /// Restores tracked worktree changes and removes the listed untracked files.
    pub fn discard_changes(&self, tracked: &[&Path], untracked: &[&Path]) -> GitResult<()> {
        self.discard_worktree_changes(tracked)?;
        self.remove_untracked(untracked)
    }

    /// Performs filesystem I/O to remove files directly.
    pub fn remove_untracked(&self, paths: &[&Path]) -> GitResult<()> {
        if paths.is_empty() {
            return Ok(());
        }
        for path in paths {
            let full_path = self.path.join(path);
            if full_path.exists() {
                std::fs::remove_file(&full_path).map_err(|e| {
                    GitError::OperationFailed(format!("Failed to remove {}: {}", path.display(), e))
                })?;
            }
        }
        Ok(())
    }

    /// Performs filesystem I/O to write `.gitignore`.
    pub fn add_to_gitignore(&self, pattern: &str) -> GitResult<()> {
        let gitignore_path = self.path.join(".gitignore");
        let mut existing = String::new();
        if gitignore_path.exists() {
            existing = std::fs::read_to_string(&gitignore_path).map_err(|e| {
                GitError::OperationFailed(format!("Failed to read .gitignore: {}", e))
            })?;
            if !existing.ends_with('\n') {
                existing.push('\n');
            }
        }
        existing.push_str(pattern);
        existing.push('\n');
        std::fs::write(&gitignore_path, existing)
            .map_err(|e| GitError::OperationFailed(format!("Failed to write .gitignore: {}", e)))?;
        Ok(())
    }

    /// Spawns a `git` subprocess.
    pub fn resolve_conflict_use_ours(&self, path: &Path) -> GitResult<()> {
        self.run_git(&["checkout", "--ours", "--"])?;
        self.stage_paths(&[path])
    }

    /// Spawns a `git` subprocess.
    pub fn resolve_conflict_use_theirs(&self, path: &Path) -> GitResult<()> {
        self.run_git(&["checkout", "--theirs", "--"])?;
        self.stage_paths(&[path])
    }

    /// Spawns a `git` subprocess.
    pub fn soft_reset_head(&self, commits: usize) -> GitResult<()> {
        let arg = format!("HEAD~{}", commits);
        self.run_git(&["reset", "--soft", &arg])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn discard_changes_keeps_index_and_removes_untracked_files() {
        let tmp = TempDir::new().unwrap();
        Command::new("git")
            .arg("init")
            .current_dir(tmp.path())
            .output()
            .unwrap();
        let repo = Repository::open(tmp.path()).unwrap();

        std::fs::write(tmp.path().join("tracked.txt"), "committed").unwrap();
        repo.stage_paths(&[Path::new("tracked.txt")]).unwrap();
        repo.run_git(&[
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.com",
            "commit",
            "-m",
            "init",
        ])
        .unwrap();

        std::fs::write(tmp.path().join("tracked.txt"), "staged").unwrap();
        repo.stage_paths(&[Path::new("tracked.txt")]).unwrap();
        std::fs::write(tmp.path().join("tracked.txt"), "unstaged").unwrap();
        std::fs::write(tmp.path().join("new.txt"), "untracked").unwrap();

        repo.discard_changes(&[Path::new("tracked.txt")], &[Path::new("new.txt")])
            .unwrap();

        assert_eq!(
            std::fs::read_to_string(tmp.path().join("tracked.txt")).unwrap(),
            "staged"
        );
        assert!(!tmp.path().join("new.txt").exists());
        assert!(!repo.status().unwrap().staged.is_empty());
    }
}
