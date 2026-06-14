#[derive(Debug, thiserror::Error)]
pub enum GitError {
    #[error("Repository not found: {0}")]
    RepositoryNotFound(String),

    #[error("Operation failed: {0}")]
    OperationFailed(String),
}

pub type GitResult<T> = Result<T, GitError>;

macro_rules! impl_repo_not_found {
    ($t:ty) => {
        impl From<$t> for GitError {
            fn from(e: $t) -> Self {
                GitError::RepositoryNotFound(e.to_string())
            }
        }
    };
}

impl_repo_not_found!(gix::open::Error);
impl_repo_not_found!(gix::discover::Error);
