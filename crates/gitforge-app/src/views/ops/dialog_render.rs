use gitforge_ui::{AppColors, rgba_to_hsla};
use gpui::*;

use crate::views::app::{AppDialog, GitForgeApp};

fn dialog_overlay_root(overlay_bg: Hsla) -> Stateful<Div> {
    div()
        .id("dialog-overlay")
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .bg(overlay_bg)
        .occlude()
        .flex()
        .items_center()
        .justify_center()
}

pub(crate) fn render_dialog_overlay(
    dialog: &AppDialog,
    input_value: &str,
    input_value_2: &str,
    dialog_force: bool,
    input_focus: &FocusHandle,
    colors: &AppColors,
    entity: WeakEntity<GitForgeApp>,
    window: &mut Window,
    hosting_repos: &[gitforge_hosting::RemoteRepo],
    hosting_repos_loading: bool,
    _hosting_accounts_from_render: &[gitforge_hosting::HostingAccount],
) -> Stateful<Div> {
    let overlay_bg = rgba_to_hsla(colors.background).opacity(0.7);
    let surface = rgba_to_hsla(colors.surface);
    let border = rgba_to_hsla(colors.border);
    let text_color = rgba_to_hsla(colors.text);
    let accent = rgba_to_hsla(colors.accent);
    let muted = rgba_to_hsla(colors.text_muted);
    let warning = rgba_to_hsla(colors.warning);

    let title = match dialog {
        AppDialog::CreateBranch { .. } => "Create Branch",
        AppDialog::RenameBranch { .. } => "Rename Branch",
        AppDialog::DeleteBranch { .. } => "Delete Branch",
        AppDialog::CreateTag { .. } => "Create Tag",
        AppDialog::StashPush => "Stash Changes",
        AppDialog::Push { .. } => "Push",
        AppDialog::Pull { .. } => "Pull",
        AppDialog::CloneRepo => "Clone Repository",
        AppDialog::AddRemote => "Add Remote",
        AppDialog::SshGenerateKey => "Generate SSH Key",
        AppDialog::SshTestConnection => "Test SSH Connection",
        AppDialog::CredentialAdd => "Add Credential",
        AppDialog::CloneFromHosting { .. } => "Clone from Hosting",
        AppDialog::SearchHosting { .. } => "Search Repositories",
        AppDialog::ForkRepo { .. } => "Fork Repository",
        AppDialog::CreateWorktree => "Create Worktree",
        AppDialog::RemoveWorktree { .. } => "Remove Worktree",
        AppDialog::InitRepo { .. } => "Init Repository",
        AppDialog::CreatePullRequest => "Create Pull Request",
        AppDialog::None => "",
    };

    let placeholder = match dialog {
        AppDialog::CreateBranch { .. } => "Branch name",
        AppDialog::RenameBranch { .. } => "New branch name",
        AppDialog::DeleteBranch { name } => {
            return render_delete_branch_overlay(
                name,
                dialog_force,
                colors,
                entity,
                input_focus,
            );
        }
        AppDialog::CreateTag { .. } => "Tag name",
        AppDialog::StashPush => "Stash message (optional)",
        AppDialog::Push { .. } => "Branch name (empty = current)",
        AppDialog::Pull { .. } => "Remote name (empty = origin)",
        AppDialog::CloneRepo => "URL destination-path",
        AppDialog::AddRemote => "name url",
        AppDialog::SshGenerateKey => "Email address",
        AppDialog::SshTestConnection => "Host (e.g. github.com)",
        AppDialog::CredentialAdd => "host username",
        AppDialog::CloneFromHosting { .. } => "Search repos...",
        AppDialog::SearchHosting { .. } => "Search query...",
        AppDialog::ForkRepo { owner, repo, .. } => {
            return render_fork_confirm_overlay(owner, repo, colors, entity);
        }
        AppDialog::CreateWorktree => {
            return render_create_worktree_overlay(
                input_value,
                input_value_2,
                input_focus,
                colors,
                entity,
                window,
            );
        }
        AppDialog::RemoveWorktree { path } => {
            return render_remove_worktree_overlay(path, colors, entity);
        }
        AppDialog::InitRepo { .. } => "Repository name",
        AppDialog::CreatePullRequest => "",
        AppDialog::None => "",
    };

    if matches!(dialog, AppDialog::CloneFromHosting { .. }) {
        return render_hosting_repos_overlay(
            dialog,
            input_value,
            input_focus,
            colors,
            entity,
            window,
            hosting_repos,
            hosting_repos_loading,
        );
    }

    if matches!(dialog, AppDialog::SearchHosting { .. }) {
        return render_hosting_repos_overlay(
            dialog,
            input_value,
            input_focus,
            colors,
            entity,
            window,
            hosting_repos,
            hosting_repos_loading,
        );
    }

    let is_focused = input_focus.is_focused(window);
    let display_text = if input_value.is_empty() && !is_focused {
        placeholder.to_string()
    } else {
        let mut t = input_value.to_string();
        if is_focused {
            t.push('\u{2502}');
        }
        t
    };
    let display_color = if input_value.is_empty() && !is_focused {
        muted
    } else {
        text_color
    };
    let border_focus_color = if is_focused { accent } else { border };

    let ent_cancel = entity.clone();
    let ent_cancel2 = entity.clone();
    let ent_confirm = entity.clone();
    let ent_confirm2 = entity.clone();
    let ent_input = entity.clone();
    let fh = input_focus.clone();

    let mut dialog_box = div()
        .id("dialog-box")
        .w(px(360.0))
        .bg(surface)
        .border_1()
        .border_color(border)
        .rounded(px(6.0))
        .p_4()
        .flex()
        .flex_col()
        .gap_3()
        .child(
            div()
                .text_sm()
                .font_weight(FontWeight::BOLD)
                .text_color(text_color)
                .child(title.to_string()),
        );

    dialog_box = dialog_box
        .child(
            div()
                .id(ElementId::Name("dialog-input".into()))
                .track_focus(&fh)
                .px_2()
                .py_1()
                .border_1()
                .border_color(border_focus_color)
                .rounded(px(3.0))
                .bg(rgba_to_hsla(colors.background))
                .cursor_pointer()
                .on_click(move |_ev, window, _cx| {
                    window.focus(&fh);
                })
                .on_key_down(move |ev: &KeyDownEvent, _window, cx| {
                    let key = &ev.keystroke.key;
                    match key.as_str() {
                        "enter" => {
                            if let Some(e) = ent_confirm.upgrade() {
                                e.update(cx, |this, cx| {
                                    this.confirm_dialog(cx);
                                });
                            }
                        }
                        "escape" => {
                            if let Some(e) = ent_cancel.upgrade() {
                                e.update(cx, |this, cx| {
                                    this.cancel_dialog(cx);
                                });
                            }
                        }
                        "backspace" => {
                            if let Some(e) = ent_input.upgrade() {
                                e.update(cx, |this, cx| {
                                    this.edit_dialog_input(None, cx);
                                });
                            }
                        }
                        _ => {
                            if let Some(ch) = ev.keystroke.key_char.clone() {
                                if !ev.keystroke.modifiers.platform {
                                    if let Some(e) = ent_input.upgrade() {
                                        let c = ch;
                                        e.update(cx, |this, cx| {
                                            this.edit_dialog_input(Some(&c), cx);
                                        });
                                    }
                                }
                            }
                        }
                    }
                })
                .child(
                    div()
                        .text_sm()
                        .text_color(display_color)
                        .child(display_text),
                ),
        )
        .child(
            div()
                .flex()
                .gap_2()
                .justify_end()
                .child(
                    div()
                        .id("dialog-cancel")
                        .px_3()
                        .py_1()
                        .border_1()
                        .border_color(border)
                        .rounded(px(3.0))
                        .cursor_pointer()
                        .text_xs()
                        .text_color(muted)
                        .child("Cancel")
                        .on_click(move |_ev, _window, cx| {
                            if let Some(e) = ent_cancel2.upgrade() {
                                e.update(cx, |this, cx| {
                                    this.cancel_dialog(cx);
                                });
                            }
                        }),
                )
                .child(
                    div()
                        .id("dialog-confirm")
                        .px_3()
                        .py_1()
                        .border_1()
                        .border_color(warning)
                        .rounded(px(3.0))
                        .cursor_pointer()
                        .text_xs()
                        .text_color(warning)
                        .child("Confirm")
                        .on_click(move |_ev, _window, cx| {
                            if let Some(e) = ent_confirm2.upgrade() {
                                e.update(cx, |this, cx| {
                                    this.confirm_dialog(cx);
                                });
                            }
                        }),
                ),
        );

    dialog_overlay_root(overlay_bg).child(dialog_box)
}

