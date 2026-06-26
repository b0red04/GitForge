use gitforge_diff::FileDiff;
use gitforge_git::BlameLine;
use gitforge_ui::{
    AppColors, ButtonKind, ButtonSize, WidgetColors, action_button, empty_state, entity_on_click,
    primary_button, rgba_to_hsla,
};
use gpui::*;
use std::ops::Range;
use std::rc::Rc;
use std::sync::Arc;

use super::diff_view::{
    DIFF_LINE_HEIGHT, DIFF_LINE_NUM_WIDTH, DiffLineSelection, SharedHighlightState,
    render_diff_empty_state, render_diff_lines, render_highlighted_segments,
};

const IMAGE_EXTENSIONS: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "bmp", "ico", "tif", "tiff", "webp", "svg", "avif",
];

const LFS_POINTER_HEADER: &str = "version https://git-lfs";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiffViewMode {
    Diff,
    Code,
    Blame,
}

impl DiffViewMode {
    /// Cheap discriminator used by [`DiffViewKey`](super::diff_panel::DiffViewKey)
    /// so the fingerprint derives `PartialEq` without importing the enum.
    pub fn tag(&self) -> u8 {
        match self {
            DiffViewMode::Diff => 0,
            DiffViewMode::Code => 1,
            DiffViewMode::Blame => 2,
        }
    }
}

struct BlameState {
    lines: Vec<BlameLine>,
    file_path: String,
}

/// Blame data captured for a render snapshot.
#[derive(Clone)]
pub struct DiffBlameSnapshot {
    pub lines: Vec<BlameLine>,
    pub file_path: String,
}

pub fn file_diff_path_label(diff: &FileDiff) -> &str {
    diff.new_path
        .as_deref()
        .or(diff.old_path.as_deref())
        .unwrap_or("(unknown)")
}

pub fn file_diff_path_or_empty(diff: &FileDiff) -> &str {
    diff.new_path
        .as_deref()
        .or(diff.old_path.as_deref())
        .unwrap_or("")
}

pub fn is_lfs_pointer(diff: &FileDiff) -> bool {
    if diff.is_binary {
        return false;
    }
    diff.lines
        .first()
        .is_some_and(|line| line.content.starts_with(LFS_POINTER_HEADER))
}

pub enum DiffViewerHeader {
    CommitHistory {
        entity: WeakEntity<super::app::GitForgeApp>,
    },
    WorkingTree {
        section_label: &'static str,
        is_staged: bool,
        has_line_selection: bool,
        entity: WeakEntity<super::app::GitForgeApp>,
    },
}

pub struct DiffViewer {
    view_mode: DiffViewMode,
    selection: DiffLineSelection,
    scroll_handle: UniformListScrollHandle,
    code_scroll_handle: UniformListScrollHandle,
    highlight: Arc<SharedHighlightState>,
    current_diff: Option<FileDiff>,
    code_view_file: Option<String>,
    code_view_content: Option<String>,
    blame: Option<BlameState>,
}

/// Render-time state copied from [`DiffViewer`] or a cached [`DiffSnapshot`].
pub struct DiffViewerRenderCtx {
    pub view_mode: DiffViewMode,
    pub scroll_handle: UniformListScrollHandle,
    pub code_scroll_handle: UniformListScrollHandle,
    pub highlight: Arc<SharedHighlightState>,
    pub code_view_file: Option<String>,
    pub code_view_content: Option<String>,
    pub blame: Option<DiffBlameSnapshot>,
    pub selection: Option<Range<usize>>,
}

impl DiffViewer {
    pub fn new() -> Self {
        Self {
            view_mode: DiffViewMode::Diff,
            selection: DiffLineSelection::new(),
            scroll_handle: UniformListScrollHandle::default(),
            code_scroll_handle: UniformListScrollHandle::default(),
            highlight: Arc::new(SharedHighlightState::new()),
            current_diff: None,
            code_view_file: None,
            code_view_content: None,
            blame: None,
        }
    }

    pub fn set_diff(&mut self, diff: FileDiff) {
        self.highlight.clear_cache();
        self.current_diff = Some(diff);
        self.view_mode = DiffViewMode::Diff;
        self.code_view_file = None;
        self.code_view_content = None;
        self.blame = None;
        self.selection.clear();
        self.reset_scroll_positions();
    }

