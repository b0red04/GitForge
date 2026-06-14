use gitforge_ui::{
    AppColors, DialogColors, TextInput, TextInputEvent, TextInputRenderOpts, attach_dialog_input_keys,
    dialog_actions, dialog_overlay, dialog_surface, dialog_title, render_text_input,
};
use gpui::*;

use crate::views::app::GitForgeApp;

pub fn confirm(app: &mut GitForgeApp, input: &str, input_2: &str, cx: &mut Context<GitForgeApp>) {
    let parts: Vec<&str> = input.split_whitespace().collect();
    if parts.len() < 2 {
        return;
    }
    let password = if input_2.is_empty() {
        return;
    } else {
        input_2
    };
    store_credential(app, parts[0].to_string(), parts[1].to_string(), password.to_string(), cx);
}

pub fn render(
    dialog_input: &TextInput,
    dialog_input_2: &TextInput,
    colors: &AppColors,
    entity: WeakEntity<GitForgeApp>,
    window: &mut Window,
) -> Stateful<Div> {
    let dc = DialogColors::from_app(colors);

    let username_field = attach_dialog_input_keys(
        render_text_input(
            dialog_input,
            colors,
            window,
            &TextInputRenderOpts::new(ElementId::Name("dialog-input".into()))
                .placeholder("host username"),
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

    let password_field = attach_dialog_input_keys(
        render_text_input(
            dialog_input_2,
            colors,
            window,
            &TextInputRenderOpts::new(ElementId::Name("dialog-input-2".into())).placeholder("password"),
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

    let dialog_box = dialog_surface(px(360.0), dc)
        .child(dialog_title("Add Credential", dc))
        .child(username_field)
        .child(password_field)
        .child(dialog_actions(
            "dialog-cancel",
            "dialog-confirm",
            "Confirm",
            entity.clone(),
            |this, cx| this.cancel_dialog(cx),
            |this, cx| this.confirm_dialog(cx),
            dc,
        ));

    dialog_overlay(dc).child(dialog_box)
}

fn store_credential(
    _app: &mut GitForgeApp,
    host: String,
    username: String,
    password: String,
    cx: &mut Context<GitForgeApp>,
) {
    cx.spawn(async move |this, cx| {
        let result = tokio::task::spawn_blocking(move || {
            gitforge_git::credential::store_credential(&host, &username, &password, None)
        })
        .await;

        match result {
            Ok(Ok(())) => {
                this.update(cx, |this, cx| {
                    this.repo_session.remote_status = "Credential stored in keyring".to_string();
                    cx.notify();
                })
                .ok();
            }
            Ok(Err(e)) => {
                this.update(cx, |this, cx| {
                    this.report_op_error("Store credential", &e.to_string(), cx);
                })
                .ok();
            }
            Err(e) => {
                this.update(cx, |this, cx| {
                    this.report_op_error("Store credential", &e.to_string(), cx);
                })
                .ok();
            }
        }
    })
    .detach();
}
