use crate::{AppColors, TextInputEvent, parse_key_event, rgba_to_hsla};
use gpui::*;

#[derive(Clone, Copy)]
pub struct DialogColors {
    pub overlay_bg: Hsla,
    pub surface: Hsla,
    pub border: Hsla,
    pub text: Hsla,
    pub accent: Hsla,
    pub muted: Hsla,
    pub warning: Hsla,
}

impl DialogColors {
    pub fn from_app(colors: &AppColors) -> Self {
        Self {
            overlay_bg: rgba_to_hsla(colors.background).opacity(0.7),
            surface: rgba_to_hsla(colors.surface),
            border: rgba_to_hsla(colors.border),
            text: rgba_to_hsla(colors.text),
            accent: rgba_to_hsla(colors.accent),
            muted: rgba_to_hsla(colors.text_muted),
            warning: rgba_to_hsla(colors.warning),
        }
    }
}

pub fn dialog_overlay(colors: DialogColors) -> Stateful<Div> {
    div()
        .id("dialog-overlay")
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .bg(colors.overlay_bg)
        .occlude()
        .flex()
        .items_center()
        .justify_center()
}

pub fn dialog_surface(width: Pixels, colors: DialogColors) -> Stateful<Div> {
    div()
        .id("dialog-box")
        .w(width)
        .bg(colors.surface)
        .border_1()
        .border_color(colors.border)
        .rounded(px(6.0))
        .p_4()
        .flex()
        .flex_col()
        .gap_3()
}

pub fn dialog_title(title: &str, colors: DialogColors) -> Div {
    div()
        .text_sm()
        .font_weight(FontWeight::BOLD)
        .text_color(colors.text)
        .child(title.to_string())
}

pub fn dialog_body(text: &str, colors: DialogColors) -> Div {
    div()
        .text_sm()
        .text_color(colors.text)
        .child(text.to_string())
}

pub fn dialog_label(text: &str, colors: DialogColors) -> Div {
    div()
        .text_xs()
        .text_color(colors.muted)
        .child(text.to_string())
}

pub fn dialog_actions<E: 'static>(
    cancel_id: &'static str,
    confirm_id: &'static str,
    confirm_label: &str,
    entity: WeakEntity<E>,
    on_cancel: impl Fn(&mut E, &mut Context<E>) + Clone + 'static,
    on_confirm: impl Fn(&mut E, &mut Context<E>) + Clone + 'static,
    colors: DialogColors,
) -> Div {
    let ent_cancel = entity.clone();
    let ent_confirm = entity;
    let on_cancel = on_cancel;
    let on_confirm = on_confirm;

    div()
        .flex()
        .gap_2()
        .justify_end()
        .child(dialog_button(
            cancel_id,
            "Cancel",
            colors.border,
            colors.muted,
            ent_cancel,
            on_cancel,
        ))
        .child(dialog_button(
            confirm_id,
            confirm_label,
            colors.warning,
            colors.warning,
            ent_confirm,
            on_confirm,
        ))
}

pub fn dialog_close_button<E: 'static>(
    id: &'static str,
    label: &str,
    entity: WeakEntity<E>,
    on_close: impl Fn(&mut E, &mut Context<E>) + Clone + 'static,
    colors: DialogColors,
) -> Stateful<Div> {
    dialog_button(id, label, colors.border, colors.muted, entity, on_close)
}

fn dialog_button<E: 'static>(
    id: &'static str,
    label: &str,
    border_color: Hsla,
    text_color: Hsla,
    entity: WeakEntity<E>,
    on_click: impl Fn(&mut E, &mut Context<E>) + Clone + 'static,
) -> Stateful<Div> {
    let label = label.to_string();
    div()
        .id(id)
        .px_3()
        .py_1()
        .border_1()
        .border_color(border_color)
        .rounded(px(3.0))
        .cursor_pointer()
        .text_xs()
        .text_color(text_color)
        .child(label)
        .on_click(move |_ev, _window, cx| {
            if let Some(e) = entity.upgrade() {
                e.update(cx, |this, cx| on_click(this, cx));
            }
        })
}

/// Vertical gap between a trigger's bottom edge and a popover menu below it.
/// Shared by titlebar menus ([`content_anchored_popover`]) and dialog selects
/// ([`window_anchored_popover`] + [`popover_anchor_below_bounds`]).
const POPOVER_BELOW_GAP: Pixels = px(2.0);

fn popover_below_offset() -> Point<Pixels> {
    point(px(0.0), POPOVER_BELOW_GAP)
}

/// Anchor point for a window-positioned popover below a trigger element.
pub fn popover_anchor_below_bounds(trigger: Bounds<Pixels>) -> Point<Pixels> {
    point(trigger.origin.x, trigger.bottom())
}

/// Wrap `child` in a window-anchored popover positioned below a trigger (Zed-style).
pub fn window_anchored_popover(anchor: Point<Pixels>, child: impl IntoElement) -> Anchored {
    anchored()
        .position_mode(AnchoredPositionMode::Window)
        .position(anchor)
        .anchor(Corner::TopLeft)
        .offset(popover_below_offset())
        .snap_to_window_with_margin(px(8.0))
        .child(child)
}

/// Wrap `child` in a popover anchored to an absolutely-positioned parent.
///
/// Use this when the anchor point is measured relative to app content (e.g.
/// titlebar dropdowns below [`TITLEBAR_HEIGHT`]) rather than raw window
/// coordinates. Place the returned [`Anchored`] inside a parent with
/// `.absolute().top(...).left(...)`.
pub fn content_anchored_popover(child: impl IntoElement) -> Anchored {
    anchored()
        .position_mode(AnchoredPositionMode::Local)
        .anchor(Corner::TopLeft)
        .offset(popover_below_offset())
        .snap_to_window_with_margin(px(8.0))
        .child(child)
}

pub fn attach_dialog_input_keys<E, F>(
    field: Stateful<Div>,
    entity: WeakEntity<E>,
    on_event: F,
) -> Stateful<Div>
where
    E: 'static,
    F: Fn(&mut E, &mut Context<E>, &mut Window, TextInputEvent) + Clone + 'static,
{
    let ent = entity;
    field.on_key_down(move |ev, window, cx| {
        if let Some(e) = ent.upgrade() {
            e.update(cx, |this, cx| {
                on_event(this, cx, window, parse_key_event(ev))
            });
        }
    })
}
