//! User-resizable side panels (sidebar on the left, detail/diff pane on the
//! right).
//!
//! This mirrors the column-resize pattern in `graph_panel.rs`: a mouse-down on
//! a thin splitter handle records the start position/width, and a transparent
//! `canvas` overlay registers *global* window mouse listeners that drive the
//! drag while the button is held and finalise it on release. The owning
//! `GitForgeApp` holds the live widths and the optional active-resize state.

use gitforge_ui::{AppColors, rgba_to_hsla};
use gpui::*;

use super::layout::{PANEL_RESIZE_HANDLE_WIDTH, SIDEBAR_MAX_WIDTH, SIDEBAR_MIN_WIDTH, SIDEBAR_WIDTH};

/// Which side a resize is acting on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelSide {
    /// The left sidebar's right edge.
    Sidebar,
    /// The right detail/diff pane's left edge.
    Right,
}

/// In-flight panel resize, captured at mouse-down.
#[derive(Debug, Clone, Copy)]
pub struct PanelResize {
    pub side: PanelSide,
    pub start_x: f32,
    pub start_width: f32,
}

/// Clamp helpers shared by the drag logic and the apply path.
pub fn clamp_sidebar_width(w: f32) -> f32 {
    w.clamp(SIDEBAR_MIN_WIDTH, SIDEBAR_MAX_WIDTH)
}

pub fn clamp_right_width(w: f32) -> f32 {
    w.clamp(super::layout::RIGHT_MIN_WIDTH, super::layout::RIGHT_MAX_WIDTH)
}

/// Default width for a side (used by double-click-to-reset).
pub fn default_width(side: PanelSide) -> f32 {
    match side {
        PanelSide::Sidebar => SIDEBAR_WIDTH,
        PanelSide::Right => super::layout::RIGHT_DEFAULT_WIDTH,
    }
}

/// Render the thin splitter handle between two panes.
///
/// - Single press-and-drag: starts a resize.
/// - Double-click (`click_count >= 2`): resets that side to its default width.
pub(crate) fn render_panel_resize_handle(
    id: &'static str,
    side: PanelSide,
    colors: &AppColors,
    entity: WeakEntity<super::app::GitForgeApp>,
) -> Stateful<Div> {
    let border = rgba_to_hsla(colors.border);
    let hover_bg = rgba_to_hsla(colors.border);

    div()
        .id(id)
        .w(px(PANEL_RESIZE_HANDLE_WIDTH))
        .h_full()
        .flex_shrink_0()
        .cursor(CursorStyle::ResizeLeftRight)
        // A 1px centered rail, tinted on hover. The hit target stays the full
        // `PANEL_RESIZE_HANDLE_WIDTH` for easy grabbing.
        .child(
            div()
                .w(px(1.0))
                .h_full()
                .mx_auto()
                .bg(border)
                .hover(move |s| s.bg(hover_bg)),
        )
        .on_mouse_down(MouseButton::Left, move |ev, _window, cx| {
            if let Some(e) = entity.upgrade() {
                let x = ev.position.x / px(1.0);
                if ev.click_count >= 2 {
                    e.update(cx, |this, cx| this.reset_panel_width(side, cx));
                } else {
                    e.update(cx, |this, cx| this.start_panel_resize(side, x, cx));
                }
            }
            cx.stop_propagation();
        })
}

/// Transparent overlay whose only job is to keep global window mouse listeners
/// registered every frame so an in-flight drag keeps receiving events even when
/// the cursor leaves the handle. Identical structure to `graph_panel`'s
/// `render_resize_event_listener`.
pub(crate) fn render_panel_resize_listener(
    entity: WeakEntity<super::app::GitForgeApp>,
) -> impl IntoElement {
    canvas(
        |_bounds, _window, _cx| {},
        move |_bounds, _: (), window: &mut Window, _cx: &mut App| {
            window.on_mouse_event({
                let entity = entity.clone();
                move |ev: &MouseMoveEvent, _, _, cx| {
                    if !ev.dragging() {
                        return;
                    }
                    if let Some(e) = entity.upgrade() {
                        let x = ev.position.x / px(1.0);
                        e.update(cx, |this, cx| {
                            if this.update_panel_resize(x) {
                                cx.notify();
                            }
                        });
                    }
                }
            });

            window.on_mouse_event({
                let entity = entity.clone();
                move |_ev: &MouseUpEvent, _, _, cx| {
                    if let Some(e) = entity.upgrade() {
                        e.update(cx, |this, cx| {
                            if this.finish_panel_resize() {
                                cx.notify();
                            }
                        });
                    }
                }
            });
        },
    )
    .absolute()
    .size_full()
}
