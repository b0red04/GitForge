use gitforge_git::RefKind;
use gitforge_hosting::{CreatePullRequestRequest, HostingAccount, PullRequest, urls};
use gpui::Context;

use crate::views::app::{AppDialog, GitForgeApp};
use crate::views::dialogs::CreatePrDropdown;
use crate::views::ops::dispatch::{
    AppError, BusyFlag, ErrorChannel, OpEffects, RemoteError, spawn_blocking_ok, with_repo_blocking,
};
use crate::views::tab_session::OpenRepoTab;

pub(crate) struct OriginHostingContext {
    pub provider_id: String,
    pub owner: String,
    pub repo: String,
    pub account: HostingAccount,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PullRequestSidebarHint {
    NoOrigin,
    UnsupportedProvider,
    NoAccount,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PullRequestRefreshMode {
    /// First load or no cached data — show loading when list is empty.
    Initial,
    /// Post-fetch / tab re-activate — keep showing cached list.
    Background,
}

/// Apply fetched pull requests to the tab cache. Returns whether the list changed.
pub(crate) fn apply_pull_request_refresh(
    current: &mut Vec<PullRequest>,
    incoming: Vec<PullRequest>,
    mode: PullRequestRefreshMode,
) -> bool {
    match mode {
        PullRequestRefreshMode::Initial => {
            *current = incoming;
            true
        }
        PullRequestRefreshMode::Background if *current == incoming => false,
        PullRequestRefreshMode::Background => {
            *current = incoming;
            true
        }
    }
}

pub(crate) fn pull_request_refresh_mode_for_tab(tab: &OpenRepoTab) -> PullRequestRefreshMode {
    if tab.pull_requests_loaded {
        PullRequestRefreshMode::Background
    } else {
        PullRequestRefreshMode::Initial
    }
}

pub(crate) fn pull_request_for_branch<'a>(
    prs: &'a [PullRequest],
    branch: &str,
) -> Option<&'a PullRequest> {
    prs.iter()
        .find(|pr| pr.head_branch.as_deref() == Some(branch))
}

impl GitForgeApp {
    pub(crate) fn resolve_origin_hosting(&self) -> Option<OriginHostingContext> {
        let rs = self.repo_session.active_repo_state()?;
        let url = rs.remote_url("origin")?.to_string();
        let clean_url = urls::normalize_remote_url(&url);
        let provider_id = urls::detect_provider_id(&clean_url)?;
        let account = self.find_hosting_account(provider_id)?;
        let full_name = urls::extract_repo_full_name(&clean_url);
        let (owner, repo) = urls::split_repo_full_name(&full_name)?;
        Some(OriginHostingContext {
            provider_id: provider_id.to_string(),
            owner: owner.to_string(),
            repo: repo.to_string(),
            account,
        })
    }

    pub(crate) fn pull_request_sidebar_hint(&self) -> Option<PullRequestSidebarHint> {
        let rs = self.repo_session.active_repo_state()?;
        let Some(origin_url) = rs.remote_url("origin") else {
            return Some(PullRequestSidebarHint::NoOrigin);
        };
        let clean_url = urls::normalize_remote_url(origin_url);
        let Some(provider_id) = urls::detect_provider_id(&clean_url) else {
            return Some(PullRequestSidebarHint::UnsupportedProvider);
        };
        if self.find_hosting_account(provider_id).is_none() {
            return Some(PullRequestSidebarHint::NoAccount);
        }
        let full_name = urls::extract_repo_full_name(&clean_url);
        if urls::split_repo_full_name(&full_name).is_none() {
            return Some(PullRequestSidebarHint::UnsupportedProvider);
        }
        None
    }

