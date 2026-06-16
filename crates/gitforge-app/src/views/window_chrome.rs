//! Client-side window frame: rounded corners, shadow, and edge resize hit targets.
//!
//! Based on Zed's `workspace::client_side_decorations`.

use gitforge_ui::{AppColors, rgba_to_hsla};
use gpui::{prelude::FluentBuilder, *};

use super::layout::{WINDOW_CHROME_INSET, WINDOW_CORNER_RADIUS};

const BORDER_SIZE: f32 = 1.0;

fn apply_corner_radius(mut el: Stateful<Div>, rounding: Pixels, tiling: Tiling) -> Stateful<Div> {
    el = el
        .when(!(tiling.top || tiling.right), |div| {
            div.rounded_tr(rounding)
        })
        .when(!(tiling.top || tiling.left), |div| div.rounded_tl(rounding))
        .when(!(tiling.bottom || tiling.right), |div| {
            div.rounded_br(rounding)
        })
        .when(!(tiling.bottom || tiling.left), |div| {
            div.rounded_bl(rounding)
        });
    el
}

fn resize_edge(
    pos: Point<Pixels>,
    shadow_size: Pixels,
    window_size: Size<Pixels>,
    tiling: Tiling,
) -> Option<ResizeEdge> {
    let bounds = Bounds::new(Point::default(), window_size).inset(shadow_size * 1.5);
    if bounds.contains(&pos) {
        return None;
    }

    let corner_size = size(shadow_size * 1.5, shadow_size * 1.5);
    let top_left_bounds = Bounds::new(Point::new(px(0.0), px(0.0)), corner_size);
    if !tiling.top && top_left_bounds.contains(&pos) {
        return Some(ResizeEdge::TopLeft);
    }

    let top_right_bounds = Bounds::new(
        Point::new(window_size.width - corner_size.width, px(0.0)),
        corner_size,
    );
    if !tiling.top && top_right_bounds.contains(&pos) {
        return Some(ResizeEdge::TopRight);
    }

    let bottom_left_bounds = Bounds::new(
        Point::new(px(0.0), window_size.height - corner_size.height),
        corner_size,
    );
    if !tiling.bottom && bottom_left_bounds.contains(&pos) {
        return Some(ResizeEdge::BottomLeft);
    }

    let bottom_right_bounds = Bounds::new(
        Point::new(
            window_size.width - corner_size.width,
            window_size.height - corner_size.height,
        ),
        corner_size,
    );
    if !tiling.bottom && bottom_right_bounds.contains(&pos) {
        return Some(ResizeEdge::BottomRight);
    }

    if !tiling.top && pos.y < shadow_size {
        Some(ResizeEdge::Top)
    } else if !tiling.bottom && pos.y > window_size.height - shadow_size {
        Some(ResizeEdge::Bottom)
    } else if !tiling.left && pos.x < shadow_size {
        Some(ResizeEdge::Left)
    } else if !tiling.right && pos.x > window_size.width - shadow_size {
        Some(ResizeEdge::Right)
    } else {
        None
    }
}

