use gitforge_diff::{DiffLine, DiffLineType};
use gitforge_syntax::highlight::HighlightedLine;
use gitforge_ui::{AppColors, rgba_to_hsla};
use gpui::*;
use std::cell::RefCell;
use std::collections::HashMap;
use std::ops::Range;
use std::rc::Rc;
use std::sync::Arc;

pub const DIFF_LINE_HEIGHT: f32 = 20.0;
pub const DIFF_LINE_NUM_WIDTH: f32 = 50.0;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct HighlightCacheKey {
    path: String,
    line_idx: usize,
}

pub struct SharedHighlightState {
    pub highlighter: gitforge_syntax::SyntaxHighlighter,
    pub cache: RefCell<HashMap<HighlightCacheKey, HighlightedLine>>,
}

impl SharedHighlightState {
    pub fn new() -> Self {
        Self {
            highlighter: gitforge_syntax::SyntaxHighlighter::new(),
            cache: RefCell::new(HashMap::new()),
        }
    }

    pub fn highlight_line(
        &self,
        path: &str,
        line_idx: usize,
        content: &str,
        prefix: &str,
    ) -> HighlightedLine {
        let key = HighlightCacheKey {
            path: format!("{}{}", prefix, path),
            line_idx,
        };

        {
            let cache = self.cache.borrow();
            if let Some(cached) = cache.get(&key) {
                return cached.clone();
            }
        }

        let hl = if let Some(lang) = self.highlighter.language_for_path(path) {
            self.highlighter.highlight_line(content, 0, &lang)
        } else {
            HighlightedLine {
                segments: vec![gitforge_syntax::highlight::HighlightedSegment {
                    text: content.to_string(),
                    scope: gitforge_syntax::theme::HighlightScope::Default,
                }],
            }
        };

        self.cache.borrow_mut().insert(key, hl.clone());
        hl
    }

    pub fn clear_cache(&self) {
        self.cache.borrow_mut().clear();
    }
}

pub struct DiffLineSelection {
    anchor: Option<usize>,
    end: Option<usize>,
}

impl DiffLineSelection {
    pub fn new() -> Self {
        Self {
            anchor: None,
            end: None,
        }
    }

    pub fn select(&mut self, line_idx: usize, extend: bool) {
        if extend {
            if self.anchor.is_some() {
                self.end = Some(line_idx);
            } else {
                self.anchor = Some(line_idx);
                self.end = Some(line_idx);
            }
        } else {
            self.anchor = Some(line_idx);
            self.end = Some(line_idx);
        }
    }

    pub fn clear(&mut self) {
        self.anchor = None;
        self.end = None;
    }

    pub fn range(&self) -> Option<Range<usize>> {
        match (self.anchor, self.end) {
            (Some(a), Some(b)) => {
                let start = a.min(b);
                let end = a.max(b) + 1;
                Some(start..end)
            }
            _ => None,
        }
    }

    pub fn indices(&self) -> Vec<usize> {
        self.range().map(|r| r.collect()).unwrap_or_default()
    }
}

pub fn render_highlighted_segments(
    highlighted: &HighlightedLine,
    colors: &AppColors,
    default_fg: Hsla,
) -> Div {
    let mut container = div().flex().flex_row();
    for seg in &highlighted.segments {
        if seg.text.is_empty() {
            continue;
        }
        let color = if seg.scope == gitforge_syntax::theme::HighlightScope::Default {
            default_fg
        } else {
            rgba_to_hsla(colors.scope_color(&seg.scope))
        };
        container = container.child(
            div()
                .text_xs()
                .font_family("monospace")
                .text_color(color)
                .child(seg.text.clone()),
        );
    }
    container
}

pub fn render_diff_empty_state(colors: &AppColors) -> Div {
    let surface = rgba_to_hsla(colors.surface);
    let muted = rgba_to_hsla(colors.text_muted);
    div()
        .flex_1()
        .h_full()
        .flex()
        .items_center()
        .justify_center()
        .bg(surface)
        .child(div().text_sm().text_color(muted).child("Select a file to view diff"))
}

