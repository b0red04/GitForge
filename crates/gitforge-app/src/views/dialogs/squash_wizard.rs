use gitforge_git::RebaseAction;
use gitforge_ui::{AppColors, TextInput, TextInputEvent, TextInputRenderOpts, attach_dialog_input_keys, dialog_body, floating_menu, popover_anchor_below_bounds, render_text_input, rgba_to_hsla, selectable_menu_row, window_anchored_popover};
use gpui::*;

use crate::views::app::GitForgeApp;

const ACTION_TRIGGER_WIDTH: Pixels = px(96.0);
const ACTION_TRIGGER_HEIGHT: Pixels = px(28.0);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SquashWizardStep {
    EditPlan,
    ReviewMessages,
}

#[derive(Debug, Clone)]
pub struct SquashWizardEntry {
    pub sha: String,
    pub short_id: String,
    pub summary: String,
    pub action: RebaseAction,
    pub message: Option<String>,
}

pub struct SquashWizardState {
    pub step: SquashWizardStep,
    pub branch: String,
    pub onto: String,
    pub entries: Vec<SquashWizardEntry>,
    pub combined_message: String,
    pub needs_force_push: bool,
    pub message_input: TextInput,
    pub submitting: bool,
    pub generating_ai_message: bool,
    pub open_action_dropdown: Option<usize>,
    pub open_action_bounds: Option<Bounds<Pixels>>,
    /// Latest window bounds per action trigger, updated each prepaint.
    pub(crate) action_trigger_bounds: Vec<Option<Bounds<Pixels>>>,
    /// Monotonic identity assigned at construction so stale AI-generation
    /// results can be discarded when the wizard has been rebuilt.
    pub generation_token: u64,
}

static NEXT_SQUASH_TOKEN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

impl SquashWizardState {
    pub fn new(branch: String, onto: String, entries: Vec<SquashWizardEntry>, cx: &mut App) -> Self {
        let combined_message = entries
            .last()
            .map(|e| e.summary.clone())
            .unwrap_or_default();
        let mut message_input = TextInput::new("Squashed commit message", cx);
        message_input.set_text(&combined_message);
        let generation_token = NEXT_SQUASH_TOKEN.fetch_add(
            1,
            std::sync::atomic::Ordering::Relaxed,
        );
        Self {
            step: SquashWizardStep::EditPlan,
            branch,
            onto,
            entries,
            combined_message,
            needs_force_push: false,
            message_input,
            submitting: false,
            generating_ai_message: false,
            open_action_dropdown: None,
            open_action_bounds: None,
            action_trigger_bounds: Vec::new(),
            generation_token,
        }
    }

    pub fn reset(&mut self) {
        self.submitting = false;
        self.generating_ai_message = false;
        self.step = SquashWizardStep::EditPlan;
        self.open_action_dropdown = None;
        self.open_action_bounds = None;
    }

    pub fn has_squash_action(&self) -> bool {
        self.entries
            .iter()
            .any(|e| matches!(e.action, RebaseAction::Squash | RebaseAction::Fixup))
    }
}

