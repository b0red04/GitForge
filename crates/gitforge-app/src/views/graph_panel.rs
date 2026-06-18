use gitforge_git::{CommitInfo, RefInfo, RefKind};
use gitforge_graph::{CommitLineSegment, CurveKind, Graph};
use gitforge_ui::{AppColors, ShellWidth, panel_shell, rgba_to_hsla};
use gpui::*;
use std::collections::{HashMap, HashSet};
use std::ops::Range;
use std::sync::Arc;

use super::layout::{self, AUTHOR_COL, HASH_COL, ROW_HEIGHT, TIME_COL};

const LEFT_PADDING: f32 = 12.0;
const LANE_WIDTH: f32 = 16.0;
const COMMIT_CIRCLE_RADIUS: f32 = 3.5;
const COMMIT_CIRCLE_STROKE_WIDTH: f32 = 1.5;
const LINE_WIDTH: f32 = 1.5;
const GRAPH_COL_MIN: f32 = 80.0;
const GRAPH_COL_MAX: f32 = 1200.0;
const HASH_COL_MIN: f32 = 48.0;
const HASH_COL_MAX: f32 = 140.0;
const TIME_COL_MIN: f32 = 70.0;
const TIME_COL_MAX: f32 = 160.0;
const AUTHOR_COL_MIN: f32 = 60.0;
const AUTHOR_COL_MAX: f32 = 200.0;
const VISIBLE_REF_PILLS: usize = 4;
const RESIZE_HANDLE_WIDTH: f32 = 6.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HistoryColumn {
    Graph,
    Sha,
    Time,
    Author,
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

#[derive(Clone)]
struct CommitRowRenderData {
    summary: SharedString,
    short_id: SharedString,
    author_name: SharedString,
    relative_time: SharedString,
}

struct CommitMessageTooltip {
    message: SharedString,
    colors: AppColors,
}

impl CommitMessageTooltip {
    fn new(message: SharedString, colors: AppColors) -> Self {
        Self { message, colors }
    }
}

impl Render for CommitMessageTooltip {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let cl = self.colors.clone();
        let (summary, body) = match self.message.find("\n\n") {
            Some(idx) => (
                self.message[..idx].to_string(),
                self.message[idx + 2..].trim().to_string(),
            ),
            None => (self.message.to_string(), String::new()),
        };
        let mut tip = div()
            .id("commit-message-tooltip")
            .p_2()
            .max_w(px(440.0))
            .max_h(px(320.0))
            .overflow_y_scroll()
            .bg(rgba_to_hsla(cl.background))
            .border_1()
            .border_color(rgba_to_hsla(cl.border))
            .rounded(px(6.0))
            .shadow(vec![BoxShadow {
                color: black(),
                offset: point(px(0.0), px(4.0)),
                blur_radius: px(12.0),
                spread_radius: px(0.0),
            }])
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgba_to_hsla(cl.text))
                    .child(summary),
            );
        if !body.is_empty() {
            tip = tip.child(
                div()
                    .mt_1()
                    .text_xs()
                    .text_color(rgba_to_hsla(cl.text_muted))
                    .child(body),
            );
        }
        tip
    }
}

#[derive(Clone)]
struct CommitGraphDecoration {
    graph: Arc<Graph>,
    graph_col_width: f32,
    has_uncommitted: bool,
    colors: AppColors,
}

pub struct GraphPanel {
    commits: Arc<[CommitInfo]>,
    row_render_data: Arc<[CommitRowRenderData]>,
    references: Arc<[RefInfo]>,
    graph: Arc<Graph>,
    selection: GraphSelection,
    has_uncommitted: bool,
    scroll_handle: UniformListScrollHandle,
    branch_filter: Option<String>,
    filtered_indices: Vec<usize>,
    use_filtered: bool,
    commit_index: HashMap<String, usize>,
    refs_by_commit: Arc<HashMap<String, Arc<[RefInfo]>>>,
    detached_head_commit: Option<String>,
    graph_col_width: f32,
    graph_col_user_resized: bool,
    hash_col_width: f32,
    time_col_width: f32,
    author_col_width: f32,
    active_resize: Option<HistoryColumnResize>,
}

