use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum GitError {
    #[error("Repository not found: {0}")]
    RepositoryNotFound(String),

    #[error("Operation failed: {0}")]
    OperationFailed(String),

    #[error("Merge conflict: {stderr}")]
    MergeConflict {
        paths: Vec<String>,
        stderr: String,
    },

    #[error("Authentication failed for {remote}: {stderr}")]
    AuthenticationFailed { remote: String, stderr: String },

    #[error("Network error: {detail}")]
    NetworkError { detail: String },

    #[error("Repository index locked: {path}")]
    IndexLock { path: PathBuf },

    #[error("Nothing to commit")]
    EmptyCommit,

    #[error("Branch not found: {name}")]
    BranchNotFound { name: String },

    #[error("Branch '{name}' is not fully merged")]
    BranchNotFullyMerged { name: String },

    #[error("Invalid {label} '{value}': {reason}")]
    InvalidReference {
        label: String,
        value: String,
        reason: String,
    },
}

impl GitError {
    /// Constructs a user-presentable single-line message for a toast. Unlike the
    /// raw `Display` impl (which may include multi-line git stderr), this is
    /// trimmed to one line and phrased for the toast card.
    pub fn toast_message(&self) -> String {
        match self {
            GitError::RepositoryNotFound(msg) => msg.clone(),
            GitError::OperationFailed(msg) => first_line(msg),
            GitError::MergeConflict { paths, .. } if paths.is_empty() => {
                "Merge conflict".to_string()
            }
            GitError::MergeConflict { paths, .. } => {
                format!("Merge conflict in {} file(s)", paths.len())
            }
            GitError::AuthenticationFailed { remote, stderr } => {
                format!("Authentication failed for {remote}: {}", first_line(stderr))
            }
            GitError::NetworkError { detail } => format!("Network error: {}", first_line(detail)),
            GitError::IndexLock { .. } => {
                "Repository locked: another git process may be running".to_string()
            }
            GitError::EmptyCommit => "Nothing to commit".to_string(),
            GitError::BranchNotFound { name } => format!("Branch '{name}' not found"),
            GitError::BranchNotFullyMerged { name } => {
                format!("Branch '{name}' is not fully merged")
            }
            GitError::InvalidReference {
                label, value, ..
            } => format!("Invalid {label} '{value}'"),
            // Note: `reason` is omitted from toast (kept in full Display).
        }
    }

    /// True if this error is an informational condition rather than an operation
    /// failure. `EmptyCommit` is the canonical case: the user asked to commit
    /// but there was nothing staged, which is not an error.
    pub fn is_info(&self) -> bool {
        matches!(self, GitError::EmptyCommit)
    }
}

pub type GitResult<T> = Result<T, GitError>;

macro_rules! impl_from_gix_error {
    ($t:ty) => {
        impl From<$t> for GitError {
            fn from(e: $t) -> Self {
                GitError::RepositoryNotFound(e.to_string())
            }
        }
    };
}

impl_from_gix_error!(gix::open::Error);
impl_from_gix_error!(gix::discover::Error);

