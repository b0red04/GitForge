use gpui::Context;

use crate::views::app::GitForgeApp;

impl GitForgeApp {
    pub(crate) fn find_hosting_account(
        &self,
        provider: &str,
    ) -> Option<gitforge_hosting::HostingAccount> {
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

    /// General async seam for hosting-API operations (pure async, no repo
    /// handle). Owns the `cx.spawn` + await + 2-arm result match. On success,
    /// `on_success` receives the value; on failure, `on_error` clears transient
    /// state, then `report_op_error` surfaces a toast. The `op` closure builds
    /// the future so the hosting provider can be captured by value (its future
    /// borrows the provider, so it must be constructed inside the spawn).
    pub(crate) fn run_hosting_op<T, Fut>(
        &mut self,
        label: &str,
        cx: &mut Context<Self>,
        op: impl FnOnce() -> Fut + Send + 'static,
        on_success: impl FnOnce(&mut Self, T, &mut Context<Self>) + Send + 'static,
        on_error: impl FnOnce(&mut Self, &mut Context<Self>) + Send + 'static,
    ) where
        Fut: std::future::Future<Output = anyhow::Result<T>> + Send + 'static,
        T: Send + 'static,
    {
        let label_owned = label.to_string();
        cx.spawn(async move |this, cx| {
            let result = op().await;
            match result {
                Ok(value) => {
                    this.update(cx, |this, cx| {
                        on_success(this, value, cx);
                    })
                    .ok();
                }
                Err(e) => {
                    tracing::error!("{} failed: {}", label_owned, e);
                    this.update(cx, |this, cx| {
                        on_error(this, cx);
                        this.report_op_error(&label_owned, &e.to_string(), cx);
                    })
                    .ok();
                }
            }
        })
        .detach();
    }

    pub fn add_hosting_account(&mut self, provider: String, token: String, cx: &mut Context<Self>) {
        let Some(p) = gitforge_hosting::get_provider(&provider) else {
            self.push_toast(
                crate::views::toasts::ToastKind::Error,
                format!("Unknown provider: {}", provider),
                cx,
            );
            return;
        };
        self.repo_session.remote_status = format!("Authenticating with {}...", provider);
        cx.notify();

        self.run_hosting_op(
            "Authentication",
            cx,
            move || async move { p.authenticate(&token).await },
            move |this, account, cx| {
                this.hosting_accounts.push(account);
                this.save_hosting_accounts();
                this.repo_session.remote_status = "Account authenticated successfully".to_string();
                this.notify_settings_window(cx);
                cx.notify();
            },
            |this, _cx| {
                this.repo_session.remote_status.clear();
            },
        );
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

        let Some(account) = self.find_hosting_account(&provider) else {
            self.hosting_repos_loading = false;
            self.push_toast(
                crate::views::toasts::ToastKind::Warning,
                format!("No {} account configured. Add one first.", provider),
                cx,
            );
            return;
        };
        let Some(p) = gitforge_hosting::get_provider(&account.provider) else {
            self.hosting_repos_loading = false;
            self.push_toast(
                crate::views::toasts::ToastKind::Error,
                "Unknown provider".to_string(),
                cx,
            );
            return;
        };

        self.run_hosting_op(
            "List repos",
            cx,
            move || async move { p.list_repos(&account).await },
            move |this, repos, cx| {
                this.hosting_repos = repos;
                this.hosting_repos_loading = false;
                cx.notify();
            },
            |this, _cx| {
                this.hosting_repos_loading = false;
            },
        );
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

        let path = dirs::home_dir()
            .unwrap_or_default()
            .join("Projects")
            .join(&repo_name);
        let open_path = path.clone();
        self.run_blocking_op_returning(
            "Clone",
            cx,
            move || gitforge_git::Repository::clone_repo(&clone_url, &path, false, None),
            move |this, _output, cx| {
                this.repo_session.remote_status.clear();
                this.open_repo_from_path(open_path, cx);
            },
            |this, _cx| {
                this.repo_session.remote_status.clear();
            },
        );
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

        let Some(account) = account else {
            self.hosting_repos_loading = false;
            self.push_toast(
                crate::views::toasts::ToastKind::Warning,
                format!("No {} account configured.", provider),
                cx,
            );
            return;
        };
        let Some(p) = gitforge_hosting::get_provider(&account.provider) else {
            self.hosting_repos_loading = false;
            self.push_toast(
                crate::views::toasts::ToastKind::Error,
                "Unknown provider".to_string(),
                cx,
            );
            return;
        };

        self.run_hosting_op(
            "Search repos",
            cx,
            move || async move { p.search_repos(&account, &query).await },
            move |this, repos, cx| {
                this.hosting_repos = repos;
                this.hosting_repos_loading = false;
                cx.notify();
            },
            |this, _cx| {
                this.hosting_repos_loading = false;
            },
        );
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

        let Some(account) = self.find_hosting_account(&provider) else {
            self.repo_session.remote_status.clear();
            self.push_toast(
                crate::views::toasts::ToastKind::Warning,
                "No account configured for fork".to_string(),
                cx,
            );
            return;
        };
        let Some(p) = gitforge_hosting::get_provider(&account.provider) else {
            self.repo_session.remote_status.clear();
            self.push_toast(
                crate::views::toasts::ToastKind::Error,
                "Unknown provider for fork".to_string(),
                cx,
            );
            return;
        };

        self.run_hosting_op(
            "Fork",
            cx,
            move || async move { p.create_fork(&account, &owner, &repo).await },
            move |this, forked, cx| {
                this.repo_session.remote_status = format!("Forked to {}", forked.full_name);
                cx.notify();
            },
            |this, _cx| {
                this.repo_session.remote_status.clear();
            },
        );
    }
}