impl GraphPanel {
    pub fn new() -> Self {
        Self {
            commits: Arc::from([]),
            row_render_data: Arc::from([]),
            references: Arc::from([]),
            graph: Arc::new(Graph::new()),
            selection: GraphSelection::None,
            has_uncommitted: false,
            scroll_handle: UniformListScrollHandle::default(),
            branch_filter: None,
            filtered_indices: Vec::new(),
            use_filtered: false,
            commit_index: HashMap::new(),
            refs_by_commit: Arc::new(HashMap::new()),
            detached_head_commit: None,
            graph_col_width: layout::GRAPH_LANE_WIDTH,
            graph_col_user_resized: false,
            hash_col_width: HASH_COL,
            time_col_width: TIME_COL,
            author_col_width: AUTHOR_COL,
            active_resize: None,
        }
    }

    pub fn set_data(
        &mut self,
        commits: Vec<CommitInfo>,
        references: Vec<RefInfo>,
        graph: Graph,
        has_uncommitted: bool,
        detached_head_commit: Option<String>,
    ) {
        self.commit_index.clear();
        for (i, c) in commits.iter().enumerate() {
            self.commit_index.insert(c.id.clone(), i);
            self.commit_index.insert(c.short_id.clone(), i);
        }
        self.row_render_data = commits
            .iter()
            .map(|commit| CommitRowRenderData {
                summary: commit.summary.clone().into(),
                short_id: commit.short_id.clone().into(),
                author_name: commit.author_name.clone().into(),
                relative_time: format_relative_time(&commit.author_date).into(),
            })
            .collect::<Vec<_>>()
            .into();
        self.refs_by_commit = Arc::new(build_refs_by_commit(&references));
        if !self.graph_col_user_resized {
            self.graph_col_width = auto_graph_col_width(&graph);
        }

        self.commits = commits.into();
        self.references = references.into();
        self.graph = Arc::new(graph);
        self.has_uncommitted = has_uncommitted;
        self.detached_head_commit = detached_head_commit;
        self.selection = GraphSelection::None;
        self.update_filtered_indices();
    }

    fn update_filtered_indices(&mut self) {
        self.filtered_indices.clear();
        self.use_filtered = false;

        let Some(ref branch_name) = self.branch_filter else {
            return;
        };

        let target_ref = self
            .references
            .iter()
            .find(|r| r.name == *branch_name);

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
            HistoryColumn::Author => self.author_col_width,
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
            HistoryColumn::Graph => (
                &mut self.graph_col_width,
                GRAPH_COL_MIN,
                GRAPH_COL_MAX.max(active_resize.start_width),
            ),
            HistoryColumn::Sha => (&mut self.hash_col_width, HASH_COL_MIN, HASH_COL_MAX),
            HistoryColumn::Time => (&mut self.time_col_width, TIME_COL_MIN, TIME_COL_MAX),
            HistoryColumn::Author => (&mut self.author_col_width, AUTHOR_COL_MIN, AUTHOR_COL_MAX),
        };
        let signed_delta = match active_resize.column {
            HistoryColumn::Time | HistoryColumn::Author => -delta,
            HistoryColumn::Graph | HistoryColumn::Sha => delta,
        };
        let next_width = (active_resize.start_width + signed_delta).clamp(min, max);

        if (*target - next_width).abs() < f32::EPSILON {
            return false;
        }