/// Classifies a failed `git` subprocess invocation into the most specific
/// `GitError` variant, falling back to `OperationFailed`. This is the single
/// place where git's human-readable stderr is pattern-matched, concentrating
/// the brittle parsing here instead of scattering it across call sites.
///
/// `args` is the argument vector passed to `git` (e.g. `["merge", "feature"]`);
/// `output` is the subprocess result (whose `status.success()` is false).
pub(crate) fn classify_git_failure(args: &[&str], output: &std::process::Output) -> GitError {
    let command = args.first().copied().unwrap_or("");
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stderr_trim = stderr.trim();

    // Command-generic: index lock can appear from any mutating command.
    if stderr_trim.contains("index.lock") {
        return GitError::IndexLock {
            path: PathBuf::from(extract_index_lock_path(stderr_trim)),
        };
    }

    // Command-generic: authentication / network failures (push, pull, fetch, clone).
    if is_auth_failure(stderr_trim) {
        let remote = extract_remote(args).unwrap_or_else(|| "remote".to_string());
        return GitError::AuthenticationFailed {
            remote,
            stderr: stderr_trim.to_string(),
        };
    }
    if is_network_error(stderr_trim) {
        return GitError::NetworkError {
            detail: stderr_trim.to_string(),
        };
    }

    // Command-specific classification.
    match command {
        "merge" | "rebase" | "cherry-pick" if is_merge_conflict(stderr_trim) => {
            GitError::MergeConflict {
                paths: Vec::new(),
                stderr: stderr_trim.to_string(),
            }
        }
        "commit" | "commit-tree" if is_empty_commit(stderr_trim) => GitError::EmptyCommit,
        "branch" if is_not_fully_merged(stderr_trim) => GitError::BranchNotFullyMerged {
            name: extract_branch_name(args).unwrap_or_default(),
        },
        "branch" | "checkout" | "switch" if is_branch_not_found(stderr_trim) => {
            GitError::BranchNotFound {
                name: extract_branch_name(args).unwrap_or_default(),
            }
        }
        _ => GitError::OperationFailed(stderr_trim.to_string()),
    }
}

fn first_line(s: &str) -> String {
    s.lines().next().unwrap_or(s).trim().to_string()
}

fn extract_index_lock_path(stderr: &str) -> String {
    stderr
        .lines()
        .find(|line| line.contains("index.lock"))
        .and_then(|line| {
            let l = line.trim();
            let idx = l.find("index.lock")?;
            let after = &l[..idx];
            if let Some(start) = after.rfind(['/', '\\']) {
                let path_part = after[start + 1..].trim_matches(['\'', '"', ' ']);
                if !path_part.is_empty() {
                    return Some(format!(".git/{}index.lock", path_part));
                }
            }
            Some(".git/index.lock".to_string())
        })
        .unwrap_or_else(|| ".git/index.lock".to_string())
}

fn is_auth_failure(stderr: &str) -> bool {
    stderr.contains("Authentication failed")
        || stderr.contains("Permission denied")
        || stderr.contains("could not read Username")
        || stderr.contains("Invalid username or password")
        || stderr.contains("fatal: could not read Password")
        || stderr.contains("Support for password authentication was removed")
}

fn is_network_error(stderr: &str) -> bool {
    stderr.contains("Could not resolve host")
        || stderr.contains("Connection timed out")
        || stderr.contains("Connection refused")
        || stderr.contains("Failed to connect")
        || stderr.contains("RPC failed")
        || stderr.contains("Connection reset")
        || stderr.contains("early EOF")
}

fn extract_remote(args: &[&str]) -> Option<String> {
    // For push/pull/fetch, the remote is the first non-flag arg after the command.
    args.iter()
        .skip(1)
        .copied()
        .find(|a| !a.starts_with('-'))
        .map(|s| s.to_string())
}

fn is_merge_conflict(stderr: &str) -> bool {
    stderr.contains("CONFLICT")
        || stderr.contains("Merge conflict")
        || stderr.contains("Automatic merge failed")
        || stderr.contains("merge conflict")
}

fn is_empty_commit(stderr: &str) -> bool {
    stderr.contains("nothing to commit")
        || stderr.contains("no changes added to commit")
        || stderr.contains("nothing added to commit")
}

fn is_not_fully_merged(stderr: &str) -> bool {
    stderr.contains("not fully merged")
}

fn is_branch_not_found(stderr: &str) -> bool {
    stderr.contains("did not match")
        || stderr.contains("not found")
        || stderr.contains("unknown revision")
        || stderr.contains("pathspec")
}

fn extract_branch_name(args: &[&str]) -> Option<String> {
    // branch/delete: ["branch", "-d", "name"] or ["branch", "-D", "name"]
    // checkout: ["checkout", "name"]
    // The branch name is the last non-flag argument that isn't the command itself.
    let command = args.first().copied().unwrap_or("");
    args.iter()
        .rev()
        .find(|a| !a.starts_with('-') && **a != command)
        .map(|s| s.to_string())
}
#[cfg(test)]
mod tests {
    use super::*;