fn render_hosting_repos_overlay(
    dialog: &AppDialog,
    _input_value: &str,
    _input_focus: &FocusHandle,
    colors: &AppColors,
    entity: WeakEntity<GitForgeApp>,
    _window: &mut Window,
    hosting_repos: &[gitforge_hosting::RemoteRepo],
    hosting_repos_loading: bool,
) -> Stateful<Div> {
    let overlay_bg = rgba_to_hsla(colors.background).opacity(0.7);
    let surface = rgba_to_hsla(colors.surface);
    let border = rgba_to_hsla(colors.border);
    let text_color = rgba_to_hsla(colors.text);
    let muted = rgba_to_hsla(colors.text_muted);
    let accent = rgba_to_hsla(colors.accent);

    let (_provider_name, dialog_title) = match dialog {
        AppDialog::CloneFromHosting { provider } => {
            (provider.clone(), format!("Clone from {}", provider))
        }
        AppDialog::SearchHosting { provider } => {
            (provider.clone(), format!("Search on {}", provider))
        }
        _ => (String::new(), "Browse Repositories".to_string()),
    };

    let ent_cancel = entity.clone();

    let mut content = div()
        .id("dialog-box")
        .w(px(500.0))
        .max_h(px(500.0))
        .bg(surface)
        .border_1()
        .border_color(border)
        .rounded(px(6.0))
        .p_4()
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::BOLD)
                        .text_color(text_color)
                        .child(dialog_title),
                )
                .child(
                    div()
                        .id("hosting-cancel")
                        .px_2()
                        .py_0()
                        .border_1()
                        .border_color(border)
                        .rounded(px(3.0))
                        .cursor_pointer()
                        .text_xs()
                        .text_color(muted)
                        .child("Close")
                        .on_click(move |_ev, _window, cx| {
                            if let Some(e) = ent_cancel.upgrade() {
                                e.update(cx, |this, cx| {
                                    this.cancel_dialog(cx);
                                });
                            }
                        }),
                ),
        );

    if hosting_repos_loading {
        content = content.child(
            div()
                .text_xs()
                .text_color(muted)
                .child("Loading repositories..."),
        );
    } else if hosting_repos.is_empty() {
        content = content.child(
            div()
                .text_xs()
                .text_color(muted)
                .child("No repositories found"),
        );
    } else {
        let mut list = div().flex().flex_col().gap_1();
        for (i, repo) in hosting_repos.iter().enumerate() {
            let ent_clone = entity.clone();
            let clone_url = repo.clone_url.clone();
            let repo_name = repo.name.clone();
            let vis = if repo.is_private { "private" } else { "public" };
            let stars = repo.stars;
            let desc = repo.description.as_deref().unwrap_or("");

            list = list.child(
                div()
                    .id(ElementId::NamedInteger("hosting-repo".into(), i as u64))
                    .px_2()
                    .py_1()
                    .border_1()
                    .border_color(border)
                    .rounded(px(3.0))
                    .cursor_pointer()
                    .hover(|s| s.bg(rgba_to_hsla(colors.surface_high)))
                    .on_click(move |_ev, _window, cx| {
                        if let Some(e) = ent_clone.upgrade() {
                            let url = clone_url.clone();
                            let name = repo_name.clone();
                            e.update(cx, |this, cx| {
                                this.clone_hosting_repo(url, name, cx);
                            });
                        }
                    })
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_0()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(text_color)
                                            .child(repo.name.clone()),
                                    )
                                    .child(div().text_xs().text_color(muted).child(vis))
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(accent)
                                            .child(format!("*{}", stars)),
                                    ),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(muted)
                                    .overflow_hidden()
                                    .child(desc.to_string()),
                            ),
                    ),
            );
        }
        content = content.child(list);
    }

    dialog_overlay_root(overlay_bg).child(content)
}