        if active_resize.column == HistoryColumn::Graph {
            self.graph_col_user_resized = true;
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
        show_graph_col: bool,
        show_sha_col: bool,
        show_time_col: bool,
        show_author_col: bool,
        entity: WeakEntity<super::app::GitForgeApp>,
    ) -> Div {
        let bg = rgba_to_hsla(colors.background);
        let border = rgba_to_hsla(colors.border);
        let muted = rgba_to_hsla(colors.text_muted);
        let accent = rgba_to_hsla(colors.accent);

        let graph_col_width = if show_graph_col {
            self.graph_col_width
        } else {
            0.0
        };
        let hash_col_width = if show_sha_col {
            self.hash_col_width
        } else {
            0.0
        };
        let time_col_width = if show_time_col {
            self.time_col_width
        } else {
            0.0
        };
        let author_col_width = if show_author_col {
            self.author_col_width
        } else {
            0.0
        };
        let resize_events = render_resize_event_listener(entity.clone());
        let column_headers = render_column_headers(
            border,
            muted,
            entity.clone(),
            show_graph_col,
            graph_col_width,
            show_sha_col,
            hash_col_width,
            show_author_col,
            author_col_width,
            show_time_col,
            time_col_width,
        );

        if self.commits.is_empty() {
            return history_panel_shell(bg, border)
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
        let commits = Arc::clone(&self.commits);
        let row_render_data = Arc::clone(&self.row_render_data);
        let refs_by_commit = Arc::clone(&self.refs_by_commit);
        let graph = Arc::clone(&self.graph);
        let selection = self.selection;
        let has_uncommitted = self.has_uncommitted;
        let detached_head_commit = self.detached_head_commit.clone();
        let cl = colors.clone();
        let scroll_handle = self.scroll_handle.clone();
        let list_entity = entity.clone();

        let mut list = uniform_list(
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
                        let mut row = div()
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
                            .h(graph_row_height())
                            .cursor_pointer()
                            .on_click(move |_ev, _window, cx| {
                                if let Some(e) = wip_entity.upgrade() {
                                    e.update(cx, |this, cx| {
                                        this.select_uncommitted(cx);
                                    });
                                }
                            });
                        if show_graph_col {
                            row = row
                                .child(graph_spacer(graph_col_width))
                                .child(resize_spacer());
                        }
                        if show_sha_col {
                            row = row
                                .child(div().w(px(hash_col_width)).flex_shrink_0())
                                .child(resize_spacer());
                        }
                        row = row.child(
                            div()
                                .flex_1()
                                .pl_2()
                                .text_xs()
                                .font_weight(FontWeight::BOLD)
                                .text_color(rgba_to_hsla(cl_for_row.warning))
                                .child("Uncommitted Changes"),
                        );
                        if show_author_col {
                            row = row
                                .child(resize_spacer())
                                .child(div().w(px(author_col_width)).flex_shrink_0());
                        }
                        if show_time_col {
                            row = row
                                .child(resize_spacer())
                                .child(div().w(px(time_col_width)).flex_shrink_0());
                        }

                        rows.push(row.into_any_element());
                        continue;
                    }

                    let commit_idx = if has_uncommitted { item_i - 1 } else { item_i };
                    let commit = &commits[commit_idx];
                    let row_data = &row_render_data[commit_idx];
                    let is_selected = selection == GraphSelection::Commit(commit_idx);
                    let row_bg = if is_selected {
                        rgba_to_hsla(cl.sidebar_selected)
                    } else {
                        rgba_to_hsla(cl.background)
                    };

                    let refs_for_commit = refs_by_commit.get(&commit.id);

                    let summary = row_data.summary.clone();
                    let short_id = row_data.short_id.clone();
                    let author_name = row_data.author_name.clone();
                    let time_label = row_data.relative_time.clone();

                    let click_entity = list_entity.clone();
                    let ref_pills = render_ref_pills(
                        refs_for_commit,
                        &cl,
                        &commit.id,
                        detached_head_commit.as_deref(),
                    );

                    let has_body = commit.message != commit.summary && !commit.message.is_empty();
                    let tip_message: SharedString = commit.message.clone().into();
                    let tip_colors = cl.clone();

                    let mut row = div()
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
                        .h(graph_row_height())
                        .cursor_pointer()
                        .on_click(move |_ev, _window, cx| {
                            if let Some(e) = click_entity.upgrade() {
                                e.update(cx, |this, cx| {
                                    this.select_commit(commit_idx, cx);
                                });
                            }
                        });
                    if show_graph_col {
                        row = row
                            .child(graph_spacer(graph_col_width))
                            .child(resize_spacer());
                    }
                    if show_sha_col {
                        row = row
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
                            .child(resize_spacer());
                    }
                    row = row.child(
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
                            .child({
                                let mut desc =
                                    div().min_w(px(0.0)).overflow_hidden().text_ellipsis().id(
                                        ElementId::Name(format!("commit-desc-{commit_idx}").into()),
                                    );
                                if has_body {
                                    desc = desc.tooltip(move |_window, cx| {
                                        cx.new(|_cx| {
                                            CommitMessageTooltip::new(
                                                tip_message.clone(),
                                                tip_colors.clone(),
                                            )
                                        })
                                        .into()
                                    });
                                }
                                desc.child(summary)
                            }),
                    );
                    if show_author_col {
                        row = row.child(resize_spacer()).child(
                            div()
                                .w(px(author_col_width))
                                .flex_shrink_0()
                                .text_xs()
                                .text_color(rgba_to_hsla(cl.text_muted))
                                .overflow_hidden()
                                .text_ellipsis()
                                .child(author_name),
                        );
                    }
                    if show_time_col {
                        row = row.child(resize_spacer()).child(
                            div()
                                .w(px(time_col_width))
                                .flex_shrink_0()
                                .pr_2()
                                .text_xs()
                                .text_color(rgba_to_hsla(cl.text_muted))
                                .text_align(TextAlign::Right)
                                .child(time_label),
                        );
                    }

                    rows.push(row.into_any_element());
                }

                rows
            },
        )
        .h_full()
        .track_scroll(scroll_handle.clone());

        if show_graph_col {
            list = list.with_decoration(CommitGraphDecoration {
                graph,
                graph_col_width,
                has_uncommitted,
                colors: colors.clone(),
            });
        }

        let content_area = div()
            .flex_1()
            .h_full()
            .overflow_hidden()
            .relative()
            .child(list);

        history_panel_shell(bg, border)
            .child(column_headers)
            .child(content_area)
            .child(resize_events)
    }
}

