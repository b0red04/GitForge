use crate::error::{GitError, GitResult};
use crate::repository::Repository;
use std::collections::{HashMap, HashSet};

impl Repository {
    /// Spawns a `git` subprocess.
    pub fn create_branch(&self, name: &str, start_point: Option<&str>) -> GitResult<()> {
        let mut args = vec!["branch", name];
        if let Some(sp) = start_point {
            args.push(sp);
        }
        self.run_git(&args)?;
        Ok(())
    }

    /// Spawns a `git` subprocess.
    pub fn delete_branch(&self, name: &str, force: bool) -> GitResult<()> {
        if self.head_branch()?.as_deref() == Some(name) {
            return Err(GitError::OperationFailed(format!(
                "Cannot delete the currently checked-out branch '{name}'"
            )));
        }
        let flag = if force { "-D" } else { "-d" };
        self.run_git(&["branch", flag, name])?;
        Ok(())
    }

    /// Spawns a `git` subprocess.
    pub fn rename_branch(&self, old: &str, new: &str) -> GitResult<()> {
        self.run_git(&["branch", "-m", old, new])?;
        Ok(())
    }

    /// Spawns a `git` subprocess.
    pub fn checkout_branch(&self, name: &str) -> GitResult<()> {
        self.run_git(&["checkout", name])?;
        Ok(())
    }

    /// Check out a remote-tracking branch as a local branch.
    ///
    /// If a local branch with the same short name already exists, switches to it.
    /// Otherwise creates a new local branch that tracks the remote ref.
    pub fn checkout_remote_branch(&self, remote_ref: &str) -> GitResult<()> {
        let Some((_, local_name)) = remote_ref.split_once('/') else {
            return self.checkout_branch(remote_ref);
        };

        if self
            .run_git(&[
                "rev-parse",
                "--verify",
                &format!("refs/heads/{local_name}"),
            ])
            .is_ok()
        {
            self.run_git(&["checkout", local_name])?;
        } else {
            self.run_git(&["checkout", "-b", local_name, "--track", remote_ref])?;
        }
        Ok(())
    }

    /// Spawns a `git` subprocess.
    pub fn checkout_commit(&self, sha: &str) -> GitResult<()> {
        self.run_git(&["checkout", sha])?;
        Ok(())
    }

    pub fn main_branch_name(&self) -> GitResult<Option<String>> {
        for branch in ["main", "master"] {
            if self
                .run_git(&["rev-parse", "--verify", &format!("refs/heads/{branch}")])
                .is_ok()
            {
                return Ok(Some(branch.to_string()));
            }
        }

        for branch in ["origin/main", "origin/master"] {
            if self
                .run_git(&["rev-parse", "--verify", &format!("refs/remotes/{branch}")])
                .is_ok()
            {
                return Ok(Some(branch.to_string()));
            }
        }

        Ok(None)
    }

    pub fn local_branches_conflicting_with_main(
        &self,
        branches: &[String],
    ) -> GitResult<HashSet<String>> {
        let Some(base) = self.main_branch_name()? else {
            return Ok(HashSet::new());
        };

        let mut conflicting = HashSet::new();
        for branch in branches {
            if branch == &base || format!("origin/{branch}") == base {
                continue;
            }

            if self.branch_conflicts_with_base(&base, branch)? {
                conflicting.insert(branch.clone());
            }
        }

        Ok(conflicting)
    }

    pub fn branch_conflicts_with_base(&self, base: &str, branch: &str) -> GitResult<bool> {
        let output = self.run_git_raw(&["merge-tree", "--write-tree", base, branch])?;

        match output.status.code() {
            Some(0) => Ok(false),
            Some(1) => Ok(true),
            Some(code) => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                if stderr.contains("usage: git merge-tree")
                    || stderr.contains("unknown option")
                    || stderr.contains("not a git command")
                {
                    return Err(GitError::OperationFailed(
                        "git merge-tree is unavailable for conflict detection".into(),
                    ));
                }
                tracing::warn!(
                    "Unexpected git merge-tree exit code {}, assuming no conflict: {}",
                    code,
                    stderr
                );
                Ok(false)
            }
            None => {
                tracing::warn!("git merge-tree terminated by signal, assuming no conflict");
                Ok(false)
            }
        }
    }

    /// Spawns a `git` subprocess.
    pub fn create_tag(
        &self,
        name: &str,
        message: Option<&str>,
        target: Option<&str>,
    ) -> GitResult<()> {
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

    /// Spawns a `git` subprocess.
    pub fn delete_tag(&self, name: &str) -> GitResult<()> {
        self.run_git(&["tag", "-d", name])?;
        Ok(())
    }

    /// Returns `(ahead, behind)` commit counts for each local branch with an upstream.
    pub fn local_branch_tracking(&self) -> GitResult<HashMap<String, (u32, u32)>> {
        let output = self.run_git(&[
            "for-each-ref",
            "--format=%(refname:short)\t%(upstream:track)",
            "refs/heads/",
        ])?;
        let text = String::from_utf8_lossy(&output.stdout);
        let mut tracking = HashMap::new();

        for line in text.lines() {
            let Some((name, track)) = line.split_once('\t') else {
                continue;
            };
            if name.is_empty() {
                continue;
            }
            let (ahead, behind) = parse_upstream_track(track);
            if ahead > 0 || behind > 0 {
                tracking.insert(name.to_string(), (ahead, behind));
            }
        }

        Ok(tracking)
    }
}

/// Parse git `%(upstream:track)` output, e.g. `[ahead 2, behind 14]`.
pub fn parse_upstream_track(track: &str) -> (u32, u32) {
    let track = track.trim();
    if track.is_empty() {
        return (0, 0);
    }

    let inner = track.trim_start_matches('[').trim_end_matches(']');
    let mut ahead = 0;
    let mut behind = 0;

    for part in inner.split(',') {
        let part = part.trim();
        if let Some(n) = part.strip_prefix("ahead ") {
            ahead = n.trim().parse().unwrap_or(0);
        } else if let Some(n) = part.strip_prefix("behind ") {
            behind = n.trim().parse().unwrap_or(0);
        }
    }

    (ahead, behind)
}

#[cfg(test)]
mod tests {
    use super::parse_upstream_track;

    #[test]
    fn parse_upstream_track_behind_only() {
        assert_eq!(parse_upstream_track("[behind 14]"), (0, 14));
    }

    #[test]
    fn parse_upstream_track_ahead_only() {
        assert_eq!(parse_upstream_track("[ahead 2]"), (2, 0));
    }

    #[test]
    fn parse_upstream_track_both() {
        assert_eq!(parse_upstream_track("[ahead 2, behind 3]"), (2, 3));
    }

    #[test]
    fn parse_upstream_track_empty() {
        assert_eq!(parse_upstream_track(""), (0, 0));
    }

    #[test]
    fn parse_upstream_track_up_to_date() {
        assert_eq!(parse_upstream_track("[]"), (0, 0));
    }
}