pub fn render_diff_lines(
    lines: &[DiffLine],
    file_path: &str,
    colors: &AppColors,
    scroll_handle: UniformListScrollHandle,
    selection: Option<Range<usize>>,
    highlight: Option<Arc<SharedHighlightState>>,
    list_id: &'static str,
    line_id_prefix: &'static str,
    on_click: Rc<dyn Fn(usize, bool, &mut App)>,
) -> Div {
    let border = rgba_to_hsla(colors.border);
    let muted = rgba_to_hsla(colors.text_muted);
    let text_color = rgba_to_hsla(colors.text);
    let surface = rgba_to_hsla(colors.surface);
    let selection_bg = rgba_to_hsla(colors.selection_bg);

    let added_bg = rgba_to_hsla(colors.diff_added_bg);
    let removed_bg = rgba_to_hsla(colors.diff_removed_bg);
    let added_fg = rgba_to_hsla(colors.diff_added);
    let removed_fg = rgba_to_hsla(colors.diff_removed);
    let hunk_header_bg = rgba_to_hsla(colors.diff_hunk_header);

    let total_lines = lines.len();
    let lines_data = lines.to_vec();
    let cl = colors.clone();
    let path_for_hl = file_path.to_string();

    let diff_lines = uniform_list(
        list_id,
        total_lines,
        move |visible_range: Range<usize>, _window: &mut Window, _cx: &mut App| {
            let mut row_elements = Vec::with_capacity(visible_range.len());

            for line_i in visible_range {
                let Some(line) = lines_data.get(line_i) else {
                    continue;
                };

                let is_conflict_marker = line.content.starts_with("<<<<<<< ")
                    || line.content.starts_with("=======\n")
                    || line.content.starts_with("=======\r")
                    || line.content == "======="
                    || line.content.starts_with(">>>>>>> ");

                let (base_bg, line_num_bg, prefix) = if is_conflict_marker {
                    (rgba_to_hsla(cl.warning).alpha(0.15), surface, "\u{26a0}")
                } else {
                    match line.line_type {
                        DiffLineType::Added => (added_bg, added_bg, "+"),
                        DiffLineType::Removed => (removed_bg, removed_bg, "-"),
                        DiffLineType::HunkHeader => (hunk_header_bg, surface, " "),
                        DiffLineType::Context => (surface, surface, " "),
                        DiffLineType::NoNewlineAtEof => (surface, surface, "\\"),
                    }
                };

                let is_selected = selection.as_ref().map_or(false, |r| r.contains(&line_i));
                let line_bg = if is_selected { selection_bg } else { base_bg };

                let line_fg = if is_conflict_marker {
                    rgba_to_hsla(cl.warning)
                } else {
                    match line.line_type {
                        DiffLineType::Added => added_fg,
                        DiffLineType::Removed => removed_fg,
                        DiffLineType::HunkHeader => muted,
                        _ => text_color,
                    }
                };

                let old_num = line
                    .old_line
                    .map(|n| format!("{:>4}", n))
                    .unwrap_or_else(|| "    ".to_string());
                let new_num = line
                    .new_line
                    .map(|n| format!("{:>4}", n))
                    .unwrap_or_else(|| "    ".to_string());

                let display_content: String = line.content.chars().take(200).collect();

                let use_syntax = matches!(
                    line.line_type,
                    DiffLineType::Context | DiffLineType::Added | DiffLineType::Removed
                );

                let content_col = if use_syntax {
                    if let Some(ref hl) = highlight {
                        let highlighted =
                            hl.highlight_line(&path_for_hl, line_i, &display_content, "");
                        render_highlighted_segments(&highlighted, &cl, line_fg)
                            .flex_1()
                            .pr_3()
                            .overflow_hidden()
                    } else {
                        div()
                            .flex_1()
                            .text_xs()
                            .font_family("monospace")
                            .text_color(line_fg)
                            .pr_3()
                            .overflow_hidden()
                            .child(display_content)
                    }
                } else {
                    div()
                        .flex_1()
                        .text_xs()
                        .font_family("monospace")
                        .text_color(line_fg)
                        .pr_3()
                        .overflow_hidden()
                        .child(display_content)
                };

                let cb = on_click.clone();
                let row = div()
                    .id(ElementId::Name(format!("{}-{line_i}", line_id_prefix).into()))
                    .h(px(DIFF_LINE_HEIGHT))
                    .flex()
                    .flex_row()
                    .items_center()
                    .bg(line_bg)
                    .cursor_pointer()
                    .on_mouse_down(MouseButton::Left, move |ev: &MouseDownEvent, _window, cx| {
                        let extend = ev.modifiers.shift;
                        cb(line_i, extend, cx);
                    })
                    .child(
                        div()
                            .w(px(DIFF_LINE_NUM_WIDTH))
                            .h_full()
                            .flex()
                            .flex_row()
                            .items_center()
                            .bg(line_num_bg)
                            .border_r_1()
                            .border_color(border)
                            .child(
                                div()
                                    .w(px(DIFF_LINE_NUM_WIDTH / 2.0))
                                    .text_xs()
                                    .font_family("monospace")
                                    .text_color(muted)
                                    .pl_2()
                                    .child(old_num),
                            )
                            .child(
                                div()
                                    .w(px(DIFF_LINE_NUM_WIDTH / 2.0))
                                    .text_xs()
                                    .font_family("monospace")
                                    .text_color(muted)
                                    .pl_1()
                                    .child(new_num),
                            ),
                    )
                    .child(
                        div()
                            .w(px(14.0))
                            .h_full()
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_xs()
                            .font_family("monospace")
                            .text_color(line_fg)
                            .child(prefix.to_string()),
                    )
                    .child(content_col);

                row_elements.push(row.into_any_element());
            }

            row_elements
        },
    )
    .track_scroll(scroll_handle);

    div().flex_1().child(diff_lines)
}
