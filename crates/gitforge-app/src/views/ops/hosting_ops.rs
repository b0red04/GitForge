use gpui::Context;

use crate::views::app::GitForgeApp;

impl GitForgeApp {
    pub(crate) fn find_hosting_account(&self, provider: &str) -> Option<gitforge_hosting::HostingAccount> {
        self.hosting_accounts
            .iter()
            .find(|a| a.provider == provider)
            .cloned()
    }

    pub(crate) fn load_hosting_accounts(&mut self) {
        let path = dirs::config_dir()
            .unwrap_or_default()
            .join("gitforge")
            .join("hosting_accounts.json");

        if path.exists() {
            if let Ok(data) = std::fs::read_to_string(&path) {
                if let Ok(accounts) = serde_json::from_str(&data) {
                    self.hosting_accounts = accounts;
                }
            }
        }
    }

    pub(crate) fn save_hosting_accounts(&self) {
        let path = dirs::config_dir()
            .unwrap_or_default()
            .join("gitforge")
            .join("hosting_accounts.json");

        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        if let Ok(data) = serde_json::to_string_pretty(&self.hosting_accounts) {
            let _ = std::fs::write(&path, data);
        }
    }

    pub fn add_hosting_account(&mut self, provider: String, token: String, cx: &mut Context<Self>) {
        self.repo_session.remote_status = format!("Authenticating with {}...", provider);
        cx.notify();

        let provider_name = provider.clone();
        let token_for_auth = token.clone();
        cx.spawn(async move |this, cx| {
            let Some(p) = gitforge_hosting::get_provider(&provider_name) else {
                this.update(cx, |this, cx| {
                    this.repo_session.remote_status = format!("Unknown provider: {}", provider_name);
                    cx.notify();
                })
                .ok();
                return;
            };

            let result = p.authenticate(&token_for_auth).await;

            match result {
                Ok(account) => {
                    this.update(cx, |this, cx| {
                        this.hosting_accounts.push(account);
                        this.save_hosting_accounts();
                        this.repo_session.remote_status = "Account authenticated successfully".to_string();
                        this.notify_settings_window(cx);
                        cx.notify();
                    })
                    .ok();
                }
                Err(e) => {
                    this.update(cx, |this, cx| {
                        this.repo_session.remote_status = format!("Authentication failed: {}", e);
                        cx.notify();
                    })
                    .ok();
                }
            }
        })
        .detach();
    }

    pub fn remove_hosting_account(
        &mut self,
        username: String,
        provider: String,
        cx: &mut Context<Self>,
    ) {
        if let Some(account) = self
            .hosting_accounts
            .iter()
            .find(|a| a.username == username && a.provider == provider)
        {
            let _ = gitforge_hosting::HostingAccount::delete_token(&account.token_key);
        }
        self.hosting_accounts
            .retain(|a| !(a.username == username && a.provider == provider));
        self.save_hosting_accounts();
        self.notify_settings_window(cx);
        cx.notify();
    }

    pub fn open_clone_from_hosting_dialog(&mut self, provider: String, cx: &mut Context<Self>) {
        self.active_dialog = super::super::app::AppDialog::CloneFromHosting {
            provider: provider.clone(),
        };
        self.hosting_repos.clear();
        self.hosting_repos_loading = true;
        cx.notify();

        let account = self.find_hosting_account(&provider);

        let provider_name = provider.clone();
        cx.spawn(async move |this, cx| {
            let Some(account) = account else {
                this.update(cx, |this, cx| {
                    this.hosting_repos_loading = false;
                    this.repo_session.remote_status =
                        format!("No {} account configured. Add one first.", provider_name);
                    cx.notify();
                })
                .ok();
                return;
            };

            let provider_name = account.provider.clone();
            let Some(p) = gitforge_hosting::get_provider(&provider_name) else {
                this.update(cx, |this, cx| {
                    this.hosting_repos_loading = false;
                    this.repo_session.remote_status = "Unknown provider".to_string();
                    cx.notify();
                })
                .ok();
                return;
            };

            let result = p.list_repos(&account).await;

            match result {
                Ok(repos) => {
                    this.update(cx, |this, cx| {
                        this.hosting_repos = repos;
                        this.hosting_repos_loading = false;
                        cx.notify();
                    })
                    .ok();
                }
                Err(e) => {
                    this.update(cx, |this, cx| {
                        this.hosting_repos_loading = false;
                        this.repo_session.remote_status = format!("Failed to list repos: {}", e);
                        cx.notify();
                    })
                    .ok();
                }
            }
        })
        .detach();
    }

