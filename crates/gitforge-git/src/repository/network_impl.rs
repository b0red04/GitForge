use crate::error::{GitError, GitResult};
use crate::repository::Repository;
use std::path::Path;
use std::process::Command;

impl Repository {
    /// Spawns a `git` subprocess.
    pub fn remote_add(&self, name: &str, url: &str) -> GitResult<()> {
        self.run_git(&["remote", "add", name, url])?;
        Ok(())
    }

    /// Spawns a `git` subprocess.
    pub fn remote_remove(&self, name: &str) -> GitResult<()> {
        self.run_git(&["remote", "remove", name])?;
        Ok(())
    }

    /// Spawns a `git` subprocess.
    pub fn remote_rename(&self, old: &str, new: &str) -> GitResult<()> {
        self.run_git(&["remote", "rename", old, new])?;
        Ok(())
    }

    /// Spawns a `git` subprocess.
    pub fn remote_set_url(&self, name: &str, url: &str) -> GitResult<()> {
        self.run_git(&["remote", "set-url", name, url])?;
        Ok(())
    }

    /// Spawns a `git` subprocess.
    pub fn remote_list(&self) -> GitResult<Vec<(String, String)>> {
        let output = self.run_git(&["remote", "-v"])?;
        let text = String::from_utf8_lossy(&output.stdout);
        let mut remotes = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for line in text.lines() {
            if let Some((name, rest)) = line.split_once('\t') {
                if let Some(url) = rest.split_whitespace().next() {
                    if seen.insert(name.to_string()) {
                        remotes.push((name.to_string(), url.to_string()));
                    }
                }
            }
        }
        Ok(remotes)
    }

    /// Spawns a `git` subprocess.
    pub fn fetch(&self, remote: Option<&str>, prune: bool) -> GitResult<String> {
        let mut args = vec!["fetch"];
        if prune {
            args.push("--prune");
        }
        if let Some(r) = remote {
            args.push(r);
        }
        self.run_git_with_combined_error(&args)
    }

    /// Spawns a `git` subprocess.
    pub fn fetch_all(&self, prune: bool) -> GitResult<String> {
        let mut args = vec!["fetch", "--all"];
        if prune {
            args.push("--prune");
        }
        self.run_git_with_combined_error(&args)
    }

    /// Spawns a `git` subprocess.
    pub fn pull(&self, remote: Option<&str>, rebase: bool) -> GitResult<String> {
        let mut args = vec!["pull"];
        if rebase {
            args.push("--rebase");
        }
        if let Some(r) = remote {
            args.push(r);
        }
        self.run_git_with_combined_error(&args)
    }

    /// Spawns a `git` subprocess.
    pub fn push(
        &self,
        remote: &str,
        branch: Option<&str>,
        force: bool,
        set_upstream: bool,
    ) -> GitResult<String> {
        let mut args = vec!["push"];
        if force {
            args.push("--force");
        }
        if set_upstream {
            args.push("-u");
        }
        args.push(remote);
        if let Some(b) = branch {
            args.push(b);
        }
        self.run_git_with_combined_error(&args)
    }

    /// Spawns a `git` subprocess.
    pub fn clone_repo(
        url: &str,
        path: &Path,
        bare: bool,
        depth: Option<usize>,
    ) -> GitResult<String> {
        let mut args: Vec<String> = vec!["clone".into()];
        if bare {
            args.push("--bare".into());
        }
        if let Some(d) = depth {
            args.push("--depth".into());
            args.push(d.to_string());
        }
        args.push(url.into());
        args.push(path.to_str().unwrap_or(".").into());

        let output = Command::new("git")
            .args(&args)
            .output()
            .map_err(|e| GitError::OperationFailed(format!("Failed to run git clone: {}", e)))?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        if !output.status.success() {
            return Err(GitError::OperationFailed(format!("{}{}", stdout, stderr)));
        }
        Ok(stderr)
    }

    /// Spawns a `git` subprocess.
    pub fn init_repo(path: &Path, bare: bool) -> GitResult<()> {
        let mut args: Vec<&str> = vec!["init"];
        if bare {
            args.push("--bare");
        }
        let output = Command::new("git")
            .args(&args)
            .current_dir(path)
            .output()
            .map_err(|e| GitError::OperationFailed(format!("Failed to run git init: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(GitError::OperationFailed(stderr.to_string()));
        }
        Ok(())
    }
}
