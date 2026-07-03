use crate::commit::CommitInfo;
use crate::error::{GitError, GitResult, classify_git_failure};
use crate::rebase::RebasePlan;
use crate::repository::Repository;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;

impl Repository {
    /// Merge base of two refs (`git merge-base`).
    pub fn merge_base(&self, ref_a: &str, ref_b: &str) -> GitResult<String> {
        let output = self.run_git(&["merge-base", ref_a, ref_b])?;
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    /// Commits reachable from `head` but not `base`, oldest first.
    pub fn commits_in_range(&self, base: &str, head: &str) -> GitResult<Vec<CommitInfo>> {
        let range = format!("{base}..{head}");
        let output = self.run_git(&[
            "log",
            "--reverse",
            "--format=%H",
            &range,
        ])?;
        let shas: Vec<String> = String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .map(str::to_string)
            .collect();

        let mut commits = Vec::with_capacity(shas.len());
        for sha in shas {
            let id = self.parse_object_id(&sha, "commit ID")?;
            let commit = self
                .repo
                .find_commit(id)
                .map_err(|e| GitError::OperationFailed(e.to_string()))?;
            commits.push(self.commit_info_from_gix(&commit)?);
        }
        Ok(commits)
    }

    pub fn is_rebase_in_progress(&self) -> bool {
        let git_dir = self.repo.git_dir();
        git_dir.join("rebase-merge").exists() || git_dir.join("rebase-apply").exists()
    }

    /// Upstream ref for a local branch, e.g. `origin/main`.
    pub fn upstream_of(&self, branch: &str) -> GitResult<Option<String>> {
        let output = self.run_git_raw(&[
            "rev-parse",
            "--abbrev-ref",
            &format!("{branch}@{{upstream}}"),
        ])?;
        if !output.status.success() {
            return Ok(None);
        }
        let upstream = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Ok(if upstream.is_empty() {
            None
        } else {
            Some(upstream)
        })
    }

    /// Integration ref for `merge_base(ref, HEAD)..HEAD` when squashing a branch.
    ///
    /// Feature branches usually track `origin/<same-name>`; in that case the commits
    /// to rewrite are those since diverging from `main`/`master`, not since the
    /// remote tip (which is already equal to HEAD after push).
    pub fn squash_onto_ref(&self, branch: &str) -> GitResult<String> {
        if let Some(upstream) = self.upstream_of(branch)? {
            let tracks_self = upstream == format!("origin/{branch}")
                || upstream
                    .rsplit('/')
                    .next()
                    .is_some_and(|name| name == branch);
            if !tracks_self {
                return Ok(upstream);
            }
        }
        self.main_branch_name()?.ok_or_else(|| {
            GitError::OperationFailed(
                "Could not find a base branch (main/master) to squash against".into(),
            )
        })
    }

    pub fn rebase_skip(&self) -> GitResult<()> {
        self.run_git(&["rebase", "--skip"])?;
        Ok(())
    }

    /// Run `git rebase -i` using a pre-built plan (todo + editor scripts).
    pub fn rebase_interactive(&self, plan: &RebasePlan) -> GitResult<()> {
        plan.validate()?;

        for entry in &plan.entries {
            if entry.sha.len() < 40 {
                return Err(GitError::OperationFailed(format!(
                    "Invalid commit id in rebase plan: {}",
                    entry.sha
                )));
            }
        }

        let work_dir = self.rebase_work_dir()?;
        let todo_path = work_dir.join("todo.txt");
        let todo_body = plan
            .todo_lines()
            .join("\n")
            .trim()
            .to_string()
            + "\n";
        fs::write(&todo_path, todo_body).map_err(|e| {
            GitError::OperationFailed(format!("Failed to write rebase todo: {e}"))
        })?;

        let seq_editor = work_dir.join("sequence-editor.sh");
        write_executable_script(
            &seq_editor,
            &format!(
                "#!/bin/sh\nexec cp \"{}\" \"$1\"\n",
                todo_path.display()
            ),
        )?;

        let messages = plan.editor_message_queue();
        let git_editor = work_dir.join("git-editor.sh");
        let msg_dir = work_dir.join("messages");
        fs::create_dir_all(&msg_dir).map_err(|e| {
            GitError::OperationFailed(format!("Failed to create rebase message dir: {e}"))
        })?;
        for (i, msg) in messages.iter().enumerate() {
            fs::write(msg_dir.join(format!("{i}.txt")), msg).map_err(|e| {
                GitError::OperationFailed(format!("Failed to write rebase message {i}: {e}"))
            })?;
        }
        fs::write(work_dir.join("msg-index"), "0").map_err(|e| {
            GitError::OperationFailed(format!("Failed to write rebase msg index: {e}"))
        })?;
        write_executable_script(
            &git_editor,
            &format!(
                r#"#!/bin/sh
IDX_FILE="{}"
MSG_DIR="{}"
if [ ! -f "$IDX_FILE" ]; then
  : > "$1"
  exit 0
fi
IDX=$(cat "$IDX_FILE")
MSG="$MSG_DIR/$IDX.txt"
if [ -f "$MSG" ]; then
  cp "$MSG" "$1"
  echo $((IDX + 1)) > "$IDX_FILE"
else
  : > "$1"
fi
"#,
                work_dir.join("msg-index").display(),
                msg_dir.display()
            ),
        )?;

        let output = Command::new("git")
            .args(["rebase", "-i", &plan.onto])
            .env(
                "GIT_SEQUENCE_EDITOR",
                shell_quote_path(&seq_editor),
            )
            .env("GIT_EDITOR", shell_quote_path(&git_editor))
            .current_dir(&self.path)
            .output()
            .map_err(|e| GitError::OperationFailed(format!("Failed to run git rebase -i: {e}")))?;

        let _ = fs::remove_dir_all(&work_dir);

        if !output.status.success() {
            return Err(classify_git_failure(&["rebase", "-i"], &output));
        }
        Ok(())
    }

    fn rebase_work_dir(&self) -> GitResult<PathBuf> {
        let dir = self.repo.git_dir().join("gitforge-rebase-tmp");
        if dir.exists() {
            fs::remove_dir_all(&dir).ok();
        }
        fs::create_dir_all(&dir).map_err(|e| {
            GitError::OperationFailed(format!("Failed to create rebase work dir: {e}"))
        })?;
        Ok(dir)
    }
}

fn write_executable_script(path: &PathBuf, body: &str) -> GitResult<()> {
    let mut file = fs::File::create(path).map_err(|e| {
        GitError::OperationFailed(format!("Failed to create script {}: {e}", path.display()))
    })?;
    file.write_all(body.as_bytes()).map_err(|e| {
        GitError::OperationFailed(format!("Failed to write script {}: {e}", path.display()))
    })?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).map_err(|e| {
            GitError::OperationFailed(format!("Failed to chmod script {}: {e}", path.display()))
        })?;
    }
    Ok(())
}

