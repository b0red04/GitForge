use gitforge_diff::{DiffLineType, FileDiff};
use gitforge_git::BlameLine;
use gitforge_git::RepoState;
use gitforge_ui::{AppColors, rgba_to_hsla};
use gpui::*;
use std::ops::Range;
use std::rc::Rc;
use std::sync::Arc;

use super::diff_view::{
    DIFF_LINE_HEIGHT, DIFF_LINE_NUM_WIDTH, DiffLineSelection, SharedHighlightState,
    render_diff_empty_state, render_diff_lines, render_highlighted_segments,
};
use super::layout::{FILE_LIST_WIDTH, RIGHT_MIN_WIDTH};

const IMAGE_EXTENSIONS: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "bmp", "ico", "tif", "tiff", "webp", "svg", "avif",
];

const LFS_POINTER_HEADER: &str = "version https://git-lfs";

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct CommitDiffState {
    pub commit_id: String,
    pub file_diffs: Vec<FileDiff>,
    pub selected_file_idx: Option<usize>,
}

pub struct DiffPanel {
    diff_state: Option<CommitDiffState>,
    scroll_handle: UniformListScrollHandle,
    highlight: Arc<SharedHighlightState>,
    view_mode: DiffViewMode,
    code_view_file: Option<String>,
    code_view_content: Option<String>,
    code_scroll_handle: UniformListScrollHandle,
    selection: DiffLineSelection,
    blame: Option<BlameState>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiffViewMode {
    Diff,
    Code,
    Blame,
}

struct BlameState {
    lines: Vec<BlameLine>,
    file_path: String,
}

#[allow(dead_code)]
impl DiffPanel {
    pub fn new() -> Self {
        Self {
            diff_state: None,
            scroll_handle: UniformListScrollHandle::default(),
            highlight: Arc::new(SharedHighlightState::new()),
            view_mode: DiffViewMode::Diff,
            code_view_file: None,
            code_view_content: None,
            code_scroll_handle: UniformListScrollHandle::default(),
            selection: DiffLineSelection::new(),
            blame: None,
        }
    }

    pub fn set_diff(&mut self, state: CommitDiffState) {
        self.highlight.clear_cache();
        self.diff_state = Some(state);
        self.view_mode = DiffViewMode::Diff;
        self.code_view_file = None;
        self.code_view_content = None;
        self.selection.clear();
    }

    pub fn clear(&mut self) {
        self.diff_state = None;
        self.highlight.clear_cache();
        self.view_mode = DiffViewMode::Diff;
        self.code_view_file = None;
        self.code_view_content = None;
    }

    pub fn select_file(&mut self, file_idx: usize) {
        if let Some(ds) = self.diff_state.as_mut() {
            ds.selected_file_idx = Some(file_idx);
        }
        self.selection.clear();
    }

    pub fn selected_file_path(&self) -> Option<String> {
        let diff_state = self.diff_state.as_ref()?;
        let file_diff = diff_state
            .selected_file_idx
            .and_then(|idx| diff_state.file_diffs.get(idx))
            .or_else(|| diff_state.file_diffs.first())?;

        file_diff
            .new_path
            .as_deref()
            .or(file_diff.old_path.as_deref())
            .map(ToOwned::to_owned)
    }

    pub fn set_code_view(&mut self, content: String, path: String) {
        self.view_mode = DiffViewMode::Code;
        self.code_view_content = Some(content);
        self.code_view_file = Some(path);
        self.highlight.clear_cache();
    }

    pub fn set_diff_mode(&mut self) {
        self.view_mode = DiffViewMode::Diff;
        self.code_view_file = None;
        self.code_view_content = None;
        self.highlight.clear_cache();
        self.selection.clear();
    }

    pub fn set_blame(&mut self, lines: Vec<BlameLine>, path: String) {
        self.view_mode = DiffViewMode::Blame;
        self.blame = Some(BlameState {
            lines,
            file_path: path,
        });
        self.highlight.clear_cache();
    }

