use gitforge_git::RefKind;
use gitforge_ui::{
    AppColors, DialogColors, TextInput, TextInputEvent, TextInputRenderOpts,
    attach_dialog_input_keys, dialog_actions, dialog_overlay, dialog_surface, dialog_title,
    render_text_input,
};
use gpui::*;

use crate::views::app::{AppDialog, GitForgeApp};

#[derive(Clone, Copy)]
pub struct SimpleInputMeta {
    pub title: &'static str,
    pub placeholder: &'static str,
    pub confirm_label: &'static str,
    pub width: Pixels,
}

pub fn is_simple(dialog: &AppDialog) -> bool {
    meta(dialog).is_some()
}

pub fn meta(dialog: &AppDialog) -> Option<SimpleInputMeta> {
    match dialog {
        AppDialog::CreateBranch { .. } => Some(SimpleInputMeta {
            title: "Create Branch",
            placeholder: "Branch name",
            confirm_label: "Confirm",
            width: px(360.0),
        }),
        AppDialog::RenameBranch { .. } => Some(SimpleInputMeta {
            title: "Rename Branch",
            placeholder: "New branch name",
            confirm_label: "Confirm",
            width: px(360.0),
        }),
        AppDialog::CreateTag { .. } => Some(SimpleInputMeta {
            title: "Create Tag",
            placeholder: "Tag name",
            confirm_label: "Confirm",
            width: px(360.0),
        }),
        AppDialog::StashPush => Some(SimpleInputMeta {
            title: "Stash Changes",
            placeholder: "Stash message (optional)",
            confirm_label: "Confirm",
            width: px(360.0),
        }),
        AppDialog::Push { .. } => Some(SimpleInputMeta {
            title: "Push",
            placeholder: "Branch name (empty = current)",
            confirm_label: "Confirm",
            width: px(360.0),
        }),
        AppDialog::Pull { .. } => Some(SimpleInputMeta {
            title: "Pull",
            placeholder: "Remote name (empty = origin)",
            confirm_label: "Confirm",
            width: px(360.0),
        }),
        AppDialog::CloneRepo => Some(SimpleInputMeta {
            title: "Clone Repository",
            placeholder: "URL destination-path",
            confirm_label: "Confirm",
            width: px(360.0),
        }),
        AppDialog::AddRemote => Some(SimpleInputMeta {
            title: "Add Remote",
            placeholder: "name url",
            confirm_label: "Confirm",
            width: px(360.0),
        }),
        AppDialog::SshGenerateKey => Some(SimpleInputMeta {
            title: "Generate SSH Key",
            placeholder: "Email address",
            confirm_label: "Confirm",
            width: px(360.0),
        }),
        AppDialog::SshTestConnection => Some(SimpleInputMeta {
            title: "Test SSH Connection",
            placeholder: "Host (e.g. github.com)",
            confirm_label: "Confirm",
            width: px(360.0),
        }),
        AppDialog::InitRepo { .. } => Some(SimpleInputMeta {
            title: "Init Repository",
            placeholder: "Repository name",
            confirm_label: "Confirm",
            width: px(360.0),
        }),
        _ => None,
    }
}

pub fn confirm(
    app: &mut GitForgeApp,
    dialog: AppDialog,
    input: &str,
    _input_2: &str,
    cx: &mut Context<GitForgeApp>,
) {
    match dialog {
        AppDialog::CreateBranch { start_point } => {
            if input.is_empty() {
                return;
            }
            app.create_branch(input.to_string(), start_point, cx);
        }
        AppDialog::RenameBranch { old_name } => {
            if input.is_empty() {
                return;
            }
            app.rename_branch(old_name, input.to_string(), cx);
        }
        AppDialog::CreateTag { target } => {
            if input.is_empty() {
                return;
            }
            app.create_tag(input.to_string(), target, cx);
        }
        AppDialog::StashPush => {
            app.stash_push(
                if input.is_empty() {
                    None
                } else {
                    Some(input.to_string())
                },
                cx,
            );
        }
        AppDialog::Push { .. } => {
            let branch = if input.is_empty() {
                app.repo_session.active_repo_state().and_then(|rs| {
                    rs.references
                        .iter()
                        .find(|r| r.is_head && r.kind == RefKind::Branch)
                        .map(|r| r.name.clone())
                })
            } else {
                Some(input.to_string())
            };
            let Some(branch_name) = branch else {
                return;
            };
            app.push_current_branch("origin".into(), branch_name, false, cx);
        }
        AppDialog::Pull { .. } => {
            let remote = if input.is_empty() {
                "origin".into()
            } else {
                input.to_string()
            };
            app.pull_from_remote(remote, false, cx);
        }
        AppDialog::CloneRepo => {
            if input.is_empty() {
                return;
            }
            let parts: Vec<&str> = input.splitn(2, ' ').collect();
            if parts.len() < 2 {
                return;
            }
            app.clone_repository(parts[0].to_string(), parts[1].to_string(), cx);
        }
        AppDialog::AddRemote => {
            let parts: Vec<&str> = input.split_whitespace().collect();
            if parts.len() < 2 {
                return;
            }
            app.add_remote(parts[0].to_string(), parts[1].to_string(), cx);
        }
        AppDialog::SshGenerateKey => {
            let email = if input.is_empty() {
                "user@example.com".to_string()
            } else {
                input.to_string()
            };
            app.generate_ssh_key("ed25519".to_string(), email, cx);
        }
        AppDialog::SshTestConnection => {
            let host = if input.is_empty() {
                "github.com".to_string()
            } else {
                input.to_string()
            };
            app.test_ssh_connection(host, cx);
        }
        AppDialog::InitRepo { parent } => {
            let name = input.trim().to_string();
            if name.is_empty() {
                return;
            }
            super::init_repo::init_repository(app, parent, name, cx);
        }
        _ => {}
    }
}

pub fn render(
    dialog: &AppDialog,
    dialog_input: &TextInput,
    colors: &AppColors,
    entity: WeakEntity<GitForgeApp>,
    window: &mut Window,
) -> Stateful<Div> {
    let Some(meta) = meta(dialog) else {
        return dialog_overlay(DialogColors::from_app(colors));
    };
    let dc = DialogColors::from_app(colors);
    let placeholder = meta.placeholder;

    let input_field = attach_dialog_input_keys(
        render_text_input(
            dialog_input,
            colors,
            window,
            &TextInputRenderOpts::new(ElementId::Name("dialog-input".into()))
                .placeholder(placeholder),
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

    let dialog_box = dialog_surface(meta.width, dc)
        .child(dialog_title(meta.title, dc))
        .child(input_field)
        .child(dialog_actions(
            "dialog-cancel",
            "dialog-confirm",
            meta.confirm_label,
            entity.clone(),
            |this, cx| this.cancel_dialog(cx),
            |this, cx| this.confirm_dialog(cx),
            dc,
        ));

    dialog_overlay(dc).child(dialog_box)
}

#[cfg(test)]
mod tests {
    #[test]
    fn clone_repo_input_requires_url_and_path() {
        let parts: Vec<&str> = "https://example.com/repo".splitn(2, ' ').collect();
        assert_eq!(parts.len(), 1);
        let parts: Vec<&str> = "https://example.com/repo /tmp/repo"
            .splitn(2, ' ')
            .collect();
        assert_eq!(parts.len(), 2);
    }

    #[test]
    fn add_remote_input_requires_two_tokens() {
        let parts: Vec<&str> = "origin https://example.com".split_whitespace().collect();
        assert_eq!(parts.len(), 2);
    }
}