    pub(crate) fn refresh_pull_requests(
        &mut self,
        cx: &mut Context<Self>,
        mode: PullRequestRefreshMode,
    ) {
        let Some(ctx) = self.resolve_origin_hosting() else {
            if let Some(tab) = self.repo_session.active_tab_mut() {
                if mode == PullRequestRefreshMode::Initial {
                    tab.pull_requests.clear();
                }
                tab.pull_requests_loading = false;
            }
            cx.notify();
            return;
        };

        let Some(tab_id) = self.repo_session.tabs.active_repo_tab_id else {
            return;
        };
        let Some(tab_path) = self.repo_session.active_tab().map(|t| t.path.clone()) else {
            return;
        };

        let provider = ctx.provider_id;
        let owner = ctx.owner;
        let repo = ctx.repo;
        let account = ctx.account;

        let guard = BusyFlag::PullRequests {
            tab_id,
            tab_path: tab_path.clone(),
        };
        let guard_ok = guard.clone();
        let guard_err = guard.clone();
        let busy = match mode {
            PullRequestRefreshMode::Initial => Some(guard),
            PullRequestRefreshMode::Background => None,
        };
        let fx = OpEffects {
            refresh_repo: false,
            refresh_prs: false,
            remote_status: None,
            error_channel: ErrorChannel::Silent,
            busy,
        };

        let mode_ok = mode;
        let mode_err = mode;
        self.run_op_full(
            "Refresh pull requests",
            cx,
            fx,
            move || async move {
                let p = gitforge_hosting::get_provider(&provider)
                    .ok_or_else(|| anyhow::anyhow!("Unknown provider: {}", provider))?;
                Ok(p.list_pull_requests(&account, &owner, &repo).await?)
            },
            move |this, prs, cx| {
                if guard_ok.still_relevant(this) && let Some(tab) = this.repo_session.active_tab_mut()
                {
                    tab.pull_requests_loaded = true;
                    if apply_pull_request_refresh(&mut tab.pull_requests, prs, mode_ok) {
                        cx.notify();
                    }
                }
            },
            Some(Box::new(move |this, _detail, _cx| {
                if guard_err.still_relevant(this) && mode_err == PullRequestRefreshMode::Initial
                    && let Some(tab) = this.repo_session.active_tab_mut() {
                        tab.pull_requests.clear();
                        tab.pull_requests_loaded = true;
                    }
            })),
            None,
        );
    }

    pub fn open_create_pr_dialog(&mut self, cx: &mut Context<Self>) {
        let Some(rs) = self.repo_session.active_repo_state() else {
            self.push_toast(
                crate::views::toasts::ToastKind::Warning,
                "Open a repository first",
                cx,
            );
            return;
        };

        let Some(ctx) = self.resolve_origin_hosting() else {
            // Mirror pull_request_sidebar_hint's split: distinguish no
            // origin, an unsupported/unparseable origin URL, and a
            // supported provider with no matching account.
            let Some(origin_url) = rs.remote_url("origin") else {
                self.push_toast(
                    crate::views::toasts::ToastKind::Warning,
                    "No origin remote configured",
                    cx,
                );
                return;
            };
            let clean_url = urls::normalize_remote_url(origin_url);
            let Some(provider_id) = urls::detect_provider_id(&clean_url) else {
                self.push_toast(
                    crate::views::toasts::ToastKind::Warning,
                    "Origin remote is not a supported hosting provider (GitHub, GitLab, Codeberg)",
                    cx,
                );
                return;
            };
            if self.find_hosting_account(provider_id).is_none() {
                let label = urls::provider_label(provider_id);
                self.push_toast(
                    crate::views::toasts::ToastKind::Warning,
                    format!("Add a {label} account in Settings → Accounts"),
                    cx,
                );
                return;
            }
            // Provider detected and an account exists, yet
            // resolve_origin_hosting still failed — the origin URL could
            // not be parsed into an owner/repo.
            self.push_toast(
                crate::views::toasts::ToastKind::Warning,
                "Origin remote URL could not be parsed into an owner/repo",
                cx,
            );
            return;
        };

        let full_name = format!("{}/{}", ctx.owner, ctx.repo);
        let from_branch = rs
            .references
            .iter()
            .find(|r| r.is_head && r.kind == RefKind::Branch)
            .map(|r| r.name.clone())
            .unwrap_or_default();

        let to_branch = default_base_branch(rs);

        self.create_pr.provider = ctx.provider_id;
        self.create_pr.from_repo = full_name.clone();
        self.create_pr.from_branch = from_branch;
        self.create_pr.to_repo = full_name;
        self.create_pr.to_branch = to_branch;
        self.create_pr.title_input.clear();
        self.create_pr.description_input.clear();
        self.create_pr.draft = false;
        self.create_pr.open_dropdown = CreatePrDropdown::None;
        self.create_pr.reset();

        self.populate_create_pr_branches();
        self.active_dialog = AppDialog::CreatePullRequest;
        cx.notify();

        self.load_create_pr_repos(cx);
        self.refresh_create_pr_to_branches(cx);
    }