    pub fn diff_state(&self) -> Option<&CommitDiffState> {
        self.diff_state.as_ref()
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

    pub fn restore_from_snapshot(
        &mut self,
        diff_state: Option<CommitDiffState>,
        view_mode: DiffViewMode,
        code_file: Option<String>,
        code_content: Option<String>,
    ) {
        if let Some(_ds) = &diff_state {
            self.highlight.clear_cache();
        }
        self.diff_state = diff_state;
        self.view_mode = view_mode;
        self.code_view_file = code_file;
        self.code_view_content = code_content;
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

    pub fn render(
        &self,
        repo_state: Option<&RepoState>,
        selected_commit_idx: Option<usize>,
        colors: &AppColors,
        entity: WeakEntity<super::app::GitForgeApp>,
        loading: bool,
    ) -> Div {
        let surface = rgba_to_hsla(colors.surface);
        let border = rgba_to_hsla(colors.border);
        let muted = rgba_to_hsla(colors.text_muted);
        let text_color = rgba_to_hsla(colors.text);

        match (repo_state, &self.diff_state, selected_commit_idx) {
            (Some(repo), Some(diff_state), Some(idx)) => {
                let commit = &repo.commits[idx];
                let commit_detail =
                    render_commit_detail(commit, colors, border, text_color, muted, entity.clone());

                if diff_state.file_diffs.is_empty() {
                    return diff_panel_root(surface).child(commit_detail).child(
                        div().flex_1().flex().items_center().justify_center().child(
                            div()
                                .text_sm()
                                .text_color(muted)
                                .child("No changes in this commit"),
                        ),
                    );
                }

                let selected_file = diff_state.selected_file_idx;
                let file_diffs = diff_state.file_diffs.clone();
                let colors = colors.clone();
                let file_click_entity = entity.clone();

                let mut file_list = div()
                    .w(px(FILE_LIST_WIDTH))
                    .h_full()
                    .border_r_1()
                    .border_color(border)
                    .flex()
                    .flex_col();

                file_list = file_list.child(
                    div()
                        .px_2()
                        .py_1()
                        .border_b_1()
                        .border_color(border)
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .text_color(muted)
                        .child(format!("FILES ({})", file_diffs.len())),
                );

                for (fi, fd) in file_diffs.iter().enumerate() {
                    let path = fd
                        .new_path
                        .as_deref()
                        .or(fd.old_path.as_deref())
                        .unwrap_or("(unknown)");

                    let is_sel = selected_file == Some(fi);
                    let bg = if is_sel {
                        rgba_to_hsla(colors.sidebar_selected)
                    } else {
                        rgba_to_hsla(colors.surface)
                    };
                    let name_color = if is_sel {
                        rgba_to_hsla(colors.text)
                    } else {
                        rgba_to_hsla(colors.text_muted)
                    };

                    let added_count = fd
                        .lines
                        .iter()
                        .filter(|l| l.line_type == DiffLineType::Added)
                        .count();
                    let removed_count = fd
                        .lines
                        .iter()
                        .filter(|l| l.line_type == DiffLineType::Removed)
                        .count();

                    let stats_color_added = rgba_to_hsla(colors.diff_added);
                    let stats_color_removed = rgba_to_hsla(colors.diff_removed);

                    let click_ent = file_click_entity.clone();
                    let path_owned = path.to_string();
                    file_list = file_list.child(
                        div()
                            .id(ElementId::Name(format!("diff-file-{fi}").into()))
                            .px_2()
                            .py_1()
                            .bg(bg)
                            .cursor_pointer()
                            .on_click(move |_ev, _window, cx| {
                                if let Some(e) = click_ent.upgrade() {
                                    e.update(cx, |this, cx| {
                                        this.select_diff_file(fi, cx);
                                    });
                                }
                            })
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_1()
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(name_color)
                                            .overflow_hidden()
                                            .text_ellipsis()
                                            .child(path_owned.clone()),
                                    )
                                    .child(div().flex_1())
                                    .child(if added_count > 0 {
                                        div()
                                            .text_xs()
                                            .text_color(stats_color_added)
                                            .child(format!("+{}", added_count))
                                    } else {
                                        div()
                                    })
                                    .child(if removed_count > 0 {
                                        div()
                                            .text_xs()
                                            .text_color(stats_color_removed)
                                            .child(format!("-{}", removed_count))
                                    } else {
                                        div()
                                    }),
                            ),
                    );
                }

                let diff_content = selected_file
                    .and_then(|idx| file_diffs.get(idx).cloned())
                    .or_else(|| file_diffs.first().cloned());

                let sel_range = self.selected_range();
                let diff_panel = match self.view_mode {
                    DiffViewMode::Diff => render_diff_content(
                        diff_content,
                        &colors,
                        self.scroll_handle.clone(),
                        self.highlight.clone(),
                        entity.clone(),
                        sel_range,
                    ),
                    DiffViewMode::Blame => {
                        if let Some(ref blame_state) = self.blame {
                            render_blame_view(
                                &blame_state.lines,
                                &blame_state.file_path,
                                &colors,
                                entity.clone(),
                            )
                        } else {
                            render_diff_content(
                                diff_content,
                                &colors,
                                self.scroll_handle.clone(),
                                self.highlight.clone(),
                                entity.clone(),
                                sel_range,
                            )
                        }
                    }
                    DiffViewMode::Code => render_code_view(
                        self.code_view_content.as_deref(),
                        self.code_view_file.as_deref(),
                        &colors,
                        self.code_scroll_handle.clone(),
                        self.highlight.clone(),
                        entity.clone(),
                    ),
                };

                diff_panel_root(surface).child(commit_detail).child(
                    div()
                        .flex_1()
                        .flex()
                        .flex_row()
                        .overflow_hidden()
                        .child(file_list)
                        .child(diff_panel),
                )
            }
            _ => diff_panel_root(surface).child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .p_4()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .items_center()
                            .gap_2()
                            .child(div().text_sm().text_color(text_color).child(if loading {
                                "Loading commit…"
                            } else {
                                "Select a commit"
                            }))
                            .child(div().text_xs().text_color(muted).child(if loading {
                                ""
                            } else {
                                "Use ↑ and ↓ to browse the commit history"
                            })),
                    ),
            ),
        }
    }
}

