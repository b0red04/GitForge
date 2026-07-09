use crate::error::{GitError, GitResult, classify_git_failure};
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
            if let Some((name, rest)) = line.split_once('\t')
                && let Some(url) = rest.split_whitespace().next()
                    && seen.insert(name.to_string()) {
                        remotes.push((name.to_string(), url.to_string()));
                    }
        }
        Ok(remotes)
    }

    /// True when `refs/remotes/{remote}/{branch}` exists (branch was published).
    pub fn remote_branch_exists(&self, remote: &str, branch: &str) -> GitResult<bool> {
        let output = self.run_git_raw(&[
            "rev-parse",
            "--verify",
            &format!("refs/remotes/{remote}/{branch}"),
        ])?;
        Ok(output.status.success())
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
        } else {
            args.push("--no-rebase");
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
        let Some(branch) = branch else {
            let mut args = vec!["push"];
            if force {
                args.push("--force-with-lease");
            }
            args.push(remote);
            return self.run_git_with_combined_error(&args);
        };

        validate_local_push_branch(branch)?;

        let refspec = format!("refs/heads/{branch}:refs/heads/{branch}");
        let mut args: Vec<String> = vec!["push".into()];

        if force {
            let tracking = format!("refs/remotes/{remote}/{branch}");
            match self.run_git_raw(&["rev-parse", &tracking]) {
                Ok(out) if out.status.success() => {
                    let expected = String::from_utf8_lossy(&out.stdout).trim().to_string();
                    args.push(format!("--force-with-lease=refs/heads/{branch}:{expected}"));
                }
                _ => args.push("--force".into()),
            }
        }

        if set_upstream && !self.branch_has_upstream(branch)? {
            args.push("-u".into());
        }

        args.push(remote.into());
        args.push(refspec);

        let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
        self.run_git_with_combined_error(&arg_refs)
    }

    fn branch_has_upstream(&self, branch: &str) -> GitResult<bool> {
        let output = self.run_git_raw(&[
            "rev-parse",
            "--abbrev-ref",
            &format!("{branch}@{{upstream}}"),
        ])?;
        Ok(output.status.success())
    }

    /// Spawns `git push <remote> --delete <branch>`. Refuses to delete the
    /// repository's default branch (main/master); on success, best-effort
    /// prunes the now-stale remote-tracking ref so the sidebar updates.
    pub fn delete_remote_branch(&self, remote: &str, branch: &str) -> GitResult<String> {
        if let Ok(Some(default)) = self.main_branch_name() {
            let default_bare = default.rsplit('/').next().unwrap_or(&default);
            if branch == default_bare {
                return Err(GitError::OperationFailed(format!(
                    "Refusing to delete the default branch '{branch}' on {remote}"
                )));
            }
        }
        let out = self.run_git_with_combined_error(&["push", remote, "--delete", branch])?;
        if let Err(e) = self.fetch(Some(remote), true) {
            tracing::warn!("remote branch deleted on {remote}, but prune failed: {e}");
        }
        Ok(out)
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

        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        if !output.status.success() {
            return Err(classify_git_failure(
                &["clone", url, path.to_str().unwrap_or(".")],
                &output,
            ));
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
            return Err(classify_git_failure(&args, &output));
        }
        Ok(())
    }
}

fn validate_local_push_branch(branch: &str) -> GitResult<()> {
    if branch.is_empty() || branch == "HEAD" {
        return Err(GitError::OperationFailed(
            "Check out a local branch before pushing".into(),
        ));
    }
    if branch.starts_with("refs/") || branch.contains("/HEAD") {
        return Err(GitError::OperationFailed(format!(
            "Can't push '{branch}' — check out a local branch first"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_local_push_branch_rejects_remote_head() {
        assert!(validate_local_push_branch("origin/HEAD").is_err());
        assert!(validate_local_push_branch("refs/remotes/origin/HEAD").is_err());
        assert!(validate_local_push_branch("feature/foo").is_ok());
    }
}
