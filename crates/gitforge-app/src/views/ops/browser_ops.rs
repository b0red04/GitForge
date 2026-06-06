use gpui::Context;


use crate::views::app::GitForgeApp;

impl GitForgeApp {
    pub fn open_in_browser(&mut self, url: String) {
        let _ = std::process::Command::new("xdg-open").arg(&url).spawn();
    }

    pub fn open_repo_in_browser(&mut self, _cx: &mut Context<Self>) {
        let Some(rs) = self.active_repo_state() else {
            return;
        };

        let remotes: Vec<_> = rs
            .references
            .iter()
            .filter(|r| r.kind == gitforge_git::RefKind::RemoteBranch)
            .filter_map(|r| r.name.split('/').next().map(|s| s.to_string()))
            .collect();

        let remote_name = if remotes.contains(&"origin".to_string()) {
            "origin"
        } else {
            match remotes.first() {
                Some(r) => r.as_str(),
                None => return,
            }
        };

        let head_branch = rs
            .references
            .iter()
            .find(|r| r.is_head && r.kind == gitforge_git::RefKind::Branch)
            .map(|r| r.name.clone());

        let remote_branch = head_branch.as_ref().and_then(|b| {
            rs.references.iter().find(|r| {
                r.kind == gitforge_git::RefKind::RemoteBranch
                    && r.name == format!("{}/{}", remote_name, b)
            })
        });

        let remote_url = rs
            .references
            .iter()
            .find(|r| {
                r.kind == gitforge_git::RefKind::RemoteBranch
                    && r.name.starts_with(&format!("{}/", remote_name))
            })
            .and_then(|_| self.get_remote_url(remote_name));

        let Some(url) = remote_url else { return };
        let clean_url = gitforge_hosting::urls::normalize_remote_url(&url);

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

    pub fn open_commit_in_browser(&mut self, commit_id: String) {
        let remote_url = self.get_first_remote_url();
        let Some(url) = remote_url else { return };

        let clean_url = gitforge_hosting::urls::normalize_remote_url(&url);
        let provider = gitforge_hosting::urls::detect_provider(&clean_url);
        let full_name = gitforge_hosting::urls::extract_repo_full_name(&clean_url);

        let browser_url = match provider {
            Some(p) => p.commit_url(&full_name, &commit_id),
            None => clean_url,
        };

        self.open_in_browser(browser_url);
    }

    pub fn open_file_at_line_in_browser(&mut self, path: String, line: Option<u32>) {
        let Some(rs) = self.active_repo_state() else {
            return;
        };

        let sha = rs.commits.first().map(|c| c.id.clone());
        let Some(sha) = sha else { return };

        let remote_url = self.get_first_remote_url();
        let Some(url) = remote_url else { return };

        let clean_url = gitforge_hosting::urls::normalize_remote_url(&url);
        let provider = gitforge_hosting::urls::detect_provider(&clean_url);
        let full_name = gitforge_hosting::urls::extract_repo_full_name(&clean_url);

        let browser_url = match provider {
            Some(p) => p.file_url(&full_name, &sha, &path, line),
            None => return,
        };

        self.open_in_browser(browser_url);
    }

    fn get_remote_url(&self, remote_name: &str) -> Option<String> {
        let open_repo = self.active_repo_handle()?;
        let repo_lock = open_repo.lock();
        let repo = repo_lock.as_ref()?;
        let remotes = repo.remote_list().ok()?;
        remotes
            .iter()
            .find(|(name, _)| name == remote_name)
            .map(|(_, url)| url.clone())
    }

    fn get_first_remote_url(&self) -> Option<String> {
        let open_repo = self.active_repo_handle()?;
        let repo_lock = open_repo.lock();
        let repo = repo_lock.as_ref()?;
        let remotes = repo.remote_list().ok()?;
        remotes.first().map(|(_, url)| url.clone())
    }
}