fn render_fork_confirm_overlay(
    owner: &str,
    repo: &str,
    colors: &AppColors,
    entity: WeakEntity<GitForgeApp>,
) -> Stateful<Div> {
    let overlay_bg = rgba_to_hsla(colors.background).opacity(0.7);
    let surface = rgba_to_hsla(colors.surface);
    let border = rgba_to_hsla(colors.border);
    let text_color = rgba_to_hsla(colors.text);
    let muted = rgba_to_hsla(colors.text_muted);
    let warning = rgba_to_hsla(colors.warning);

    let ent_cancel = entity.clone();
    let ent_confirm = entity.clone();
    let owner_owned = owner.to_string();
    let repo_owned = repo.to_string();

    dialog_overlay_root(overlay_bg).child(
            div()
                .id("dialog-box")
                .w(px(360.0))
                .bg(surface)
                .border_1()
                .border_color(border)
                .rounded(px(6.0))
                .p_4()
                .flex()
                .flex_col()
                .gap_3()
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::BOLD)
                        .text_color(text_color)
                        .child("Fork Repository"),
                )
                .child(
                    div()
                        .text_sm()
                        .text_color(text_color)
                        .child(format!("Fork {}/{} to your account?", owner, repo)),
                )
                .child(
                    div()
                        .flex()
                        .gap_2()
                        .justify_end()
                        .child(
                            div()
                                .id("fork-cancel")
                                .px_3()
                                .py_1()
                                .border_1()
                                .border_color(border)
                                .rounded(px(3.0))
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
                                .id("fork-confirm")
                                .px_3()
                                .py_1()
                                .border_1()
                                .border_color(warning)
                                .rounded(px(3.0))
                                .cursor_pointer()
                                .text_xs()
                                .text_color(warning)
                                .child("Fork")
                                .on_click(move |_ev, _window, cx| {
                                    if let Some(e) = ent_confirm.upgrade() {
                                        let o = owner_owned.clone();
                                        let r = repo_owned.clone();
                                        e.update(cx, |this, cx| {
                                            let provider = this
                                                .hosting_accounts
                                                .first()
                                                .map(|a| a.provider.clone())
                                                .unwrap_or_default();
                                            this.fork_repo(o, r, provider, cx);
                                        });
                                    }
                                }),
                        ),
                ),
        )
}

