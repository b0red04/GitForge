use gpui::Context;

use crate::views::app::GitForgeApp;

impl GitForgeApp {
    pub fn open_in_browser(&mut self, url: String) {
        let _ = std::process::Command::new("xdg-open").arg(&url).spawn();
    }

    pub fn open_repo_in_browser(&mut self, _cx: &mut Context<Self>) {
        let Some(rs) = self.repo_session.active_repo_state() else {
            return;
        };

        let head_branch = rs
            .references
            .iter()
            .find(|r| r.is_head && r.kind == gitforge_git::RefKind::Branch)
            .map(|r| r.name.clone());

        // Prefer the remote the current branch actually tracks — the one
        // holding its `{remote}/{branch}` ref — so we open the right host
        // when the branch was checked out from a non-origin remote. Fall
        // back to origin, then the first configured remote, when no
        // upstream ref is found.
        let remote_name = head_branch
            .as_deref()
            .and_then(|branch| {
                rs.references.iter().find_map(|r| {
                    if r.kind == gitforge_git::RefKind::RemoteBranch
                        && r.bare_remote_name() == Some(branch)
                    {
                        r.remote_name.clone()
                    } else {
                        None
                    }
                })
            })
            .or_else(|| {
                rs.remotes
                    .iter()
                    .any(|(n, _)| n == "origin")
                    .then(|| "origin".to_string())
            })
            .or_else(|| rs.remotes.first().map(|(n, _)| n.clone()));

        let Some(remote_name) = remote_name else {
            return;
        };

        let remote_branch = head_branch.as_ref().and_then(|b| {
            rs.references.iter().find(|r| {
                r.kind == gitforge_git::RefKind::RemoteBranch
                    && r.name == format!("{remote_name}/{b}")
            })
        });

        let Some(url) = rs.remote_url(&remote_name) else {
            return;
        };
        let clean_url = gitforge_hosting::urls::normalize_remote_url(url);

        let sha = remote_branch
            .map(|r| r.target_commit_id.clone())
            .or_else(|| rs.commits.first().map(|c| c.id.clone()));

        let provider = gitforge_hosting::urls::detect_provider(&clean_url);
        let full_name = gitforge_hosting::urls::extract_repo_full_name(&clean_url);

        let browser_url = match (&provider, &sha) {
            (Some(p), Some(_s)) => p.repo_url(&full_name),
            (Some(p), None) => p.repo_url(&full_name),
            (None, _) => clean_url.clone(),
        };

        self.open_in_browser(browser_url);
    }
}
