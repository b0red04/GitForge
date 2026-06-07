use crate::error::{GitError, GitResult};
use crate::repository::Repository;
use crate::commit::CommitInfo;
use crate::reference::{RefInfo, RefKind};
use gix::revision::walk::Sorting;
use gix::traverse::commit::simple::CommitTimeOrder;
use std::process::Command;

/// Options for walking commit history (`git log --date-order` with selected ref tips).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CommitLogOptions {
    /// When false (default), only tips on `refs/heads`, `refs/remotes`, `refs/tags`, and `refs/stash`.
    /// When true, every ref in the repo is a tip (includes e.g. `refs/t3/checkpoints/...`).
    pub include_custom_refs: bool,
}

/// Returns true for normal Git ref namespaces (excludes agent/checkpoint namespaces like `refs/t3/`).
pub fn is_standard_git_ref(full_name: &str) -> bool {
    full_name.starts_with("refs/heads/")
        || full_name.starts_with("refs/remotes/")
        || full_name.starts_with("refs/tags/")
        || full_name.starts_with("refs/stash")
}

impl Repository {
    fn head_ref(&self) -> Option<gix::Head<'_>> {
        self.repo.head().ok()
    }

    pub fn head_commit(&self) -> GitResult<Option<CommitInfo>> {
        let head = match self.head_ref() {
            Some(h) => h,
            None => return Ok(None),
        };

        let Some(id) = head.id() else {
            return Ok(None);
        };

        let commit = id.object()
            .map_err(|e| GitError::OperationFailed(e.to_string()))?
            .try_into_commit()
            .map_err(|_| GitError::OperationFailed("HEAD is not a commit".into()))?;

        Ok(Some(self.commit_info_from_gix(&commit)?))
    }

    pub fn head_branch(&self) -> GitResult<Option<String>> {
        let Some(head) = self.head_ref() else {
            return Ok(None);
        };

        Ok(head.referent_name().map(|n| n.shorten().to_string()))
    }

    pub fn commit_log(&self, max_count: usize) -> GitResult<Vec<CommitInfo>> {
        self.commit_log_with_options(max_count, CommitLogOptions::default())
    }

    /// Walk history in **commit-date order** (like `git log --date-order`), not DFS on parents.
    pub fn commit_log_with_options(
        &self,
        max_count: usize,
        options: CommitLogOptions,
    ) -> GitResult<Vec<CommitInfo>> {
        let tips = self.log_tips(options.include_custom_refs)?;

        if tips.is_empty() {
            return Ok(Vec::new());
        }

        let walk = self
            .repo
            .rev_walk(tips)
            .sorting(Sorting::ByCommitTime(CommitTimeOrder::NewestFirst))
            .all()
            .map_err(|e| GitError::OperationFailed(e.to_string()))?;

        let mut commits = Vec::with_capacity(max_count.min(256));

        for item in walk {
            if commits.len() >= max_count {
                break;
            }
            let info = item.map_err(|e| GitError::OperationFailed(e.to_string()))?;
            let commit = info
                .object()
                .map_err(|e| GitError::OperationFailed(e.to_string()))?;
            commits.push(self.commit_info_from_gix(&commit)?);
        }

        tracing::info!(
            "[DIAG] commit_log_with_options({options:?}) returned {} commits",
            commits.len()
        );
        Ok(commits)
    }

    fn log_tips(&self, include_custom_refs: bool) -> GitResult<Vec<gix::ObjectId>> {
        let refs = self
            .repo
            .references()
            .map_err(|e| GitError::OperationFailed(e.to_string()))?;

        let mut tips = Vec::new();
        for reference in refs
            .all()
            .map_err(|e| GitError::OperationFailed(e.to_string()))?
        {
            let reference = reference.map_err(|e| GitError::OperationFailed(e.to_string()))?;
            let full_name = reference.name().as_bstr().to_string();
            if !include_custom_refs && !is_standard_git_ref(&full_name) {
                continue;
            }
            if let Some(id) = reference.target().try_id() {
                tips.push(id.to_owned());
            }
        }

        if tips.is_empty() {
            if let Ok(head) = self.repo.head_id() {
                tips.push(head.detach());
            }
        }

        Ok(tips)
    }

    pub fn references(&self) -> GitResult<Vec<RefInfo>> {
        let refs = self.repo.references()
            .map_err(|e| GitError::OperationFailed(e.to_string()))?;

        let mut result = Vec::new();
        let head_branch = self.head_branch()?;

        for reference in refs.all()
            .map_err(|e| GitError::OperationFailed(e.to_string()))?
        {
            let r = reference.map_err(|e| GitError::OperationFailed(e.to_string()))?;
            let name = r.name().shorten().to_string();
            let full_name = r.name().as_bstr().to_string();

            if !is_standard_git_ref(&full_name) {
                continue;
            }

            let kind = if full_name.starts_with("refs/heads/") {
                RefKind::Branch
            } else if full_name.starts_with("refs/remotes/") {
                RefKind::RemoteBranch
            } else if full_name.starts_with("refs/tags/") {
                RefKind::Tag
            } else if full_name.starts_with("refs/stash") {
                RefKind::Stash
            } else {
                continue;
            };

            let target = r.target();
            let target_commit = match target.try_id().map(|id| id.to_hex().to_string()) {
                Some(id) => id,
                None => continue,
            };

            let is_head = head_branch.as_ref() == Some(&name);

            let remote_name = if kind == RefKind::RemoteBranch {
                name.split('/').next().map(String::from)
            } else {
                None
            };

            result.push(RefInfo {
                name,
                kind,
                target_commit_id: target_commit,
                is_head,
                remote_name,
            });
        }

        result.sort_by(|a, b| {
            let kind_order = |k: &RefKind| match k {
                RefKind::Branch => 0,
                RefKind::RemoteBranch => 1,
                RefKind::Tag => 2,
                RefKind::Stash => 3,
            };
            kind_order(&a.kind).cmp(&kind_order(&b.kind))
                .then_with(|| a.name.cmp(&b.name))
        });

        Ok(result)
    }

    /// Spawns a `git` subprocess.
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

    /// Spawns a `git` subprocess.
    pub fn is_detached_head(&self) -> GitResult<bool> {
        let output = Command::new("git")
            .args(["symbolic-ref", "-q", "HEAD"])
            .current_dir(&self.path)
            .output()
            .map_err(|e| GitError::OperationFailed(format!("Failed to run git symbolic-ref: {}", e)))?;
        Ok(!output.status.success())
    }

    fn commit_info_from_gix(&self, commit: &gix::Commit<'_>) -> GitResult<CommitInfo> {
        let id = commit.id.to_hex().to_string();
        let short_id = id[..7].to_string();
        let message_ref = commit.message().map_err(|e| GitError::OperationFailed(e.to_string()))?;
        let summary = message_ref.title.to_string();
        let body = message_ref.body.map(|b| b.to_string()).unwrap_or_default();
        let message = if body.is_empty() {
            summary.clone()
        } else {
            format!("{}\n\n{}", summary, body)
        };

        let author = commit.author().map_err(|e| GitError::OperationFailed(e.to_string()))?;
        let committer = commit.committer().map_err(|e| GitError::OperationFailed(e.to_string()))?;

        let author_time = author.time()
            .map_err(|e| GitError::OperationFailed(e.to_string()))?;
        let committer_time = committer.time()
            .map_err(|e| GitError::OperationFailed(e.to_string()))?;

        let author_dt = chrono::DateTime::from_timestamp(author_time.seconds, 0)
            .unwrap_or_default();
        let committer_dt = chrono::DateTime::from_timestamp(committer_time.seconds, 0)
            .unwrap_or_default();

        let parent_ids: Vec<String> = commit.parent_ids()
            .map(|id| id.to_hex().to_string())
            .collect();

        Ok(CommitInfo {
            id,
            short_id,
            message,
            summary,
            author_name: author.name.to_string(),
            author_email: author.email.to_string(),
            author_date: author_dt,
            committer_name: committer.name.to_string(),
            committer_email: committer.email.to_string(),
            committer_date: committer_dt,
            parent_ids,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::is_standard_git_ref;

    #[test]
    fn standard_ref_namespaces() {
        assert!(is_standard_git_ref("refs/heads/main"));
        assert!(is_standard_git_ref("refs/remotes/origin/main"));
        assert!(is_standard_git_ref("refs/tags/v1"));
        assert!(is_standard_git_ref("refs/stash"));
        assert!(!is_standard_git_ref("refs/t3/checkpoints/abc/turn/1"));
        assert!(!is_standard_git_ref("refs/custom/foo"));
    }
}
