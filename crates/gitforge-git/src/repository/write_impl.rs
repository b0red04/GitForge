use crate::error::{GitError, GitResult};
use crate::repository::Repository;
use std::path::Path;
use std::process::Command;

impl Repository {
    pub(crate) fn run_git(&self, args: &[&str]) -> GitResult<std::process::Output> {
        let label = args[0];
        let output = Command::new("git")
            .args(args)
            .current_dir(&self.path)
            .output()
            .map_err(|e| GitError::OperationFailed(format!("Failed to run git {}: {}", label, e)))?;

        if !output.status.success() {
            return Err(GitError::OperationFailed(
                String::from_utf8_lossy(&output.stderr).to_string(),
            ));
        }
        Ok(output)
    }

    fn run_git_with_combined_error(&self, args: &[&str]) -> GitResult<String> {
        let label = args[0];
        let output = Command::new("git")
            .args(args)
            .current_dir(&self.path)
            .output()
            .map_err(|e| GitError::OperationFailed(format!("Failed to run git {}: {}", label, e)))?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let combined = format!("{}{}", stdout, stderr);

        if !output.status.success() {
            return Err(GitError::OperationFailed(combined));
        }
        Ok(combined)
    }

    pub fn stage_paths(&self, paths: &[&Path]) -> GitResult<()> {
        if paths.is_empty() {
            return Ok(());
        }
        let mut args = vec!["add", "--"];
        args.extend(paths.iter().map(|p| p.to_str().unwrap_or("")));
        self.run_git(&args)?;
        Ok(())
    }

    pub fn stage_all(&self) -> GitResult<()> {
        self.run_git(&["add", "--all"])?;
        Ok(())
    }

    pub fn unstage_paths(&self, paths: &[&Path]) -> GitResult<()> {
        if paths.is_empty() {
            return Ok(());
        }
        let mut args = vec!["reset", "HEAD", "--"];
        args.extend(paths.iter().map(|p| p.to_str().unwrap_or("")));
        self.run_git(&args)?;
        Ok(())
    }

    pub fn unstage_all(&self) -> GitResult<()> {
        self.run_git(&["reset", "HEAD"])?;
        Ok(())
    }

    pub fn commit(&self, message: &str) -> GitResult<String> {
        let output = self.run_git(&["commit", "-m", message])?;
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    pub fn commit_amend(&self, message: &str) -> GitResult<String> {
        let output = self.run_git(&["commit", "--amend", "-m", message])?;
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    pub fn discard_worktree_changes(&self, paths: &[&Path]) -> GitResult<()> {
        if paths.is_empty() {
            return Ok(());
        }
        let mut args = vec!["checkout", "--"];
        args.extend(paths.iter().map(|p| p.to_str().unwrap_or("")));
        self.run_git(&args)?;
        Ok(())
    }

    pub fn remove_untracked(&self, paths: &[&Path]) -> GitResult<()> {
        if paths.is_empty() {
            return Ok(());
        }
        for path in paths {
            let full_path = self.path.join(path);
            if full_path.exists() {
                std::fs::remove_file(&full_path)
                    .map_err(|e| GitError::OperationFailed(format!("Failed to remove {}: {}", path.display(), e)))?;
            }
        }
        Ok(())
    }

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
            stdin.write_all(patch.as_bytes())
                .map_err(|e| GitError::OperationFailed(format!("Failed to write patch: {}", e)))?;
        }

        let output = child.wait_with_output()
            .map_err(|e| GitError::OperationFailed(format!("Failed to wait for git apply: {}", e)))?;

        if !output.status.success() {
            return Err(GitError::OperationFailed(
                String::from_utf8_lossy(&output.stderr).to_string(),
            ));
        }
        Ok(())
    }

    pub fn soft_reset_head(&self, commits: usize) -> GitResult<()> {
        let arg = format!("HEAD~{}", commits);
        self.run_git(&["reset", "--soft", &arg])?;
        Ok(())
    }