impl UniformListDecoration for CommitGraphDecoration {
    fn compute(
        &self,
        visible_range: Range<usize>,
        _bounds: Bounds<Pixels>,
        _scroll_offset: Point<Pixels>,
        item_height: Pixels,
        item_count: usize,
        _window: &mut Window,
        _cx: &mut App,
    ) -> AnyElement {
        let graph = Arc::clone(&self.graph);
        let has_uncommitted = self.has_uncommitted;
        let colors = self.colors.clone();
        let content_height = item_height * item_count;

        canvas(
            move |_bounds, _w, _cx| {},
            move |bounds: Bounds<Pixels>, _: (), window: &mut Window, _cx: &mut App| {
                paint_graph_overlay(
                    bounds,
                    &graph,
                    has_uncommitted,
                    visible_range.clone(),
                    item_height,
                    &colors,
                    window,
                );
            },
        )
        .w(px(self.graph_col_width))
        .h(content_height)
        .into_any_element()
    }
}

fn graph_spacer(width: f32) -> Div {
    div().w(px(width)).h(graph_row_height()).flex_shrink_0()
}

fn graph_row_height() -> Pixels {
    px(ROW_HEIGHT)
}

fn build_refs_by_commit(references: &[RefInfo]) -> HashMap<String, Arc<[RefInfo]>> {
    let mut grouped: HashMap<String, Vec<RefInfo>> = HashMap::new();
    for rf in references {
        grouped
            .entry(rf.target_commit_id.clone())
            .or_default()
            .push(rf.clone());
    }

    grouped
        .into_iter()
        .map(|(commit_id, refs)| (commit_id, Arc::from(refs)))
        .collect()
}