    pub fn cancel_create_pr_dialog(&mut self, cx: &mut Context<Self>) {
        self.active_dialog = AppDialog::None;
        self.create_pr.reset();
        self.create_pr.title_input.clear();
        self.create_pr.description_input.clear();
        cx.notify();
    }

    pub fn close_create_pr_dropdown(&mut self, cx: &mut Context<Self>) {
        if self.create_pr.open_dropdown != CreatePrDropdown::None {
            self.create_pr.open_dropdown = CreatePrDropdown::None;
            self.create_pr.open_dropdown_bounds = None;
            cx.notify();
        }
    }

    pub fn toggle_create_pr_dropdown(
        &mut self,
        dropdown: CreatePrDropdown,
        cx: &mut Context<Self>,
    ) {
        if self.create_pr.open_dropdown == dropdown {
            self.create_pr.open_dropdown = CreatePrDropdown::None;
            self.create_pr.open_dropdown_bounds = None;
        } else {
            let slot = crate::views::dialogs::create_pr::dropdown_bounds_slot(dropdown);
            self.create_pr.open_dropdown = dropdown;
            self.create_pr.open_dropdown_bounds = self.create_pr.trigger_bounds[slot];
        }
        cx.notify();
    }

    pub fn select_create_pr_dropdown(
        &mut self,
        dropdown: CreatePrDropdown,
        value: String,
        cx: &mut Context<Self>,
    ) {
        match dropdown {
            CreatePrDropdown::FromRepo => {
                self.create_pr.from_repo = value;
            }
            CreatePrDropdown::FromBranch => {
                self.create_pr.from_branch = value;
            }
            CreatePrDropdown::ToRepo => {
                self.create_pr.to_repo = value.clone();
                self.refresh_create_pr_to_branches(cx);
            }
            CreatePrDropdown::ToBranch => {
                self.create_pr.to_branch = value;
            }
            CreatePrDropdown::None => {}
        }
        self.create_pr.open_dropdown = CreatePrDropdown::None;
        self.create_pr.open_dropdown_bounds = None;
        cx.notify();
    }

    pub fn toggle_create_pr_draft(&mut self, cx: &mut Context<Self>) {
        self.create_pr.draft = !self.create_pr.draft;
        cx.notify();
    }

    pub fn edit_create_pr_title(&mut self, typed_char: Option<&str>, cx: &mut Context<Self>) {
        self.create_pr.title_input.edit(typed_char);
        cx.notify();
    }

    pub fn edit_create_pr_description(&mut self, typed_char: Option<&str>, cx: &mut Context<Self>) {
        self.create_pr.description_input.edit(typed_char);
        cx.notify();
    }

    fn populate_create_pr_branches(&mut self) {
        let Some(rs) = self.repo_session.active_repo_state() else {
            self.create_pr.from_branches.clear();
            return;
        };
        let mut branches: Vec<String> = rs
            .references
            .iter()
            .filter(|r| r.kind == RefKind::Branch)
            .map(|r| r.name.clone())
            .collect();
        branches.sort();
        self.create_pr.from_branches = branches;
    }