fn render_delete_branch_overlay(
    name: &str,
    force: bool,
    colors: &AppColors,
    entity: WeakEntity<GitForgeApp>,
    input_focus: &FocusHandle,
) -> Stateful<Div> {
    let overlay_bg = rgba_to_hsla(colors.background).opacity(0.7);
    let surface = rgba_to_hsla(colors.surface);
    let border = rgba_to_hsla(colors.border);
    let text_color = rgba_to_hsla(colors.text);
    let muted = rgba_to_hsla(colors.text_muted);
    let warning = rgba_to_hsla(colors.warning);
    let accent = rgba_to_hsla(colors.accent);

    let ent_cancel = entity.clone();
    let ent_confirm = entity.clone();
    let ent_toggle = entity.clone();
    let branch_name = name.to_string();
    let fh = input_focus.clone();
    let ent_key_cancel = entity.clone();
    let ent_key_confirm = entity.clone();
    let branch_name_key = name.to_string();

    dialog_overlay_root(overlay_bg).child(
            div()
                .id("dialog-box")
                .track_focus(&fh)
                .w(px(380.0))
                .bg(surface)
                .border_1()
                .border_color(border)
                .rounded(px(6.0))
                .p_4()
                .flex()
                .flex_col()
                .gap_3()
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
                                    this.delete_branch(name, force, cx);
                                });
                            }
                        }
                        _ => {}
                    }
                })
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::BOLD)
                        .text_color(text_color)
                        .child("Delete Branch"),
                )
                .child(
                    div()
                        .text_sm()
                        .text_color(text_color)
                        .child(format!("Delete branch '{}'? This cannot be undone.", name)),
                )
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
                                .border_color(if force { accent } else { border })
                                .bg(if force { accent } else { surface })
                                .text_color(gpui::hsla(0.0, 0.0, 1.0, 1.0))
                                .text_xs()
                                .child(if force { "\u{2713}" } else { "" }),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(muted)
                                .child("Force delete (allows removing unmerged branches)"),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .gap_2()
                        .justify_end()
                        .child(
                            div()
                                .id("delete-branch-cancel")
                                .px_3()
                                .py_1()
                                .border_1()
                                .border_color(border)
                                .rounded(px(3.0))
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
                                .id("delete-branch-confirm")
                                .px_3()
                                .py_1()
                                .border_1()
                                .border_color(warning)
                                .rounded(px(3.0))
                                .cursor_pointer()
                                .text_xs()
                                .text_color(warning)
                                .child("Delete")
                                .on_click(move |_ev, _window, cx| {
                                    if let Some(e) = ent_confirm.upgrade() {
                                        let name = branch_name.clone();
                                        e.update(cx, |this, cx| {
                                            this.active_dialog = AppDialog::None;
                                            this.delete_branch(name, force, cx);
                                        });
                                    }
                                }),
                        ),
                ),
        )
}