/// Wraps a script path in POSIX single quotes so `GIT_SEQUENCE_EDITOR` /
/// `GIT_EDITOR` values survive `sh -c` invocation when the work-tree (and
/// thus the rebase temp dir) contains spaces. Embedded single quotes are
/// escaped with the standard `'\''` sequence.
fn shell_quote_path(path: &std::path::Path) -> String {
    let s = path.to_string_lossy();
    let escaped = s.replace('\'', "'\\''");
    format!("'{escaped}'")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rebase::RebasePlan;
    use std::process::Command;
    use tempfile::TempDir;

    fn init_repo() -> (TempDir, Repository) {
        let tmp = TempDir::new().unwrap();
        Command::new("git")
            .args(["init"])
            .current_dir(tmp.path())
            .output()
            .unwrap();
        Command::new("git")
            .args(["config", "user.email", "t@e.st"])
            .current_dir(tmp.path())
            .output()
            .unwrap();
        Command::new("git")
            .args(["config", "user.name", "test"])
            .current_dir(tmp.path())
            .output()
            .unwrap();
        let repo = Repository::open(tmp.path()).unwrap();
        (tmp, repo)
    }

    fn commit_file(repo: &Repository, name: &str, msg: &str) {
        std::fs::write(repo.path().join(name), msg).unwrap();
        repo.run_git(&["add", name]).unwrap();
        repo.run_git(&["commit", "-m", msg]).unwrap();
    }

    #[test]
    fn commits_in_range_oldest_first() {
        let (_tmp, repo) = init_repo();
        commit_file(&repo, "a.txt", "one");
        let base = repo.merge_base("HEAD", "HEAD").unwrap();
        commit_file(&repo, "b.txt", "two");
        commit_file(&repo, "c.txt", "three");
        let commits = repo.commits_in_range(&base, "HEAD").unwrap();
        assert_eq!(commits.len(), 2);
        assert_eq!(commits[0].summary, "two");
        assert_eq!(commits[1].summary, "three");
    }

    #[test]
    fn squash_onto_ref_uses_main_when_tracking_self() {
        let (_tmp, repo) = init_repo();
        commit_file(&repo, "a.txt", "on main");
        repo.run_git(&["branch", "-M", "main"]).unwrap();
        repo.run_git(&["checkout", "-b", "feature"]).unwrap();
        commit_file(&repo, "b.txt", "on feature");
        repo.run_git(&["config", "branch.feature.remote", "origin"]).unwrap();
        repo.run_git(&[
            "config",
            "branch.feature.merge",
            "refs/heads/feature",
        ])
        .unwrap();
        assert_eq!(repo.squash_onto_ref("feature").unwrap(), "main");
    }

    #[test]
    fn squash_all_via_interactive_rebase() {
        let (_tmp, repo) = init_repo();
        commit_file(&repo, "a.txt", "one");
        let onto = repo.merge_base("HEAD", "HEAD").unwrap();
        commit_file(&repo, "b.txt", "two");
        commit_file(&repo, "c.txt", "three");
        let range = repo.commits_in_range(&onto, "HEAD").unwrap();
        let plan = RebasePlan::squash_all_into_one(&onto, &range, "squashed");
        repo.rebase_interactive(&plan).unwrap();
        let log = repo
            .run_git(&["log", "--oneline", &format!("{onto}..HEAD")])
            .unwrap();
        let stdout = String::from_utf8_lossy(&log.stdout);
        let lines: Vec<_> = stdout
            .lines()
            .filter(|l| !l.is_empty())
            .collect();
        assert_eq!(lines.len(), 1);
        assert!(lines[0].contains("squashed"));
    }
}
