//! Shared layout dimensions for the three-pane shell.

use gpui::*;

pub const SIDEBAR_WIDTH: f32 = 260.0;
pub const CENTER_MIN_WIDTH: f32 = 480.0;
pub const RIGHT_MIN_WIDTH: f32 = 360.0;
pub const CENTER_FLEX: f32 = 3.0;
pub const RIGHT_FLEX: f32 = 2.0;

pub const GRAPH_LANE_WIDTH: f32 = 140.0;
pub const HASH_COL: f32 = 60.0;
pub const TIME_COL: f32 = 90.0;
pub const FILE_LIST_WIDTH: f32 = 240.0;

pub const TITLEBAR_HEIGHT: f32 = 32.0;
/// Invisible resize margin and outer padding for client-side decorations.
pub const WINDOW_CHROME_INSET: f32 = 10.0;
/// Corner radius for the window frame (Zed-style).
pub const WINDOW_CORNER_RADIUS: f32 = 10.0;
pub const TOOLBAR_HEIGHT: f32 = 40.0;
pub const STATUS_BAR_HEIGHT: f32 = 24.0;

pub const ROW_HEIGHT: f32 = 28.0;

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