    fn load_create_pr_repos(&mut self, cx: &mut Context<Self>) {
        let provider = self.create_pr.provider.clone();
        let Some(account) = self.find_hosting_account(&provider) else {
            self.create_pr.repos.clear();
            self.create_pr.loading_repos = false;
            cx.notify();
            return;
        };

        let guard = BusyFlag::CreatePrRepos(provider.clone());
        let fx = OpEffects {
            busy: Some(guard.clone()),
            ..OpEffects::QUIET
        };

        self.run_hosting_op(
            "List repositories",
            cx,
            fx,
            move || async move {
                let p = gitforge_hosting::get_provider(&provider)
                    .ok_or_else(|| anyhow::anyhow!("Unknown provider: {}", provider))?;
                p.list_repos(&account).await
            },
            move |this, repos, cx| {
                if guard.still_relevant(this) {
                    this.create_pr.repos = repos;
                    cx.notify();
                }
            },
            None,
        );
    }

    fn refresh_create_pr_to_branches(&mut self, cx: &mut Context<Self>) {
        let Some(rs) = self.repo_session.active_repo_state() else {
            return;
        };
        let origin_repo = rs
            .remote_url("origin")
            .map(|u| urls::extract_repo_full_name(&urls::normalize_remote_url(u)));

        if origin_repo.as_deref() == Some(self.create_pr.to_repo.as_str()) {
            let mut branches: Vec<String> = rs
                .references
                .iter()
                .filter(|r| r.kind == RefKind::RemoteBranch && r.name.starts_with("origin/"))
                .map(|r| {
                    r.name
                        .strip_prefix("origin/")
                        .unwrap_or(&r.name)
                        .to_string()
                })
                .collect();
            branches.sort();
            if !branches.is_empty() {
                self.create_pr.to_branches = branches;
                self.create_pr.loading_branches = false;
                cx.notify();
                return;
            }
        }

        let provider = self.create_pr.provider.clone();
        let to_repo = self.create_pr.to_repo.clone();
        let Some((owner, repo)) = urls::split_repo_full_name(&to_repo) else {
            self.create_pr.to_branches.clear();
            self.create_pr.loading_branches = false;
            cx.notify();
            return;
        };
        let owner = owner.to_string();
        let repo = repo.to_string();

        let Some(account) = self.find_hosting_account(&provider) else {
            self.create_pr.loading_branches = false;
            cx.notify();
            return;
        };

        let guard = BusyFlag::CreatePrBranches {
            provider: provider.clone(),
            to_repo: to_repo.clone(),
        };
        let fx = OpEffects {
            busy: Some(guard.clone()),
            ..OpEffects::QUIET
        };

        self.run_hosting_op(
            "List branches",
            cx,
            fx,
            move || async move {
                let p = gitforge_hosting::get_provider(&provider)
                    .ok_or_else(|| anyhow::anyhow!("Unknown provider: {}", provider))?;
                p.list_branches(&account, &owner, &repo).await
            },
            move |this, branches, cx| {
                if guard.still_relevant(this) {
                    this.create_pr.to_branches = branches;
                    cx.notify();
                }
            },
            None,
        );
    }