    fn fail_output(stderr: &str) -> std::process::Output {
        #[cfg(unix)]
        {
            use std::os::unix::process::ExitStatusExt;
            return std::process::Output {
                status: std::process::ExitStatus::from_raw(1),
                stdout: Vec::new(),
                stderr: stderr.as_bytes().to_vec(),
            };
        }
        #[cfg(not(unix))]
        {
            return std::process::Output {
                status: std::process::ExitStatus::default(),
                stdout: Vec::new(),
                stderr: stderr.as_bytes().to_vec(),
            };
        }
    }

    #[test]
    fn classifies_index_lock() {
        let out = fail_output(
            "error: unable to create '.git/index.lock': File exists.\n\
             Another git process seems to be running in this repository.",
        );
        assert!(matches!(
            classify_git_failure(&["merge", "feature"], &out),
            GitError::IndexLock { .. }
        ));
    }

    #[test]
    fn classifies_merge_conflict() {
        let out = fail_output("Auto-merging file.txt\nCONFLICT (content): Merge conflict in file.txt\nAutomatic merge failed; fix conflicts and then commit the result.");
        assert!(matches!(
            classify_git_failure(&["merge", "feature"], &out),
            GitError::MergeConflict { .. }
        ));
    }

    #[test]
    fn classifies_empty_commit() {
        let out = fail_output("On branch main\nnothing to commit, working tree clean");
        assert!(matches!(
            classify_git_failure(&["commit", "-m", "msg"], &out),
            GitError::EmptyCommit
        ));
    }

    #[test]
    fn classifies_branch_not_fully_merged() {
        let out = fail_output(
            "error: The branch 'feature/x' is not fully merged.\n\
             If you are sure you want to delete it, run 'git branch -D feature/x'",
        );
        match classify_git_failure(&["branch", "-d", "feature/x"], &out) {
            GitError::BranchNotFullyMerged { name } => assert_eq!(name, "feature/x"),
            other => panic!("expected BranchNotFullyMerged, got {other:?}"),
        }
    }

    #[test]
    fn classifies_branch_not_found() {
        let out = fail_output("error: branch 'nope' not found.");
        match classify_git_failure(&["branch", "-d", "nope"], &out) {
            GitError::BranchNotFound { name } => assert_eq!(name, "nope"),
            other => panic!("expected BranchNotFound, got {other:?}"),
        }
    }

    #[test]
    fn classifies_auth_failure() {
        let out = fail_output("fatal: Authentication failed for 'https://github.com/user/repo.git'");
        match classify_git_failure(&["push", "origin", "main"], &out) {
            GitError::AuthenticationFailed { remote, .. } => assert_eq!(remote, "origin"),
            other => panic!("expected AuthenticationFailed, got {other:?}"),
        }
    }

    #[test]
    fn classifies_network_error() {
        let out = fail_output("fatal: unable to access 'https://github.com/user/repo.git': Could not resolve host: github.com");
        assert!(matches!(
            classify_git_failure(&["fetch", "origin"], &out),
            GitError::NetworkError { .. }
        ));
    }

    #[test]
    fn falls_back_to_operation_failed() {
        let out = fail_output("some random error");
        assert!(matches!(
            classify_git_failure(&["merge", "feature"], &out),
            GitError::OperationFailed(_)
        ));
    }

    #[test]
    fn toast_message_handles_multiline() {
        let e = GitError::OperationFailed("first line\nsecond line".to_string());
        assert_eq!(e.toast_message(), "first line");
    }

    #[test]
    fn toast_message_for_merge_conflict() {
        let e = GitError::MergeConflict {
            paths: vec!["a.txt".into(), "b.txt".into()],
            stderr: "...".into(),
        };
        assert_eq!(e.toast_message(), "Merge conflict in 2 file(s)");
    }

    #[test]
    fn toast_message_for_empty_commit() {
        assert_eq!(GitError::EmptyCommit.toast_message(), "Nothing to commit");
    }
}
