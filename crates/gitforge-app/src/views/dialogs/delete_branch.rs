use gitforge_ui::{AppColors, DialogColors, dialog_actions, dialog_body, dialog_overlay, dialog_surface, dialog_title};
use gpui::*;

use crate::views::app::{AppDialog, GitForgeApp};

pub fn confirm(app: &mut GitForgeApp, name: String, force: bool, cx: &mut Context<GitForgeApp>) {
    app.delete_branch(name, force, cx);
}

pub fn render(
    name: &str,
    force: bool,
    colors: &AppColors,
    entity: WeakEntity<GitForgeApp>,
    input_focus: &FocusHandle,
) -> Stateful<Div> {
    let dc = DialogColors::from_app(colors);
    let _ent_cancel = entity.clone();
    let _ent_confirm = entity.clone();
    let ent_toggle = entity.clone();
    let branch_name = name.to_string();
    let fh = input_focus.clone();
    let ent_key_cancel = entity.clone();
    let ent_key_confirm = entity.clone();
    let branch_name_key = name.to_string();

    let dialog_box = dialog_surface(px(380.0), dc)
        .track_focus(&fh)
        .on_click(move |_ev, window, _cx| {
            window.focus(&fh);
        })
        .on_key_down(move |ev: &KeyDownEvent, _window, cx| {
            match ev.keystroke.key.as_str() {
                "escape" => {
                    if let Some(e) = ent_key_cancel.upgrade() {
                        e.update(cx, |this, cx| {
                            this.cancel_dialog(cx);
                        });
                    }
                }
                "enter" => {
                    if let Some(e) = ent_key_confirm.upgrade() {
                        let name = branch_name_key.clone();
                        e.update(cx, |this, cx| {
                            this.active_dialog = AppDialog::None;
                            confirm(this, name, force, cx);
                        });
                    }
                }
                _ => {}
            }
        })
        .child(dialog_title("Delete Branch", dc))
        .child(dialog_body(
            &format!("Delete branch '{name}'? This cannot be undone."),
            dc,
        ))
        .child(
            div()
                .id("delete-branch-force")
                .flex()
                .items_center()
                .gap_2()
                .cursor_pointer()
                .on_click(move |_ev, _window, cx| {
                    if let Some(e) = ent_toggle.upgrade() {
                        e.update(cx, |this, cx| {
                            this.toggle_dialog_force(cx);
                        });
                    }
                })
                .child(
                    div()
                        .w(px(14.0))
                        .h(px(14.0))
                        .flex_shrink_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(2.0))
                        .border_1()
                        .border_color(if force { dc.accent } else { dc.border })
                        .bg(if force { dc.accent } else { dc.surface })
                        .text_color(gpui::hsla(0.0, 0.0, 1.0, 1.0))
                        .text_xs()
                        .child(if force { "\u{2713}" } else { "" }),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(dc.muted)
                        .child("Force delete (allows removing unmerged branches)"),
                ),
        )
        .child(dialog_actions(
            "delete-branch-cancel",
            "delete-branch-confirm",
            "Delete",
            entity.clone(),
            |this, cx| this.cancel_dialog(cx),
            {
                let branch_name = branch_name.clone();
                move |this, cx| {
                    this.active_dialog = AppDialog::None;
                    confirm(this, branch_name.clone(), force, cx);
                }
            },
            dc,
        ));

    dialog_overlay(dc).child(dialog_box)
}

pub fn confirm_from_dialog(
    app: &mut GitForgeApp,
    dialog: AppDialog,
    force: bool,
    cx: &mut Context<GitForgeApp>,
) {
    if let AppDialog::DeleteBranch { name } = dialog {
        confirm(app, name, force, cx);
    }
}
