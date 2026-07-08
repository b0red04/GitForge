//! Shared floating-menu primitives.
//!
//! [`floating_menu`] provides the consistent surface shell (occlude + accent
//! border + drop shadow + click capture) used by every popover-style select in
//! the app - the squash wizard action menu, create-PR dropdowns, sidebar
//! context menu, and titlebar menus. [`selectable_menu_row`] provides the
//! hover/selection interaction shell for a single row. Both pair with
//! [`crate::window_anchored_popover`] for window-positioned, snap-to-window
//! placement.

use crate::{AppColors, rgba_to_hsla};
use gpui::*;

/// Standard drop shadow painted by every floating menu. Centralised here so all
/// popover-style selects share the same elevation. Callers that need a stronger
/// elevation (e.g. the local-branch dropdown) override `.shadow(...)` after
/// [`floating_menu`] returns.
fn menu_shadow() -> BoxShadow {
    BoxShadow {
        color: black().opacity(0.38),
        offset: point(px(0.0), px(4.0)),
        blur_radius: px(12.0),
        spread_radius: px(0.0),
    }
}

/// Default corner radius for floating menus. Matches the majority of popover
/// selects (create-PR, sidebar context, titlebar menus). Callers wanting a
/// softer corner override `.rounded(...)` after [`floating_menu`] returns.
const MENU_RADIUS: f32 = 4.0;

/// The shared floating-menu surface shell: occluded, accent-bordered, rounded,
/// shadowed, and click-capturing. Render it below a trigger via
/// [`crate::window_anchored_popover`]. Callers set width/padding/layout and add
/// rows (typically [`selectable_menu_row`]) as children.
pub fn floating_menu(id: impl Into<ElementId>, colors: &AppColors) -> Stateful<Div> {
    div()
        .id(id)
        .occlude()
        .bg(rgba_to_hsla(colors.surface))
        .border_1()
        .border_color(rgba_to_hsla(colors.accent))
        .rounded(px(MENU_RADIUS))
        .shadow(vec![menu_shadow()])
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
}

/// A hoverable, click-selecting menu row with an optional selected state.
///
/// Sets a transparent base background (or the theme selection background when
/// `selected`), a hover background that respects the selected state, and
/// selects on `click` - not `mouse_down` - so a press that drags off the row
/// before release does not commit (important for destructive actions). The
/// `mouse_down` event is still captured (`stop_propagation`) so the parent
/// overlay's dismiss handler doesn't fire before the click lands. Callers own
/// the row's padding, height, and content.
pub fn selectable_menu_row(
    id: impl Into<ElementId>,
    selected: bool,
    colors: &AppColors,
    on_select: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    let selected_bg = rgba_to_hsla(colors.selection_bg);
    let hover_bg = rgba_to_hsla(colors.sidebar_hover);
    let base_bg = if selected {
        selected_bg
    } else {
        transparent_black()
    };
    div()
        .id(id)
        .bg(base_bg)
        .cursor_pointer()
        .hover(move |s| s.bg(if selected { selected_bg } else { hover_bg }))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_click(move |ev, window, cx| {
            cx.stop_propagation();
            on_select(ev, window, cx);
        })
}
