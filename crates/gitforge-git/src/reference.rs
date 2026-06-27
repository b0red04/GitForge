use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum RefKind {
    Branch,
    RemoteBranch,
    Tag,
    Stash,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefInfo {
    pub name: String,
    pub kind: RefKind,
    pub target_commit_id: String,
    pub is_head: bool,
    pub remote_name: Option<String>,
    pub commits_ahead: u32,
    pub commits_behind: u32,
}

impl RefInfo {
    /// The remote branch name with its `{remote}/` prefix stripped.
    /// `origin/main` -> `main`, `upstream/feature` -> `feature`. Returns `None`
    /// for non-remote refs or malformed names.
    pub fn bare_remote_name(&self) -> Option<&str> {
        let remote = self.remote_name.as_deref()?;
        self.name.strip_prefix(remote).and_then(|s| s.strip_prefix('/'))
    }

    /// Matches `origin/HEAD`, `upstream/HEAD`, etc. — symbolic refs that just
    /// point at the remote's default branch and add noise in ref listings.
    pub fn is_remote_head(&self) -> bool {
        self.bare_remote_name() == Some("HEAD")
    }
}
