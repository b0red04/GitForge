use std::time::Instant;

use gitforge_ui::{AppColors, rgba_to_hsla};
use gpui::*;

use crate::views::app::GitForgeApp;

/// The maximum number of toasts kept on screen at once. Older toasts are
/// dropped when the cap is exceeded so the notification stack can never grow
/// unbounded.
const MAX_TOASTS: usize = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) enum ToastKind {
    Info,
    Success,
    Warning,
    Error,
}

impl ToastKind {
    fn color(self, colors: &AppColors) -> Hsla {
        match self {
            ToastKind::Info => rgba_to_hsla(colors.accent),
            ToastKind::Success => rgba_to_hsla(colors.success),
            ToastKind::Warning => rgba_to_hsla(colors.warning),
            ToastKind::Error => rgba_to_hsla(colors.error),
        }
    }

    /// How long a toast of this kind stays on screen before auto-dismissing.
    pub(crate) fn auto_dismiss_secs(self) -> u64 {
        match self {
            // Errors and warnings need to be read; give them more time.
            ToastKind::Error | ToastKind::Warning => 7,
            ToastKind::Info | ToastKind::Success => 4,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Toast {
    pub id: u64,
    pub kind: ToastKind,
    pub message: String,
    #[allow(dead_code)]
    pub created_at: Instant,
}

#[derive(Debug, Default)]
pub(crate) struct Toasts {
    toasts: Vec<Toast>,
    next_id: u64,
}

impl Toasts {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = &Toast> {
        self.toasts.iter()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.toasts.is_empty()
    }

    /// Pushes a toast, returning the id assigned to it. If the cap is exceeded
    /// the oldest toast is dropped.
    pub(crate) fn push(&mut self, kind: ToastKind, message: String) -> u64 {
        self.next_id += 1;
        let id = self.next_id;
        self.toasts.push(Toast {
            id,
            kind,
            message,
            created_at: Instant::now(),
        });
        while self.toasts.len() > MAX_TOASTS {
            self.toasts.remove(0);
        }
        id
    }

    pub(crate) fn dismiss(&mut self, id: u64) {
        self.toasts.retain(|t| t.id != id);
    }
}

/// Renders the toast stack as an absolute-positioned overlay anchored to the
/// bottom-right of its (relative) parent. Each card is clickable to dismiss.
pub(crate) fn render_toasts(
    toasts: &Toasts,
    colors: &AppColors,
    entity: WeakEntity<GitForgeApp>,
) -> Div {
    let surface = rgba_to_hsla(colors.surface_high);
    let border = rgba_to_hsla(colors.border);
    let text_color = rgba_to_hsla(colors.text);
    let muted = rgba_to_hsla(colors.text_muted);

    let mut stack = div()
        .absolute()
        .bottom(px(12.0))
        .right(px(12.0))
        .w(px(380.0))
        .flex()
        .flex_col()
        .gap_2();

    for toast in toasts.iter() {
        let id = toast.id;
        let color = toast.kind.color(colors);
        let msg = toast.message.clone();
        let ent_dismiss = entity.clone();

        let card = div()
            .id(ElementId::Name(format!("toast-{id}").into()))
            .bg(surface)
            .border_1()
            .border_color(border)
            .rounded(px(6.0))
            .py_2()
            .px_3()
            .flex()
            .gap_3()
            .shadow(vec![BoxShadow {
                color: black().opacity(0.4),
                offset: point(px(0.0), px(4.0)),
                blur_radius: px(12.0),
                spread_radius: px(0.0),
            }])
            .on_click(move |_ev, _window, cx| {
                if let Some(e) = ent_dismiss.upgrade() {
                    e.update(cx, |this, cx| this.dismiss_toast(id, cx));
                }
            })
            .child(
                div()
                    .w(px(3.0))
                    .flex_shrink_0()
                    .bg(color)
                    .rounded(px(2.0)),
            )
            .child(div().flex_1().text_xs().text_color(text_color).child(msg))
            .child(
                div()
                    .id(ElementId::Name(format!("toast-dismiss-{id}").into()))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .cursor_pointer()
                    .text_xs()
                    .text_color(muted)
                    .child("\u{00d7}"),
            );

        stack = stack.child(card);
    }

    stack
}
