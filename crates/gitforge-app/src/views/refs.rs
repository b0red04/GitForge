use gitforge_git::RefInfo;

/// The remote branch name with its `{remote}/` prefix stripped.
/// `origin/main` -> `main`, `upstream/feature` -> `feature`. Returns `None`
/// for non-remote refs or malformed names.
pub(super) fn bare_remote_name(rf: &RefInfo) -> Option<&str> {
    let remote = rf.remote_name.as_deref()?;
    let prefix = format!("{remote}/");
    rf.name.strip_prefix(&prefix)
}

/// Matches `origin/HEAD`, `upstream/HEAD`, etc. — symbolic refs that just point
/// at the remote's default branch and add noise in ref listings.
pub(super) fn is_remote_head(rf: &RefInfo) -> bool {
    bare_remote_name(rf) == Some("HEAD")
}
