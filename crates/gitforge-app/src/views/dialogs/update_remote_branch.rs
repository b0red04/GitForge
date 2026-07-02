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
    app.push_current_branch(remote, branch, true, cx);
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

    let dialog_box = dialog_surface(px(420.0), dc)
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
        .child(dialog_title("Update remote branch?", dc))
        .child(dialog_body(
            &format!(
                "Your local branch '{branch}' no longer matches {remote}/{branch}. \
                 This often happens after combining commits (squash).\n\n\
                 Update the remote to match your local branch? Anyone else using this branch \
                 will need to reset their copy."
            ),
            dc,
        ))
        .child(dialog_actions(
            "update-remote-cancel",
            "update-remote-confirm",
            "Update remote",
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
    if let AppDialog::UpdateRemoteBranch { remote, branch } = dialog {
        confirm(app, remote, branch, cx);
    }
}