/// Wraps content in Zed-style client-side decorations.
///
/// Structure (matching Zed):
/// ```text
/// #window-backdrop (transparent, shadow inset padding, outer corner radius, resize targets)
///   └─ frame div (border color bg, 1px borders, shadow, inner corner radius)
///        └─ content (titlebar + workspace)
/// ```
pub fn render_window_chrome(
    content: impl IntoElement,
    colors: &AppColors,
    window: &mut Window,
) -> impl IntoElement {
    let decorations = window.window_decorations();
    let tiling = match decorations {
        Decorations::Server => Tiling::default(),
        Decorations::Client { tiling } => tiling,
    };
    let border = rgba_to_hsla(colors.border);
    let inset = px(WINDOW_CHROME_INSET);
    let rounding = px(WINDOW_CORNER_RADIUS);
    let border_size = px(BORDER_SIZE);

    match decorations {
        Decorations::Client { .. } => window.set_client_inset(inset),
        Decorations::Server => window.set_client_inset(px(0.0)),
    }

    div()
        .id("window-backdrop")
        .bg(gpui::transparent_black())
        .map(|backdrop| match decorations {
            Decorations::Server => backdrop,
            Decorations::Client { .. } => apply_corner_radius(backdrop, rounding, tiling)
                .when(!tiling.top, |div| div.pt(inset))
                .when(!tiling.bottom, |div| div.pb(inset))
                .when(!tiling.left, |div| div.pl(inset))
                .when(!tiling.right, |div| div.pr(inset))
                .on_mouse_move(|_e, window, _cx| window.refresh())
                .on_mouse_down(MouseButton::Left, move |e, window, _cx| {
                    let size = window.window_bounds().get_bounds().size;
                    if let Some(edge) = resize_edge(e.position, inset, size, tiling) {
                        window.start_window_resize(edge);
                    }
                }),
        })
        .size_full()
        .child(
            div()
                .id("window-frame")
                .cursor(CursorStyle::Arrow)
                .on_mouse_move(|_e, _, cx| cx.stop_propagation())
                .bg(border)
                .size_full()
                .overflow_hidden()
                .map(|frame| match decorations {
                    Decorations::Server => frame,
                    Decorations::Client { .. } => apply_corner_radius(
                        frame
                            .border_color(border)
                            .when(!tiling.top, |div| div.border_t(border_size))
                            .when(!tiling.bottom, |div| div.border_b(border_size))
                            .when(!tiling.left, |div| div.border_l(border_size))
                            .when(!tiling.right, |div| div.border_r(border_size))
                            .when(!tiling.is_tiled(), |div| {
                                div.shadow(vec![gpui::BoxShadow {
                                    color: Hsla {
                                        h: 0.0,
                                        s: 0.0,
                                        l: 0.0,
                                        a: 0.4,
                                    },
                                    blur_radius: inset / 2.0,
                                    spread_radius: px(0.0),
                                    offset: point(px(0.0), px(0.0)),
                                }])
                            }),
                        rounding,
                        tiling,
                    ),
                })
                .child(content),
        )
        .map(|backdrop| match decorations {
            Decorations::Server => backdrop,
            Decorations::Client { tiling, .. } => backdrop.child(
                canvas(
                    |_bounds, window, _cx| {
                        window.insert_hitbox(
                            Bounds::new(
                                point(px(0.0), px(0.0)),
                                window.window_bounds().get_bounds().size,
                            ),
                            HitboxBehavior::Normal,
                        )
                    },
                    move |_bounds, hitbox, window, _cx| {
                        let mouse = window.mouse_position();
                        let size = window.window_bounds().get_bounds().size;
                        let Some(edge) = resize_edge(mouse, inset, size, tiling) else {
                            return;
                        };
                        window.set_cursor_style(
                            match edge {
                                ResizeEdge::Top | ResizeEdge::Bottom => CursorStyle::ResizeUpDown,
                                ResizeEdge::Left | ResizeEdge::Right => {
                                    CursorStyle::ResizeLeftRight
                                }
                                ResizeEdge::TopLeft | ResizeEdge::BottomRight => {
                                    CursorStyle::ResizeUpLeftDownRight
                                }
                                ResizeEdge::TopRight | ResizeEdge::BottomLeft => {
                                    CursorStyle::ResizeUpRightDownLeft
                                }
                            },
                            &hitbox,
                        );
                    },
                )
                .size_full()
                .absolute(),
            ),
        })
}

/// Top corners only (title bar).
pub fn apply_top_corner_radius(
    el: Stateful<Div>,
    rounding: Pixels,
    tiling: Tiling,
) -> Stateful<Div> {
    el.when(!(tiling.top || tiling.right), |div| {
        div.rounded_tr(rounding)
    })
    .when(!(tiling.top || tiling.left), |div| div.rounded_tl(rounding))
}

/// Bottom corners only (workspace row).
pub fn apply_bottom_corner_radius(
    el: Stateful<Div>,
    rounding: Pixels,
    tiling: Tiling,
) -> Stateful<Div> {
    el.when(!(tiling.bottom || tiling.right), |div| {
        div.rounded_br(rounding)
    })
    .when(!(tiling.bottom || tiling.left), |div| {
        div.rounded_bl(rounding)
    })
}

/// 1px border overlap to hide anti-aliased gaps at rounded corners (Zed pattern).
pub fn seal_rounded_corners(el: Stateful<Div>, seal_color: Hsla) -> Stateful<Div> {
    el.mt(px(-1.0))
        .mb(px(-1.0))
        .border(px(1.0))
        .border_color(seal_color)
}
