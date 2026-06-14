use gitforge_hosting::RemoteRepo;
use gitforge_ui::{
    AppColors, TextInput, TextInputEvent, TextInputMode, TextInputRenderOpts, parse_key_event,
    render_text_input,
    rgba_to_hsla,
};
use gpui::*;

use super::app::GitForgeApp;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CreatePrDropdown {
    None,
    FromRepo,
    FromBranch,
    ToRepo,
    ToBranch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CreatePrActiveField {
    Title,
    Description,
}

pub struct CreatePrState {
    pub provider: String,
    pub from_repo: String,
    pub from_branch: String,
    pub to_repo: String,
    pub to_branch: String,
    pub title_input: TextInput,
    pub description_input: TextInput,
    pub draft: bool,
    pub repos: Vec<RemoteRepo>,
    pub from_branches: Vec<String>,
    pub to_branches: Vec<String>,
    pub loading_repos: bool,
    pub loading_branches: bool,
    pub submitting: bool,
    pub generating_ai: bool,
    pub open_dropdown: CreatePrDropdown,
    pub active_field: CreatePrActiveField,
}

impl CreatePrState {
    pub fn new(cx: &mut App) -> Self {
        Self {
            provider: "github".to_string(),
            from_repo: String::new(),
            from_branch: String::new(),
            to_repo: String::new(),
            to_branch: String::new(),
            title_input: TextInput::new("Pull request title", cx),
            description_input: TextInput::new("Pull request description", cx)
                .with_mode(TextInputMode::MULTILINE),
            draft: false,
            repos: Vec::new(),
            from_branches: Vec::new(),
            to_branches: Vec::new(),
            loading_repos: false,
            loading_branches: false,
            submitting: false,
            generating_ai: false,
            open_dropdown: CreatePrDropdown::None,
            active_field: CreatePrActiveField::Title,
        }
    }

    pub fn reset(&mut self) {
        self.open_dropdown = CreatePrDropdown::None;
        self.submitting = false;
        self.generating_ai = false;
        self.loading_repos = false;
        self.loading_branches = false;
    }

    pub fn can_submit(&self) -> bool {
        !self.title_input.text().trim().is_empty()
            && !self.from_repo.is_empty()
            && !self.to_repo.is_empty()
            && !self.from_branch.is_empty()
            && !self.to_branch.is_empty()
            && !self.submitting
    }
}

pub fn render_create_pr_overlay(
    state: &CreatePrState,
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
    let success = rgba_to_hsla(colors.success);
    let purple = gpui::hsla(270.0 / 360.0, 0.55, 0.65, 1.0);

    let can_submit = state.can_submit();

    let ent_close = entity.clone();
    let ent_cancel = entity.clone();
    let ent_submit = entity.clone();
    let ent_ai = entity.clone();
    let ent_draft = entity.clone();

    let providers = [("github", "GitHub"), ("gitlab", "GitLab"), ("codeberg", "Codeberg")];
    let mut tabs = div().flex().gap_4().border_b_1().border_color(border).pb_2();
    for (id, label) in providers {
        let is_active = state.provider == id;
        let ent_tab = entity.clone();
        let provider_id = id.to_string();
        tabs = tabs.child(
            div()
                .id(ElementId::Name(format!("pr-tab-{id}").into()))
                .flex()
                .flex_col()
                .items_center()
                .gap_1()
                .cursor_pointer()
                .text_xs()
                .text_color(if is_active { text_color } else { muted })
                .border_b_2()
                .border_color(if is_active { accent } else { gpui::transparent_black() })
                .pb_1()
                .child(label)
                .on_click(move |_ev, _window, cx| {
                    if let Some(e) = ent_tab.upgrade() {
                        e.update(cx, |this, cx| {
                            this.set_create_pr_provider(provider_id.clone(), cx);
                        });
                    }
                }),
        );
    }

    let title_field = render_text_input(
        &state.title_input,
        colors,
        window,
        &TextInputRenderOpts::new(ElementId::Name("create-pr-title".into()))
            .background(rgba_to_hsla(colors.background)),
        |_| {},
    )
    .on_key_down({
        let ent_title = entity.clone();
        let ent_title2 = entity.clone();
        let ent_title3 = entity.clone();
        let fh_desc = state.description_input.focus_handle().clone();
        move |ev, window, cx| {
            match parse_key_event(ev) {
                TextInputEvent::Escape => {
                    if let Some(e) = ent_title.upgrade() {
                        e.update(cx, |this, cx| this.cancel_create_pr_dialog(cx));
                    }
                }
                TextInputEvent::Enter { .. } => {
                    window.focus(&fh_desc);
                    if let Some(e) = ent_title2.upgrade() {
                        e.update(cx, |this, cx| {
                            this.create_pr.active_field = CreatePrActiveField::Description;
                            cx.notify();
                        });
                    }
                }
                TextInputEvent::Backspace => {
                    if let Some(e) = ent_title3.upgrade() {
                        e.update(cx, |this, cx| this.edit_create_pr_title(None, cx));
                    }
                }
                TextInputEvent::Typed(c) => {
                    if let Some(e) = ent_title3.upgrade() {
                        e.update(cx, |this, cx| this.edit_create_pr_title(Some(&c), cx));
                    }
                }
                _ => {}
            }
        }
    });

    let description_field = render_text_input(
        &state.description_input,
        colors,
        window,
        &TextInputRenderOpts::new(ElementId::Name("create-pr-description".into()))
            .min_h(px(96.0))
            .max_h(px(200.0))
            .overflow_y_scroll()
            .overflow_x_hidden()
            .background(rgba_to_hsla(colors.background)),
        |_| {},
    )
    .on_key_down({
        let ent_desc = entity.clone();
        let ent_desc2 = entity.clone();
        let ent_desc3 = entity.clone();
        move |ev, _window, cx| {
            match parse_key_event(ev) {
                TextInputEvent::Escape => {
                    if let Some(e) = ent_desc.upgrade() {
                        e.update(cx, |this, cx| this.cancel_create_pr_dialog(cx));
                    }
                }
                TextInputEvent::Backspace => {
                    if let Some(e) = ent_desc2.upgrade() {
                        e.update(cx, |this, cx| this.edit_create_pr_description(None, cx));
                    }
                }
                TextInputEvent::Typed(c) => {
                    if let Some(e) = ent_desc3.upgrade() {
                        e.update(cx, |this, cx| this.edit_create_pr_description(Some(&c), cx));
                    }
                }
                TextInputEvent::Enter { .. } => {
                    if let Some(e) = ent_desc3.upgrade() {
                        e.update(cx, |this, cx| {
                            this.edit_create_pr_description(Some("\n"), cx);
                        });
                    }
                }
                _ => {}
            }
        }
    });

    let from_repo_label = if state.from_repo.is_empty() {
        "Select...".to_string()
    } else {
        state.from_repo.clone()
    };
    let from_branch_label = if state.from_branch.is_empty() {
        "Select...".to_string()
    } else {
        state.from_branch.clone()
    };
    let to_repo_label = if state.to_repo.is_empty() {
        "Select...".to_string()
    } else {
        state.to_repo.clone()
    };
    let to_branch_label = if state.to_branch.is_empty() {
        "Select...".to_string()
    } else {
        state.to_branch.clone()
    };

    let dialog = div()
        .id("dialog-box")
        .w(px(520.0))
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
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
                            svg()
                                .size(px(16.0))
                                .path("icons/git-pull-request.svg")
                                .text_color(text_color),
                        )
                        .child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::BOLD)
                                .text_color(text_color)
                                .child("Create Pull Request"),
                        ),
                )
                .child(
                    div()
                        .id("create-pr-close")
                        .px_2()
                        .cursor_pointer()
                        .text_color(muted)
                        .child("×")
                        .on_click(move |_ev, _window, cx| {
                            if let Some(e) = ent_close.upgrade() {
                                e.update(cx, |this, cx| {
                                    this.cancel_create_pr_dialog(cx);
                                });
                            }
                        }),
                ),
        )
        .child(tabs)
        .child(
            div()
                .flex()
                .gap_3()
                .items_start()
                .child(
                    div()
                        .flex_1()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(
                            div()
                                .text_xs()
                                .text_color(muted)
                                .child("From Repo"),
                        )
                        .child(render_dropdown_trigger(
                            "from-repo",
                            &from_repo_label,
                            state.open_dropdown == CreatePrDropdown::FromRepo,
                            border,
                            accent,
                            muted,
                            text_color,
                            entity.clone(),
                            CreatePrDropdown::FromRepo,
                        ))
                        .child(
                            div()
                                .text_xs()
                                .text_color(muted)
                                .child("Branch"),
                        )
                        .child(render_dropdown_trigger(
                            "from-branch",
                            &from_branch_label,
                            state.open_dropdown == CreatePrDropdown::FromBranch,
                            border,
                            accent,
                            muted,
                            text_color,
                            entity.clone(),
                            CreatePrDropdown::FromBranch,
                        )),
                )
                .child(
                    div()
                        .pt_6()
                        .text_color(muted)
                        .child("→"),
                )
                .child(
                    div()
                        .flex_1()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(
                            div()
                                .text_xs()
                                .text_color(muted)
                                .child("To Repo"),
                        )
                        .child(render_dropdown_trigger(
                            "to-repo",
                            &to_repo_label,
                            state.open_dropdown == CreatePrDropdown::ToRepo,
                            border,
                            accent,
                            muted,
                            text_color,
                            entity.clone(),
                            CreatePrDropdown::ToRepo,
                        ))
                        .child(
                            div()
                                .text_xs()
                                .text_color(muted)
                                .child("Branch"),
                        )
                        .child(render_dropdown_trigger(
                            "to-branch",
                            &to_branch_label,
                            state.open_dropdown == CreatePrDropdown::ToBranch,
                            border,
                            accent,
                            muted,
                            text_color,
                            entity.clone(),
                            CreatePrDropdown::ToBranch,
                        )),
                ),
        )
        .child(
            div()
                .relative()
                .child({
                    let dropdown = render_open_dropdown(state, colors, entity.clone());
                    if let Some(menu) = dropdown {
                        div().relative().child(menu)
                    } else {
                        div()
                    }
                }),
        )
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_xs()
                        .text_color(muted)
                        .child("Title"),
                )
                .child(
                    div()
                        .id("create-pr-ai")
                        .px_2()
                        .py_0p5()
                        .border_1()
                        .border_color(purple)
                        .rounded(px(3.0))
                        .cursor_pointer()
                        .text_xs()
                        .text_color(purple)
                        .child(if state.generating_ai {
                            "Generating..."
                        } else {
                            "✦ Generate title and description"
                        })
                        .on_click(move |_ev, _window, cx| {
                            if let Some(e) = ent_ai.upgrade() {
                                e.update(cx, |this, cx| {
                                    this.generate_pr_title_description(cx);
                                });
                            }
                        }),
                ),
        )
        .child(title_field)
        .child(
            div()
                .text_xs()
                .text_color(muted)
                .child("Description"),
        )
        .child(description_field)
        .child(
            div()
                .id("create-pr-draft")
                .flex()
                .items_center()
                .gap_2()
                .cursor_pointer()
                .on_click(move |_ev, _window, cx| {
                    if let Some(e) = ent_draft.upgrade() {
                        e.update(cx, |this, cx| {
                            this.toggle_create_pr_draft(cx);
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
                        .border_color(if state.draft { accent } else { border })
                        .bg(if state.draft { accent } else { surface })
                        .text_color(gpui::hsla(0.0, 0.0, 1.0, 1.0))
                        .text_xs()
                        .child(if state.draft { "\u{2713}" } else { "" }),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(muted)
                        .child("Submit as draft"),
                ),
        )
        .child(
            div()
                .flex()
                .gap_2()
                .justify_end()
                .child(
                    div()
                        .id("create-pr-cancel")
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
                                    this.cancel_create_pr_dialog(cx);
                                });
                            }
                        }),
                )
                .child(
                    div()
                        .id("create-pr-submit")
                        .px_3()
                        .py_1()
                        .rounded(px(3.0))
                        .cursor_pointer()
                        .text_xs()
                        .text_color(if can_submit {
                            gpui::hsla(0.0, 0.0, 1.0, 1.0)
                        } else {
                            muted
                        })
                        .bg(if can_submit {
                            success
                        } else {
                            rgba_to_hsla(colors.surface_high)
                        })
                        .child(if state.submitting {
                            "Creating..."
                        } else {
                            "Create Pull Request"
                        })
                        .on_click(move |_ev, _window, cx| {
                            if let Some(e) = ent_submit.upgrade() {
                                e.update(cx, |this, cx| {
                                    if this.create_pr.can_submit() {
                                        this.submit_create_pr(cx);
                                    }
                                });
                            }
                        }),
                ),
        );

    div()
        .id("dialog-overlay")
        .absolute()
        .inset_0()
        .flex()
        .items_center()
        .justify_center()
        .bg(overlay_bg)
        .occlude()
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child(dialog)
}

