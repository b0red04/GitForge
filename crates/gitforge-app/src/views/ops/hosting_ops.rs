use std::future::Future;

use gpui::Context;

use crate::views::app::{AppDialog, GitForgeApp};
use crate::views::dialogs::AddRepoTab;
use crate::views::ops::dispatch::{AppError, BusyFlag, ErrorHandler, OpEffects};
use crate::views::settings_window::SettingsSection;
use gitforge_hosting::HostingResult;

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

    pub(crate) fn backfill_avatar_caches(&mut self, cx: &mut Context<Self>) {
        let accounts = self.hosting_accounts.clone();
        if accounts.is_empty() {
            return;
        }
        cx.spawn(async move |this, cx| {
            let mut changed = false;
            for account in accounts {
                match gitforge_hosting::ensure_avatar_cached(&account).await {
                    Ok(Some(_)) => changed = true,
                    Ok(None) => {}
                    Err(e) => tracing::warn!(
                        "avatar cache failed for {} ({}): {e}",
                        account.provider,
                        account.username
                    ),
                }
            }
            if changed {
                this.update(cx, |_, cx| cx.notify()).ok();
            }
        })
        .detach();
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
    /// handle). Adapter over `run_op_full`: hosts the `cx.spawn` + await,
    /// converts [`HostingResult`] failures through [`AppError::from`] so
    /// structured [`HostingError`] variants stay intact,
    /// surfaces an error toast on failure, and runs `on_error(detail)` first when
    /// provided so callers can clear transient state. The `op` closure builds the
    /// future so the hosting provider can be captured by value (its future borrows
    /// the provider, so it must be constructed inside the spawn).
    pub(crate) fn run_hosting_op<T, Fut, FOk>(
        &mut self,
        label: &str,
        cx: &mut Context<Self>,
        fx: OpEffects,
        op: impl FnOnce() -> Fut + Send + 'static,
        on_success: FOk,
        on_error: Option<ErrorHandler>,
    ) where
        Fut: Future<Output = HostingResult<T>> + Send + 'static,
        T: Send + 'static,
        FOk: FnOnce(&mut Self, T, &mut Context<Self>) + Send + 'static,
    {
        self.run_op_full(
            label,
            cx,
            fx,
            move || async move { op().await.map_err(AppError::from) },
            on_success,
            on_error,
            None,
        );
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

        self.run_hosting_op(
            "Authentication",
            cx,
            OpEffects::QUIET,
            move || async move { p.authenticate(&token).await },
            move |this, account, cx| {
                let display_name = account.display_name.clone();
                let provider_name = account.provider.clone();
                this.hosting_accounts.push(account.clone());
                this.save_hosting_accounts();
                let account_for_avatar = account;
                this.push_toast(
                    crate::views::toasts::ToastKind::Success,
                    format!("Signed in as {display_name} ({provider_name})"),
                    cx,
                );
                cx.spawn(async move |this, cx| {
                    match gitforge_hosting::ensure_avatar_cached(&account_for_avatar).await {
                        Ok(Some(_)) => {
                            this.update(cx, |_, cx| cx.notify()).ok();
                        }
                        Ok(None) => {}
                        Err(e) => tracing::warn!(
                            "avatar cache failed for {} ({}): {e}",
                            account_for_avatar.provider,
                            account_for_avatar.username
                        ),
                    }
                })
                .detach();
                this.notify_settings_window(cx);
                cx.notify();
            },
            None,
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
        self.load_hosting_repos(provider, cx);
    }

    /// Look up the account for `provider`, set `hosting_repos_loading = true`,
    /// and fetch its repo list off-thread via `run_hosting_op`. Populates
    /// `self.hosting_repos` on success and clears the loading flag on any
    /// outcome. Shared by `open_clone_from_hosting_dialog` and the unified
    /// `AddRepo` dialog's account-tab switching. The success and error
    /// callbacks discard stale responses whose requested provider no longer
    /// matches the active dialog/tab, so a slow in-flight list cannot
    /// clobber the view after a switch.
    fn load_hosting_repos(&mut self, provider: String, cx: &mut Context<Self>) {
        self.hosting_repos.clear();
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

        let guard = BusyFlag::HostingRepos {
            expect_provider: Some(provider.clone()),
        };
        let fx = OpEffects {
            busy: Some(guard.clone()),
            ..OpEffects::QUIET
        };
        self.run_hosting_op(
            "List repos",
            cx,
            fx,
            move || async move { p.list_repos(&account).await },
            move |this, repos, cx| {
                if guard.still_relevant(this) {
                    this.hosting_repos = repos;
                    cx.notify();
                }
            },
            None,
        );
    }

    /// The provider whose repo list the currently-active dialog/tab is
    /// expecting, or `None` when no hosting-repo view is on screen (the
    /// AddRepo dialog is on its Local tab, or a different dialog is open).
    /// Used to discard stale `list_repos` responses after a tab/dialog switch.
    pub(crate) fn active_hosting_repo_provider(&self) -> Option<&str> {
        match &self.active_dialog {
            AppDialog::CloneFromHosting { provider } => Some(provider),
            AppDialog::AddRepo => match &self.add_repo_tab {
                AddRepoTab::Account(provider) => Some(provider),
                AddRepoTab::Local => None,
            },
            _ => None,
        }
    }

    /// Open the unified "Add Repository" dialog. Defaults the active tab to
    /// the first connected account (and pre-fetches its repos) so the user
    /// lands on a populated list when any account is available; falls back to
    /// the Local tab when zero accounts are connected.
    pub fn open_add_repo_dialog(&mut self, cx: &mut Context<Self>) {
        self.active_dialog = super::super::app::AppDialog::AddRepo;
        self.hosting_repos.clear();
        self.dialog_input.clear();
        let default_tab = self
            .hosting_accounts
            .first()
            .map(|a| AddRepoTab::Account(a.provider.clone()))
            .unwrap_or(AddRepoTab::Local);
        self.add_repo_tab = default_tab.clone();
        cx.notify();

        if let AddRepoTab::Account(provider) = default_tab {
            self.load_hosting_repos(provider, cx);
        } else {
            self.hosting_repos_loading = false;
        }
    }

    /// Switch the active tab in the `AddRepo` dialog. Refetches the repo
    /// list only when landing on a *different* `Account` tab; switching to
    /// `Local` just clears any stale list. No-op if the tab is unchanged.
    pub fn switch_add_repo_tab(&mut self, tab: AddRepoTab, cx: &mut Context<Self>) {
        if self.add_repo_tab == tab {
            return;
        }
        let needs_fetch = matches!(tab, AddRepoTab::Account(_));
        self.add_repo_tab = tab;
        self.dialog_input.clear();
        self.hosting_repos.clear();
        if needs_fetch {
            if let AddRepoTab::Account(provider) = &self.add_repo_tab {
                self.load_hosting_repos(provider.clone(), cx);
            }
        } else {
            self.hosting_repos_loading = false;
            cx.notify();
        }
    }

    /// "Open Folder…" affordance from the AddRepo dialog's Local tab. Closes
    /// the dialog first so the modal isn't obscuring the native picker, then
    /// spawns the existing folder-picker flow.
    pub fn add_repo_open_local_folder(&mut self, cx: &mut Context<Self>) {
        self.cancel_dialog(cx);
        self.spawn_open_dialog(cx);
    }

    /// "Clone" affordance from the AddRepo dialog's Local tab URL input. The
    /// shared `dialog_input` carries `"URL destination-path"` (same shape as
    /// the standalone CloneRepo dialog). A missing or empty destination path
    /// surfaces a warning toast rather than no-op'ing silently.
    pub fn add_repo_clone_from_url(&mut self, cx: &mut Context<Self>) {
        let input = self.dialog_input.text().trim().to_string();
        let parts: Vec<&str> = input.splitn(2, ' ').collect();
        let Some(path) = parts.get(1).map(|s| s.trim()).filter(|s| !s.is_empty()) else {
            self.push_toast(
                crate::views::toasts::ToastKind::Warning,
                "Enter a URL and a destination path, e.g. https://example.com/repo /path/to/dir"
                    .to_string(),
                cx,
            );
            return;
        };
        let url = parts[0].to_string();
        self.cancel_dialog(cx);
        self.clone_repository(url, path.to_string(), cx);
    }

    /// "Open Settings" affordance shown in the AddRepo dialog's zero-account
    /// state. Closes the dialog and opens Settings → Accounts.
    pub fn add_repo_open_settings(&mut self, cx: &mut Context<Self>) {
        self.cancel_dialog(cx);
        self.open_settings_window(Some(SettingsSection::Accounts), cx);
    }

    pub fn clone_hosting_repo(
        &mut self,
        clone_url: String,
        repo_name: String,
        cx: &mut Context<Self>,
    ) {
        self.active_dialog = super::super::app::AppDialog::None;
        cx.notify();

        // Spawn a native folder picker so the user chooses the parent dir;
        // the clone lands in `{picked_parent}/{repo_name}`. Defaults the
        // picker to `~/Projects` (the previous hardcoded destination) when
        // it exists, else the home dir.
        cx.spawn(async move |this, cx| {
            let default_parent = dirs::home_dir()
                .map(|h| h.join("Projects"))
                .filter(|p| p.exists())
                .or_else(dirs::home_dir);
            let title = format!("Choose parent folder for \"{repo_name}\"");
            let picker = cx.update(|_cx| {
                let mut dialog = rfd::AsyncFileDialog::new().set_title(title);
                if let Some(dir) = default_parent {
                    dialog = dialog.set_directory(dir);
                }
                dialog
            });
            let folder = match picker {
                Ok(dialog) => dialog.pick_folder().await,
                Err(_) => None,
            };

            let Some(folder) = folder else {
                // User cancelled the picker — abandon the clone.
                return;
            };

            let dest = std::path::PathBuf::from(folder.path()).join(&repo_name);
            let open_path = dest.clone();
            let url = clone_url.clone();

            this.update(cx, |this, cx| {
                this.run_blocking(
                    "Clone",
                    cx,
                    super::dispatch::OpEffects::QUIET,
                    move || gitforge_git::Repository::clone_repo(&url, &dest, false, None),
                    move |this, _output, cx| {
                        this.open_repo_from_path(open_path, cx);
                    },
                    |_, _, _| {},
                );
            })
            .ok();
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

        let guard = BusyFlag::HostingRepos {
            expect_provider: Some(provider.clone()),
        };
        let fx = OpEffects {
            busy: Some(guard.clone()),
            ..OpEffects::QUIET
        };
        self.run_hosting_op(
            "Search repos",
            cx,
            fx,
            move || async move { p.search_repos(&account, &query).await },
            move |this, repos, cx| {
                if guard.still_relevant(this) {
                    this.hosting_repos = repos;
                    cx.notify();
                }
            },
            None,
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

        let Some(account) = self.find_hosting_account(&provider) else {
            self.push_toast(
                crate::views::toasts::ToastKind::Warning,
                "No account configured for fork".to_string(),
                cx,
            );
            return;
        };
        let Some(p) = gitforge_hosting::get_provider(&account.provider) else {
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
            OpEffects::QUIET,
            move || async move { p.create_fork(&account, &owner, &repo).await },
            move |this, forked, cx| {
                this.push_toast(
                    crate::views::toasts::ToastKind::Success,
                    format!("Forked to {}", forked.full_name),
                    cx,
                );
                cx.notify();
            },
            None,
        );
    }
}
