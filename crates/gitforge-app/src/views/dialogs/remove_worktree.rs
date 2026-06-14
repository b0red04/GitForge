use gitforge_ui::{AppColors, DialogColors, dialog_actions, dialog_body, dialog_overlay, dialog_surface, dialog_title};
use gpui::*;

use crate::views::app::{AppDialog, GitForgeApp};

pub fn confirm(app: &mut GitForgeApp, path: String, cx: &mut Context<GitForgeApp>) {
    app.remove_worktree(path, true, cx);
}

pub fn render(path: &str, colors: &AppColors, entity: WeakEntity<GitForgeApp>) -> Stateful<Div> {
    let dc = DialogColors::from_app(colors);

    let dialog_box = dialog_surface(px(400.0), dc)
        .child(dialog_title("Remove Worktree", dc))
        .child(dialog_body(&format!("Remove worktree at {path}?"), dc))
        .child(dialog_actions(
            "dialog-cancel",
            "dialog-confirm",
            "Remove",
            entity.clone(),
            |this, cx| this.cancel_dialog(cx),
            |this, cx| this.confirm_dialog(cx),
            dc,
        ));

    dialog_overlay(dc).child(dialog_box)
}

pub fn confirm_from_dialog(app: &mut GitForgeApp, dialog: AppDialog, cx: &mut Context<GitForgeApp>) {
    if let AppDialog::RemoveWorktree { path } = dialog {
        confirm(app, path, cx);
    }
}