fn render_create_worktree_overlay(
    input_value: &str,
    input_value_2: &str,
    input_focus: &FocusHandle,
    colors: &AppColors,
    entity: WeakEntity<GitForgeApp>,
    window: &mut Window,
) -> Stateful<Div> {
    let overlay_bg = rgba_to_hsla(colors.background).opacity(0.7);
    let surface = rgba_to_hsla(colors.surface);
    let border = rgba_to_hsla(colors.border);
    let text_color = rgba_to_hsla(colors.text);
    let accent = rgba_to_hsla(colors.accent);
    let muted = rgba_to_hsla(colors.text_muted);
    let warning = rgba_to_hsla(colors.warning);

    let fh = input_focus.clone();
    let is_focused = input_focus.is_focused(window);
    let display_text = if input_value.is_empty() && !is_focused {
        "Directory path (relative or absolute)".to_string()
    } else {
        let mut t = input_value.to_string();
        if is_focused {
            t.push('\u{2502}');
        }
        t
    };
    let display_color = if input_value.is_empty() && !is_focused {
        muted
    } else {
        text_color
    };
    let border_focus = if is_focused { accent } else { border };

    let display_text_2 = if input_value_2.is_empty() {
        "Branch/tag/commit (optional)".to_string()
    } else {
        input_value_2.to_string()
    };
    let display_color_2 = if input_value_2.is_empty() {
        muted
    } else {
        text_color
    };

    let ent_cancel = entity.clone();
    let ent_cancel2 = entity.clone();
    let ent_cancel3 = entity.clone();
    let ent_confirm = entity.clone();
    let ent_confirm2 = entity.clone();
    let ent_confirm3 = entity.clone();
    let ent_input = entity.clone();
    let ent_input2 = entity.clone();
    let fh1 = fh.clone();
    let fh2 = fh.clone();

    let dialog_box = div()
        .id("dialog-box")
        .w(px(420.0))
        .bg(surface)
        .border_1()
        .border_color(border)
        .rounded(px(6.0))
        .p_4()
        .flex()
        .flex_col()
        .gap_3()
        .child(
            div()
                .text_sm()
                .font_weight(FontWeight::BOLD)
                .text_color(text_color)
                .child("Create Worktree"),
        )
        .child(div().text_xs().text_color(muted).child("Target directory:"))
        .child(
            div()
                .id(ElementId::Name("dialog-input".into()))
                .track_focus(&fh1)
                .px_2()
                .py_1()
                .border_1()
                .border_color(border_focus)
                .rounded(px(3.0))
                .bg(rgba_to_hsla(colors.background))
                .cursor_pointer()
                .on_click(move |_ev, window, _cx| {
                    window.focus(&fh1);
                })
                .on_key_down(move |ev: &KeyDownEvent, _window, cx| {
                    match ev.keystroke.key.as_str() {
                        "enter" => {
                            if let Some(e) = ent_confirm.upgrade() {
                                e.update(cx, |this, cx| {
                                    this.confirm_dialog(cx);
                                });
                            }
                        }
                        "escape" => {
                            if let Some(e) = ent_cancel.upgrade() {
                                e.update(cx, |this, cx| {
                                    this.cancel_dialog(cx);
                                });
                            }
                        }
                        "backspace" => {
                            if let Some(e) = ent_input.upgrade() {
                                e.update(cx, |this, cx| {
                                    this.edit_dialog_input(None, cx);
                                });
                            }
                        }
                        _ => {
                            if let Some(ch) = ev.keystroke.key_char.clone() {
                                if !ev.keystroke.modifiers.platform {
                                    if let Some(e) = ent_input.upgrade() {
                                        let c = ch;
                                        e.update(cx, |this, cx| {
                                            this.edit_dialog_input(Some(&c), cx);
                                        });
                                    }
                                }
                            }
                        }
                    }
                })
                .child(
                    div()
                        .text_sm()
                        .text_color(display_color)
                        .child(display_text),
                ),
        )
        .child(
            div()
                .text_xs()
                .text_color(muted)
                .child("Checkout ref (branch, tag, or commit):"),
        )
        .child(
            div()
                .id(ElementId::Name("dialog-input-2".into()))
                .track_focus(&fh2)
                .px_2()
                .py_1()
                .border_1()
                .border_color(border)
                .rounded(px(3.0))
                .bg(rgba_to_hsla(colors.background))
                .cursor_pointer()
                .on_click(move |_ev, window, _cx| {
                    window.focus(&fh2);
                })
                .on_key_down(move |ev: &KeyDownEvent, _window, cx| {
                    match ev.keystroke.key.as_str() {
                        "enter" => {
                            if let Some(e) = ent_confirm2.upgrade() {
                                e.update(cx, |this, cx| {
                                    this.confirm_dialog(cx);
                                });
                            }
                        }
                        "escape" => {
                            if let Some(e) = ent_cancel2.upgrade() {
                                e.update(cx, |this, cx| {
                                    this.cancel_dialog(cx);
                                });
                            }
                        }
                        "backspace" => {
                            if let Some(e) = ent_input2.upgrade() {
                                e.update(cx, |this, cx| {
                                    this.dialog_input_2.pop();
                                    cx.notify();
                                });
                            }
                        }
                        _ => {
                            if let Some(ch) = ev.keystroke.key_char.clone() {
                                if !ev.keystroke.modifiers.platform {
                                    if let Some(e) = ent_input2.upgrade() {
                                        let c = ch;
                                        e.update(cx, |this, cx| {
                                            this.dialog_input_2.push_str(&c);
                                            cx.notify();
                                        });
                                    }
                                }
                            }
                        }
                    }
                })
                .child(
                    div()
                        .text_sm()
                        .text_color(display_color_2)
                        .child(display_text_2),
                ),
        )
        .child(
            div()
                .flex()
                .gap_2()
                .justify_end()
                .child(
                    div()
                        .id("dialog-cancel")
                        .px_3()
                        .py_1()
                        .border_1()
                        .border_color(border)
                        .rounded(px(3.0))
                        .cursor_pointer()
                        .text_xs()
                        .text_color(muted)
                        .child("Cancel")
                        .on_click(move |_ev, _window, cx| {
                            if let Some(e) = ent_cancel3.upgrade() {
                                e.update(cx, |this, cx| {
                                    this.cancel_dialog(cx);
                                });
                            }
                        }),
                )
                .child(
                    div()
                        .id("dialog-confirm")
                        .px_3()
                        .py_1()
                        .border_1()
                        .border_color(warning)
                        .rounded(px(3.0))
                        .cursor_pointer()
                        .text_xs()
                        .text_color(warning)
                        .child("Create")
                        .on_click(move |_ev, _window, cx| {
                            if let Some(e) = ent_confirm3.upgrade() {
                                e.update(cx, |this, cx| {
                                    this.confirm_dialog(cx);
                                });
                            }
                        }),
                ),
        );

    dialog_overlay_root(overlay_bg).child(dialog_box)
}