    pub fn clear_diff(&mut self) {
        self.current_diff = None;
        self.highlight.clear_cache();
        self.view_mode = DiffViewMode::Diff;
        self.code_view_file = None;
        self.code_view_content = None;
        self.blame = None;
        self.selection.clear();
        self.reset_scroll_positions();
    }

    pub fn clear_highlight_cache(&mut self) {
        self.highlight.clear_cache();
    }

    pub fn set_code_view(&mut self, content: String, path: String) {
        self.view_mode = DiffViewMode::Code;
        self.code_view_content = Some(content);
        self.code_view_file = Some(path);
        self.highlight.clear_cache();
        self.reset_scroll_positions();
    }

    pub fn set_diff_mode(&mut self) {
        self.view_mode = DiffViewMode::Diff;
        self.code_view_file = None;
        self.code_view_content = None;
        self.blame = None;
        self.highlight.clear_cache();
        self.selection.clear();
        self.reset_scroll_positions();
    }

    pub fn set_blame(&mut self, lines: Vec<BlameLine>, path: String) {
        self.view_mode = DiffViewMode::Blame;
        self.blame = Some(BlameState {
            lines,
            file_path: path,
        });
        self.highlight.clear_cache();
        self.reset_scroll_positions();
    }

    pub fn view_mode(&self) -> DiffViewMode {
        self.view_mode.clone()
    }

    pub fn code_view_file(&self) -> Option<&str> {
        self.code_view_file.as_deref()
    }

    pub fn code_view_content(&self) -> Option<&str> {
        self.code_view_content.as_deref()
    }

    pub fn current_diff(&self) -> Option<&FileDiff> {
        self.current_diff.as_ref()
    }

    pub fn select_line(&mut self, line_idx: usize, extend: bool) {
        self.selection.select(line_idx, extend);
    }

    pub fn clear_selection(&mut self) {
        self.selection.clear();
    }

    pub fn selected_range(&self) -> Option<Range<usize>> {
        self.selection.range()
    }

    pub fn selected_indices(&self) -> Vec<usize> {
        self.selection.indices()
    }

    pub fn render_ctx(&self) -> DiffViewerRenderCtx {
        DiffViewerRenderCtx {
            view_mode: self.view_mode.clone(),
            scroll_handle: self.scroll_handle.clone(),
            code_scroll_handle: self.code_scroll_handle.clone(),
            highlight: self.highlight.clone(),
            code_view_file: self.code_view_file.clone(),
            code_view_content: self.code_view_content.clone(),
            blame: self.blame_snapshot(),
            selection: self.selected_range(),
        }
    }

    pub fn view_mode_tag(&self) -> u8 {
        self.view_mode.tag()
    }

    pub fn blame_file_for_key(&self) -> Option<String> {
        if self.view_mode == DiffViewMode::Blame {
            self.blame.as_ref().map(|b| b.file_path.clone())
        } else {
            None
        }
    }

    pub fn blame_snapshot(&self) -> Option<DiffBlameSnapshot> {
        if self.view_mode == DiffViewMode::Blame {
            self.blame.as_ref().map(|b| DiffBlameSnapshot {
                lines: b.lines.clone(),
                file_path: b.file_path.clone(),
            })
        } else {
            None
        }
    }

    pub fn restore(
        &mut self,
        view_mode: DiffViewMode,
        code_file: Option<String>,
        code_content: Option<String>,
    ) {
        self.highlight.clear_cache();
        self.view_mode = view_mode;
        self.code_view_file = code_file;
        self.code_view_content = code_content;
        self.selection.clear();
        self.reset_scroll_positions();
    }

    fn reset_scroll_positions(&mut self) {
        self.scroll_handle.scroll_to_item(0, ScrollStrategy::Top);
        self.code_scroll_handle
            .scroll_to_item(0, ScrollStrategy::Top);
    }
}

