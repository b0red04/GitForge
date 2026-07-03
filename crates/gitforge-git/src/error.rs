use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum GitError {
    #[error("Repository not found: {0}")]
    RepositoryNotFound(String),

    #[error("Operation failed: {0}")]
    OperationFailed(String),

    #[error("Merge conflict: {stderr}")]
    MergeConflict { paths: Vec<String>, stderr: String },

    #[error("Local changes would be overwritten by {command}: {stderr}")]
    LocalChangesOverwritten {
        command: String,
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

    /// `git push` rejected because the remote has commits not in local history.
    #[error("Push rejected: remote has different commits")]
    NonFastForwardPush {
        remote: String,
        branch: String,
        stderr: String,
    },

    /// `git pull` blocked because local and remote histories diverged.
    #[error("Pull blocked: branches have diverged")]
    DivergentBranches { stderr: String },
}

impl GitError {
    /// Constructs a user-presentable single-line message for a toast. Unlike the
    /// raw `Display` impl (which may include multi-line git stderr), this is
    /// trimmed to one line, phrased for the toast card, and has any credential
    /// URL userinfo redacted to prevent tokens/passwords from appearing in the
    /// toast UI.
    pub fn toast_message(&self) -> String {
        match self {
            GitError::RepositoryNotFound(msg) => redact_credentials(msg),
            GitError::OperationFailed(msg) => redact_credentials(&first_line(msg)),
            GitError::MergeConflict { paths, .. } if paths.is_empty() => {
                "Merge conflict".to_string()
            }
            GitError::MergeConflict { paths, .. } => {
                format!("Merge conflict in {} file(s)", paths.len())
            }
            GitError::LocalChangesOverwritten { paths, command, .. } if paths.is_empty() => {
                format!(
                    "Local changes would be overwritten — commit or stash before {}",
                    action_gerund(command)
                )
            }
            GitError::LocalChangesOverwritten { paths, command, .. } => {
                format!(
                    "Local changes in {} file(s) would be overwritten — commit or stash before {}",
                    paths.len(),
                    action_gerund(command)
                )
            }
            GitError::AuthenticationFailed { remote, stderr } => {
                format!(
                    "Authentication failed for {}: {}",
                    redact_credentials(remote),
                    redact_credentials(&first_line(stderr))
                )
            }
            GitError::NetworkError { detail } => {
                format!("Network error: {}", redact_credentials(&first_line(detail)))
            }
            GitError::IndexLock { .. } => {
                "Repository locked: another git process may be running".to_string()
            }
            GitError::EmptyCommit => "Nothing to commit".to_string(),
            GitError::BranchNotFound { name } => format!("Branch '{name}' not found"),
            GitError::BranchNotFullyMerged { name } => {
                format!("Branch '{name}' is not fully merged")
            }
            GitError::InvalidReference { label, value, .. } => format!("Invalid {label} '{value}'"),
            GitError::NonFastForwardPush { .. } => {
                "The remote still has older commits. If you combined commits locally (squash), \
                 update the remote to match your branch."
                    .to_string()
            }
            GitError::DivergentBranches { .. } => {
                "Your branch and the remote have different histories. If you combined commits \
                 locally, update the remote with Push instead of Pull."
                    .to_string()
            }
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

/// Inspects the gix error kind rather than collapsing every open failure to
/// `RepositoryNotFound`. Only [`gix::open::Error::NotARepository`] (the path
/// exists but isn't a git dir) is genuinely "not found"; everything else —
/// permission denied, unsafe ownership, corrupt config, I/O — becomes
/// `OperationFailed` so the toast surfaces the real cause instead of the
/// misleading "Repository not found".
impl From<gix::open::Error> for GitError {
    fn from(e: gix::open::Error) -> Self {
        match e {
            gix::open::Error::NotARepository { path, .. } => {
                GitError::RepositoryNotFound(path.display().to_string())
            }
            other => GitError::OperationFailed(other.to_string()),
        }
    }
}

/// Splits discover failures the same way as open: only the `NoGitRepository*`
/// upwards variants are genuine "not found"; trust failures, inaccessible
/// directories, and ceiling mismatches become `OperationFailed`. The wrapped
/// [`gix::discover::Error::Open`] variant recurses through
/// [`From<gix::open::Error>`] so the open-side classification still applies
/// when a repo is found but can't be opened.
impl From<gix::discover::Error> for GitError {
    fn from(e: gix::discover::Error) -> Self {
        match e {
            gix::discover::Error::Open(open_err) => GitError::from(open_err),
            gix::discover::Error::Discover(upwards_err) => match upwards_err {
                gix::discover::upwards::Error::NoGitRepository { path }
                | gix::discover::upwards::Error::NoGitRepositoryWithinCeiling { path, .. }
                | gix::discover::upwards::Error::NoGitRepositoryWithinFs { path, .. } => {
                    GitError::RepositoryNotFound(path.display().to_string())
                }
                other => GitError::OperationFailed(other.to_string()),
            },
        }
    }
}

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
            path: PathBuf::from(".git/index.lock"),
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
                paths: extract_conflict_paths(stderr_trim),
                stderr: stderr_trim.to_string(),
            }
        }
        "pull" | "merge" if is_local_changes_overwritten(stderr_trim) => {
            GitError::LocalChangesOverwritten {
                command: command.to_string(),
                paths: extract_overwritten_paths(stderr_trim),
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
        "push" if is_non_fast_forward(stderr_trim) => GitError::NonFastForwardPush {
            remote: extract_remote(args).unwrap_or_else(|| "origin".to_string()),
            branch: extract_push_branch(args).unwrap_or_default(),
            stderr: stderr_trim.to_string(),
        },
        "pull" if is_divergent_branches(stderr_trim) => GitError::DivergentBranches {
            stderr: stderr_trim.to_string(),
        },
        "push" if is_unresolved_push_branch(stderr_trim) => GitError::OperationFailed(
            "Can't push this ref — check out your local branch and try again. \
             After combining commits, use Update remote when prompted."
                .into(),
        ),
        _ => GitError::OperationFailed(stderr_trim.to_string()),
    }
}

pub fn first_line(s: &str) -> String {
    s.lines().next().unwrap_or(s).trim().to_string()
}

/// Redacts credential userinfo from URLs embedded in a string. Replaces the
/// `user:password@` or `token@` portion of any `scheme://...@host` URL with
/// `***@` so tokens/passwords don't leak into toast messages.
pub fn redact_credentials(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut remaining = s;

    while let Some(scheme_end) = remaining.find("://") {
        result.push_str(&remaining[..scheme_end + 3]);
        let after_scheme = &remaining[scheme_end + 3..];

        // Scan the authority segment (up to the next path separator or quote).
        let host_end = after_scheme
            .find(['/', ' ', '\'', '"'])
            .unwrap_or(after_scheme.len());
        let authority = &after_scheme[..host_end];

        if let Some(at_pos) = authority.rfind('@') {
            result.push_str("***@");
            result.push_str(&authority[at_pos + 1..]);
        } else {
            result.push_str(authority);
        }
        remaining = &after_scheme[host_end..];
    }
    result.push_str(remaining);
    result
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

/// Maps the git subcommand stored in [`GitError::LocalChangesOverwritten`]
/// to the gerund used in user-facing toast messages. Only "pull" and "merge"
/// produce that variant; anything else falls back to "pulling".
fn action_gerund(command: &str) -> &str {
    match command {
        "merge" => "merging",
        _ => "pulling",
    }
}

/// Detects git's "Your local changes ... would be overwritten by merge" refusal.
/// Emitted by `git pull` (and `git merge`) when the working tree has uncommitted
/// edits that conflict with the incoming changes.
fn is_local_changes_overwritten(stderr: &str) -> bool {
    stderr.contains("Your local changes") && stderr.contains("would be overwritten")
}

/// Extracts the file paths listed under the "would be overwritten by merge"
/// header. Git prints them one per line, indented by a tab, between the header
/// and the trailing "Please commit your changes or stash them before you merge"
/// (or "Please move or remove them before you merge.") line.
fn extract_overwritten_paths(stderr: &str) -> Vec<String> {
    let mut paths = Vec::new();
    let mut in_list = false;
    for line in stderr.lines() {
        let trimmed = line.trim();
        if in_list {
            if trimmed.is_empty()
                || trimmed.starts_with("Please ")
                || trimmed.starts_with("Aborting")
            {
                in_list = false;
                continue;
            }
            // Git prints each path tab-indented; accept any non-empty line in
            // the list region as a path (the region is delimited above/below).
            let path = trimmed.trim_matches('\'').trim_matches('"');
            if !path.is_empty() {
                paths.push(path.to_string());
            }
        } else if trimmed.contains("Your local changes to the following files") {
            in_list = true;
        }
    }
    paths
}

/// Extracts file paths from git merge/rebase conflict output. Git emits lines
/// like `CONFLICT (content): Merge conflict in path/to/file.txt` — the path
/// follows `in ` and may be quoted.
fn extract_conflict_paths(stderr: &str) -> Vec<String> {
    stderr
        .lines()
        .filter_map(|line| {
            line.trim()
                .strip_prefix("CONFLICT")
                .and_then(|rest| rest.rsplit_once(" in "))
                .map(|(_, path)| path.trim().trim_matches('\'').trim_matches('"').to_string())
                .filter(|p| !p.is_empty())
        })
        .collect()
}

fn is_empty_commit(stderr: &str) -> bool {
    stderr.contains("nothing to commit")
        || stderr.contains("no changes added to commit")
        || stderr.contains("nothing added to commit")
}

fn is_not_fully_merged(stderr: &str) -> bool {
    stderr.contains("not fully merged")
}

fn is_non_fast_forward(stderr: &str) -> bool {
    stderr.contains("non-fast-forward") || stderr.contains("failed to push some refs")
}

fn is_divergent_branches(stderr: &str) -> bool {
    stderr.contains("Need to specify how to reconcile divergent branches")
        || stderr.contains("have diverged")
}

fn is_unresolved_push_branch(stderr: &str) -> bool {
    stderr.contains("cannot be resolved to branch")
}

fn extract_push_branch(args: &[&str]) -> Option<String> {
    // `git push [-u] [--force...] <remote> <refspec>` — the refspec is the
    // second positional arg after the subcommand. It may be a bare branch
    // name or a `src:dst` refspec; derive the branch from the dst, stripping
    // any `refs/heads/` prefix.
    let positional: Vec<&&str> = args
        .iter()
        .skip(1)
        .filter(|a| !a.starts_with('-'))
        .collect();
    let refspec = positional.get(1)?;
    let dst = refspec.rsplit(':').next().unwrap_or(refspec);
    let branch = dst.strip_prefix("refs/heads/").unwrap_or(dst);
    if branch.is_empty() {
        return None;
    }
    Some(branch.to_string())
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
        let out = fail_output(
            "Auto-merging file.txt\n\
             CONFLICT (content): Merge conflict in file.txt\n\
             CONFLICT (content): Merge conflict in src/main.rs\n\
             Automatic merge failed; fix conflicts and then commit the result.",
        );
        match classify_git_failure(&["merge", "feature"], &out) {
            GitError::MergeConflict { paths, .. } => {
                assert_eq!(paths, vec!["file.txt", "src/main.rs"]);
            }
            other => panic!("expected MergeConflict, got {other:?}"),
        }
    }

    #[test]
    fn classifies_pull_local_changes_overwritten_commit_wording() {
        // The "Please commit your changes or stash them before you merge"
        // wording — git's default when the working tree blocks a pull/merge.
        let out = fail_output(
            "error: Your local changes to the following files would be overwritten by merge:\n\
            \tREADME.md\n\
            \tsrc/main.rs\n\
            Please commit your changes or stash them before you merge.\n\
            Aborting",
        );
        match classify_git_failure(&["pull", "origin", "main"], &out) {
            GitError::LocalChangesOverwritten { command, paths, .. } => {
                assert_eq!(command, "pull");
                assert_eq!(paths, vec!["README.md", "src/main.rs"]);
            }
            other => panic!("expected LocalChangesOverwritten, got {other:?}"),
        }
    }

    #[test]
    fn classifies_pull_local_changes_overwritten_move_wording() {
        // The alternate "Please move or remove them before you merge." wording.
        let out = fail_output(
            "error: Your local changes to the following files would be overwritten by merge:\n\
            \tconfig.toml\n\
            Please move or remove them before you merge.\n\
            Aborting",
        );
        match classify_git_failure(&["pull", "origin"], &out) {
            GitError::LocalChangesOverwritten { paths, .. } => {
                assert_eq!(paths, vec!["config.toml"]);
            }
            other => panic!("expected LocalChangesOverwritten, got {other:?}"),
        }
    }

    #[test]
    fn local_changes_overwritten_is_real_error_not_info() {
        let e = GitError::LocalChangesOverwritten {
            command: "pull".into(),
            paths: vec!["a.txt".into()],
            stderr: "...".into(),
        };
        assert!(!e.is_info());
    }

    #[test]
    fn local_changes_overwritten_toast_mentions_stash() {
        let e = GitError::LocalChangesOverwritten {
            command: "pull".into(),
            paths: vec!["a.txt".into(), "b.txt".into()],
            stderr: "...".into(),
        };
        let msg = e.toast_message();
        assert!(msg.contains("commit or stash"), "toast: {msg}");
        assert!(msg.contains("2 file"), "toast: {msg}");
    }

    #[test]
    fn classifies_non_fast_forward_push() {
        let out = fail_output(
            "To https://github.com/example/repo.git\n\
             ! [rejected]        main -> main (non-fast-forward)\n\
             error: failed to push some refs to 'https://github.com/example/repo.git'",
        );
        match classify_git_failure(&["push", "origin", "main"], &out) {
            GitError::NonFastForwardPush { remote, branch, .. } => {
                assert_eq!(remote, "origin");
                assert_eq!(branch, "main");
            }
            other => panic!("expected NonFastForwardPush, got {other:?}"),
        }
    }

    #[test]
    fn extract_push_branch_handles_upstream_and_refspec_forms() {
        // `["push", remote, branch]`
        assert_eq!(extract_push_branch(&["push", "origin", "main"]), Some("main".into()));
        // `["push", "-u", remote, refspec]` — must not return the remote.
        assert_eq!(
            extract_push_branch(&["push", "-u", "origin", "refs/heads/feature:refs/heads/feature"]),
            Some("feature".into())
        );
        // `["push", "--force", remote, refspec]`
        assert_eq!(
            extract_push_branch(&["push", "--force", "origin", "HEAD:refs/heads/dev"]),
            Some("dev".into())
        );
        // No refspec (push without explicit branch) → None.
        assert_eq!(extract_push_branch(&["push", "origin"]), None);
    }

    #[test]
    fn classifies_divergent_pull() {
        let out = fail_output(
            "hint: You have divergent branches and need to specify how to reconcile them.\n\
             fatal: Need to specify how to reconcile divergent branches.",
        );
        match classify_git_failure(&["pull", "origin"], &out) {
            GitError::DivergentBranches { .. } => {}
            other => panic!("expected DivergentBranches, got {other:?}"),
        }
    }

    #[test]
    fn non_fast_forward_push_toast_is_actionable() {
        let e = GitError::NonFastForwardPush {
            remote: "origin".into(),
            branch: "feature".into(),
            stderr: String::new(),
        };
        let msg = e.toast_message();
        assert!(msg.contains("squash") || msg.contains("combined"), "toast: {msg}");
    }

    #[test]
    fn local_changes_overwritten_toast_reflects_command() {
        // The toast must mirror the operation that actually failed: "merging"
        // for a failed `git merge`, "pulling" for a failed `git pull`.
        let merge = GitError::LocalChangesOverwritten {
            command: "merge".into(),
            paths: vec!["a.txt".into()],
            stderr: "...".into(),
        };
        let merge_empty = GitError::LocalChangesOverwritten {
            command: "merge".into(),
            paths: vec![],
            stderr: "...".into(),
        };
        let pull = GitError::LocalChangesOverwritten {
            command: "pull".into(),
            paths: vec!["a.txt".into()],
            stderr: "...".into(),
        };

        assert!(
            merge.toast_message().contains("before merging"),
            "merge toast should say 'merging': {}",
            merge.toast_message()
        );
        assert!(
            merge_empty.toast_message().contains("before merging"),
            "empty-paths merge toast should say 'merging': {}",
            merge_empty.toast_message()
        );
        assert!(
            pull.toast_message().contains("before pulling"),
            "pull toast should say 'pulling': {}",
            pull.toast_message()
        );
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
        let out =
            fail_output("fatal: Authentication failed for 'https://github.com/user/repo.git'");
        match classify_git_failure(&["push", "origin", "main"], &out) {
            GitError::AuthenticationFailed { remote, .. } => assert_eq!(remote, "origin"),
            other => panic!("expected AuthenticationFailed, got {other:?}"),
        }
    }

    #[test]
    fn classifies_network_error() {
        let out = fail_output(
            "fatal: unable to access 'https://github.com/user/repo.git': Could not resolve host: github.com",
        );
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

    #[test]
    fn toast_message_redacts_credentials_in_auth_failure() {
        let e = GitError::AuthenticationFailed {
            remote: "https://user:ghp_token@github.com/owner/repo.git".into(),
            stderr: "fatal: Authentication failed for 'https://user:secret@github.com/repo.git'"
                .into(),
        };
        let msg = e.toast_message();
        assert!(
            msg.contains("ghp_token") == false,
            "token leaked in toast: {msg}"
        );
        assert!(
            msg.contains("secret") == false,
            "password leaked in toast: {msg}"
        );
        assert!(msg.contains("***@"), "expected redaction marker: {msg}");
    }

    #[test]
    fn redact_credentials_strips_userinfo_from_urls() {
        assert_eq!(
            redact_credentials("https://user:token@github.com/repo.git"),
            "https://***@github.com/repo.git"
        );
        assert_eq!(
            redact_credentials("fatal: failed for 'https://x:yz@host.com/path'"),
            "fatal: failed for 'https://***@host.com/path'"
        );
        // URL without credentials is unchanged.
        assert_eq!(
            redact_credentials("https://github.com/owner/repo.git"),
            "https://github.com/owner/repo.git"
        );
        // Non-URL text is unchanged.
        assert_eq!(redact_credentials("nothing to commit"), "nothing to commit");
    }

    #[test]
    fn merge_conflict_paths_parsed_from_stderr() {
        let e = GitError::MergeConflict {
            paths: vec!["a.txt".into(), "b.txt".into(), "c.txt".into()],
            stderr: "...".into(),
        };
        assert_eq!(e.toast_message(), "Merge conflict in 3 file(s)");
    }

    // --- gix::open::Error classification ---

    #[test]
    fn open_io_permission_denied_is_not_repository_not_found() {
        // Regression: a permission-denied error used to be reported as
        // "Repository not found", hiding the real cause from the user.
        let io_err = std::io::Error::from_raw_os_error(13); // EACCES
        let open_err: gix::open::Error = io_err.into();
        let git_err: GitError = open_err.into();
        assert!(
            matches!(&git_err, GitError::OperationFailed(msg) if !msg.is_empty()),
            "expected OperationFailed with a non-empty cause message for permission denied, got {git_err:?}",
        );
    }

    #[test]
    fn open_unsafe_git_dir_is_operation_failed() {
        // An untrusted git dir (owned by another user) is a security
        // condition, not "not found" — the repo IS there.
        let open_err = gix::open::Error::UnsafeGitDir {
            path: std::path::PathBuf::from("/some/repo"),
        };
        let git_err: GitError = open_err.into();
        assert!(
            matches!(&git_err, GitError::OperationFailed(msg) if !msg.is_empty()),
            "expected OperationFailed with a non-empty cause message for UnsafeGitDir, got {git_err:?}",
        );
    }

    // --- gix::discover::Error classification ---

    #[test]
    fn discover_no_git_repository_is_repository_not_found() {
        // The genuine "not found" case must still classify correctly.
        let upwards_err = gix::discover::upwards::Error::NoGitRepository {
            path: std::path::PathBuf::from("/no/repo/here"),
        };
        let discover_err: gix::discover::Error = upwards_err.into();
        let git_err: GitError = discover_err.into();
        match git_err {
            GitError::RepositoryNotFound(path) => {
                assert_eq!(path, "/no/repo/here");
            }
            other => panic!("expected RepositoryNotFound, got {other:?}"),
        }
    }

    #[test]
    fn discover_untrusted_repo_is_operation_failed() {
        // NoTrustedGitRepository is a security/trust failure, not "not found".
        let upwards_err = gix::discover::upwards::Error::NoTrustedGitRepository {
            path: std::path::PathBuf::from("/untrusted/repo"),
            candidate: std::path::PathBuf::from("/untrusted/repo/.git"),
            required: gix::sec::Trust::Full,
        };
        let discover_err: gix::discover::Error = upwards_err.into();
        let git_err: GitError = discover_err.into();
        assert!(
            matches!(&git_err, GitError::OperationFailed(msg) if !msg.is_empty()),
            "expected OperationFailed with a non-empty cause message for untrusted repo, got {git_err:?}",
        );
    }

    #[test]
    fn discover_open_recurses_into_open_classification() {
        // When discover finds a repo but can't open it, the wrapped
        // open::Error must still get kind-aware classification.
        let io_err = std::io::Error::from_raw_os_error(13); // EACCES
        let open_err: gix::open::Error = io_err.into();
        let discover_err: gix::discover::Error = gix::discover::Error::Open(open_err);
        let git_err: GitError = discover_err.into();
        assert!(
            matches!(git_err, GitError::OperationFailed(_)),
            "expected OperationFailed (not RepositoryNotFound) for permission-denied open inside discover, got {git_err:?}"
        );
    }
}
