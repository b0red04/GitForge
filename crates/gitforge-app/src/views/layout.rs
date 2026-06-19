//! Shared layout dimensions for the three-pane shell.

use gpui::*;

pub const SIDEBAR_WIDTH: f32 = 260.0;
pub const SIDEBAR_MIN_WIDTH: f32 = 180.0;
pub const SIDEBAR_MAX_WIDTH: f32 = 520.0;
pub const CENTER_MIN_WIDTH: f32 = 480.0;
pub const RIGHT_MIN_WIDTH: f32 = 360.0;
pub const RIGHT_DEFAULT_WIDTH: f32 = 560.0;
pub const RIGHT_MAX_WIDTH: f32 = 1100.0;
/// Width of the draggable splitter between the sidebar / center / right pane.
pub const PANEL_RESIZE_HANDLE_WIDTH: f32 = 6.0;
pub const CENTER_FLEX: f32 = 3.0;
pub const RIGHT_FLEX: f32 = 2.0;

pub const GRAPH_LANE_WIDTH: f32 = 140.0;
pub const HASH_COL: f32 = 60.0;
pub const TIME_COL: f32 = 90.0;
pub const AUTHOR_COL: f32 = 120.0;
pub const FILE_LIST_WIDTH: f32 = 240.0;

pub const TITLEBAR_HEIGHT: f32 = 32.0;
/// Invisible resize margin and outer padding for client-side decorations.
pub const WINDOW_CHROME_INSET: f32 = 10.0;
/// Corner radius for the window frame (Zed-style).
pub const WINDOW_CORNER_RADIUS: f32 = 10.0;
pub const TOOLBAR_HEIGHT: f32 = 40.0;

pub const ROW_HEIGHT: f32 = 28.0;

/// Snap a logical pixel length to the nearest whole device pixel at `scale`.
///
/// Fractional display scaling (e.g. 125% on 4K) otherwise leaves 1px strokes
/// and flex text at sub-pixel sizes, which makes icons look faint and can
/// trigger premature ellipsis.
pub fn snap_px(value: f32, scale: f32) -> Pixels {
    (px(value) * scale).round() / scale
}

/// Center history pane: flex-grow weight in the main content row.
pub fn grow_center(mut pane: Div) -> Div {
    pane = pane
        .flex_1()
        .min_w(px(CENTER_MIN_WIDTH))
        .h_full()
        .overflow_hidden();
    pane.style().flex_grow = Some(CENTER_FLEX);
    pane.style().flex_shrink = Some(1.0);
    pane
}

/// Right detail/diff pane: flex-grow weight in the main content row.
pub fn grow_right(mut pane: Div) -> Div {
    pane = pane
        .flex_1()
        .min_w(px(RIGHT_MIN_WIDTH))
        .h_full()
        .overflow_hidden();
    pane.style().flex_grow = Some(RIGHT_FLEX);
    pane.style().flex_shrink = Some(1.0);
    pane
}

/// Right detail/diff pane at a user-controlled fixed width. The center pane
/// is `flex_1`, so it absorbs the remaining horizontal space; we only need to
/// pin the right pane's width and let it not grow/shrink.
pub fn right_pane_fixed(mut pane: Div, width: f32) -> Div {
    pane = pane
        .h_full()
        .overflow_hidden()
        .min_w(px(RIGHT_MIN_WIDTH))
        .w(px(width));
    pane.style().flex_grow = Some(0.0);
    pane.style().flex_shrink = Some(0.0);
    pane
}