pub fn render(
    state: &SquashWizardState,
    colors: &AppColors,
    entity: WeakEntity<GitForgeApp>,
    window: &mut Window,
) -> Stateful<Div> {
    let dc = gitforge_ui::DialogColors::from_app(colors);
    let border = dc.border;
    let muted = dc.muted;
    let text_color = dc.text;
    let accent = dc.accent;
    let surface = dc.surface;
    let purple = gpui::hsla(270.0 / 360.0, 0.55, 0.65, 1.0);

    let subtitle = format!(
        "{} — {} commits to rewrite",
        state.branch,
        state.entries.len()
    );

    let mut body = div().flex().flex_col().gap_2();

    match state.step {
        SquashWizardStep::EditPlan => {
            body = body.child(
                div()
                    .px_3()
                    .text_xs()
                    .text_color(muted)
                    .child(
                        "Combine several commits into fewer. Oldest commits are at the top. \
                         Use Squash all into one for the common case, or set each row's action manually.",
                    ),
            );
            body = body.child(
                div()
                    .px_3()
                    .flex()
                    .justify_end()
                    .child(action_chip(
                        "squash-all-btn",
                        "Squash all into one",
                        accent,
                        border,
                        muted,
                        entity.clone(),
                        |app, cx| app.squash_wizard_squash_all(cx),
                    )),
            );
            body = body.child(
                div()
                    .px_3()
                    .flex()
                    .gap_2()
                    .text_xs()
                    .text_color(muted)
                    .child(div().w(px(52.0)).child("Commit"))
                    .child(div().flex_1().child("Message"))
                    .child(div().w(px(96.0)).child("Action"))
                    .child(div().w(px(28.0)).child("Move")),
            );
            body = body.child({
                let mut list = div()
                    .id("squash-commit-list")
                    .max_h(px(280.0))
                    .overflow_y_scroll()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .px_3();
                for (idx, entry) in state.entries.iter().enumerate() {
                    list = list.child(render_commit_row(
                        idx,
                        entry,
                        state.entries.len(),
                        state.open_action_dropdown == Some(idx),
                        accent,
                        border,
                        muted,
                        text_color,
                        surface,
                        entity.clone(),
                    ));
                }
                list
            });
        }
        SquashWizardStep::ReviewMessages => {
            if state.needs_force_push {
                body = body.child(
                    div()
                        .px_3()
                        .py_2()
                        .rounded(px(4.0))
                        .border_1()
                        .border_color(accent)
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(text_color)
                                .child("This branch is already on the remote"),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(muted)
                                .child(
                                    "Combining commits rewrites history. GitForge will update the \
                                     remote branch for you when you continue — you don't need to \
                                     do anything else afterward.",
                                ),
                        ),
                );
            }
            body = body.child(
                div()
                    .px_3()
                    .child(
                        div()
                            .text_xs()
                            .text_color(muted)
                            .child("Commit message"),
                    )
                    .child(
                        attach_dialog_input_keys(
                            render_text_input(
                                &state.message_input,
                                colors,
                                window,
                                &TextInputRenderOpts::new(ElementId::Name(
                                    "squash-message-input".into(),
                                ))
                                .min_h(px(80.0))
                                .max_h(px(200.0))
                                .overflow_y_scroll()
                                .overflow_x_hidden()
                                .font_family("monospace"),
                                |_| {},
                            ),
                            entity.clone(),
                            |this, cx, _window, event| match event {
                                TextInputEvent::Escape => this.cancel_dialog(cx),
                                TextInputEvent::Backspace => {
                                    this.edit_squash_message(None, cx)
                                }
                                TextInputEvent::Typed(c) => {
                                    this.edit_squash_message(Some(&c), cx);
                                }
                                _ => {}
                            },
                        )
                        .mt_1(),
                    ),
            );
        }
    }

    let step = state.step;
    let can_next = !state.entries.is_empty() && !state.submitting;
    let can_confirm = !state.message_input.text().trim().is_empty() && !state.submitting;
    let needs_force_push = state.needs_force_push;

    let ent_cancel = entity.clone();
    let ent_back = entity.clone();
    let ent_next = entity.clone();
    let ent_confirm = entity.clone();

    let footer = if step == SquashWizardStep::ReviewMessages {
        let ent_ai = entity.clone();
        let generating_ai = state.generating_ai_message;
        div()
            .mt_4()
            .flex()
            .items_center()
            .justify_between()
            .gap_2()
            .child(
                div()
                    .id("squash-ai-generate")
                    .px_2()
                    .py_1()
                    .border_1()
                    .border_color(purple)
                    .rounded(px(3.0))
                    .cursor_pointer()
                    .text_xs()
                    .text_color(purple)
                    .child(if generating_ai {
                        "Generating..."
                    } else {
                        "✦ Generate message"
                    })
                    .on_click(move |_ev, _window, cx| {
                        if generating_ai {
                            return;
                        }
                        if let Some(e) = ent_ai.upgrade() {
                            e.update(cx, |this, cx| {
                                this.generate_squash_commit_message(cx);
                            });
                        }
                    }),
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(footer_button(
                        "squash-cancel",
                        "Cancel",
                        border,
                        muted,
                        text_color,
                        move |_ev, _window, cx| {
                            if let Some(e) = ent_cancel.upgrade() {
                                e.update(cx, |this, cx| this.cancel_dialog(cx));
                            }
                        },
                    ))
                    .child(footer_button(
                        "squash-back",
                        "Back",
                        border,
                        muted,
                        text_color,
                        move |_ev, _window, cx| {
                            if let Some(e) = ent_back.upgrade() {
                                e.update(cx, |this, cx| this.squash_wizard_back(cx));
                            }
                        },
                    ))
                    .child(footer_button(
                        "squash-confirm",
                        if state.submitting {
                            "Squashing..."
                        } else if needs_force_push {
                            "Squash & update remote"
                        } else {
                            "Squash commits"
                        },
                        if can_confirm { accent } else { muted },
                        rgba_to_hsla(colors.background),
                        text_color,
                        move |_ev, _window, cx| {
                            if !can_confirm {
                                return;
                            }
                            if let Some(e) = ent_confirm.upgrade() {
                                e.update(cx, |this, cx| this.execute_squash_wizard(cx));
                            }
                        },
                    )),
            )
    } else {
        let mut footer = div()
            .mt_4()
            .flex()
            .gap_2()
            .justify_end()
            .child(footer_button(
                "squash-cancel",
                "Cancel",
                border,
                muted,
                text_color,
                move |_ev, _window, cx| {
                    if let Some(e) = ent_cancel.upgrade() {
                        e.update(cx, |this, cx| this.cancel_dialog(cx));
                    }
                },
            ));
        footer = footer.child(footer_button(
            "squash-next",
            "Next",
            if can_next { accent } else { muted },
            rgba_to_hsla(colors.background),
            text_color,
            move |_ev, _window, cx| {
                if !can_next {
                    return;
                }
                if let Some(e) = ent_next.upgrade() {
                    e.update(cx, |this, cx| this.squash_wizard_next(cx));
                }
            },
        ));
        footer
    };

    let dialog = gitforge_ui::dialog_surface(px(560.0), dc)
        .child(gitforge_ui::dialog_title("Squash commits", dc))
        .child(dialog_body(&subtitle, dc))
        .child(body)
        .child(footer);

    let mut overlay = gitforge_ui::dialog_overlay(dc).inset_0();
    if state.step == SquashWizardStep::EditPlan && state.open_action_dropdown.is_some() {
        let ent_overlay = entity.clone();
        overlay = overlay.on_click(move |_, _, cx| {
            cx.stop_propagation();
            if let Some(e) = ent_overlay.upgrade() {
                e.update(cx, |this, cx| this.close_squash_action_dropdown(cx));
            }
        });
    } else {
        overlay = overlay.on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation());
    }

    overlay = overlay.child(dialog);

    if state.step == SquashWizardStep::EditPlan
        && let Some(idx) = state.open_action_dropdown
        && let Some(bounds) = state.open_action_bounds
        && let Some(entry) = state.entries.get(idx)
    {
        overlay = overlay.child(window_anchored_popover(
            popover_anchor_below_bounds(bounds),
            render_action_menu(idx, entry.action, colors, entity.clone()),
        ));
    }

    overlay
}

