use gitforge_git::{CommitInfo, RefInfo, RefKind};
use gitforge_graph::{CommitLineSegment, CurveKind, Graph};
use gitforge_ui::{AppColors, rgba_to_hsla};
use gpui::*;
use std::ops::Range;

use super::layout::{self, HASH_COL, ROW_HEIGHT, TIME_COL};

const LEFT_PADDING: f32 = 12.0;
const LANE_WIDTH: f32 = 16.0;
const COMMIT_CIRCLE_RADIUS: f32 = 3.5;
const COMMIT_CIRCLE_STROKE_WIDTH: f32 = 1.5;
const LINE_WIDTH: f32 = 1.5;
const GRAPH_COL_MIN: f32 = 80.0;
const GRAPH_COL_MAX: f32 = 320.0;
const HASH_COL_MIN: f32 = 48.0;
const HASH_COL_MAX: f32 = 140.0;
const TIME_COL_MIN: f32 = 70.0;
const TIME_COL_MAX: f32 = 160.0;
const VISIBLE_REF_PILLS: usize = 4;
const RESIZE_HANDLE_WIDTH: f32 = 6.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HistoryColumn {
    Graph,
    Sha,
    Time,
}

#[derive(Debug, Clone, Copy)]
struct HistoryColumnResize {
    column: HistoryColumn,
    start_x: f32,
    start_width: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphSelection {
    None,
    Uncommitted,
    Commit(usize),
}

pub struct GraphPanel {
    commits: Vec<CommitInfo>,
    references: Vec<RefInfo>,
    graph: Graph,
    selection: GraphSelection,
    has_uncommitted: bool,
    scroll_handle: UniformListScrollHandle,
    branch_filter: Option<String>,
    filtered_indices: Vec<usize>,
    use_filtered: bool,
    commit_index: std::collections::HashMap<String, usize>,
    graph_col_width: f32,
    hash_col_width: f32,
    time_col_width: f32,
    active_resize: Option<HistoryColumnResize>,
}

impl GraphPanel {
    pub fn new() -> Self {
        Self {
            commits: Vec::new(),
            references: Vec::new(),
            graph: Graph::new(),
            selection: GraphSelection::None,
            has_uncommitted: false,
            scroll_handle: UniformListScrollHandle::default(),
            branch_filter: None,
            filtered_indices: Vec::new(),
            use_filtered: false,
            commit_index: std::collections::HashMap::new(),
            graph_col_width: layout::GRAPH_LANE_WIDTH,
            hash_col_width: HASH_COL,
            time_col_width: TIME_COL,
            active_resize: None,
        }
    }

    pub fn set_data(
        &mut self,
        commits: Vec<CommitInfo>,
        references: Vec<RefInfo>,
        graph: Graph,
        has_uncommitted: bool,
    ) {
        self.commit_index.clear();
        for (i, c) in commits.iter().enumerate() {
            self.commit_index.insert(c.id.clone(), i);
            self.commit_index.insert(c.short_id.clone(), i);
        }
        self.commits = commits;
        self.references = references;
        self.graph = graph;
        self.has_uncommitted = has_uncommitted;
        self.selection = GraphSelection::None;
        self.update_filtered_indices();
    }

    fn update_filtered_indices(&mut self) {
        self.filtered_indices.clear();
        self.use_filtered = false;

        let Some(ref branch_name) = self.branch_filter else {
            return;
        };

        let target_ref = self.references.iter().find(|r| {
            r.name == *branch_name || (r.kind == RefKind::RemoteBranch && r.name == *branch_name)
        });

        let Some(target) = target_ref else { return };

        let target_id = &target.target_commit_id;
        let mut reachable = std::collections::HashSet::new();
        let mut queue = vec![target_id.clone()];

        while let Some(id) = queue.pop() {
            if reachable.contains(&id) {
                continue;
            }
            reachable.insert(id.clone());
            if let Some(&idx) = self.commit_index.get(&id) {
                if let Some(commit) = self.commits.get(idx) {
                    for pid in &commit.parent_ids {
                        queue.push(pid.clone());
                    }
                }
            }
        }

        for (idx, commit) in self.commits.iter().enumerate() {
            if reachable.contains(&commit.id) {
                self.filtered_indices.push(idx);
            }
        }

        self.use_filtered = !self.filtered_indices.is_empty();
    }