fn resize_spacer() -> Div {
    div().w(px(RESIZE_HANDLE_WIDTH)).flex_shrink_0()
}

fn auto_graph_col_width(graph: &Graph) -> f32 {
    let max_node_lane = graph.nodes().iter().map(|node| node.lane).max();
    let max_line_lane = graph.lines().iter().fold(None, |max_lane, line| {
        let line_max = line.segments.iter().fold(
            line.child_column.max(line.color_lane),
            |segment_max, segment| match segment {
                CommitLineSegment::Straight { .. } => segment_max,
                CommitLineSegment::Curve { to_column, .. } => segment_max.max(*to_column),
            },
        );

        Some(max_lane.map_or(line_max, |lane: usize| lane.max(line_max)))
    });

    let max_lane = max_node_lane
        .into_iter()
        .chain(max_line_lane)
        .max()
        .unwrap_or(0);
    let required_width = LEFT_PADDING + (max_lane as f32 + 1.0) * LANE_WIDTH + LEFT_PADDING;

    required_width
        .max(layout::GRAPH_LANE_WIDTH)
        .max(GRAPH_COL_MIN)
        .min(GRAPH_COL_MAX)
}

fn lane_center_x(bounds: Bounds<Pixels>, lane: f32) -> Pixels {
    bounds.origin.x + px(LEFT_PADDING) + px(lane * LANE_WIDTH) + px(LANE_WIDTH / 2.0)
}

fn list_row_center_y(list_row: usize, row_height: Pixels, bounds: Bounds<Pixels>) -> Pixels {
    bounds.origin.y + list_row as f32 * row_height + row_height / 2.0
}

fn graph_row_to_list_row(graph_row: usize, uncommitted_offset: usize) -> usize {
    graph_row + uncommitted_offset
}

