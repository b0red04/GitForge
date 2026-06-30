use crate::commit::CommitInfo;
use crate::error::GitResult;
use crate::reference::RefInfo;
use crate::repository::Repository;
use crate::repository::log_impl::CommitLogOptions;
use crate::status::RepoStatus;
use crate::worktree::WorktreeInfo;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

#[derive(Clone)]
pub struct RepoState {
    pub path: PathBuf,
    pub head_branch: Option<String>,
    pub head_commit: Option<String>,
    pub commits: Vec<CommitInfo>,
    pub references: Vec<RefInfo>,
    pub remotes: Vec<(String, String)>,
    pub conflicting_local_branches: HashSet<String>,
    pub status: RepoStatus,
    pub worktrees: Vec<WorktreeInfo>,
}

#[derive(Debug, Clone)]
pub struct RepoLoadOptions {
    pub commit_limit: usize,
    pub log_options: CommitLogOptions,
}

impl Default for RepoLoadOptions {
    fn default() -> Self {
        Self {
            commit_limit: 1000,
            log_options: CommitLogOptions::default(),
        }
    }
}

impl RepoState {
    pub fn from_repository(repo: &Repository) -> GitResult<Self> {
        Self::from_repository_with_options(repo, RepoLoadOptions::default())
    }

    pub fn from_repository_with_options(
        repo: &Repository,
        options: RepoLoadOptions,
    ) -> GitResult<Self> {
        let start = std::time::Instant::now();

        let head_branch = repo.head_branch()?;
        let head_commit = repo.head_commit()?.map(|c| c.short_id.clone());
        let commits = repo.commit_log_with_options(options.commit_limit, options.log_options)?;
        let mut references = repo.references()?;
        let branch_tracking = repo.local_branch_tracking().unwrap_or_else(|e| {
            tracing::warn!("Failed to read branch upstream tracking: {}", e);
            std::collections::HashMap::new()
        });
        for rf in &mut references {
            if rf.kind == crate::reference::RefKind::Branch {
                if let Some(&(ahead, behind)) = branch_tracking.get(&rf.name) {
                    rf.commits_ahead = ahead;
                    rf.commits_behind = behind;
                }
            }
        }
        let local_branches = references
            .iter()
            .filter(|r| r.kind == crate::reference::RefKind::Branch)
            .map(|r| r.name.clone())
            .collect::<Vec<_>>();
        let conflicting_local_branches = repo
            .local_branches_conflicting_with_main(&local_branches)
            .unwrap_or_else(|e| {
                tracing::warn!("Failed to detect branch conflicts with main: {}", e);
                HashSet::new()
            });
        let status = repo.status()?;
        let worktrees = repo.worktree_list().unwrap_or_default();
        let remotes = repo.remote_list().unwrap_or_else(|e| {
            tracing::warn!("Failed to read remotes: {}", e);
            Vec::new()
        });

        let elapsed = start.elapsed();
        tracing::info!(
            "RepoState::from_repository: {} commits, {} refs in {:.1}ms",
            commits.len(),
            references.len(),
            elapsed.as_secs_f64() * 1000.0,
        );

        Ok(Self {
            path: repo.path().to_path_buf(),
            head_branch,
            head_commit,
            commits,
            references,
            remotes,
            conflicting_local_branches,
            status,
            worktrees,
        })
    }

    /// Look up a remote URL by name from the snapshot taken at
    /// [`RepoState::from_repository`] time. Render-path callers must use this
    /// instead of acquiring the live `Repository` lock — remote URLs are
    /// immutable between refreshes, and `add_remote`/`remove_remote` already
    /// rebuild the snapshot via `OpEffects::GIT`.
    pub fn remote_url(&self, name: &str) -> Option<&str> {
        self.remotes
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, url)| url.as_str())
    }

    pub fn discover(path: &Path) -> GitResult<Self> {
        Self::discover_with_options(path, RepoLoadOptions::default())
    }

    pub fn discover_with_options(path: &Path, options: RepoLoadOptions) -> GitResult<Self> {
        let repo = Repository::discover(path)?;
        Self::from_repository_with_options(&repo, options)
    }

    pub fn discover_with_repo(
        path: &Path,
        options: RepoLoadOptions,
    ) -> GitResult<(Repository, Self)> {
        let repo = Repository::discover(path)?;
        let state = Self::from_repository_with_options(&repo, options)?;
        Ok((repo, state))
    }
}
