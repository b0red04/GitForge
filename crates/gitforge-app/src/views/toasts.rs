use std::time::{Duration, Instant};

use gitforge_ui::{AppColors, rgba_to_hsla};
use gpui::*;

use crate::views::app::GitForgeApp;

/// The maximum number of toasts kept on screen at once. Older toasts are
/// dropped when the cap is exceeded so the notification stack can never grow
/// unbounded.
const MAX_TOASTS: usize = 5;

const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

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
    pub loading: bool,
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
            loading: false,
            created_at: Instant::now(),
        });
        while self.toasts.len() > MAX_TOASTS {
            self.toasts.remove(0);
        }
        id
    }

    /// Pushes a loading toast that stays on screen until completed or dismissed.
    pub(crate) fn push_progress(&mut self, message: String) -> u64 {
        self.next_id += 1;
        let id = self.next_id;
        self.toasts.push(Toast {
            id,
            kind: ToastKind::Info,
            message,
            loading: true,
            created_at: Instant::now(),
        });
        while self.toasts.len() > MAX_TOASTS {
            self.toasts.remove(0);
        }
        id
    }

    pub(crate) fn update_message(&mut self, id: u64, message: String) -> bool {
        let Some(toast) = self.toasts.iter_mut().find(|t| t.id == id) else {
            return false;
        };
        toast.message = message;
        true
    }

    /// Marks a progress toast as finished with the given kind and message.
    pub(crate) fn complete(&mut self, id: u64, kind: ToastKind, message: String) -> bool {
        let Some(toast) = self.toasts.iter_mut().find(|t| t.id == id) else {
            return false;
        };
        toast.loading = false;
        toast.kind = kind;
        toast.message = message;
        true
    }

    pub(crate) fn dismiss(&mut self, id: u64) {
        self.toasts.retain(|t| t.id != id);
    }
}

/// Reduces a raw error string (which may include trailing git "hint:" lines and
/// a leading `Operation failed:` wrapper) to a single presentable line for a
/// toast. Multi-line git diagnostics would render poorly in a small card.
pub(crate) fn clean_error_message(s: &str) -> String {
    let first = s.lines().next().unwrap_or(s).trim();
    let stripped = first
        .strip_prefix("Operation failed: ")
        .or_else(|| first.strip_prefix("Operation failed:"))
        .unwrap_or(first)
        .trim();
    stripped
        .strip_prefix("error: ")
        .or_else(|| stripped.strip_prefix("error:"))
        .unwrap_or(stripped)
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::clean_error_message;

    #[test]
    fn cleans_unmerged_branch_error() {
        let raw = "Operation failed: error: the branch 'feature/x' is not fully merged\n\
                   hint: If you are sure you want to delete it, run 'git branch -D feature/x'\n\
                   hint: Disable this message with \"git config set advice.forceDeleteBranch false\"";
        assert_eq!(
            clean_error_message(raw),
            "the branch 'feature/x' is not fully merged"
        );
    }

    #[test]
    fn strips_only_operation_failed_wrapper() {
        let raw = "Operation failed: some plain message";
        assert_eq!(clean_error_message(raw), "some plain message");
    }

    #[test]
    fn strips_only_error_prefix() {
        assert_eq!(clean_error_message("error: boom"), "boom");
    }

    #[test]
    fn leaves_plain_message_intact() {
        assert_eq!(clean_error_message("nothing to strip"), "nothing to strip");
    }

    #[test]
    fn takes_first_line_of_multiline() {
        assert_eq!(clean_error_message("first line\nsecond line"), "first line");
    }

    #[test]
    fn handles_empty_input() {
        assert_eq!(clean_error_message(""), "");
    }

    #[test]
    fn progress_toast_updates_and_completes() {
        let mut toasts = super::Toasts::new();
        let id = toasts.push_progress("Step one".into());
        assert!(toasts.iter().any(|t| t.id == id && t.loading));

        assert!(toasts.update_message(id, "Step two".into()));
        assert_eq!(
            toasts.iter().find(|t| t.id == id).unwrap().message,
            "Step two"
        );

        assert!(toasts.complete(id, super::ToastKind::Success, "Done".into()));
        let toast = toasts.iter().find(|t| t.id == id).unwrap();
        assert!(!toast.loading);
        assert_eq!(toast.kind, super::ToastKind::Success);
        assert_eq!(toast.message, "Done");
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
        let loading = toast.loading;
        let ent_dismiss = entity.clone();

        let mut card = div()
            .id(ElementId::Name(format!("toast-{id}").into()))
            .bg(surface)
            .border_1()
            .border_color(border)
            .rounded(px(6.0))
            .py_2()
            .px_3()
            .flex()
            .items_start()
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
            });

        if loading {
            card = card.child(render_toast_spinner(id, color));
        } else {
            card = card.child(div().w(px(3.0)).flex_shrink_0().bg(color).rounded(px(2.0)));
        }

        card = card
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.0))
                    .line_clamp(3)
                    .text_xs()
                    .text_color(text_color)
                    .child(msg),
            )
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

fn render_toast_spinner(id: u64, color: Hsla) -> impl IntoElement {
    div()
        .id(ElementId::Name(format!("toast-spinner-{id}").into()))
        .flex_shrink_0()
        .w(px(14.0))
        .h(px(14.0))
        .flex()
        .items_center()
        .justify_center()
        .text_sm()
        .text_color(color)
        .child(SPINNER_FRAMES[0])
        .with_animation(
            ElementId::Name(format!("toast-spinner-anim-{id}").into()),
            Animation::new(Duration::from_millis(80)).repeat(),
            move |el, delta| {
                let idx =
                    (delta * SPINNER_FRAMES.len() as f32) as usize % SPINNER_FRAMES.len();
                el.child(SPINNER_FRAMES[idx])
            },
        )
}