fn paint_graph_overlay(
    bounds: Bounds<Pixels>,
    graph: &Graph,
    has_uncommitted: bool,
    visible_list_rows: Range<usize>,
    row_height: Pixels,
    colors: &AppColors,
    window: &mut Window,
) {
    if bounds.size.height <= px(0.) || visible_list_rows.start >= visible_list_rows.end {
        return;
    }

    let uncommitted_offset = usize::from(has_uncommitted);
    let first_visible_graph_row = visible_list_rows.start.saturating_sub(uncommitted_offset);
    let last_visible_graph_row_exclusive = visible_list_rows.end.saturating_sub(uncommitted_offset);

    // Commit dots for visible graph rows.
    let visible_node_start = first_visible_graph_row.min(graph.nodes().len());
    let visible_node_end = last_visible_graph_row_exclusive.min(graph.nodes().len());
    for (graph_row, node) in graph.nodes()[visible_node_start..visible_node_end]
        .iter()
        .enumerate()
    {
        let graph_row = visible_node_start + graph_row;
        let list_row = graph_row_to_list_row(graph_row, uncommitted_offset);
        let x = lane_center_x(bounds, node.lane as f32);
        let y = list_row_center_y(list_row, row_height, bounds);
        let color = rgba_to_hsla(colors.graph_lane_color(node.lane));
        draw_commit_circle(x, y, color, node.is_merge, colors, window);
    }

    // Uncommitted changes indicator.
    if has_uncommitted && visible_list_rows.start == 0 {
        let x = lane_center_x(bounds, 0.0);
        let y = list_row_center_y(0, row_height, bounds);
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

    for line_idx in
        graph.visible_line_indices(first_visible_graph_row..last_visible_graph_row_exclusive)
    {
        let Some(line) = graph.line_at(line_idx) else {
            continue;
        };

        if line.full_interval.end < first_visible_graph_row
            || line.full_interval.start >= last_visible_graph_row_exclusive
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
        let from_y =
            list_row_center_y(start_list_row, row_height, bounds) + px(COMMIT_CIRCLE_RADIUS);

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
                    let mut dest_row = list_row_center_y(list_row, row_height, bounds);
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
                    let mut to_row_y = list_row_center_y(list_row, row_height, bounds);

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
    window.paint_quad(
        fill(
            Bounds::new(
                point(center_x - radius, center_y - radius),
                size(radius * 2.0, radius * 2.0),
            ),
            color,
        )
        .corner_radii(radius),
    );

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
    panel_shell(ShellWidth::Full, bg, true, true).border_color(border)
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
    show_graph_col: bool,
    graph_col_width: f32,
    show_sha_col: bool,
    hash_col_width: f32,
    show_author_col: bool,
    author_col_width: f32,
    show_time_col: bool,
    time_col_width: f32,
) -> Div {
    let mut headers = div()
        .px_2()
        .py_1()
        .border_b_1()
        .border_color(border)
        .flex()
        .flex_row()
        .items_center()
        .text_xs()
        .text_color(muted);

    if show_graph_col {
        headers = headers
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
            ));
    }
    if show_sha_col {
        headers = headers
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
            ));
    }
    headers = headers.child(div().flex_1().pl_1().child("DESCRIPTION"));
    if show_author_col {
        headers = headers
            .child(render_resize_handle(
                HistoryColumn::Author,
                entity.clone(),
                border,
            ))
            .child(
                div()
                    .w(px(author_col_width))
                    .flex_shrink_0()
                    .child("AUTHOR"),
            );
    }
    if show_time_col {
        headers = headers.child(render_resize_handle(
            HistoryColumn::Time,
            entity.clone(),
            border,
        ));
        headers = headers.child(
            div()
                .w(px(time_col_width))
                .flex_shrink_0()
                .pr_2()
                .text_align(TextAlign::Right)
                .child("TIME"),
        );
    }
    headers
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