pub fn render_binary_or_lfs(diff: &FileDiff, path_label: &str, colors: &AppColors) -> Option<Div> {
    let muted = rgba_to_hsla(colors.text_muted);
    let accent = rgba_to_hsla(colors.accent);
    let wc = WidgetColors::from_app(colors);

    if diff.is_binary {
        let ext = path_label.rsplit('.').next().unwrap_or("").to_lowercase();
        let is_image = IMAGE_EXTENSIONS.contains(&ext.as_str());

        return Some(if is_image {
            div()
                .flex_1()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap_2()
                .child(
                    div()
                        .text_sm()
                        .text_color(muted)
                        .child(format!("Image file ({})", ext.to_uppercase())),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(muted)
                        .child("Image preview not available in this version"),
                )
        } else {
            empty_state("Binary file (not displayed)", wc)
        });
    }

    if is_lfs_pointer(diff) {
        let mut oid = None;
        let mut size = None;
        for line in diff.lines.iter() {
            if let Some(rest) = line.content.strip_prefix("oid sha256:") {
                oid = Some(rest.trim().to_string());
            }
            if let Some(rest) = line.content.strip_prefix("size ") {
                size = Some(rest.trim().to_string());
            }
        }
        return Some(
            div()
                .flex_1()
                .p_4()
                .flex()
                .flex_col()
                .gap_2()
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::BOLD)
                        .text_color(accent)
                        .child("Git LFS Pointer"),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(muted)
                        .child(format!("Object: {}", oid.unwrap_or_default())),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(muted)
                        .child(format!("Size: {} bytes", size.unwrap_or_default())),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(muted)
                        .child("File content is stored in Git LFS"),
                ),
        );
    }

    None
}

fn render_diff_file_header(path_label: &str, colors: &AppColors) -> Div {
    let border = rgba_to_hsla(colors.border);
    let text_color = rgba_to_hsla(colors.text);

    div()
        .px_3()
        .py_2()
        .border_b_1()
        .border_color(border)
        .flex()
        .items_center()
        .child(
            div()
                .text_sm()
                .font_family("monospace")
                .text_color(text_color)
                .child(path_label.to_string()),
        )
        .child(div().flex_1())
}

fn append_working_tree_header_actions(
    mut header: Div,
    colors: &AppColors,
    section_label: &'static str,
    is_staged: bool,
    has_line_selection: bool,
    entity: WeakEntity<super::app::GitForgeApp>,
) -> Div {
    let muted = rgba_to_hsla(colors.text_muted);

    if has_line_selection {
        let label = if is_staged {
            "Unstage Lines"
        } else {
            "Stage Lines"
        };
        let lines_ent = entity;
        let stage_label = label.to_string();
        header = header.child(primary_button(
            "stage-lines-btn",
            stage_label,
            ButtonSize::Small,
            false,
            entity_on_click(lines_ent, move |this, cx| {
                if is_staged {
                    this.unstage_selected_lines(cx);
                } else {
                    this.stage_selected_lines(cx);
                }
            }),
            WidgetColors::from_app(colors),
        ));
    }

    header.child(div().text_xs().text_color(muted).child(section_label))
}

pub fn render_diff_viewer(
    ctx: &DiffViewerRenderCtx,
    diff: Option<&FileDiff>,
    colors: &AppColors,
    header: DiffViewerHeader,
    entity: WeakEntity<super::app::GitForgeApp>,
    on_select_line: Rc<dyn Fn(usize, bool, &mut App)>,
    list_id: &'static str,
    line_id_prefix: &'static str,
) -> Div {
    match ctx.view_mode {
        DiffViewMode::Code => render_code_view(
            ctx.code_view_content.as_deref(),
            ctx.code_view_file.as_deref(),
            colors,
            ctx.code_scroll_handle.clone(),
            ctx.highlight.clone(),
            entity,
        ),
        DiffViewMode::Blame => {
            if let Some(ref blame) = ctx.blame {
                render_blame_view(&blame.lines, &blame.file_path, colors, entity)
            } else if let Some(diff) = diff {
                render_diff_mode(
                    diff,
                    ctx,
                    colors,
                    header,
                    on_select_line,
                    list_id,
                    line_id_prefix,
                )
            } else {
                render_diff_empty_state(colors)
            }
        }
        DiffViewMode::Diff => {
            let Some(diff) = diff else {
                return render_diff_empty_state(colors);
            };
            render_diff_mode(
                diff,
                ctx,
                colors,
                header,
                on_select_line,
                list_id,
                line_id_prefix,
            )
        }
    }
}

