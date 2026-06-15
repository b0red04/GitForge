use crate::widgets::WidgetColors;
use gpui::*;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ButtonKind {
    Accent,
    Muted,
    Warning,
    Danger,
}

impl ButtonKind {
    fn text_color(self, c: WidgetColors) -> Hsla {
        match self {
            ButtonKind::Accent => c.accent,
            ButtonKind::Muted => c.muted,
            ButtonKind::Warning => c.warning,
            ButtonKind::Danger => c.diff_removed,
        }
    }

    fn border_color(self, c: WidgetColors) -> Hsla {
        match self {
            ButtonKind::Accent | ButtonKind::Muted => c.border,
            ButtonKind::Warning => c.warning,
            ButtonKind::Danger => c.diff_removed,
        }
    }

    fn hover_bg(self, c: WidgetColors) -> Option<Hsla> {
        match self {
            ButtonKind::Accent | ButtonKind::Warning | ButtonKind::Danger => None,
            ButtonKind::Muted => Some(c.surface_high),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ButtonSize {
    Compact,
    Small,
    Medium,
    Wide,
}

impl ButtonSize {
    fn px(self) -> impl FnOnce(Stateful<Div>) -> Stateful<Div> {
        move |d| match self {
            ButtonSize::Compact => d.px_1().py_0().rounded(px(2.0)),
            ButtonSize::Small => d.px_2().py_0().rounded(px(3.0)),
            ButtonSize::Medium => d.px_3().py_1().rounded(px(4.0)),
            ButtonSize::Wide => d
                .w_full()
                .py_2()
                .rounded(px(4.0))
                .flex()
                .items_center()
                .justify_center(),
        }
    }

    fn text_size(self, d: Stateful<Div>) -> Stateful<Div> {
        match self {
            ButtonSize::Compact | ButtonSize::Small | ButtonSize::Medium => d.text_xs(),
            ButtonSize::Wide => d.text_sm(),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum IconSize {
    Tiny,
    Small,
    Medium,
}

impl IconSize {
    fn pixels(self) -> f32 {
        match self {
            IconSize::Tiny => 14.0,
            IconSize::Small => 18.0,
            IconSize::Medium => 20.0,
        }
    }
}

pub fn action_button(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    kind: ButtonKind,
    size: ButtonSize,
    selected: bool,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    colors: WidgetColors,
) -> Stateful<Div> {
    let label = label.into();
    let mut btn = div()
        .id(id.into())
        .cursor_pointer()
        .text_xs()
        .child(label);

    if selected {
        btn = btn
            .bg(colors.accent)
            .border_1()
            .border_color(colors.accent)
            .text_color(colors.background)
            .font_weight(FontWeight::SEMIBOLD);
    } else {
        btn = btn.border_1().border_color(kind.border_color(colors)).text_color(kind.text_color(colors));
    }

    if let Some(hover_bg) = kind.hover_bg(colors) {
        btn = btn.hover(move |s| s.bg(hover_bg));
    }

    let sized = (size.px())(btn);
    size.text_size(sized).on_click(on_click)
}

pub fn primary_button(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    size: ButtonSize,
    disabled: bool,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    colors: WidgetColors,
) -> Stateful<Div> {
    let label = label.into();
    let bg = if disabled { colors.muted } else { colors.accent };
    let btn = div()
        .id(id.into())
        .cursor_pointer()
        .text_xs()
        .text_color(colors.background)
        .font_weight(FontWeight::SEMIBOLD)
        .bg(bg)
        .child(label);

    let sized = (size.px())(btn);
    size.text_size(sized).on_click(on_click)
}

pub fn icon_button(
    id: impl Into<ElementId>,
    kind: ButtonKind,
    size: IconSize,
    child: impl IntoElement,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    colors: WidgetColors,
) -> Stateful<Div> {
    let dim = px(size.pixels());
    div()
        .id(id.into())
        .w(dim)
        .h(dim)
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(2.0))
        .border_1()
        .border_color(kind.border_color(colors))
        .cursor_pointer()
        .text_xs()
        .text_color(kind.text_color(colors))
        .child(child)
        .on_click(on_click)
}

pub fn ghost_button(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    text_color: Hsla,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    let label = label.into();
    div()
        .id(id.into())
        .px_1()
        .cursor_pointer()
        .text_xs()
        .text_color(text_color)
        .child(label)
        .on_click(on_click)
}

pub fn window_control_button(
    id: impl Into<ElementId>,
    icon_path: &'static str,
    icon_color: Hsla,
    icon_hover: Hsla,
    hover_bg: Hsla,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    div()
        .id(id.into())
        .group("")
        .flex()
        .items_center()
        .justify_center()
        .w(px(20.0))
        .h(px(20.0))
        .rounded(px(10.0))
        .cursor_pointer()
        .hover(|s| s.bg(hover_bg))
        .active(|s| s.bg(hover_bg))
        .child(
            svg()
                .flex_none()
                .size(px(16.0))
                .path(icon_path)
                .text_color(icon_color)
                .group_hover("", |s| s.text_color(icon_hover)),
        )
        .on_click(on_click)
        .on_mouse_move(|_, _, cx| cx.stop_propagation())
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
}

pub fn entity_on_click<E: 'static>(
    entity: WeakEntity<E>,
    on_click: impl Fn(&mut E, &mut Context<E>) + 'static,
) -> impl Fn(&ClickEvent, &mut Window, &mut App) + 'static {
    move |_ev, _window, cx| {
        if let Some(e) = entity.upgrade() {
            e.update(cx, |this, cx| on_click(this, cx));
        }
    }
}

pub fn entity_on_click_stop_propagation<E: 'static>(
    entity: WeakEntity<E>,
    on_click: impl Fn(&mut E, &mut Context<E>) + 'static,
) -> impl Fn(&ClickEvent, &mut Window, &mut App) + 'static {
    move |_ev, _window, cx| {
        cx.stop_propagation();
        if let Some(e) = entity.upgrade() {
            e.update(cx, |this, cx| on_click(this, cx));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ButtonKind, IconSize};

    fn colors() -> super::WidgetColors {
        super::WidgetColors {
            surface: gpui::hsla(0.0, 0.0, 0.1, 1.0),
            surface_high: gpui::hsla(0.0, 0.0, 0.2, 1.0),
            background: gpui::hsla(0.0, 0.0, 0.3, 1.0),
            border: gpui::hsla(0.0, 0.0, 0.4, 1.0),
            text: gpui::hsla(0.0, 0.0, 0.5, 1.0),
            muted: gpui::hsla(0.0, 0.0, 0.6, 1.0),
            accent: gpui::hsla(0.0, 1.0, 0.5, 1.0),
            warning: gpui::hsla(0.1, 1.0, 0.5, 1.0),
            sidebar_background: gpui::hsla(0.0, 0.0, 0.15, 1.0),
            sidebar_hover: gpui::hsla(0.0, 0.0, 0.25, 1.0),
            sidebar_selected: gpui::hsla(0.0, 0.0, 0.35, 1.0),
            diff_removed: gpui::hsla(0.0, 1.0, 0.4, 1.0),
        }
    }

    #[test]
    fn kind_accent_resolves_to_accent() {
        let c = colors();
        assert_eq!(ButtonKind::Accent.text_color(c), c.accent);
        assert_eq!(ButtonKind::Accent.border_color(c), c.border);
        assert_eq!(ButtonKind::Accent.hover_bg(c), None);
    }

    #[test]
    fn kind_muted_has_hover() {
        let c = colors();
        assert_eq!(ButtonKind::Muted.hover_bg(c), Some(c.surface_high));
        assert_eq!(ButtonKind::Muted.border_color(c), c.border);
        assert_eq!(ButtonKind::Muted.text_color(c), c.muted);
    }

    #[test]
    fn kind_danger_uses_diff_removed() {
        let c = colors();
        assert_eq!(ButtonKind::Danger.text_color(c), c.diff_removed);
        assert_eq!(ButtonKind::Danger.border_color(c), c.diff_removed);
    }

    #[test]
    fn kind_warning_uses_warning() {
        let c = colors();
        assert_eq!(ButtonKind::Warning.text_color(c), c.warning);
        assert_eq!(ButtonKind::Warning.border_color(c), c.warning);
        assert_eq!(ButtonKind::Warning.hover_bg(c), None);
    }

    #[test]
    fn icon_size_pixels() {
        assert_eq!(IconSize::Tiny.pixels(), 14.0);
        assert_eq!(IconSize::Small.pixels(), 18.0);
        assert_eq!(IconSize::Medium.pixels(), 20.0);
    }
}