fn diff_panel_root(surface: Hsla) -> Div {
    div()
        .w_full()
        .h_full()
        .min_w(px(RIGHT_MIN_WIDTH))
        .bg(surface)
        .flex()
        .flex_col()
}

fn author_initials(name: &str) -> String {
    name.split_whitespace()
        .filter_map(|w| w.chars().next())
        .take(2)
        .collect::<String>()
        .to_uppercase()
}

fn render_commit_detail(
    commit: &gitforge_git::CommitInfo,
    colors: &AppColors,
    border: Hsla,
    text_color: Hsla,
    muted: Hsla,
    entity: WeakEntity<super::app::GitForgeApp>,
) -> Div {
    let accent = rgba_to_hsla(colors.accent);
    let surface_high = rgba_to_hsla(colors.surface_high);
    let action_border = border;
    let action_text = muted;
    let action_hover_bg = surface_high;

    let parents = match commit.parent_ids.len() {
        0 => String::new(),
        1 => " · 1 parent".into(),
        n => format!(" · {n} parents"),
    };

    let ent_cp = entity.clone();
    let ent_rv = entity.clone();
    let sha_cp = commit.id.clone();
    let sha_rv = commit.id.clone();

    let initials = author_initials(&commit.author_name);
    let body = if commit.message != commit.summary && !commit.message.is_empty() {
        Some(commit.message.clone())
    } else {
        None
    };

    let mut detail = div()
        .flex_shrink_0()
        .p_3()
        .border_b_1()
        .border_color(border)
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .flex()
                .flex_row()
                .gap_3()
                .child(
                    div()
                        .w(px(32.0))
                        .h(px(32.0))
                        .rounded_full()
                        .border_1()
                        .border_color(accent)
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .text_color(accent)
                        .child(initials),
                )
                .child(
                    div()
                        .flex_1()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(text_color)
                                .child(commit.summary.clone()),
                        )
                        .child(div().text_xs().text_color(muted).child(format!(
                            "{} · {}",
                            commit.author_name,
                            commit.author_date.format("%Y-%m-%d %H:%M")
                        )))
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap_2()
                                .child(
                                    div()
                                        .text_xs()
                                        .font_family("monospace")
                                        .text_color(accent)
                                        .child(commit.short_id.clone()),
                                )
                                .child(div().text_xs().text_color(muted).child(parents)),
                        ),
                ),
        );

    if let Some(body_text) = body {
        detail = detail.child(
            div()
                .text_xs()
                .text_color(muted)
                .max_h(px(80.0))
                .overflow_y_hidden()
                .child(body_text),
        );
    }

    detail.child(
        div()
            .flex()
            .gap_1()
            .child(
                div()
                    .id("diff-cherry-pick")
                    .px_1()
                    .py_0()
                    .rounded(px(2.0))
                    .border_1()
                    .border_color(action_border)
                    .cursor_pointer()
                    .text_xs()
                    .text_color(action_text)
                    .hover(|s| s.bg(action_hover_bg))
                    .child("Cherry-pick")
                    .on_click(move |_ev, _window, cx| {
                        if let Some(e) = ent_cp.upgrade() {
                            let sha = sha_cp.clone();
                            e.update(cx, |this, cx| {
                                this.cherry_pick(sha, cx);
                            });
                        }
                    }),
            )
            .child(
                div()
                    .id("diff-revert")
                    .px_1()
                    .py_0()
                    .rounded(px(2.0))
                    .border_1()
                    .border_color(action_border)
                    .cursor_pointer()
                    .text_xs()
                    .text_color(action_text)
                    .hover(|s| s.bg(action_hover_bg))
                    .child("Revert")
                    .on_click(move |_ev, _window, cx| {
                        if let Some(e) = ent_rv.upgrade() {
                            let sha = sha_rv.clone();
                            e.update(cx, |this, cx| {
                                this.revert_commit(sha, cx);
                            });
                        }
                    }),
            ),
    )
}

