use gitforge_ui::{
    AppColors, DialogColors, dialog_actions, dialog_body, dialog_overlay, dialog_surface,
    dialog_title,
};
use gpui::*;

use crate::views::app::GitForgeApp;

pub fn render(
    tracked_count: usize,
    untracked_count: usize,
    colors: &AppColors,
    entity: WeakEntity<GitForgeApp>,
) -> Stateful<Div> {
    let dc = DialogColors::from_app(colors);
    let message = format!(
        "Discard changes in {tracked_count} tracked files and permanently delete {untracked_count} untracked files? This cannot be undone. Staged changes will remain."
    );

    dialog_overlay(dc).child(
        dialog_surface(px(420.0), dc)
            .child(dialog_title("Discard All Changes", dc))
            .child(dialog_body(&message, dc))
            .child(dialog_actions(
                "discard-all-cancel",
                "discard-all-confirm",
                "Discard All",
                entity,
                |this, cx| this.cancel_dialog(cx),
                |this, cx| this.confirm_dialog(cx),
                dc,
            )),
    )
}
