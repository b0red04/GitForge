use gitforge_ui::{
    AppColors, DialogColors, dialog_body, dialog_overlay, dialog_surface, dialog_title,
    rgba_to_hsla,
};
use gpui::*;

use crate::views::app::{AppDialog, CommitPushMode, GitForgeApp};

pub fn confirm(
    app: &mut GitForgeApp,
    current_branch: String,
    mode: CommitPushMode,
    cx: &mut Context<GitForgeApp>,
) {
    app.commit_push_mode = mode;
    app.confirm_commit_and_push(current_branch, cx);
}

pub fn render(
    current_branch: &str,
    detached: bool,
    mode: CommitPushMode,
    colors: &AppColors,
    entity: WeakEntity<GitForgeApp>,
) -> Stateful<Div> {
    let dc = DialogColors::from_app(colors);
    let border = dc.border;
    let text_color = dc.text;
    let muted = dc.muted;
    let accent = dc.accent;
    let surface = dc.surface;

    let current_selected = mode == CommitPushMode::CurrentBranch;
    let feature_selected = mode == CommitPushMode::FeatureBranch;
    let can_confirm = if feature_selected {
        true
    } else {
        !detached
    };

    let ent_key_cancel = entity.clone();
    let ent_key_confirm = entity.clone();
    let current_branch_key = current_branch.to_string();
    let mode_key = mode;

    let branch_label = if detached {
        "(detached HEAD)".to_string()
    } else if current_branch.is_empty() {
        "(unknown)".to_string()
    } else {
        current_branch.to_string()
    };

    let mut dialog_box = dialog_surface(px(420.0), dc)
        .on_key_down(
            move |ev: &KeyDownEvent, _window, cx| match ev.keystroke.key.as_str() {
                "escape" => {
                    if let Some(e) = ent_key_cancel.upgrade() {
                        e.update(cx, |this, cx| {
                            this.cancel_dialog(cx);
                        });
                    }
                }
                "enter" if can_confirm => {
                    if let Some(e) = ent_key_confirm.upgrade() {
                        let current_branch = current_branch_key.clone();
                        e.update(cx, |this, cx| {
                            this.active_dialog = AppDialog::None;
                            confirm(this, current_branch, mode_key, cx);
                        });
                    }
                }
                _ => {}
            },
        )
        .child(dialog_title("Commit & Push", dc))
        .child(dialog_body(
            "All changes have been staged. Choose where to commit and push.",
            dc,
        ));

    dialog_box = dialog_box.child(radio_row(
        "commit-push-current",
        "Current branch",
        &branch_label,
        current_selected,
        detached,
        accent,
        border,
        muted,
        text_color,
        surface,
        entity.clone(),
        CommitPushMode::CurrentBranch,
    ));

    dialog_box = dialog_box.child(radio_row(
        "commit-push-feature",
        "Create feature branch",
        "Branch from current HEAD",
        feature_selected,
        false,
        accent,
        border,
        muted,
        text_color,
        surface,
        entity.clone(),
        CommitPushMode::FeatureBranch,
    ));

    let confirm_bg = if can_confirm {
        rgba_to_hsla(colors.accent)
    } else {
        muted
    };
    let confirm_tc = rgba_to_hsla(colors.background);
    let ent_cancel = entity.clone();
    let ent_confirm = entity.clone();
    let current_branch_confirm = current_branch.to_string();
    let mode_confirm = mode;

    dialog_box = dialog_box.child(
        div()
            .mt_4()
            .flex()
            .gap_2()
            .justify_end()
            .child(
                div()
                    .id("commit-push-cancel")
                    .px_3()
                    .py_1()
                    .rounded(px(4.0))
                    .border_1()
                    .border_color(border)
                    .cursor_pointer()
                    .text_xs()
                    .text_color(muted)
                    .child("Cancel")
                    .on_click(move |_ev, _window, cx| {
                        if let Some(e) = ent_cancel.upgrade() {
                            e.update(cx, |this, cx| {
                                this.cancel_dialog(cx);
                            });
                        }
                    }),
            )
            .child(
                div()
                    .id("commit-push-confirm")
                    .px_3()
                    .py_1()
                    .rounded(px(4.0))
                    .bg(confirm_bg)
                    .cursor_pointer()
                    .text_xs()
                    .text_color(confirm_tc)
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("Commit & Push")
                    .on_click(move |_ev, _window, cx| {
                        if !can_confirm {
                            return;
                        }
                        if let Some(e) = ent_confirm.upgrade() {
                            e.update(cx, |this, cx| {
                                this.active_dialog = AppDialog::None;
                                confirm(
                                    this,
                                    current_branch_confirm.clone(),
                                    mode_confirm,
                                    cx,
                                );
                            });
                        }
                    }),
            ),
    );

    dialog_overlay(dc).child(dialog_box)
}

#[allow(clippy::too_many_arguments)]
fn radio_row(
    id: &'static str,
    title: &str,
    subtitle: &str,
    selected: bool,
    disabled: bool,
    accent: Hsla,
    border: Hsla,
    muted: Hsla,
    text_color: Hsla,
    surface: Hsla,
    entity: WeakEntity<GitForgeApp>,
    mode: CommitPushMode,
) -> Stateful<Div> {
    let radio_border = if selected { accent } else { border };
    let radio_bg = if selected { accent } else { surface };
    let row_border = if selected { accent } else { border };
    let title_color = if disabled { muted } else { text_color };

    let mut row = div()
        .id(id)
        .mt_2()
        .px_3()
        .py_2()
        .rounded(px(4.0))
        .border_1()
        .border_color(row_border)
        .flex()
        .items_start()
        .gap_2();
    if !disabled {
        row = row.cursor_pointer();
    }
    row.on_click(move |_ev, _window, cx| {
        if disabled {
            return;
        }
        if let Some(e) = entity.upgrade() {
            e.update(cx, |this, cx| {
                this.set_commit_push_mode(mode, cx);
            });
        }
    })
    .child(
        div()
            .w(px(14.0))
            .h(px(14.0))
            .mt(px(1.0))
            .flex_shrink_0()
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(7.0))
            .border_1()
            .border_color(radio_border)
            .bg(radio_bg)
            .text_color(gpui::hsla(0.0, 0.0, 1.0, 1.0))
            .text_xs()
            .child(if selected { "\u{2713}" } else { "" }),
    )
    .child(
        div()
            .flex()
            .flex_col()
            .gap_0p5()
            .child(
                div()
                    .text_sm()
                    .text_color(title_color)
                    .child(title.to_string()),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(muted)
                    .child(subtitle.to_string()),
            ),
    )
}

pub fn confirm_from_dialog(
    app: &mut GitForgeApp,
    dialog: AppDialog,
    _input: &str,
    cx: &mut Context<GitForgeApp>,
) {
    if let AppDialog::CommitAndPush {
        current_branch,
        detached: _,
    } = dialog
    {
        let mode = app.commit_push_mode;
        confirm(app, current_branch, mode, cx);
    }
}