    pub fn generate_pr_title_description(&mut self, cx: &mut Context<Self>) {
        let provider_name = self.settings.ai.provider.clone();
        if provider_name == "disabled" {
            self.push_toast(
                crate::views::toasts::ToastKind::Warning,
                "Enable an AI provider in Settings",
                cx,
            );
            return;
        }

        let mut provider_config = self.settings.ai.provider_config();
        let model = provider_config.model_or_default(&provider_name);
        if model.is_empty() {
            self.push_toast(
                crate::views::toasts::ToastKind::Warning,
                format!("No model configured for provider \"{provider_name}\""),
                cx,
            );
            return;
        }
        provider_config.model = model;

        let base_branch = self.create_pr.to_branch.clone();
        let head_branch = self.create_pr.from_branch.clone();
        let req_base = base_branch.clone();
        let req_head = head_branch.clone();
        let max_diff_chars = self.settings.ai.max_diff_chars;

        let Some(handle) = self.repo_session.require_active_repo_handle() else {
            return;
        };

        self.run_op_full(
            "Generate PR description",
            cx,
            OpEffects {
                busy: Some(BusyFlag::CreatePrGeneratingAi),
                ..OpEffects::QUIET
            },
            move || async move {
                let diff = with_repo_blocking(handle, move |repo| {
                    let base = format!("origin/{}", base_branch);
                    repo.unified_diff_between_refs(&base, &head_branch)
                })
                .await?;
                if diff.trim().is_empty() {
                    return Err(AppError::Remote(RemoteError::info(
                        "No changes between selected branches",
                    )));
                }
                let provider = spawn_blocking_ok(move || {
                    gitforge_ai::create_provider(&provider_name, &provider_config)
                })
                .await?;
                let (title, body) = provider
                    .generate_pull_request_content(&diff, max_diff_chars)
                    .await?;
                Ok((title, body))
            },
            move |this, (title, body), cx| {
                if this.create_pr.to_branch == req_base && this.create_pr.from_branch == req_head {
                    this.create_pr.title_input.set_text(title);
                    this.create_pr.description_input.set_text(body);
                    cx.notify();
                }
            },
            None,
            None,
        );
    }

    pub fn submit_create_pr(&mut self, cx: &mut Context<Self>) {
        if self.create_pr.title_input.text().trim().is_empty() {
            self.push_toast(
                crate::views::toasts::ToastKind::Warning,
                "A title is required to create a pull request",
                cx,
            );
            return;
        }

        let provider = self.create_pr.provider.clone();
        let Some(account) = self.find_hosting_account(&provider) else {
            self.push_toast(
                crate::views::toasts::ToastKind::Warning,
                "No hosting account for selected provider",
                cx,
            );
            return;
        };

        if self.create_pr.from_branch.is_empty() {
            self.push_toast(
                crate::views::toasts::ToastKind::Warning,
                "Select a source branch before creating a pull request",
                cx,
            );
            return;
        }

        if !self.is_branch_pushed_to_origin(&self.create_pr.from_branch) {
            self.push_toast(
                crate::views::toasts::ToastKind::Warning,
                format!(
                    "Push branch '{}' to origin before creating a pull request",
                    self.create_pr.from_branch
                ),
                cx,
            );
            return;
        }

        let Some((to_owner, to_repo)) = urls::split_repo_full_name(&self.create_pr.to_repo) else {
            self.push_toast(
                crate::views::toasts::ToastKind::Warning,
                "Invalid target repository",
                cx,
            );
            return;
        };

        let head_owner = urls::split_repo_full_name(&self.create_pr.from_repo)
            .map(|(o, _)| o.to_string())
            .unwrap_or_else(|| account.username.clone());

        let req = CreatePullRequestRequest {
            owner: to_owner.to_string(),
            repo: to_repo.to_string(),
            title: self.create_pr.title_input.text().trim().to_string(),
            body: self.create_pr.description_input.text().to_string(),
            head_owner,
            head_branch: self.create_pr.from_branch.clone(),
            base_branch: self.create_pr.to_branch.clone(),
            draft: self.create_pr.draft,
        };

        self.run_hosting_op(
            "Create pull request",
            cx,
            OpEffects {
                busy: Some(BusyFlag::CreatePrSubmitting),
                ..OpEffects::QUIET
            },
            move || async move {
                let p = gitforge_hosting::get_provider(&provider)
                    .ok_or_else(|| anyhow::anyhow!("Unknown provider: {}", provider))?;
                p.create_pull_request(&account, &req).await
            },
            move |this, pr, cx| {
                let url = pr.html_url.clone();
                this.active_dialog = AppDialog::None;
                this.create_pr.reset();
                this.push_toast(
                    crate::views::toasts::ToastKind::Success,
                    format!("Pull request #{} created", pr.number),
                    cx,
                );
                this.open_in_browser(url);
                let pr_mode = this
                    .repo_session
                    .active_tab()
                    .map(pull_request_refresh_mode_for_tab)
                    .unwrap_or(PullRequestRefreshMode::Initial);
                this.refresh_pull_requests(cx, pr_mode);
            },
            None,
        );
    }