    pub fn set_branch_filter(&mut self, branch: Option<String>) {
        self.branch_filter = branch;
        self.clear_selection();
        self.update_filtered_indices();
    }

    pub fn selection(&self) -> GraphSelection {
        self.selection
    }

    pub fn is_uncommitted_selected(&self) -> bool {
        self.selection == GraphSelection::Uncommitted
    }

    pub fn selected_commit_idx(&self) -> Option<usize> {
        match self.selection {
            GraphSelection::Commit(idx) => Some(idx),
            _ => None,
        }
    }

    /// Alias for [`Self::selected_commit_idx`].
    pub fn selected_idx(&self) -> Option<usize> {
        self.selected_commit_idx()
    }

    pub fn clear_selection(&mut self) {
        self.selection = GraphSelection::None;
    }

    pub fn select_uncommitted(&mut self) {
        if self.has_uncommitted {
            self.selection = GraphSelection::Uncommitted;
        }
    }

    pub fn select_commit(&mut self, idx: usize) {
        if idx < self.commits.len() {
            self.selection = GraphSelection::Commit(idx);
        }
    }

    pub fn select_prev(&mut self) -> bool {
        self.select_delta(-1)
    }

    pub fn select_next(&mut self) -> bool {
        self.select_delta(1)
    }

    fn select_delta(&mut self, delta: isize) -> bool {
        if self.commits.is_empty() && !self.has_uncommitted {
            return false;
        }

        let new_selection = match (self.selection, delta) {
            (GraphSelection::None, 1) => {
                if self.has_uncommitted {
                    GraphSelection::Uncommitted
                } else if !self.commits.is_empty() {
                    GraphSelection::Commit(0)
                } else {
                    return false;
                }
            }
            (GraphSelection::None, -1) => return false,
            (GraphSelection::Uncommitted, 1) => {
                if self.commits.is_empty() {
                    return false;
                }
                GraphSelection::Commit(0)
            }
            (GraphSelection::Uncommitted, -1) => return false,
            (GraphSelection::Commit(0), -1) => {
                if self.has_uncommitted {
                    GraphSelection::Uncommitted
                } else {
                    return false;
                }
            }
            (GraphSelection::Commit(idx), d) => {
                let candidate = idx as isize + d;
                if candidate < 0 || candidate as usize >= self.commits.len() {
                    return false;
                }
                GraphSelection::Commit(candidate as usize)
            }
            _ => return false,
        };

        self.selection = new_selection;
        true
    }

    pub fn commit_id_at(&self, idx: usize) -> Option<&str> {
        self.commits.get(idx).map(|c| c.id.as_str())
    }

    pub fn find_commit_idx(&self, commit_id: &str) -> Option<usize> {
        self.commits.iter().position(|c| c.id == commit_id)
    }

    fn start_column_resize(&mut self, column: HistoryColumn, start_x: f32) {
        let start_width = match column {
            HistoryColumn::Graph => self.graph_col_width,
            HistoryColumn::Sha => self.hash_col_width,
            HistoryColumn::Time => self.time_col_width,
        };
        self.active_resize = Some(HistoryColumnResize {
            column,
            start_x,
            start_width,
        });
    }

    fn update_column_resize(&mut self, current_x: f32) -> bool {
        let Some(active_resize) = self.active_resize else {
            return false;
        };

        let delta = current_x - active_resize.start_x;
        let (target, min, max) = match active_resize.column {
            HistoryColumn::Graph => (&mut self.graph_col_width, GRAPH_COL_MIN, GRAPH_COL_MAX),
            HistoryColumn::Sha => (&mut self.hash_col_width, HASH_COL_MIN, HASH_COL_MAX),
            HistoryColumn::Time => (&mut self.time_col_width, TIME_COL_MIN, TIME_COL_MAX),
        };
        let signed_delta = match active_resize.column {
            HistoryColumn::Time => -delta,
            HistoryColumn::Graph | HistoryColumn::Sha => delta,
        };
        let next_width = (active_resize.start_width + signed_delta).clamp(min, max);

        if (*target - next_width).abs() < f32::EPSILON {
            return false;
        }

        *target = next_width;
        true
    }

