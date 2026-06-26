pub mod add_repo;
pub mod create_pr;
pub mod credential_add;
pub mod delete_branch;
pub mod delete_remote_branch;
pub mod fork_confirm;
pub mod hosting_browse;
pub mod init_repo;
pub mod remove_worktree;
pub mod simple_input;
pub mod worktree;

use gitforge_ui::TextInput;
use gpui::*;

use crate::views::app::{AppDialog, GitForgeApp};

pub use add_repo::AddRepoTab;
pub use create_pr::{CreatePrDropdown, CreatePrState};

pub fn confirm(
    app: &mut GitForgeApp,
    dialog: AppDialog,
    input: &str,
    input_2: &str,
    dialog_force: bool,
    cx: &mut Context<GitForgeApp>,
) {
    match &dialog {
        AppDialog::None | AppDialog::CreatePullRequest | AppDialog::AddRepo => {}
        AppDialog::CloneFromHosting { .. } => {}
        d if simple_input::is_simple(d) => simple_input::confirm(app, dialog, input, input_2, cx),
        AppDialog::CredentialAdd => credential_add::confirm(app, input, input_2, cx),
        AppDialog::DeleteBranch { .. } => {
            delete_branch::confirm_from_dialog(app, dialog, dialog_force, cx)
        }
        AppDialog::DeleteRemoteBranch { .. } => {
            delete_remote_branch::confirm_from_dialog(app, dialog, cx)
        }
        AppDialog::ForkRepo { .. } => fork_confirm::confirm_from_dialog(app, dialog, cx),
        AppDialog::CreateWorktree => worktree::confirm_from_dialog(app, dialog, input, input_2, cx),
        AppDialog::RemoveWorktree { .. } => remove_worktree::confirm_from_dialog(app, dialog, cx),
        AppDialog::SearchHosting { provider } => {
            hosting_browse::confirm_search(app, input, provider.clone(), cx);
        }
        _ => {}
    }
}

pub fn render(
    dialog: &AppDialog,
    dialog_input: &TextInput,
    dialog_input_2: &TextInput,
    dialog_force: bool,
    colors: &gitforge_ui::AppColors,
    entity: WeakEntity<GitForgeApp>,
    window: &mut Window,
    hosting_accounts: &[gitforge_hosting::HostingAccount],
    hosting_repos: &[gitforge_hosting::RemoteRepo],
    hosting_repos_loading: bool,
    add_repo_tab: &AddRepoTab,
    create_pr: &CreatePrState,
) -> Stateful<Div> {
    match dialog {
        AppDialog::CreatePullRequest => create_pr::render(create_pr, colors, entity, window),
        AppDialog::DeleteBranch { name } => delete_branch::render(
            name,
            dialog_force,
            colors,
            entity,
            dialog_input.focus_handle(),
        ),
        AppDialog::DeleteRemoteBranch { remote, branch } => delete_remote_branch::render(
            remote,
            branch,
            colors,
            entity,
            dialog_input.focus_handle(),
        ),
        AppDialog::ForkRepo { owner, repo, .. } => {
            fork_confirm::render(owner, repo, colors, entity)
        }
        AppDialog::CreateWorktree => {
            worktree::render(dialog_input, dialog_input_2, colors, entity, window)
        }
        AppDialog::RemoveWorktree { path } => remove_worktree::render(path, colors, entity),
        AppDialog::CloneFromHosting { .. } | AppDialog::SearchHosting { .. } => {
            hosting_browse::render(dialog, colors, entity, hosting_repos, hosting_repos_loading)
        }
        AppDialog::AddRepo => add_repo::render(
            colors,
            entity,
            window,
            hosting_accounts,
            add_repo_tab,
            dialog_input,
            hosting_repos,
            hosting_repos_loading,
        ),
        AppDialog::CredentialAdd => {
            credential_add::render(dialog_input, dialog_input_2, colors, entity, window)
        }
        d if simple_input::is_simple(d) => {
            simple_input::render(d, dialog_input, colors, entity, window)
        }
        AppDialog::None => dialog_overlay_empty(colors),
        _ => dialog_overlay_empty(colors),
    }
}

fn dialog_overlay_empty(colors: &gitforge_ui::AppColors) -> Stateful<Div> {
    gitforge_ui::dialog_overlay(gitforge_ui::DialogColors::from_app(colors))
}
