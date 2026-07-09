use gitforge_hosting::RemoteRepo;
use gitforge_ui::{
    AppColors, DialogColors, TextInput, TextInputEvent, TextInputMode, TextInputRenderOpts,
    attach_dialog_input_keys, dialog_overlay, dialog_surface, floating_menu,
    popover_anchor_below_bounds, render_text_input, rgba_to_hsla, selectable_menu_row,
    window_anchored_popover,
};
use gpui::*;

use crate::views::app::GitForgeApp;

const DROPDOWN_TRIGGER_WIDTH: Pixels = px(220.0);
const DROPDOWN_TRIGGER_HEIGHT: Pixels = px(28.0);

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
    pub open_dropdown_bounds: Option<Bounds<Pixels>>,
    /// Latest window bounds for each dropdown trigger, updated each prepaint.
    pub(crate) trigger_bounds: [Option<Bounds<Pixels>>; 4],
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
            open_dropdown_bounds: None,
            trigger_bounds: [None; 4],
            active_field: CreatePrActiveField::Title,
        }
    }

    pub fn reset(&mut self) {
        self.open_dropdown = CreatePrDropdown::None;
        self.open_dropdown_bounds = None;
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

pub(crate) fn dropdown_bounds_slot(dropdown: CreatePrDropdown) -> usize {
    match dropdown {
        CreatePrDropdown::FromRepo => 0,
        CreatePrDropdown::FromBranch => 1,
        CreatePrDropdown::ToRepo => 2,
        CreatePrDropdown::ToBranch => 3,
        CreatePrDropdown::None => 0,
    }
}

pub fn render(
    state: &CreatePrState,
    colors: &AppColors,
    entity: WeakEntity<GitForgeApp>,
    window: &mut Window,
) -> Stateful<Div> {
    let dc = DialogColors::from_app(colors);
    let border = dc.border;
    let text_color = dc.text;
    let accent = dc.accent;
    let muted = dc.muted;
    let success = rgba_to_hsla(colors.success);
    let purple = gpui::hsla(270.0 / 360.0, 0.55, 0.65, 1.0);

    let can_submit = state.can_submit();

    let ent_close = entity.clone();
    let ent_cancel = entity.clone();
    let ent_submit = entity.clone();
    let ent_ai = entity.clone();
    let ent_draft = entity.clone();

    let provider_label = gitforge_hosting::urls::provider_label(&state.provider);

    let desc_focus = state.description_input.focus_handle().clone();

    let title_field = attach_dialog_input_keys(
        render_text_input(
            &state.title_input,
            colors,
            window,
            &TextInputRenderOpts::new(ElementId::Name("create-pr-title".into()))
                .placeholder("Pull request title"),
            |_| {},
        ),
        entity.clone(),
        {
            let desc_focus = desc_focus.clone();
            move |this, cx, window, event| match event {
                TextInputEvent::Enter { .. } => {
                    this.create_pr.active_field = CreatePrActiveField::Description;
                    window.focus(&desc_focus);
                    cx.notify();
                }
                TextInputEvent::Escape => this.cancel_create_pr_dialog(cx),
                TextInputEvent::Backspace => this.edit_create_pr_title(None, cx),
                TextInputEvent::Typed(c) => this.edit_create_pr_title(Some(&c), cx),
                _ => {}
            }
        },
    );

    let description_field = attach_dialog_input_keys(
        render_text_input(
            &state.description_input,
            colors,
            window,
            &TextInputRenderOpts::new(ElementId::Name("create-pr-description".into()))
                .placeholder("Pull request description")
                .min_h(px(96.0))
                .max_h(px(200.0))
                .overflow_y_scroll()
                .overflow_x_hidden(),
            |_| {},
        ),
        entity.clone(),
        |this, cx, _window, event| match event {
            TextInputEvent::Enter { .. } => this.edit_create_pr_description(Some("\n"), cx),
            TextInputEvent::Escape => this.cancel_create_pr_dialog(cx),
            TextInputEvent::Backspace => this.edit_create_pr_description(None, cx),
            TextInputEvent::Typed(c) => this.edit_create_pr_description(Some(&c), cx),
            _ => {}
        },
    );

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

    let dialog = dialog_surface(px(520.0), dc)
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
                        )
                        .child(
                            div()
                                .px_2()
                                .py_0p5()
                                .border_1()
                                .border_color(border)
                                .rounded(px(3.0))
                                .text_xs()
                                .text_color(muted)
                                .child(provider_label.to_string()),
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
                        .child(div().text_xs().text_color(muted).child("From Repo"))
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
                        .child(div().text_xs().text_color(muted).child("Branch"))
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
                .child(div().pt_6().text_color(muted).child("→"))
                .child(
                    div()
                        .flex_1()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(div().text_xs().text_color(muted).child("To Repo"))
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
                        .child(div().text_xs().text_color(muted).child("Branch"))
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
                .flex()
                .items_center()
                .justify_between()
                .child(div().text_xs().text_color(muted).child("Title"))
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
        .child(div().text_xs().text_color(muted).child("Description"))
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
                        .bg(if state.draft { accent } else { dc.surface })
                        .text_color(gpui::hsla(0.0, 0.0, 1.0, 1.0))
                        .text_xs()
                        .child(if state.draft { "\u{2713}" } else { "" }),
                )
                .child(div().text_xs().text_color(muted).child("Submit as draft")),
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

    let mut overlay = dialog_overlay(dc).inset_0();
    if state.open_dropdown != CreatePrDropdown::None {
        let ent_dismiss = entity.clone();
        overlay = overlay.on_click(move |_, _, cx| {
            cx.stop_propagation();
            if let Some(e) = ent_dismiss.upgrade() {
                e.update(cx, |this, cx| this.close_create_pr_dropdown(cx));
            }
        });
    } else {
        overlay = overlay.on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation());
    }

    overlay = overlay.child(dialog);

    if state.open_dropdown != CreatePrDropdown::None
        && let Some(bounds) = state.open_dropdown_bounds
        && let Some(menu) = render_dropdown_menu(state, colors, entity.clone(), state.open_dropdown)
    {
        overlay = overlay.child(window_anchored_popover(
            popover_anchor_below_bounds(bounds),
            menu,
        ));
    }

    overlay
}