fn render_dropdown_trigger(
    id: &str,
    label: &str,
    open: bool,
    border: Hsla,
    accent: Hsla,
    muted: Hsla,
    text_color: Hsla,
    entity: WeakEntity<GitForgeApp>,
    dropdown: CreatePrDropdown,
) -> Stateful<Div> {
    let ent = entity;
    let id_owned = id.to_string();
    div()
        .id(ElementId::Name(format!("pr-trigger-{id_owned}").into()))
        .px_2()
        .py_1()
        .border_1()
        .border_color(if open { accent } else { border })
        .rounded(px(3.0))
        .flex()
        .items_center()
        .justify_between()
        .cursor_pointer()
        .on_click(move |_ev, _window, cx| {
            if let Some(e) = ent.upgrade() {
                e.update(cx, |this, cx| {
                    this.toggle_create_pr_dropdown(dropdown, cx);
                });
            }
        })
        .child(
            div()
                .text_xs()
                .text_color(if label == "Select..." { muted } else { text_color })
                .overflow_hidden()
                .text_ellipsis()
                .child(label.to_string()),
        )
        .child(div().text_xs().text_color(muted).child("▾"))
}

fn render_open_dropdown(
    state: &CreatePrState,
    colors: &AppColors,
    entity: WeakEntity<GitForgeApp>,
) -> Option<Stateful<Div>> {
    let (items, dropdown) = match state.open_dropdown {
        CreatePrDropdown::FromRepo => (
            state.repos.iter().map(|r| r.full_name.clone()).collect::<Vec<_>>(),
            CreatePrDropdown::FromRepo,
        ),
        CreatePrDropdown::ToRepo => (
            state.repos.iter().map(|r| r.full_name.clone()).collect::<Vec<_>>(),
            CreatePrDropdown::ToRepo,
        ),
        CreatePrDropdown::FromBranch => (state.from_branches.clone(), CreatePrDropdown::FromBranch),
        CreatePrDropdown::ToBranch => (state.to_branches.clone(), CreatePrDropdown::ToBranch),
        CreatePrDropdown::None => return None,
    };

    if state.loading_repos
        && matches!(
            dropdown,
            CreatePrDropdown::FromRepo | CreatePrDropdown::ToRepo
        )
    {
        return Some(loading_dropdown(colors));
    }
    if state.loading_branches
        && matches!(
            dropdown,
            CreatePrDropdown::FromBranch | CreatePrDropdown::ToBranch
        )
    {
        return Some(loading_dropdown(colors));
    }

    let surface = rgba_to_hsla(colors.surface);
    let border = rgba_to_hsla(colors.border);
    let text_color = rgba_to_hsla(colors.text);
    let muted = rgba_to_hsla(colors.text_muted);
    let hover_bg = rgba_to_hsla(colors.sidebar_hover);

    let mut menu = div()
        .id("create-pr-dropdown")
        .absolute()
        .top(px(0.0))
        .left(px(0.0))
        .w(px(220.0))
        .max_h(px(180.0))
        .overflow_y_scroll()
        .bg(surface)
        .border_1()
        .border_color(border)
        .rounded(px(4.0))
        .py_1()
        .shadow(vec![BoxShadow {
            color: gpui::black().opacity(0.38),
            offset: point(px(0.0), px(4.0)),
            blur_radius: px(12.0),
            spread_radius: px(0.0),
        }]);

    if items.is_empty() {
        menu = menu.child(
            div()
                .px_3()
                .py_2()
                .text_xs()
                .text_color(muted)
                .child("No options"),
        );
    } else {
        for (idx, item) in items.iter().enumerate() {
            let value = item.clone();
            let ent = entity.clone();
            menu = menu.child(
                div()
                    .id(ElementId::Name(format!("pr-dd-{idx}").into()))
                    .px_3()
                    .py_1()
                    .text_xs()
                    .text_color(text_color)
                    .cursor_pointer()
                    .hover(move |s| s.bg(hover_bg))
                    .on_click(move |_ev, _window, cx| {
                        if let Some(e) = ent.upgrade() {
                            let v = value.clone();
                            e.update(cx, |this, cx| {
                                this.select_create_pr_dropdown(dropdown, v, cx);
                            });
                        }
                    })
                    .child(item.clone()),
            );
        }
    }

    Some(menu)
}

fn loading_dropdown(colors: &AppColors) -> Stateful<Div> {
    let surface = rgba_to_hsla(colors.surface);
    let border = rgba_to_hsla(colors.border);
    let muted = rgba_to_hsla(colors.text_muted);
    div()
        .id("create-pr-dropdown-loading")
        .absolute()
        .top(px(0.0))
        .left(px(0.0))
        .w(px(220.0))
        .bg(surface)
        .border_1()
        .border_color(border)
        .rounded(px(4.0))
        .px_3()
        .py_2()
        .text_xs()
        .text_color(muted)
        .child("Loading...")
}
