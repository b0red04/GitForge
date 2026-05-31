use crate::error::GitResult;
use crate::repository::Repository;
use crate::repository::log_impl::CommitLogOptions;
use crate::commit::CommitInfo;
use crate::reference::RefInfo;
use crate::status::RepoStatus;
use crate::worktree::WorktreeInfo;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use parking_lot::Mutex;

#[derive(Clone)]
pub struct RepoState {
    pub path: PathBuf,
    pub head_branch: Option<String>,
    pub head_commit: Option<String>,
    pub commits: Vec<CommitInfo>,
    pub references: Vec<RefInfo>,
    pub status: RepoStatus,
    pub worktrees: Vec<WorktreeInfo>,
}

impl RepoState {
    pub fn from_repository(repo: &Repository) -> GitResult<Self> {
        Self::from_repository_with_options(repo, CommitLogOptions::default())
    }

    pub fn from_repository_with_options(repo: &Repository, log_options: CommitLogOptions) -> GitResult<Self> {
        tracing::info!("[DIAG] RepoState::from_repository starting for {:?}", repo.path());

        let head_branch = repo.head_branch()?;
        tracing::info!("[DIAG] head_branch: {:?}", head_branch);

        let head_commit = repo.head_commit()?.map(|c| c.short_id.clone());
        tracing::info!("[DIAG] head_commit: {:?}", head_commit);

        let commits = repo.commit_log_with_options(1000, log_options)?;
        tracing::info!("[DIAG] commit_log returned {} commits", commits.len());

        let references = repo.references()?;
        tracing::info!("[DIAG] references returned {} refs", references.len());

        let status = repo.status()?;
        let worktrees = repo.worktree_list().unwrap_or_default();

        Ok(Self {
            path: repo.path().to_path_buf(),
            head_branch,
            head_commit,
            commits,
            references,
            status,
            worktrees,
        })
    }

    pub fn discover(path: &Path) -> GitResult<Self> {
        let repo = Repository::discover(path)?;
        Self::from_repository(&repo)
    }
}

pub struct RepoLoader {
    state: Arc<Mutex<Option<RepoState>>>,
}

impl RepoLoader {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(None)),
        }
    }

    pub fn state(&self) -> Arc<Mutex<Option<RepoState>>> {
        self.state.clone()
    }

    pub fn spawn_load(&self, path: PathBuf) -> tokio::task::JoinHandle<GitResult<()>> {
        let state = self.state.clone();
        tokio::task::spawn_blocking(move || {
            let repo_state = RepoState::discover(&path)?;
            *state.lock() = Some(repo_state);
            Ok(())
        })
    }
}