    fn finish_column_resize(&mut self) -> bool {
        self.active_resize.take().is_some()
    }

    pub fn render(
        &self,
        colors: &AppColors,
        show_checkpoint_refs: bool,
        entity: WeakEntity<super::app::GitForgeApp>,
    ) -> Div {
        let bg = rgba_to_hsla(colors.background);
        let border = rgba_to_hsla(colors.border);
        let muted = rgba_to_hsla(colors.text_muted);
        let accent = rgba_to_hsla(colors.accent);

        let filter_label = self.branch_filter.as_deref().unwrap_or("All branches");
        let toggle_entity = entity.clone();
        let checkpoint_label = if show_checkpoint_refs {
            "Checkpoints: on"
        } else {
            "Checkpoints: off"
        };
        let checkpoint_color = if show_checkpoint_refs { accent } else { muted };

        let header = div()
            .px_3()
            .py_2()
            .border_b_1()
            .border_color(border)
            .flex()
            .items_center()
            .gap_2()
            .child(div().text_xs().text_color(muted).child("COMMIT HISTORY"))
            .child(div().flex_1())
            .child(
                div()
                    .id("toggle-checkpoint-refs")
                    .text_xs()
                    .cursor_pointer()
                    .text_color(checkpoint_color)
                    .on_click(move |_ev, _window, cx| {
                        if let Some(e) = toggle_entity.upgrade() {
                            e.update(cx, |this, cx| {
                                this.toggle_checkpoint_refs(cx);
                            });
                        }
                    })
                    .child(checkpoint_label),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(accent)
                    .child(filter_label.to_string()),
            );

        let graph_col_width = self.graph_col_width;
        let hash_col_width = self.hash_col_width;
        let time_col_width = self.time_col_width;
        let resize_events = render_resize_event_listener(entity.clone());
        let column_headers = render_column_headers(
            border,
            muted,
            entity.clone(),
            graph_col_width,
            hash_col_width,
            time_col_width,
        );

        if self.commits.is_empty() {
            return history_panel_shell(bg, border)
                .child(header)
                .child(column_headers)
                .child(
                    div()
                        .flex_1()
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(div().text_color(accent).child("No commits to display")),
                )
                .child(resize_events);
        }

        let total_items = self.commits.len() + if self.has_uncommitted { 1 } else { 0 };
        let commits = self.commits.clone();
        let references = self.references.clone();
        let graph = self.graph.clone();
        let selection = self.selection;
        let has_uncommitted = self.has_uncommitted;
        let cl = colors.clone();
        let cl_canvas = cl.clone();
        let scroll_handle = self.scroll_handle.clone();
        let list_entity = entity.clone();

        let list = uniform_list(
            "commit-list",
            total_items,
            move |visible_range: Range<usize>, _window: &mut Window, _cx: &mut App| {
                let mut rows = Vec::with_capacity(visible_range.len());

                for item_i in visible_range {
                    if has_uncommitted && item_i == 0 {
                        let cl_for_row = cl.clone();
                        let wip_selected = selection == GraphSelection::Uncommitted;
                        let row_bg = if wip_selected {
                            rgba_to_hsla(cl_for_row.sidebar_selected)
                        } else {
                            rgba_to_hsla(cl_for_row.background)
                        };
                        let wip_entity = list_entity.clone();
                        let row = div()
                            .id("uncommitted-row")
                            .px_0()
                            .py_0()
                            .bg(row_bg)
                            .border_b_1()
                            .border_color(rgba_to_hsla(Rgba {
                                r: 0.3,
                                g: 0.3,
                                b: 0.15,
                                a: 0.5,
                            }))
                            .flex()
                            .flex_row()
                            .items_center()
                            .h(px(ROW_HEIGHT))
                            .cursor_pointer()
                            .on_click(move |_ev, _window, cx| {
                                if let Some(e) = wip_entity.upgrade() {
                                    e.update(cx, |this, cx| {
                                        this.select_uncommitted(cx);
                                    });
                                }
                            })
                            .child(graph_spacer(graph_col_width))
                            .child(resize_spacer())
                            .child(div().w(px(hash_col_width)).flex_shrink_0())
                            .child(resize_spacer())
                            .child(
                                div()
                                    .flex_1()
                                    .pl_2()
                                    .text_xs()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(rgba_to_hsla(cl_for_row.warning))
                                    .child("Uncommitted Changes"),
                            )
                            .child(resize_spacer())
                            .child(div().w(px(time_col_width)).flex_shrink_0());

                        rows.push(row.into_any_element());
                        continue;
                    }

                    let commit_idx = if has_uncommitted { item_i - 1 } else { item_i };
                    let commit = &commits[commit_idx];
                    let is_selected = selection == GraphSelection::Commit(commit_idx);
                    let row_bg = if is_selected {
                        rgba_to_hsla(cl.sidebar_selected)
                    } else {
                        rgba_to_hsla(cl.background)
                    };

                    let refs_for_commit: Vec<&RefInfo> = references
                        .iter()
                        .filter(|r| {
                            r.target_commit_id == commit.id || r.target_commit_id == commit.short_id
                        })
                        .collect();

                    let summary = commit.summary.clone();
                    let short_id = commit.short_id.clone();

                    let click_entity = list_entity.clone();
                    let ref_pills = render_ref_pills(&refs_for_commit, &cl);
                    let time_label = format_relative_time(&commit.author_date);

                    let row = div()
                        .id(ElementId::Name(format!("commit-row-{commit_idx}").into()))
                        .px_0()
                        .py_0()
                        .bg(row_bg)
                        .border_b_1()
                        .border_color(rgba_to_hsla(Rgba {
                            r: 0.2,
                            g: 0.2,
                            b: 0.2,
                            a: 0.3,
                        }))
                        .flex()
                        .flex_row()
                        .items_center()
                        .h(px(ROW_HEIGHT))
                        .cursor_pointer()
                        .on_click(move |_ev, _window, cx| {
                            if let Some(e) = click_entity.upgrade() {
                                e.update(cx, |this, cx| {
                                    this.select_commit(commit_idx, cx);
                                });
                            }
                        })
                        .child(graph_spacer(graph_col_width))
                        .child(resize_spacer())
                        .child(
                            div()
                                .w(px(hash_col_width))
                                .flex_shrink_0()
                                .pl_2()
                                .text_xs()
                                .font_family("monospace")
                                .text_color(rgba_to_hsla(cl.accent))
                                .child(short_id),
                        )
                        .child(resize_spacer())
                        .child(
                            div()
                                .flex_1()
                                .min_w(px(0.0))
                                .text_sm()
                                .pl_1()
                                .pr_2()
                                .text_color(rgba_to_hsla(cl.text))
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap_1()
                                .overflow_hidden()
                                .child(ref_pills)
                                .child(
                                    div()
                                        .min_w(px(0.0))
                                        .overflow_hidden()
                                        .text_ellipsis()
                                        .child(summary),
                                ),
                        )
                        .child(resize_spacer())
                        .child(
                            div()
                                .w(px(time_col_width))
                                .flex_shrink_0()
                                .pr_2()
                                .text_xs()
                                .text_color(rgba_to_hsla(cl.text_muted))
                                .text_align(TextAlign::Right)
                                .child(time_label),
                        );

                    rows.push(row.into_any_element());
                }

                rows
            },
        )
        .h_full()
        .track_scroll(scroll_handle.clone());

        let graph_canvas = canvas(
            move |_bounds, _w, _cx| {},
            move |bounds: Bounds<Pixels>, _: (), window: &mut Window, _cx: &mut App| {
                paint_graph_overlay(
                    bounds,
                    &graph,
                    has_uncommitted,
                    total_items,
                    selection,
                    &scroll_handle,
                    &cl_canvas,
                    window,
                );
            },
        )
        .w(px(graph_col_width))
        .h_full();

        history_panel_shell(bg, border)
            .child(header)
            .child(column_headers)
            .child(
                div()
                    .flex_1()
                    .h_full()
                    .overflow_hidden()
                    .relative()
                    .child(list)
                    .child(
                        div()
                            .absolute()
                            .left(px(0.0))
                            .top(px(0.0))
                            .w(px(graph_col_width))
                            .h_full()
                            .overflow_hidden()
                            .child(graph_canvas),
                    ),
            )
            .child(resize_events)
    }
}

fn graph_spacer(width: f32) -> Div {
    div().w(px(width)).h(px(ROW_HEIGHT)).flex_shrink_0()
}

fn resize_spacer() -> Div {
    div().w(px(RESIZE_HANDLE_WIDTH)).flex_shrink_0()
}

fn lane_center_x(bounds: Bounds<Pixels>, lane: f32) -> Pixels {
    bounds.origin.x + px(LEFT_PADDING) + px(lane * LANE_WIDTH) + px(LANE_WIDTH / 2.0)
}

fn list_row_center_y(
    list_row: usize,
    first_visible_list_row: usize,
    row_height: Pixels,
    vertical_scroll_offset: Pixels,
    bounds: Bounds<Pixels>,
) -> Pixels {
    let relative = list_row as f32 - first_visible_list_row as f32;
    bounds.origin.y + relative as f32 * row_height + row_height / 2.0 - vertical_scroll_offset
}

fn graph_row_to_list_row(graph_row: usize, uncommitted_offset: usize) -> usize {
    graph_row + uncommitted_offset
}

fn paint_graph_overlay(
    bounds: Bounds<Pixels>,
    graph: &Graph,
    has_uncommitted: bool,
    total_list_items: usize,
    _selection: GraphSelection,
    scroll_handle: &UniformListScrollHandle,
    colors: &AppColors,
    window: &mut Window,
) {
    if bounds.size.height <= px(0.) {
        return;
    }

    let row_height = px(ROW_HEIGHT);
    let uncommitted_offset = usize::from(has_uncommitted);

    let scroll_state = scroll_handle.0.borrow();
    let viewport_height = scroll_state
        .last_item_size
        .map(|s| s.item.height)
        .unwrap_or(bounds.size.height);
    let content_height = row_height * total_list_items as f32;
    let max_scroll = (content_height - viewport_height).max(px(0.));
    let scroll_offset_y = (-scroll_state.base_handle.offset().y).clamp(px(0.), max_scroll);

    let first_visible_list_row = (scroll_offset_y / row_height).floor() as usize;
    let vertical_scroll_offset = scroll_offset_y - first_visible_list_row as f32 * row_height;
    let visible_list_row_count = (viewport_height / row_height).ceil() as usize + 2;

    let first_visible_graph_row = first_visible_list_row.saturating_sub(uncommitted_offset);
    let last_visible_graph_row = first_visible_list_row
        .saturating_add(visible_list_row_count)
        .saturating_sub(uncommitted_offset);

    // Commit dots for visible graph rows.
    for (graph_row, node) in graph.nodes().iter().enumerate() {
        let list_row = graph_row_to_list_row(graph_row, uncommitted_offset);
        if list_row < first_visible_list_row
            || list_row > first_visible_list_row + visible_list_row_count
        {
            continue;
        }

        let x = lane_center_x(bounds, node.lane as f32);
        let y = list_row_center_y(
            list_row,
            first_visible_list_row,
            row_height,
            vertical_scroll_offset,
            bounds,
        );
        let color = rgba_to_hsla(colors.graph_lane_color(node.lane));
        draw_commit_circle(x, y, color, node.is_merge, colors, window);
    }

    // Uncommitted changes indicator.
    if has_uncommitted && first_visible_list_row == 0 {
        let x = lane_center_x(bounds, 0.0);
        let y = list_row_center_y(
            0,
            first_visible_list_row,
            row_height,
            vertical_scroll_offset,
            bounds,
        );
        let r = px(COMMIT_CIRCLE_RADIUS);
        window.paint_quad(
            fill(
                Bounds::new(point(x - r, y - r), size(r * 2.0, r * 2.0)),
                rgba_to_hsla(colors.warning),
            )
            .corner_radii(r),
        );
    }

    let desired_curve_height = row_height / 3.0;
    let desired_curve_width = px(LANE_WIDTH / 3.0);

    for line in graph.lines() {
        if line.full_interval.end < first_visible_graph_row
            || line.full_interval.start > last_visible_graph_row
        {
            continue;
        }

        let Some((start_segment_idx, start_column)) =
            line.first_visible_segment(first_visible_graph_row)
        else {
            continue;
        };

        let line_x = lane_center_x(bounds, start_column as f32);
        let start_list_row = graph_row_to_list_row(line.full_interval.start, uncommitted_offset);
        let from_y = list_row_center_y(
            start_list_row,
            first_visible_list_row,
            row_height,
            vertical_scroll_offset,
            bounds,
        ) + px(COMMIT_CIRCLE_RADIUS);

        let mut current_row = from_y;
        let mut current_column = line_x;
        let mut builder = PathBuilder::stroke(px(LINE_WIDTH));
        builder.move_to(point(line_x, from_y));

        let segments = &line.segments[start_segment_idx..];
        let line_color = rgba_to_hsla(colors.graph_lane_color(line.color_lane));

        for (segment_idx, segment) in segments.iter().enumerate() {
            let is_last = segment_idx + 1 == segments.len();

            match segment {
                CommitLineSegment::Straight { to_row } => {
                    let list_row = graph_row_to_list_row(*to_row, uncommitted_offset);
                    let mut dest_row = list_row_center_y(
                        list_row,
                        first_visible_list_row,
                        row_height,
                        vertical_scroll_offset,
                        bounds,
                    );
                    if is_last {
                        dest_row -= px(COMMIT_CIRCLE_RADIUS);
                    }
                    let dest = point(current_column, dest_row);
                    current_row = dest.y;
                    builder.line_to(dest);
                    builder.move_to(dest);
                }
                CommitLineSegment::Curve {
                    to_column,
                    on_row,
                    curve_kind,
                } => {
                    let mut to_column_x = lane_center_x(bounds, *to_column as f32);
                    let list_row = graph_row_to_list_row(*on_row, uncommitted_offset);
                    let mut to_row_y = list_row_center_y(
                        list_row,
                        first_visible_list_row,
                        row_height,
                        vertical_scroll_offset,
                        bounds,
                    );

                    let going_right = to_column_x > current_column;
                    let column_shift = if going_right {
                        px(COMMIT_CIRCLE_RADIUS + COMMIT_CIRCLE_STROKE_WIDTH)
                    } else {
                        -px(COMMIT_CIRCLE_RADIUS + COMMIT_CIRCLE_STROKE_WIDTH)
                    };

                    match curve_kind {
                        CurveKind::Checkout => {
                            if is_last {
                                to_column_x -= column_shift;
                            }
                            let available_curve_width = (to_column_x - current_column).abs();
                            let available_curve_height = (to_row_y - current_row).abs();
                            let curve_width = desired_curve_width.min(available_curve_width);
                            let curve_height = desired_curve_height.min(available_curve_height);
                            let signed_curve_width = if going_right {
                                curve_width
                            } else {
                                -curve_width
                            };
                            let curve_start = point(current_column, to_row_y - curve_height);
                            let curve_end = point(current_column + signed_curve_width, to_row_y);
                            let curve_control = point(current_column, to_row_y);

                            builder.move_to(point(current_column, current_row));
                            builder.line_to(curve_start);
                            builder.move_to(curve_start);
                            builder.curve_to(curve_end, curve_control);
                            builder.move_to(curve_end);
                            builder.line_to(point(to_column_x, to_row_y));
                        }
                        CurveKind::Merge => {
                            if is_last {
                                to_row_y -= px(COMMIT_CIRCLE_RADIUS);
                            }
                            let merge_start = point(
                                current_column + column_shift,
                                current_row - px(COMMIT_CIRCLE_RADIUS),
                            );
                            let available_curve_width = (to_column_x - merge_start.x).abs();
                            let available_curve_height = (to_row_y - merge_start.y).abs();
                            let curve_width = desired_curve_width.min(available_curve_width);
                            let curve_height = desired_curve_height.min(available_curve_height);
                            let signed_curve_width = if going_right {
                                curve_width
                            } else {
                                -curve_width
                            };
                            let curve_start =
                                point(to_column_x - signed_curve_width, merge_start.y);
                            let curve_end = point(to_column_x, merge_start.y + curve_height);
                            let curve_control = point(to_column_x, merge_start.y);

                            builder.move_to(merge_start);
                            builder.line_to(curve_start);
                            builder.move_to(curve_start);
                            builder.curve_to(curve_end, curve_control);
                            builder.move_to(curve_end);
                            builder.line_to(point(to_column_x, to_row_y));
                        }
                    }
                    current_row = to_row_y;
                    current_column = to_column_x;
                    builder.move_to(point(current_column, current_row));
                }
            }
        }

        if let Ok(path) = builder.build() {
            window.paint_path(path, line_color);
        }
    }
}

fn draw_commit_circle(
    center_x: Pixels,
    center_y: Pixels,
    color: Hsla,
    is_merge: bool,
    colors: &AppColors,
    window: &mut Window,
) {
    let radius = px(COMMIT_CIRCLE_RADIUS);
    let mut builder = PathBuilder::fill();
    builder.move_to(point(center_x + radius, center_y));
    builder.arc_to(
        point(radius, radius),
        px(0.),
        false,
        true,
        point(center_x - radius, center_y),
    );
    builder.arc_to(
        point(radius, radius),
        px(0.),
        false,
        true,
        point(center_x + radius, center_y),
    );
    builder.close();

    if let Ok(path) = builder.build() {
        window.paint_path(path, color);
    }

    if is_merge {
        let inner_r = px(2.0);
        window.paint_quad(
            fill(
                Bounds::new(
                    point(center_x - inner_r, center_y - inner_r),
                    size(inner_r * 2.0, inner_r * 2.0),
                ),
                rgba_to_hsla(colors.background),
            )
            .corner_radii(inner_r),
        );
    }
}

fn history_panel_shell(bg: Hsla, border: Hsla) -> Div {
    div()
        .w_full()
        .h_full()
        .bg(bg)
        .border_r_1()
        .border_color(border)
        .relative()
        .flex()
        .flex_col()
}

fn render_resize_event_listener(entity: WeakEntity<super::app::GitForgeApp>) -> impl IntoElement {
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
                        e.update(cx, |this, cx| {
                            let current_x = ev.position.x / px(1.0);
                            if this
                                .repo_session
                                .graph_panel
                                .update_column_resize(current_x)
                            {
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
                            if this.repo_session.graph_panel.finish_column_resize() {
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

fn render_column_headers(
    border: Hsla,
    muted: Hsla,
    entity: WeakEntity<super::app::GitForgeApp>,
    graph_col_width: f32,
    hash_col_width: f32,
    time_col_width: f32,
) -> Div {
    div()
        .px_2()
        .py_1()
        .border_b_1()
        .border_color(border)
        .flex()
        .flex_row()
        .items_center()
        .text_xs()
        .text_color(muted)
        .child(
            div()
                .w(px(graph_col_width))
                .flex_shrink_0()
                .text_align(TextAlign::Center)
                .child("GRAPH"),
        )
        .child(render_resize_handle(
            HistoryColumn::Graph,
            entity.clone(),
            border,
        ))
        .child(
            div()
                .w(px(hash_col_width))
                .flex_shrink_0()
                .pl_2()
                .child("SHA"),
        )
        .child(render_resize_handle(
            HistoryColumn::Sha,
            entity.clone(),
            border,
        ))
        .child(div().flex_1().pl_1().child("DESCRIPTION"))
        .child(render_resize_handle(HistoryColumn::Time, entity, border))
        .child(
            div()
                .w(px(time_col_width))
                .flex_shrink_0()
                .pr_2()
                .text_align(TextAlign::Right)
                .child("TIME"),
        )
}

fn render_resize_handle(
    column: HistoryColumn,
    entity: WeakEntity<super::app::GitForgeApp>,
    border: Hsla,
) -> impl IntoElement {
    div()
        .id(ElementId::Name(format!("history-resize-{column:?}").into()))
        .w(px(RESIZE_HANDLE_WIDTH))
        .h(px(ROW_HEIGHT - 8.0))
        .flex_shrink_0()
        .cursor(CursorStyle::ResizeColumn)
        .rounded(px(2.0))
        .hover(move |div| div.bg(border))
        .on_mouse_down(MouseButton::Left, move |ev, _window, cx| {
            if let Some(e) = entity.upgrade() {
                e.update(cx, |this, cx| {
                    this.repo_session
                        .graph_panel
                        .start_column_resize(column, ev.position.x / px(1.0));
                    cx.notify();
                });
            }
            cx.stop_propagation();
        })
}

fn render_ref_pills(refs: &[&RefInfo], cl: &AppColors) -> Div {
    let mut row = div()
        .flex()
        .flex_row()
        .items_center()
        .gap_1()
        .overflow_hidden();

    for rf in refs.iter().take(VISIBLE_REF_PILLS) {
        row = row.child(render_ref_pill(rf, cl));
    }

    let hidden_count = refs.len().saturating_sub(VISIBLE_REF_PILLS);
    if hidden_count > 0 {
        let bg = cl.surface_high;
        row = row.child(
            div()
                .px_2()
                .border_1()
                .border_color(rgba_to_hsla(cl.border))
                .rounded(px(3.0))
                .bg(rgba_to_hsla(bg))
                .text_xs()
                .text_color(contrast_text_for(bg))
                .flex_shrink_0()
                .child(format!("+{hidden_count}")),
        );
    }

    row
}

fn render_ref_pill(rf: &RefInfo, cl: &AppColors) -> Div {
    let pill_color = ref_pill_color(rf, cl);
    div()
        .px_2()
        .border_1()
        .border_color(rgba_to_hsla(cl.border))
        .rounded(px(3.0))
        .bg(rgba_to_hsla(pill_color))
        .text_xs()
        .text_color(contrast_text_for(pill_color))
        .flex_shrink_0()
        .child(ref_pill_label(rf))
}

fn ref_pill_color(rf: &RefInfo, cl: &AppColors) -> Rgba {
    if rf.is_head {
        return cl.ref_head;
    }

    match rf.kind {
        RefKind::Branch => cl.ref_branch,
        RefKind::RemoteBranch => cl.ref_remote,
        RefKind::Tag => cl.ref_tag,
        _ => cl.surface_high,
    }
}

fn ref_pill_label(rf: &RefInfo) -> String {
    if rf.is_head {
        return "HEAD".to_string();
    }

    truncate_chars(&rf.name, 20)
}

fn truncate_chars(value: &str, max_chars: usize) -> String {
    let mut chars = value.chars();
    let prefix: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        format!("{prefix}...")
    } else {
        prefix
    }
}

fn contrast_text_for(bg: Rgba) -> Hsla {
    let luminance = 0.2126 * bg.r + 0.7152 * bg.g + 0.0722 * bg.b;
    if luminance > 0.5 {
        hsla(0.0, 0.0, 0.08, 1.0)
    } else {
        hsla(0.0, 0.0, 0.96, 1.0)
    }
}

fn format_relative_time(dt: &chrono::DateTime<chrono::Utc>) -> String {
    let now = chrono::Utc::now();
    let diff = now.signed_duration_since(*dt);
    if diff.num_seconds() < 60 {
        "just now".into()
    } else if diff.num_minutes() < 60 {
        format!("{}m ago", diff.num_minutes())
    } else if diff.num_hours() < 24 {
        format!("{}h ago", diff.num_hours())
    } else if diff.num_days() < 30 {
        format!("{}d ago", diff.num_days())
    } else if diff.num_days() < 365 {
        format!("{}mo ago", diff.num_days() / 30)
    } else {
        format!("{}y ago", diff.num_days() / 365)
    }
}
