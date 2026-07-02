use gitforge_ui::TextInput;
use gpui::*;

use crate::views::app::{AppDialog, CommitPushMode, GitForgeApp};
use crate::views::dialogs::{self, AddRepoTab, CreatePrState, SquashWizardState};

pub(crate) fn render_dialog_overlay(
    dialog: &AppDialog,
    dialog_input: &TextInput,
    dialog_input_2: &TextInput,
    dialog_force: bool,
    commit_push_mode: CommitPushMode,
    colors: &gitforge_ui::AppColors,
    entity: WeakEntity<GitForgeApp>,
    window: &mut Window,
    hosting_accounts: &[gitforge_hosting::HostingAccount],
    hosting_repos: &[gitforge_hosting::RemoteRepo],
    hosting_repos_loading: bool,
    add_repo_tab: &AddRepoTab,
    create_pr: &CreatePrState,
    squash_wizard: Option<&SquashWizardState>,
) -> Stateful<Div> {
    dialogs::render(
        dialog,
        dialog_input,
        dialog_input_2,
        dialog_force,
        commit_push_mode,
        colors,
        entity,
        window,
        hosting_accounts,
        hosting_repos,
        hosting_repos_loading,
        add_repo_tab,
        create_pr,
        squash_wizard,
    )
}