fn render_diff_content(
    file_diff: Option<FileDiff>,
    colors: &AppColors,
    diff_scroll_handle: UniformListScrollHandle,
    highlight: Arc<SharedHighlightState>,
    entity: WeakEntity<super::app::GitForgeApp>,
    selection_range: Option<Range<usize>>,
) -> Div {
    let border = rgba_to_hsla(colors.border);
    let muted = rgba_to_hsla(colors.text_muted);
    let text_color = rgba_to_hsla(colors.text);
    let surface = rgba_to_hsla(colors.surface);
    let accent = rgba_to_hsla(colors.accent);

    let Some(diff) = file_diff else {
        return render_diff_empty_state(colors);
    };

    let path_label = diff
        .new_path
        .as_deref()
        .or(diff.old_path.as_deref())
        .unwrap_or("(unknown)");

    if diff.is_binary {
        let ext = path_label.rsplit('.').next().unwrap_or("").to_lowercase();
        let is_image = IMAGE_EXTENSIONS.contains(&ext.as_str());

        return div()
            .flex_1()
            .h_full()
            .bg(surface)
            .flex()
            .flex_col()
            .child(
                div()
                    .px_3()
                    .py_2()
                    .border_b_1()
                    .border_color(border)
                    .text_sm()
                    .font_family("monospace")
                    .text_color(muted)
                    .child(path_label.to_string()),
            )
            .child(if is_image {
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
                div().flex_1().flex().items_center().justify_center().child(
                    div()
                        .text_sm()
                        .text_color(muted)
                        .child("Binary file (not displayed)"),
                )
            });
    }

    if !diff.is_binary {
        if let Some(first_line) = diff.lines.first() {
            if first_line.content.starts_with(LFS_POINTER_HEADER) {
                let mut oid = None;
                let mut size = None;
                for line in &diff.lines {
                    if let Some(rest) = line.content.strip_prefix("oid sha256:") {
                        oid = Some(rest.trim().to_string());
                    }
                    if let Some(rest) = line.content.strip_prefix("size ") {
                        size = Some(rest.trim().to_string());
                    }
                }
                return div()
                    .flex_1()
                    .h_full()
                    .bg(surface)
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .px_3()
                            .py_2()
                            .border_b_1()
                            .border_color(border)
                            .text_sm()
                            .font_family("monospace")
                            .text_color(text_color)
                            .child(path_label.to_string()),
                    )
                    .child(
                        div()
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
        }
    }

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
                .child(path_label.to_string()),
        )
        .child(div().flex_1())
        .child({
            let view_path = path_label.to_string();
            let view_ent = entity.clone();
            div()
                .id("view-file-btn")
                .px_2()
                .py_0()
                .rounded(px(3.0))
                .border_1()
                .border_color(border)
                .cursor_pointer()
                .text_xs()
                .text_color(accent)
                .child("View File")
                .on_click(move |_ev, _window, cx| {
                    if let Some(e) = view_ent.upgrade() {
                        e.update(cx, |this, cx| {
                            this.view_file_at_commit(view_path.clone(), cx);
                        });
                    }
                })
        })
        .child({
            let blame_path = path_label.to_string();
            let blame_ent = entity.clone();
            div()
                .id("blame-file-btn")
                .px_2()
                .py_0()
                .rounded(px(3.0))
                .border_1()
                .border_color(border)
                .cursor_pointer()
                .text_xs()
                .text_color(accent)
                .child("Blame")
                .on_click(move |_ev, _window, cx| {
                    if let Some(e) = blame_ent.upgrade() {
                        e.update(cx, |this, cx| {
                            this.view_blame(blame_path.clone(), cx);
                        });
                    }
                })
        });

    let on_click = {
        let ent = entity.clone();
        Rc::new(move |line_i: usize, extend: bool, cx: &mut App| {
            if let Some(e) = ent.upgrade() {
                e.update(cx, |this, cx| {
                    this.select_diff_line(line_i, extend, cx);
                });
            }
        })
    };

    let diff_lines = render_diff_lines(
        &diff.lines,
        path_label,
        colors,
        diff_scroll_handle,
        selection_range,
        Some(highlight),
        "diff-lines",
        "diff-line",
        on_click,
    );

    div()
        .flex_1()
        .h_full()
        .bg(surface)
        .flex()
        .flex_col()
        .child(file_header)
        .child(diff_lines)
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
    let accent = rgba_to_hsla(colors.accent);

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
            div()
                .id("back-to-diff-btn")
                .ml_2()
                .px_2()
                .py_0()
                .rounded(px(3.0))
                .border_1()
                .border_color(border)
                .cursor_pointer()
                .text_xs()
                .text_color(accent)
                .child("Back to Diff")
                .on_click(move |_ev, _window, cx| {
                    if let Some(e) = back_ent.upgrade() {
                        e.update(cx, |this, cx| {
                            this.back_to_diff_mode(cx);
                        });
                    }
                })
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
    let accent = rgba_to_hsla(colors.accent);

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
            div()
                .id("back-to-diff-from-blame")
                .ml_2()
                .px_2()
                .py_0()
                .rounded(px(3.0))
                .border_1()
                .border_color(border)
                .cursor_pointer()
                .text_xs()
                .text_color(accent)
                .child("Back to Diff")
                .on_click(move |_ev, _window, cx| {
                    if let Some(e) = back_ent.upgrade() {
                        e.update(cx, |this, cx| {
                            this.back_to_diff_mode(cx);
                        });
                    }
                })
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