fn render_diff_mode(
    diff: &FileDiff,
    ctx: &DiffViewerRenderCtx,
    colors: &AppColors,
    header: DiffViewerHeader,
    on_select_line: Rc<dyn Fn(usize, bool, &mut App)>,
    list_id: &'static str,
    line_id_prefix: &'static str,
) -> Div {
    let surface = rgba_to_hsla(colors.surface);
    let path_label = file_diff_path_label(diff);

    let file_header = match header {
        DiffViewerHeader::CommitHistory { .. } => render_diff_file_header(path_label, colors),
        DiffViewerHeader::WorkingTree {
            section_label,
            is_staged,
            has_line_selection,
            entity,
        } => append_working_tree_header_actions(
            render_diff_file_header(path_label, colors),
            colors,
            section_label,
            is_staged,
            has_line_selection,
            entity,
        ),
    }
    .flex_shrink_0();

    let body = if let Some(special) = render_binary_or_lfs(diff, path_label, colors) {
        special
    } else {
        render_diff_lines(
            diff.lines.clone(),
            path_label,
            colors,
            ctx.scroll_handle.clone(),
            ctx.selection.clone(),
            Some(ctx.highlight.clone()),
            list_id,
            line_id_prefix,
            on_select_line,
        )
    };

    div()
        .flex_1()
        .min_h(px(0.0))
        .h_full()
        .bg(surface)
        .flex()
        .flex_col()
        .overflow_hidden()
        .child(file_header)
        .child(
            div()
                .flex_1()
                .min_h(px(0.0))
                .flex()
                .flex_col()
                .overflow_hidden()
                .child(body),
        )
}

fn render_code_view(
    content: Option<&str>,
    path: Option<&str>,
    colors: &AppColors,
    scroll_handle: UniformListScrollHandle,
    highlight: Arc<SharedHighlightState>,
    entity: WeakEntity<super::app::GitForgeApp>,
) -> Div {
    let border = rgba_to_hsla(colors.border);
    let muted = rgba_to_hsla(colors.text_muted);
    let text_color = rgba_to_hsla(colors.text);
    let surface = rgba_to_hsla(colors.surface);
    let wc = WidgetColors::from_app(colors);

    let Some(content_owned) = content.map(String::from) else {
        return div()
            .flex_1()
            .h_full()
            .bg(surface)
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .text_sm()
                    .text_color(muted)
                    .child("No file content available"),
            );
    };

    let path_str = path.unwrap_or("(unknown)");
    let lines: Vec<String> = content_owned.lines().map(String::from).collect();
    let total_lines = lines.len();
    let cl = colors.clone();
    let path_owned = path_str.to_string();

    let file_header = div()
        .px_3()
        .py_2()
        .border_b_1()
        .border_color(border)
        .flex()
        .items_center()
        .child(
            div()
                .text_sm()
                .font_family("monospace")
                .text_color(text_color)
                .child(path_str.to_string()),
        )
        .child(div().flex_1())
        .child(
            div()
                .text_xs()
                .text_color(muted)
                .child(format!("{} lines", total_lines)),
        )
        .child({
            let back_ent = entity.clone();
            action_button(
                "back-to-diff-btn",
                "Back to Diff",
                ButtonKind::Accent,
                ButtonSize::Small,
                false,
                entity_on_click(back_ent, |this, cx| this.back_to_diff_mode(cx)),
                wc,
            )
            .ml_2()
        });

    let code_lines = uniform_list(
        "code-lines",
        total_lines,
        move |visible_range: Range<usize>, _window: &mut Window, _cx: &mut App| {
            let mut row_elements = Vec::with_capacity(visible_range.len());

            for line_i in visible_range {
                let Some(line_content) = lines.get(line_i) else {
                    continue;
                };
                let display: String = line_content.chars().take(200).collect();

                let line_num = format!("{:>4}", line_i + 1);

                let highlighted =
                    highlight.highlight_line(&path_owned, line_i, &display, "__code__");
                let content_col = render_highlighted_segments(&highlighted, &cl, text_color)
                    .flex_1()
                    .pr_3()
                    .overflow_hidden();

                let row = div()
                    .id(ElementId::Name(format!("code-line-{line_i}").into()))
                    .h(px(DIFF_LINE_HEIGHT))
                    .flex()
                    .flex_row()
                    .items_center()
                    .bg(surface)
                    .child(
                        div()
                            .w(px(DIFF_LINE_NUM_WIDTH))
                            .h_full()
                            .flex()
                            .items_center()
                            .bg(surface)
                            .border_r_1()
                            .border_color(border)
                            .child(
                                div()
                                    .w_full()
                                    .text_xs()
                                    .font_family("monospace")
                                    .text_color(muted)
                                    .pl_2()
                                    .child(line_num),
                            ),
                    )
                    .child(content_col);

                row_elements.push(row.into_any_element());
            }

            row_elements
        },
    )
    .track_scroll(scroll_handle);

    div()
        .flex_1()
        .h_full()
        .bg(surface)
        .flex()
        .flex_col()
        .child(file_header)
        .child(div().flex_1().child(code_lines))
}

