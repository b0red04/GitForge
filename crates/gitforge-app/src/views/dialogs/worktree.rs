use gitforge_ui::{
    AppColors, DialogColors, TextInput, TextInputEvent, TextInputRenderOpts, attach_dialog_input_keys,
    dialog_actions, dialog_label, dialog_overlay, dialog_surface, dialog_title, render_text_input,
};
use gpui::*;

use crate::views::app::{AppDialog, GitForgeApp};

pub fn confirm(
    app: &mut GitForgeApp,
    input: &str,
    input_2: &str,
    cx: &mut Context<GitForgeApp>,
) {
    if input.is_empty() {
        return;
    }
    let path = input.to_string();
    let refname = if input_2.is_empty() {
        None
    } else {
        Some(input_2.to_string())
    };
    app.create_worktree(path, refname, None, cx);
}

pub fn render(
    dialog_input: &TextInput,
    dialog_input_2: &TextInput,
    colors: &AppColors,
    entity: WeakEntity<GitForgeApp>,
    window: &mut Window,
) -> Stateful<Div> {
    let dc = DialogColors::from_app(colors);

    let path_field = attach_dialog_input_keys(
        render_text_input(
            dialog_input,
            colors,
            window,
            &TextInputRenderOpts::new(ElementId::Name("dialog-input".into()))
                .placeholder("Directory path (relative or absolute)"),
            |_| {},
        ),
        entity.clone(),
        |this, cx, _window, event| match event {
            TextInputEvent::Enter { .. } => this.confirm_dialog(cx),
            TextInputEvent::Escape => this.cancel_dialog(cx),
            TextInputEvent::Backspace => this.edit_dialog_input(None, cx),
            TextInputEvent::Typed(c) => this.edit_dialog_input(Some(&c), cx),
            _ => {}
        },
    );

    let ref_field = attach_dialog_input_keys(
        render_text_input(
            dialog_input_2,
            colors,
            window,
            &TextInputRenderOpts::new(ElementId::Name("dialog-input-2".into()))
                .placeholder("Branch/tag/commit (optional)"),
            |_| {},
        ),
        entity.clone(),
        |this, cx, _window, event| match event {
            TextInputEvent::Enter { .. } => this.confirm_dialog(cx),
            TextInputEvent::Escape => this.cancel_dialog(cx),
            TextInputEvent::Backspace => this.edit_dialog_input_2(None, cx),
            TextInputEvent::Typed(c) => this.edit_dialog_input_2(Some(&c), cx),
            _ => {}
        },
    );

    let dialog_box = dialog_surface(px(420.0), dc)
        .child(dialog_title("Create Worktree", dc))
        .child(dialog_label("Target directory:", dc))
        .child(path_field)
        .child(dialog_label("Checkout ref (branch, tag, or commit):", dc))
        .child(ref_field)
        .child(dialog_actions(
            "dialog-cancel",
            "dialog-confirm",
            "Create",
            entity.clone(),
            |this, cx| this.cancel_dialog(cx),
            |this, cx| this.confirm_dialog(cx),
            dc,
        ));

    dialog_overlay(dc).child(dialog_box)
}

pub fn confirm_from_dialog(
    app: &mut GitForgeApp,
    dialog: AppDialog,
    input: &str,
    input_2: &str,
    cx: &mut Context<GitForgeApp>,
) {
    if matches!(dialog, AppDialog::CreateWorktree) {
        confirm(app, input, input_2, cx);
    }
}