#[allow(clippy::too_many_arguments)]
fn render_commit_row(
    idx: usize,
    entry: &SquashWizardEntry,
    total: usize,
    action_open: bool,
    accent: Hsla,
    border: Hsla,
    muted: Hsla,
    text_color: Hsla,
    _surface: Hsla,
    entity: WeakEntity<GitForgeApp>,
) -> Stateful<Div> {
    let mut row = div()
        .id(ElementId::Name(format!("squash-row-{idx}").into()))
        .py_1p5()
        .rounded(px(4.0))
        .border_1()
        .border_color(border)
        .flex()
        .items_center()
        .gap_2();

    row = row
        .child(
            div()
                .w(px(52.0))
                .flex_shrink_0()
                .text_xs()
                .font_family("monospace")
                .text_color(muted)
                .child(entry.short_id.clone()),
        )
        .child(
            div()
                .flex_1()
                .min_w(px(0.0))
                .text_xs()
                .text_color(text_color)
                .overflow_hidden()
                .text_ellipsis()
                .child(entry.summary.clone()),
        )
        .child(render_action_trigger(
            idx,
            entry.action,
            action_open,
            accent,
            border,
            muted,
            text_color,
            entity.clone(),
        ))
        .child(render_reorder_controls(idx, total, muted, text_color, entity));

    row
}