fn render_blame_view(
    blame_lines: &[BlameLine],
    file_path: &str,
    colors: &AppColors,
    entity: WeakEntity<super::app::GitForgeApp>,
) -> Div {
    let border = rgba_to_hsla(colors.border);
    let muted = rgba_to_hsla(colors.text_muted);
    let text_color = rgba_to_hsla(colors.text);
    let surface = rgba_to_hsla(colors.surface);
    let wc = WidgetColors::from_app(colors);

    let total_lines = blame_lines.len();
    let lines_data = blame_lines.to_vec();
    let cl = colors.clone();

    let back_ent = entity.clone();
    let file_header = div()
        .px_3()
        .py_2()
        .border_b_1()
        .border_color(border)
        .flex()
        .items_center()
        .child(
            div()
                .text_sm()
                .font_family("monospace")
                .text_color(text_color)
                .child(file_path.to_string()),
        )
        .child(div().flex_1())
        .child(div().text_xs().text_color(muted).child("BLAME"))
        .child({
            action_button(
                "back-to-diff-from-blame",
                "Back to Diff",
                ButtonKind::Accent,
                ButtonSize::Small,
                false,
                entity_on_click(back_ent, |this, cx| this.back_to_diff_mode(cx)),
                wc,
            )
            .ml_2()
        });

    let blame_rows = uniform_list(
        "blame-lines",
        total_lines,
        move |visible_range: Range<usize>, _window: &mut Window, _cx: &mut App| {
            let mut row_elements = Vec::with_capacity(visible_range.len());

            for line_i in visible_range {
                let Some(blame) = lines_data.get(line_i) else {
                    continue;
                };

                let short_id = blame.short_id.clone();
                let author: String = blame.author.chars().take(12).collect();
                let line_num = format!("{:>4}", blame.line_number);
                let display: String = blame.content.chars().take(200).collect();

                let id_color = if blame.is_boundary {
                    rgba_to_hsla(cl.text_muted)
                } else {
                    rgba_to_hsla(cl.accent)
                };

                let row = div()
                    .id(ElementId::Name(format!("blame-line-{line_i}").into()))
                    .h(px(DIFF_LINE_HEIGHT))
                    .flex()
                    .flex_row()
                    .items_center()
                    .bg(surface)
                    .child(
                        div()
                            .w(px(DIFF_LINE_NUM_WIDTH))
                            .h_full()
                            .flex()
                            .items_center()
                            .bg(surface)
                            .border_r_1()
                            .border_color(border)
                            .child(
                                div()
                                    .w_full()
                                    .text_xs()
                                    .font_family("monospace")
                                    .text_color(muted)
                                    .pl_2()
                                    .child(line_num),
                            ),
                    )
                    .child(
                        div()
                            .w(px(56.0))
                            .text_xs()
                            .font_family("monospace")
                            .text_color(id_color)
                            .overflow_hidden()
                            .pl_2()
                            .child(short_id),
                    )
                    .child(
                        div()
                            .w(px(80.0))
                            .text_xs()
                            .text_color(rgba_to_hsla(cl.text_muted))
                            .overflow_hidden()
                            .pl_1()
                            .child(author),
                    )
                    .child(
                        div()
                            .flex_1()
                            .text_xs()
                            .font_family("monospace")
                            .text_color(text_color)
                            .pr_3()
                            .overflow_hidden()
                            .child(display),
                    );

                row_elements.push(row.into_any_element());
            }

            row_elements
        },
    );

    div()
        .flex_1()
        .h_full()
        .bg(surface)
        .flex()
        .flex_col()
        .child(file_header)
        .child(div().flex_1().child(blame_rows))
}

#[cfg(test)]
mod tests {
    use super::DiffViewMode;

    #[test]
    fn view_mode_tag_is_stable_and_distinct() {
        // The tag is used as a PartialEq discriminator inside DiffViewKey.
        // If these numbers change, cached diffs will wrongly invalidate (or
        // fail to invalidate) across versions — so they are part of the
        // cache contract.
        assert_eq!(DiffViewMode::Diff.tag(), 0);
        assert_eq!(DiffViewMode::Code.tag(), 1);
        assert_eq!(DiffViewMode::Blame.tag(), 2);
        assert_ne!(DiffViewMode::Diff.tag(), DiffViewMode::Code.tag());
        assert_ne!(DiffViewMode::Code.tag(), DiffViewMode::Blame.tag());
    }
}