fn render_ref_pills(
    refs: Option<&Arc<[RefInfo]>>,
    cl: &AppColors,
    commit_id: &str,
    detached_head_commit: Option<&str>,
) -> Div {
    let mut row = div()
        .flex()
        .flex_row()
        .items_center()
        .gap_1()
        .overflow_hidden();

    let Some(refs) = refs else {
        // Even with no other refs on this commit, show the detached HEAD pill.
        if detached_head_commit == Some(commit_id) {
            row = row.child(render_detached_head_pill(cl));
        }
        return row;
    };

    // Drop remote symbolic HEAD refs (origin/HEAD, upstream/HEAD, ...) — they
    // just mirror the remote's default branch and add noise.
    let visible: Vec<&RefInfo> = refs
        .iter()
        .filter(|rf| !(rf.kind == RefKind::RemoteBranch && is_remote_head(rf)))
        .collect();

    // Detect remote branch names that are ambiguous across multiple remotes on
    // this commit (e.g. origin/main + upstream/main). These keep their remote
    // prefix in the label so the user can tell them apart; everything else
    // strips the origin/ prefix since the globe icon already signals "remote".
    let mut remotes_by_bare: HashMap<&str, HashSet<&str>> = HashMap::new();
    for rf in visible.iter() {
        if rf.kind == RefKind::RemoteBranch
            && let Some(bare) = bare_remote_name(rf)
        {
            let remote = rf.remote_name.as_deref().unwrap_or("");
            remotes_by_bare.entry(bare).or_default().insert(remote);
        }
    }
    let ambiguous: HashSet<&str> = remotes_by_bare
        .into_iter()
        .filter(|(_, remotes)| remotes.len() > 1)
        .map(|(bare, _)| bare)
        .collect();

    // Detached HEAD injects a "HEAD" pill on the commit HEAD points to. When
    // attached, the underlying branch renders as a normal branch pill (the
    // is_head flag still tints it with ref_head so the "you are here" cue stays).
    let head_injected = detached_head_commit == Some(commit_id);
    if head_injected {
        row = row.child(render_detached_head_pill(cl));
    }

    // The injected HEAD pill consumes one of the visible slots.
    let visible_limit = if head_injected {
        VISIBLE_REF_PILLS.saturating_sub(1)
    } else {
        VISIBLE_REF_PILLS
    };

    for rf in visible.iter().take(visible_limit) {
        let label = ref_pill_label(rf, &ambiguous);
        let icon = ref_pill_icon(rf);
        row = row.child(render_ref_pill(rf, cl, label, icon));
    }

    let hidden_count = visible.len().saturating_sub(visible_limit);
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

fn render_ref_pill(
    rf: &RefInfo,
    cl: &AppColors,
    label: String,
    icon_path: Option<&'static str>,
) -> Div {
    let pill_color = ref_pill_color(rf, cl);
    let text_color = contrast_text_for(pill_color);
    let mut pill = div()
        .px_2()
        .border_1()
        .border_color(rgba_to_hsla(cl.border))
        .rounded(px(3.0))
        .bg(rgba_to_hsla(pill_color))
        .text_xs()
        .text_color(text_color)
        .flex_shrink_0()
        .flex()
        .flex_row()
        .items_center()
        .gap_0p5();
    if let Some(path) = icon_path {
        pill = pill.child(
            svg()
                .flex_none()
                .size(px(11.0))
                .path(path)
                .text_color(text_color),
        );
    }
    pill.child(label)
}

fn render_detached_head_pill(cl: &AppColors) -> Div {
    // Same visual treatment as an attached HEAD (ref_head color + laptop icon),
    // but with the literal "HEAD" label since there is no branch name to show.
    let pill_color = cl.ref_head;
    let text_color = contrast_text_for(pill_color);
    div()
        .px_2()
        .border_1()
        .border_color(rgba_to_hsla(cl.border))
        .rounded(px(3.0))
        .bg(rgba_to_hsla(pill_color))
        .text_xs()
        .text_color(text_color)
        .flex_shrink_0()
        .flex()
        .flex_row()
        .items_center()
        .gap_0p5()
        .child(
            svg()
                .flex_none()
                .size(px(11.0))
                .path("icons/laptop.svg")
                .text_color(text_color),
        )
        .child("HEAD")
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

fn ref_pill_icon(rf: &RefInfo) -> Option<&'static str> {
    // The is_head flag is intentionally ignored here: an attached HEAD on a
    // branch renders with that branch's icon (laptop). Detached HEAD is
    // rendered via render_detached_head_pill, not through this path.
    match rf.kind {
        RefKind::Branch => Some("icons/laptop.svg"),
        RefKind::RemoteBranch => Some("icons/globe.svg"),
        RefKind::Tag => Some("icons/tag.svg"),
        _ => None,
    }
}

fn is_remote_head(rf: &RefInfo) -> bool {
    // Matches origin/HEAD, upstream/HEAD, etc. — symbolic refs that just point
    // at the remote's default branch.
    bare_remote_name(rf) == Some("HEAD")
}

fn bare_remote_name(rf: &RefInfo) -> Option<&str> {
    let remote = rf.remote_name.as_deref()?;
    let prefix = format!("{remote}/");
    rf.name.strip_prefix(&prefix)
}

fn ref_pill_label(rf: &RefInfo, ambiguous: &HashSet<&str>) -> String {
    // The is_head flag is intentionally ignored here: an attached HEAD on a
    // branch renders with that branch's name. Detached HEAD is rendered via
    // render_detached_head_pill, not through this path.
    if rf.kind == RefKind::RemoteBranch
        && let Some(bare) = bare_remote_name(rf)
        && !ambiguous.contains(bare)
    {
        return truncate_chars(bare, 20);
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