#[allow(clippy::too_many_arguments)]
fn render_action_trigger(
    idx: usize,
    current: RebaseAction,
    open: bool,
    accent: Hsla,
    border: Hsla,
    muted: Hsla,
    text_color: Hsla,
    entity: WeakEntity<GitForgeApp>,
) -> Stateful<Div> {
    let ent_toggle = entity.clone();
    let bounds_ent = entity;
    div()
        .id(ElementId::Name(format!("squash-action-trigger-{idx}").into()))
        .relative()
        .w(ACTION_TRIGGER_WIDTH)
        .flex_shrink_0()
        .child(
            div()
                .w_full()
                .h(ACTION_TRIGGER_HEIGHT)
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
                    if let Some(e) = ent_toggle.upgrade() {
                        e.update(cx, |this, cx| {
                            this.toggle_squash_action_dropdown(idx, cx);
                        });
                    }
                })
                .child(
                    div()
                        .text_xs()
                        .text_color(text_color)
                        .child(current.label()),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(muted)
                        .child("▾"),
                )
                .child(
                    canvas(
                        move |bounds, _, cx| {
                            if let Some(e) = bounds_ent.upgrade() {
                                e.update(cx, |app, _cx| {
                                    if let Some(wizard) = app.squash_wizard.as_mut() {
                                        if wizard.action_trigger_bounds.len() <= idx {
                                            wizard
                                                .action_trigger_bounds
                                                .resize(idx + 1, None);
                                        }
                                        wizard.action_trigger_bounds[idx] = Some(bounds);
                                    }
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

fn render_action_menu(
    row_idx: usize,
    current: RebaseAction,
    colors: &AppColors,
    entity: WeakEntity<GitForgeApp>,
) -> Stateful<Div> {
    let accent = rgba_to_hsla(colors.accent);
    let border = rgba_to_hsla(colors.border);
    let muted = rgba_to_hsla(colors.text_muted);
    let text_color = rgba_to_hsla(colors.text);
    let actions = RebaseAction::available_for_entry(row_idx);
    let last_ix = actions.len().saturating_sub(1);

    let mut menu = floating_menu("squash-action-menu", colors)
        .w(px(280.0))
        .flex()
        .flex_col()
        .gap_1()
        .rounded(px(6.0))
        .p_1()
        .on_click(|_, _, cx| cx.stop_propagation());

    for (ix, action) in actions.iter().enumerate() {
        let selected = current == *action;
        let ent = entity.clone();
        let action = *action;

        let mut row = selectable_menu_row(
            ElementId::Name(
                format!("squash-action-item-{row_idx}-{}", action.as_str()).into(),
            ),
            selected,
            colors,
            move |_, _, cx| {
                if let Some(e) = ent.upgrade() {
                    e.update(cx, |this, cx| {
                        this.select_squash_action(row_idx, action, cx);
                    });
                }
            },
        )
        .w_full()
        .px_2()
        .py_1p5()
        .rounded(px(4.0))
        .flex()
        .items_start()
        .gap_2();

        if ix < last_ix {
            row = row.border_b_1().border_color(border);
        }

        row = row
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.0))
                    .flex()
                    .flex_col()
                    .gap_0p5()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(text_color)
                            .child(action.label()),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(muted)
                            .line_height(px(14.0))
                            .child(action.hint()),
                    ),
            )
            .child(
                div()
                    .w(px(16.0))
                    .flex_shrink_0()
                    .pt_0p5()
                    .text_sm()
                    .text_color(accent)
                    .child(if selected { "\u{2713}" } else { "" }),
            );

        menu = menu.child(row);
    }

    menu
}

fn render_reorder_controls(
    idx: usize,
    total: usize,
    muted: Hsla,
    text_color: Hsla,
    entity: WeakEntity<GitForgeApp>,
) -> Stateful<Div> {
    let ent_up = entity.clone();
    let ent_down = entity.clone();
    div()
        .id(ElementId::Name(format!("squash-reorder-{idx}").into()))
        .w(px(28.0))
        .flex_shrink_0()
        .flex()
        .flex_col()
        .gap_0p5()
        .child(
            div()
                .id(ElementId::Name(format!("squash-up-{idx}").into()))
                .px_1()
                .cursor_pointer()
                .text_xs()
                .text_color(if idx == 0 { muted } else { text_color })
                .child("▲")
                .on_click(move |_ev, _window, cx| {
                    if idx == 0 {
                        return;
                    }
                    if let Some(e) = ent_up.upgrade() {
                        e.update(cx, |this, cx| this.move_squash_entry(idx, true, cx));
                    }
                }),
        )
        .child(
            div()
                .id(ElementId::Name(format!("squash-down-{idx}").into()))
                .px_1()
                .cursor_pointer()
                .text_xs()
                .text_color(if idx + 1 >= total {
                    muted
                } else {
                    text_color
                })
                .child("▼")
                .on_click(move |_ev, _window, cx| {
                    if idx + 1 >= total {
                        return;
                    }
                    if let Some(e) = ent_down.upgrade() {
                        e.update(cx, |this, cx| this.move_squash_entry(idx, false, cx));
                    }
                }),
        )
}

fn action_chip(
    id: &'static str,
    label: &str,
    accent: Hsla,
    _border: Hsla,
    _muted: Hsla,
    entity: WeakEntity<GitForgeApp>,
    on_click: impl Fn(&mut GitForgeApp, &mut Context<GitForgeApp>) + 'static,
) -> Stateful<Div> {
    div()
        .id(id)
        .px_2()
        .py_1()
        .rounded(px(4.0))
        .border_1()
        .border_color(accent)
        .cursor_pointer()
        .text_xs()
        .text_color(accent)
        .child(label.to_string())
        .on_click(move |_ev, _window, cx| {
            if let Some(e) = entity.upgrade() {
                e.update(cx, |this, cx| on_click(this, cx));
            }
        })
}

fn footer_button(
    id: &'static str,
    label: impl Into<SharedString>,
    bg: Hsla,
    text_color: Hsla,
    _muted: Hsla,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    div()
        .id(id)
        .px_3()
        .py_1()
        .rounded(px(4.0))
        .bg(bg)
        .cursor_pointer()
        .text_xs()
        .text_color(text_color)
        .child(label.into())
        .on_click(on_click)
}

pub fn render_rebase_banner(colors: &AppColors, entity: WeakEntity<GitForgeApp>) -> Div {
    let dc = gitforge_ui::DialogColors::from_app(colors);
    let border = dc.border;
    let accent = dc.accent;
    let text_color = dc.text;
    let surface = dc.surface;

    let ent_continue = entity.clone();
    let ent_skip = entity.clone();
    let ent_abort = entity.clone();

    div()
        .w_full()
        .px_3()
        .py_2()
        .bg(surface)
        .border_b_1()
        .border_color(border)
        .flex()
        .items_center()
        .gap_2()
        .child(
            div()
                .flex_1()
                .text_xs()
                .text_color(text_color)
                .child("Rebase in progress"),
        )
        .child(banner_button(
            "rebase-continue",
            "Continue",
            accent,
            text_color,
            move |_ev, _window, cx| {
                if let Some(e) = ent_continue.upgrade() {
                    e.update(cx, |this, cx| this.rebase_continue_op(cx));
                }
            },
        ))
        .child(banner_button(
            "rebase-skip",
            "Skip",
            border,
            text_color,
            move |_ev, _window, cx| {
                if let Some(e) = ent_skip.upgrade() {
                    e.update(cx, |this, cx| this.rebase_skip_op(cx));
                }
            },
        ))
        .child(banner_button(
            "rebase-abort",
            "Abort",
            border,
            text_color,
            move |_ev, _window, cx| {
                if let Some(e) = ent_abort.upgrade() {
                    e.update(cx, |this, cx| this.rebase_abort_op(cx));
                }
            },
        ))
}

fn banner_button(
    id: &'static str,
    label: &'static str,
    border: Hsla,
    text_color: Hsla,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    div()
        .id(id)
        .px_2()
        .py_0p5()
        .rounded(px(3.0))
        .border_1()
        .border_color(border)
        .cursor_pointer()
        .text_xs()
        .text_color(text_color)
        .child(label)
        .on_click(on_click)
}