    fn is_branch_pushed_to_origin(&self, branch: &str) -> bool {
        let Some(rs) = self.repo_session.active_repo_state() else {
            return false;
        };
        let remote_ref = format!("origin/{}", branch);
        rs.references
            .iter()
            .any(|r| r.kind == RefKind::RemoteBranch && r.name == remote_ref)
    }
}

fn default_base_branch(rs: &gitforge_git::RepoState) -> String {
    let candidates = ["main", "master", "develop"];
    for candidate in candidates {
        if rs.references.iter().any(|r| {
            (r.kind == RefKind::Branch && r.name == candidate)
                || (r.kind == RefKind::RemoteBranch && r.name == format!("origin/{candidate}"))
        }) {
            return candidate.to_string();
        }
    }
    rs.references
        .iter()
        .find(|r| r.kind == RefKind::RemoteBranch && r.name.starts_with("origin/"))
        .map(|r| {
            r.name
                .strip_prefix("origin/")
                .unwrap_or(&r.name)
                .to_string()
        })
        .unwrap_or_else(|| "main".to_string())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Arc;

    use parking_lot::Mutex;

    use super::{
        PullRequestRefreshMode, apply_pull_request_refresh, pull_request_refresh_mode_for_tab,
    };
    use crate::views::tab_session::OpenRepoTab;
    use gitforge_hosting::PullRequest;

    fn sample_pr(number: u64, title: &str) -> PullRequest {
        PullRequest {
            number,
            title: title.to_string(),
            html_url: format!("https://example.com/pr/{number}"),
            state: "open".to_string(),
            head_branch: Some("feature".to_string()),
            draft: false,
        }
    }

    #[test]
    fn initial_refresh_always_replaces_list() {
        let mut current = vec![sample_pr(1, "Old")];
        let incoming = vec![sample_pr(2, "New")];

        assert!(apply_pull_request_refresh(
            &mut current,
            incoming.clone(),
            PullRequestRefreshMode::Initial
        ));
        assert_eq!(current, incoming);
    }

    #[test]
    fn background_refresh_skips_unchanged_list() {
        let mut current = vec![sample_pr(1, "Feature"), sample_pr(2, "Bugfix")];
        let incoming = current.clone();

        assert!(!apply_pull_request_refresh(
            &mut current,
            incoming,
            PullRequestRefreshMode::Background
        ));
        assert_eq!(current.len(), 2);
    }

    #[test]
    fn refresh_mode_uses_loaded_flag_not_list_len() {
        let tab = OpenRepoTab {
            id: 1,
            path: PathBuf::from("/tmp/repo"),
            repo: Arc::new(Mutex::new(None)),
            repo_state: None,
            loading: false,
            last_error: None,
            panel_snapshot: None,
            pull_requests: Vec::new(),
            pull_requests_loading: false,
            pull_requests_loaded: true,
        };
        assert_eq!(
            pull_request_refresh_mode_for_tab(&tab),
            PullRequestRefreshMode::Background
        );
        assert_eq!(
            pull_request_refresh_mode_for_tab(&OpenRepoTab {
                pull_requests_loaded: false,
                ..tab
            }),
            PullRequestRefreshMode::Initial
        );
    }

    #[test]
    fn background_refresh_updates_when_list_changes() {
        let mut current = vec![sample_pr(1, "Feature"), sample_pr(2, "Bugfix")];
        let incoming = vec![sample_pr(1, "Feature")];

        assert!(apply_pull_request_refresh(
            &mut current,
            incoming.clone(),
            PullRequestRefreshMode::Background
        ));
        assert_eq!(current, incoming);
    }
}