    pub fn diff_index_to_worktree(&self, path: Option<&Path>) -> GitResult<String> {
        let mut args = vec!["diff", "--no-color"];
        if let Some(p) = path {
            args.push("--");
            args.push(p.to_str().unwrap_or(""));
        }
        let output = self.run_git(&args)?;
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    pub fn create_branch(&self, name: &str, start_point: Option<&str>) -> GitResult<()> {
        let mut args = vec!["branch", name];
        if let Some(sp) = start_point {
            args.push(sp);
        }
        self.run_git(&args)?;
        Ok(())
    }

    pub fn delete_branch(&self, name: &str, force: bool) -> GitResult<()> {
        let flag = if force { "-D" } else { "-d" };
        self.run_git(&["branch", flag, name])?;
        Ok(())
    }

    pub fn rename_branch(&self, old: &str, new: &str) -> GitResult<()> {
        self.run_git(&["branch", "-m", old, new])?;
        Ok(())
    }

    pub fn checkout_branch(&self, name: &str) -> GitResult<()> {
        self.run_git(&["checkout", name])?;
        Ok(())
    }

    pub fn checkout_commit(&self, sha: &str) -> GitResult<()> {
        self.run_git(&["checkout", sha])?;
        Ok(())
    }

    pub fn fast_forward(&self, branch: &str) -> GitResult<()> {
        self.run_git(&["merge", "--ff-only", branch])?;
        Ok(())
    }

    pub fn merge(&self, branch: &str, no_ff: bool) -> GitResult<String> {
        let mut args = vec!["merge"];
        if no_ff {
            args.push("--no-ff");
        }
        args.push(branch);
        let output = self.run_git(&args)?;
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    pub fn rebase(&self, branch: &str) -> GitResult<()> {
        self.run_git(&["rebase", branch])?;
        Ok(())
    }

    pub fn rebase_abort(&self) -> GitResult<()> {
        self.run_git(&["rebase", "--abort"])?;
        Ok(())
    }

    pub fn rebase_continue(&self) -> GitResult<()> {
        let output = Command::new("git")
            .args(["rebase", "--continue"])
            .env("GIT_EDITOR", "true")
            .current_dir(&self.path)
            .output()
            .map_err(|e| GitError::OperationFailed(format!("Failed to run git rebase --continue: {}", e)))?;

        if !output.status.success() {
            return Err(GitError::OperationFailed(
                String::from_utf8_lossy(&output.stderr).to_string(),
            ));
        }
        Ok(())
    }

    pub fn mixed_reset(&self, reference: &str) -> GitResult<()> {
        self.run_git(&["reset", "--mixed", reference])?;
        Ok(())
    }

    pub fn hard_reset(&self, reference: &str) -> GitResult<()> {
        self.run_git(&["reset", "--hard", reference])?;
        Ok(())
    }

    pub fn cherry_pick(&self, sha: &str) -> GitResult<()> {
        self.run_git(&["cherry-pick", sha])?;
        Ok(())
    }

    pub fn cherry_pick_abort(&self) -> GitResult<()> {
        self.run_git(&["cherry-pick", "--abort"])?;
        Ok(())
    }

    pub fn cherry_pick_continue(&self) -> GitResult<()> {
        let output = Command::new("git")
            .args(["cherry-pick", "--continue"])
            .env("GIT_EDITOR", "true")
            .current_dir(&self.path)
            .output()
            .map_err(|e| GitError::OperationFailed(format!("Failed to run git cherry-pick --continue: {}", e)))?;

        if !output.status.success() {
            return Err(GitError::OperationFailed(
                String::from_utf8_lossy(&output.stderr).to_string(),
            ));
        }
        Ok(())
    }

    pub fn revert(&self, sha: &str) -> GitResult<()> {
        self.run_git(&["revert", "--no-edit", sha])?;
        Ok(())
    }

    pub fn create_tag(&self, name: &str, message: Option<&str>, target: Option<&str>) -> GitResult<()> {
        let mut args = vec!["tag"];
        if let Some(msg) = message {
            args.extend(["-a", name, "-m", msg]);
        } else {
            args.push(name);
        }
        if let Some(t) = target {
            args.push(t);
        }
        self.run_git(&args)?;
        Ok(())
    }

    pub fn delete_tag(&self, name: &str) -> GitResult<()> {
        self.run_git(&["tag", "-d", name])?;
        Ok(())
    }

    pub fn stash_push(&self, message: Option<&str>) -> GitResult<()> {
        let mut args = vec!["stash", "push"];
        if let Some(msg) = message {
            args.extend(["-m", msg]);
        }
        self.run_git(&args)?;
        Ok(())
    }

    pub fn stash_pop(&self) -> GitResult<()> {
        self.run_git(&["stash", "pop"])?;
        Ok(())
    }

    pub fn stash_apply(&self) -> GitResult<()> {
        self.run_git(&["stash", "apply"])?;
        Ok(())
    }

    pub fn stash_drop(&self) -> GitResult<()> {
        self.run_git(&["stash", "drop"])?;
        Ok(())
    }

    pub fn stash_list(&self) -> GitResult<String> {
        let output = self.run_git(&["stash", "list"])?;
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    pub fn current_branch(&self) -> GitResult<Option<String>> {
        let output = Command::new("git")
            .args(["rev-parse", "--abbrev-ref", "HEAD"])
            .current_dir(&self.path)
            .output()
            .map_err(|e| GitError::OperationFailed(format!("Failed to run git rev-parse: {}", e)))?;

        if !output.status.success() {
            return Ok(None);
        }
        let name = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Ok(if name == "HEAD" { None } else { Some(name) })
    }

    pub fn is_detached_head(&self) -> GitResult<bool> {
        let output = Command::new("git")
            .args(["symbolic-ref", "-q", "HEAD"])
            .current_dir(&self.path)
            .output()
            .map_err(|e| GitError::OperationFailed(format!("Failed to run git symbolic-ref: {}", e)))?;
        Ok(!output.status.success())
    }

    pub fn diff_head_to_index(&self, path: Option<&Path>) -> GitResult<String> {
        let mut args = vec!["diff", "--cached", "--no-color"];
        if let Some(p) = path {
            args.push("--");
            args.push(p.to_str().unwrap_or(""));
        }
        let output = self.run_git(&args)?;
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    pub fn add_to_gitignore(&self, pattern: &str) -> GitResult<()> {
        let gitignore_path = self.path.join(".gitignore");
        let mut existing = String::new();
        if gitignore_path.exists() {
            existing = std::fs::read_to_string(&gitignore_path)
                .map_err(|e| GitError::OperationFailed(format!("Failed to read .gitignore: {}", e)))?;
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

    pub fn resolve_conflict_use_ours(&self, path: &Path) -> GitResult<()> {
        self.run_git(&["checkout", "--ours", "--"])?;
        self.stage_paths(&[path])
    }

    pub fn resolve_conflict_use_theirs(&self, path: &Path) -> GitResult<()> {
        self.run_git(&["checkout", "--theirs", "--"])?;
        self.stage_paths(&[path])
    }

    pub fn remote_add(&self, name: &str, url: &str) -> GitResult<()> {
        self.run_git(&["remote", "add", name, url])?;
        Ok(())
    }

    pub fn remote_remove(&self, name: &str) -> GitResult<()> {
        self.run_git(&["remote", "remove", name])?;
        Ok(())
    }

    pub fn remote_rename(&self, old: &str, new: &str) -> GitResult<()> {
        self.run_git(&["remote", "rename", old, new])?;
        Ok(())
    }

    pub fn remote_set_url(&self, name: &str, url: &str) -> GitResult<()> {
        self.run_git(&["remote", "set-url", name, url])?;
        Ok(())
    }

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

    pub fn fetch_all(&self, prune: bool) -> GitResult<String> {
        let mut args = vec!["fetch", "--all"];
        if prune {
            args.push("--prune");
        }
        self.run_git_with_combined_error(&args)
    }

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

    pub fn push(&self, remote: &str, branch: Option<&str>, force: bool, set_upstream: bool) -> GitResult<String> {
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

    pub fn clone_repo(url: &str, path: &Path, bare: bool, depth: Option<usize>) -> GitResult<String> {
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

    pub fn submodule_status(&self) -> GitResult<String> {
        let output = self.run_git(&["submodule", "status"])?;
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    pub fn submodule_init(&self, path: Option<&Path>) -> GitResult<()> {
        let mut args = vec!["submodule", "init"];
        if let Some(p) = path {
            args.push("--");
            args.push(p.to_str().unwrap_or(""));
        }
        self.run_git(&args)?;
        Ok(())
    }

    pub fn submodule_update(&self, path: Option<&Path>, init: bool, recursive: bool) -> GitResult<()> {
        let mut args = vec!["submodule", "update"];
        if init {
            args.push("--init");
        }
        if recursive {
            args.push("--recursive");
        }
        if let Some(p) = path {
            args.push("--");
            args.push(p.to_str().unwrap_or(""));
        }
        self.run_git(&args)?;
        Ok(())
    }
}