fn render_remove_worktree_overlay(
    path: &str,
    colors: &AppColors,
    entity: WeakEntity<GitForgeApp>,
) -> Stateful<Div> {
    let overlay_bg = rgba_to_hsla(colors.background).opacity(0.7);
    let surface = rgba_to_hsla(colors.surface);
    let border = rgba_to_hsla(colors.border);
    let text_color = rgba_to_hsla(colors.text);
    let warning = rgba_to_hsla(colors.warning);
    let muted = rgba_to_hsla(colors.text_muted);

    let ent_cancel = entity.clone();
    let ent_confirm = entity.clone();

    let dialog_box = div()
        .id("dialog-box")
        .w(px(400.0))
        .bg(surface)
        .border_1()
        .border_color(border)
        .rounded(px(6.0))
        .p_4()
        .flex()
        .flex_col()
        .gap_3()
        .child(
            div()
                .text_sm()
                .font_weight(FontWeight::BOLD)
                .text_color(text_color)
                .child("Remove Worktree"),
        )
        .child(
            div()
                .text_sm()
                .text_color(text_color)
                .child(format!("Remove worktree at {}?", path)),
        )
        .child(
            div()
                .flex()
                .gap_2()
                .justify_end()
                .child(
                    div()
                        .id("dialog-cancel")
                        .px_3()
                        .py_1()
                        .border_1()
                        .border_color(border)
                        .rounded(px(3.0))
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
                        .id("dialog-confirm")
                        .px_3()
                        .py_1()
                        .border_1()
                        .border_color(warning)
                        .rounded(px(3.0))
                        .cursor_pointer()
                        .text_xs()
                        .text_color(warning)
                        .child("Remove")
                        .on_click(move |_ev, _window, cx| {
                            if let Some(e) = ent_confirm.upgrade() {
                                e.update(cx, |this, cx| {
                                    this.confirm_dialog(cx);
                                });
                            }
                        }),
                ),
        );

    dialog_overlay_root(overlay_bg).child(dialog_box)
}
