use gitforge_ui::{
    AppColors, DialogColors, dialog_actions, dialog_body, dialog_overlay, dialog_surface,
    dialog_title,
};
use gpui::*;

use crate::views::app::{AppDialog, GitForgeApp};

pub fn confirm(
    app: &mut GitForgeApp,
    owner: String,
    repo: String,
    provider: String,
    cx: &mut Context<GitForgeApp>,
) {
    app.fork_repo(owner, repo, provider, cx);
}

pub fn render(
    owner: &str,
    repo: &str,
    colors: &AppColors,
    entity: WeakEntity<GitForgeApp>,
) -> Stateful<Div> {
    let dc = DialogColors::from_app(colors);
    let owner_owned = owner.to_string();
    let repo_owned = repo.to_string();

    let dialog_box = dialog_surface(px(360.0), dc)
        .child(dialog_title("Fork Repository", dc))
        .child(dialog_body(
            &format!("Fork {owner}/{repo} to your account?"),
            dc,
        ))
        .child(dialog_actions(
            "fork-cancel",
            "fork-confirm",
            "Fork",
            entity.clone(),
            |this, cx| this.cancel_dialog(cx),
            {
                let owner_owned = owner_owned.clone();
                let repo_owned = repo_owned.clone();
                move |this, cx| {
                    let provider = this
                        .hosting_accounts
                        .first()
                        .map(|a| a.provider.clone())
                        .unwrap_or_default();
                    confirm(this, owner_owned.clone(), repo_owned.clone(), provider, cx);
                }
            },
            dc,
        ));

    dialog_overlay(dc).child(dialog_box)
}

pub fn confirm_from_dialog(
    app: &mut GitForgeApp,
    dialog: AppDialog,
    cx: &mut Context<GitForgeApp>,
) {
    if let AppDialog::ForkRepo {
        owner,
        repo,
        provider,
    } = dialog
    {
        confirm(app, owner, repo, provider, cx);
    }
}