#[allow(clippy::too_many_arguments)]
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
    let ent = entity.clone();
    let bounds_ent = entity.clone();
    let id_owned = id.to_string();
    let bounds_slot = dropdown_bounds_slot(dropdown);
    div()
        .id(ElementId::Name(format!("pr-trigger-{id_owned}").into()))
        .relative()
        .w(DROPDOWN_TRIGGER_WIDTH)
        .child(
            div()
                .w_full()
                .h(DROPDOWN_TRIGGER_HEIGHT)
                .px_2()
                .border_1()
                .border_color(if open { accent } else { border })
                .rounded(px(3.0))
                .flex()
                .items_center()
                .justify_between()
                .cursor_pointer()
                .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                    cx.stop_propagation();
                    if let Some(e) = ent.upgrade() {
                        e.update(cx, |this, cx| {
                            this.toggle_create_pr_dropdown(dropdown, cx);
                        });
                    }
                })
                .child(
                    div()
                        .text_xs()
                        .text_color(if label == "Select..." {
                            muted
                        } else {
                            text_color
                        })
                        .overflow_hidden()
                        .text_ellipsis()
                        .child(label.to_string()),
                )
                .child(div().text_xs().text_color(muted).child("▾"))
                .child(
                    canvas(
                        move |bounds, _, cx| {
                            if let Some(e) = bounds_ent.upgrade() {
                                e.update(cx, |app, _cx| {
                                    app.create_pr.trigger_bounds[bounds_slot] = Some(bounds);
                                });
                            }
                        },
                        |_, _, _, _| {},
                    )
                    .absolute()
                    .size_full(),
                ),
        )
}

fn render_dropdown_menu(
    state: &CreatePrState,
    colors: &AppColors,
    entity: WeakEntity<GitForgeApp>,
    dropdown: CreatePrDropdown,
) -> Option<Stateful<Div>> {
    let (items, dropdown) = match dropdown {
        CreatePrDropdown::FromRepo => (
            state
                .repos
                .iter()
                .map(|r| r.full_name.clone())
                .collect::<Vec<_>>(),
            CreatePrDropdown::FromRepo,
        ),
        CreatePrDropdown::ToRepo => (
            state
                .repos
                .iter()
                .map(|r| r.full_name.clone())
                .collect::<Vec<_>>(),
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
        return Some(loading_dropdown_menu(colors));
    }
    if state.loading_branches
        && matches!(
            dropdown,
            CreatePrDropdown::FromBranch | CreatePrDropdown::ToBranch
        )
    {
        return Some(loading_dropdown_menu(colors));
    }

    let text_color = rgba_to_hsla(colors.text);
    let muted = rgba_to_hsla(colors.text_muted);

    let mut menu = floating_menu("create-pr-dropdown", colors)
        .w(px(220.0))
        .max_h(px(180.0))
        .overflow_y_scroll()
        .py_1();

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
                selectable_menu_row(
                    ElementId::Name(format!("pr-dd-{idx}").into()),
                    false,
                    colors,
                    move |_, _, cx| {
                        if let Some(e) = ent.upgrade() {
                            let v = value.clone();
                            e.update(cx, |this, cx| {
                                this.select_create_pr_dropdown(dropdown, v, cx);
                            });
                        }
                    },
                )
                .px_3()
                .py_1()
                .text_xs()
                .text_color(text_color)
                .child(item.clone()),
            );
        }
    }

    Some(menu)
}

fn loading_dropdown_menu(colors: &AppColors) -> Stateful<Div> {
    let muted = rgba_to_hsla(colors.text_muted);
    floating_menu("create-pr-dropdown-loading", colors)
        .w(px(220.0))
        .px_3()
        .py_2()
        .text_xs()
        .text_color(muted)
        .child("Loading...")
}