    pub fn clone_hosting_repo(
        &mut self,
        clone_url: String,
        repo_name: String,
        cx: &mut Context<Self>,
    ) {
        self.active_dialog = super::super::app::AppDialog::None;
        self.repo_session.remote_status = format!("Cloning {}...", repo_name);
        cx.notify();

        cx.spawn(async move |this, cx| {
            let path = dirs::home_dir()
                .unwrap_or_default()
                .join("Projects")
                .join(&repo_name);
            let path_display = path.display().to_string();
            let url = clone_url;

            let result = tokio::task::spawn_blocking(move || {
                gitforge_git::Repository::clone_repo(&url, &path, false, None)
            })
            .await;

            match result {
                Ok(Ok(_)) => {
                    let p = std::path::PathBuf::from(path_display);
                    this.update(cx, |this, cx| {
                        this.repo_session.remote_status.clear();
                        this.open_repo_from_path(p, cx);
                    })
                    .ok();
                }
                Ok(Err(e)) => {
                    this.update(cx, |this, cx| {
                        this.repo_session.remote_status = format!("Clone failed: {}", e);
                        cx.notify();
                    })
                    .ok();
                }
                Err(e) => {
                    this.update(cx, |this, cx| {
                        this.repo_session.remote_status = format!("Clone error: {}", e);
                        cx.notify();
                    })
                    .ok();
                }
            }
        })
        .detach();
    }

    pub fn search_hosting_repos(
        &mut self,
        query: String,
        provider: String,
        cx: &mut Context<Self>,
    ) {
        let account = self.find_hosting_account(&provider);

        self.hosting_repos.clear();
        self.hosting_repos_loading = true;
        cx.notify();

        let provider_name = provider.clone();
        cx.spawn(async move |this, cx| {
            let Some(account) = account else {
                this.update(cx, |this, cx| {
                    this.hosting_repos_loading = false;
                    this.repo_session.remote_status = format!("No {} account configured.", provider_name);
                    cx.notify();
                })
                .ok();
                return;
            };

            let provider_name = account.provider.clone();
            let Some(p) = gitforge_hosting::get_provider(&provider_name) else {
                this.update(cx, |this, cx| {
                    this.hosting_repos_loading = false;
                    cx.notify();
                })
                .ok();
                return;
            };

            let result = p.search_repos(&account, &query).await;

            match result {
                Ok(repos) => {
                    this.update(cx, |this, cx| {
                        this.hosting_repos = repos;
                        this.hosting_repos_loading = false;
                        cx.notify();
                    })
                    .ok();
                }
                Err(e) => {
                    this.update(cx, |this, cx| {
                        this.hosting_repos_loading = false;
                        this.repo_session.remote_status = format!("Search failed: {}", e);
                        cx.notify();
                    })
                    .ok();
                }
            }
        })
        .detach();
    }

    pub fn fork_repo(
        &mut self,
        owner: String,
        repo: String,
        provider: String,
        cx: &mut Context<Self>,
    ) {
        self.active_dialog = super::super::app::AppDialog::None;
        self.repo_session.remote_status = format!("Forking {}/{}...", owner, repo);
        cx.notify();

        let account = self.find_hosting_account(&provider);

        cx.spawn(async move |this, cx| {
            let Some(account) = account else {
                this.update(cx, |this, cx| {
                    this.repo_session.remote_status = "No account configured for fork".to_string();
                    cx.notify();
                })
                .ok();
                return;
            };

            let provider_name = account.provider.clone();
            let Some(p) = gitforge_hosting::get_provider(&provider_name) else {
                this.update(cx, |this, cx| {
                    this.repo_session.remote_status = "Unknown provider for fork".to_string();
                    cx.notify();
                })
                .ok();
                return;
            };

            let result = p.create_fork(&account, &owner, &repo).await;

            match result {
                Ok(forked) => {
                    this.update(cx, |this, cx| {
                        this.repo_session.remote_status = format!("Forked to {}", forked.full_name);
                        cx.notify();
                    })
                    .ok();
                }
                Err(e) => {
                    this.update(cx, |this, cx| {
                        this.repo_session.remote_status = format!("Fork failed: {}", e);
                        cx.notify();
                    })
                    .ok();
                }
            }
        })
        .detach();
    }
}
