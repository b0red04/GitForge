use gitforge_ui::{
    AppColors, DialogColors, dialog_actions, dialog_body, dialog_overlay, dialog_surface,
    dialog_title,
};
use gpui::*;

use crate::views::app::{AppDialog, GitForgeApp};

pub fn confirm(
    app: &mut GitForgeApp,
    remote: String,
    branch: String,
    cx: &mut Context<GitForgeApp>,
) {
    app.delete_remote_branch(remote, branch, cx);
}

pub fn render(
    remote: &str,
    branch: &str,
    colors: &AppColors,
    entity: WeakEntity<GitForgeApp>,
    input_focus: &FocusHandle,
) -> Stateful<Div> {
    let dc = DialogColors::from_app(colors);
    let ent_key_cancel = entity.clone();
    let ent_key_confirm = entity.clone();
    let remote_key = remote.to_string();
    let branch_key = branch.to_string();
    let fh = input_focus.clone();

    let dialog_box = dialog_surface(px(380.0), dc)
        .track_focus(&fh)
        .on_click(move |_ev, window, _cx| {
            window.focus(&fh);
        })
        .on_key_down(move |ev: &KeyDownEvent, _window, cx| match ev.keystroke.key.as_str() {
            "escape" => {
                if let Some(e) = ent_key_cancel.upgrade() {
                    e.update(cx, |this, cx| {
                        this.cancel_dialog(cx);
                    });
                }
            }
            "enter" => {
                if let Some(e) = ent_key_confirm.upgrade() {
                    let (remote, branch) = (remote_key.clone(), branch_key.clone());
                    e.update(cx, |this, cx| {
                        this.active_dialog = AppDialog::None;
                        confirm(this, remote, branch, cx);
                    });
                }
            }
            _ => {}
        })
        .child(dialog_title("Delete Remote Branch", dc))
        .child(dialog_body(
            &format!(
                "Delete remote branch '{remote}/{branch}'? This affects all collaborators and cannot be undone."
            ),
            dc,
        ))
        .child(dialog_actions(
            "delete-remote-branch-cancel",
            "delete-remote-branch-confirm",
            "Delete",
            entity.clone(),
            |this, cx| this.cancel_dialog(cx),
            {
                let (remote, branch) = (remote.to_string(), branch.to_string());
                move |this, cx| {
                    this.active_dialog = AppDialog::None;
                    confirm(this, remote.clone(), branch.clone(), cx);
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
    if let AppDialog::DeleteRemoteBranch { remote, branch } = dialog {
        confirm(app, remote, branch, cx);
    }
}
