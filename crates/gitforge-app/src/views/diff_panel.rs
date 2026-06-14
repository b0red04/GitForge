use gitforge_diff::{DiffLineType, FileDiff};
use gitforge_git::BlameLine;
use gitforge_git::CommitInfo;
use gitforge_ui::{AppColors, rgba_to_hsla};
use gpui::*;
use std::ops::Range;
use std::rc::Rc;
use std::sync::Arc;

use super::diff_view::SharedHighlightState;
use super::diff_view::render_diff_empty_state;
use super::diff_viewer::{
    DiffViewer, DiffViewerHeader, DiffViewerRenderCtx, file_diff_path_label, render_diff_viewer,
};
use super::layout::{FILE_LIST_WIDTH, RIGHT_MIN_WIDTH};

pub use super::diff_viewer::{DiffBlameSnapshot, DiffViewMode};

/// Per-file added/removed line counts, computed once when a diff is loaded so
/// they never have to be recounted during a render.
#[derive(Debug, Clone, Copy, Default)]
pub struct FileLineStats {
    pub added: usize,
    pub removed: usize,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct CommitDiffState {
    pub commit_id: String,
    /// Stored behind an `Arc` so that cloning the diff state per render is a
    /// cheap reference-count bump instead of deep-cloning every file's lines.
    pub file_diffs: Arc<[FileDiff]>,
    pub file_stats: Arc<[FileLineStats]>,
    pub selected_file_idx: Option<usize>,
}

impl CommitDiffState {
    pub fn new(
        commit_id: String,
        file_diffs: Vec<FileDiff>,
        selected_file_idx: Option<usize>,
    ) -> Self {
        let file_stats: Arc<[FileLineStats]> = file_diffs
            .iter()
            .map(|fd| {
                let mut stats = FileLineStats::default();
                for line in fd.lines.iter() {
                    match line.line_type {
                        DiffLineType::Added => stats.added += 1,
                        DiffLineType::Removed => stats.removed += 1,
                        _ => {}
                    }
                }
                stats
            })
            .collect();

        Self {
            commit_id,
            file_diffs: file_diffs.into(),
            file_stats,
            selected_file_idx,
        }
    }
}

pub struct DiffPanel {
    diff_state: Option<CommitDiffState>,
    viewer: DiffViewer,
}

/// An immutable, render-ready copy of everything `DiffPanel` needs to draw.
///
/// `DiffPanel` remains the single source of truth in `RepoSession`; this
/// snapshot is rebuilt from it only when the diff's observable state changes,
/// then handed to [`DiffViewMirror`] (a cached GPUI view). Because scrolling
/// the commit history does not change the snapshot, the mirror's painted output
/// is recycled by GPUI instead of being rebuilt every scroll frame.
#[derive(Clone)]
pub struct DiffSnapshot {
    pub colors: AppColors,
    pub loading: bool,
    pub selected_commit: Option<CommitInfo>,
    pub diff_state: Option<CommitDiffState>,
    pub view_mode: DiffViewMode,
    pub code_view_file: Option<String>,
    pub code_view_content: Option<String>,
    pub blame: Option<DiffBlameSnapshot>,
    pub selection: Option<Range<usize>>,
    pub highlight: Arc<SharedHighlightState>,
    pub scroll_handle: UniformListScrollHandle,
    pub code_scroll_handle: UniformListScrollHandle,
    pub app: WeakEntity<super::app::GitForgeApp>,
}

/// A cheap, comparable fingerprint of the diff panel's visible state. When this
/// is unchanged between frames, the mirror view does not need re-rendering and
/// GPUI's view caching recycles the previous paint.
#[derive(Clone, PartialEq, Eq, Default)]
pub struct DiffViewKey {
    pub theme: String,
    pub loading: bool,
    pub selected_commit_id: Option<String>,
    pub diff_commit_id: Option<String>,
    pub selected_file_idx: Option<usize>,
    pub view_mode_tag: u8,
    pub code_view_file: Option<String>,
    pub blame_file: Option<String>,
    pub selection: Option<Range<usize>>,
}

/// A render-only GPUI view that mirrors the diff panel. It is embedded with
/// `.cached(...)` so that, unless its snapshot changes, GPUI recycles its
/// layout and paint instead of recomputing it on unrelated re-renders (such as
/// scrolling the commit history).
pub struct DiffViewMirror {
    snapshot: Option<DiffSnapshot>,
    key: DiffViewKey,
}

impl DiffViewMirror {
    pub fn new() -> Self {
        Self {
            snapshot: None,
            key: DiffViewKey::default(),
        }
    }

    pub fn key(&self) -> &DiffViewKey {
        &self.key
    }

    pub fn update_snapshot(&mut self, key: DiffViewKey, snapshot: DiffSnapshot) {
        self.key = key;
        self.snapshot = Some(snapshot);
    }
}

impl Render for DiffViewMirror {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        match &self.snapshot {
            Some(snap) => render_diff_panel(snap),
            None => div().w_full().h_full(),
        }
    }
}

#[allow(dead_code)]
impl DiffPanel {
    pub fn new() -> Self {
        Self {
            diff_state: None,
            viewer: DiffViewer::new(),
        }
    }

    pub fn set_diff(&mut self, state: CommitDiffState) {
        self.viewer.clear_highlight_cache();
        self.diff_state = Some(state);
        self.viewer.set_diff_mode();
        self.viewer.clear_selection();
    }

    pub fn clear(&mut self) {
        self.diff_state = None;
        self.viewer.clear_diff();
    }

    pub fn select_file(&mut self, file_idx: usize) {
        if let Some(ds) = self.diff_state.as_mut() {
            ds.selected_file_idx = Some(file_idx);
        }
        self.viewer.set_diff_mode();
    }

    pub fn selected_file_path(&self) -> Option<String> {
        let diff_state = self.diff_state.as_ref()?;
        let file_diff = diff_state
            .selected_file_idx
            .and_then(|idx| diff_state.file_diffs.get(idx))
            .or_else(|| diff_state.file_diffs.first())?;

        Some(file_diff_path_label(file_diff).to_string())
    }

    pub fn set_code_view(&mut self, content: String, path: String) {
        self.viewer.set_code_view(content, path);
    }

    pub fn set_diff_mode(&mut self) {
        self.viewer.set_diff_mode();
    }

    pub fn set_blame(&mut self, lines: Vec<BlameLine>, path: String) {
        self.viewer.set_blame(lines, path);
    }

    pub fn diff_state(&self) -> Option<&CommitDiffState> {
        self.diff_state.as_ref()
    }

    pub fn view_mode(&self) -> DiffViewMode {
        self.viewer.view_mode()
    }

    pub fn code_view_file(&self) -> Option<&str> {
        self.viewer.code_view_file()
    }

    pub fn code_view_content(&self) -> Option<&str> {
        self.viewer.code_view_content()
    }

    pub fn restore_from_snapshot(
        &mut self,
        diff_state: Option<CommitDiffState>,
        view_mode: DiffViewMode,
        code_file: Option<String>,
        code_content: Option<String>,
    ) {
        if diff_state.is_some() {
            self.viewer.clear_highlight_cache();
        }
        self.diff_state = diff_state;
        self.viewer.restore(view_mode, code_file, code_content);
    }

    pub fn select_line(&mut self, line_idx: usize, extend: bool) {
        self.viewer.select_line(line_idx, extend);
    }

    pub fn clear_selection(&mut self) {
        self.viewer.clear_selection();
    }

    pub fn selected_range(&self) -> Option<Range<usize>> {
        self.viewer.selected_range()
    }

    pub fn selected_indices(&self) -> Vec<usize> {
        self.viewer.selected_indices()
    }

    /// Build a cheap fingerprint of the diff panel's visible state, used to
    /// decide whether the cached mirror view needs to be refreshed.
    pub fn build_key(
        &self,
        theme: String,
        loading: bool,
        selected_commit_id: Option<String>,
    ) -> DiffViewKey {
        DiffViewKey {
            theme,
            loading,
            selected_commit_id,
            diff_commit_id: self.diff_state.as_ref().map(|d| d.commit_id.clone()),
            selected_file_idx: self.diff_state.as_ref().and_then(|d| d.selected_file_idx),
            view_mode_tag: self.viewer.view_mode_tag(),
            code_view_file: self.viewer.code_view_file().map(String::from),
            blame_file: self.viewer.blame_file_for_key(),
            selection: self.viewer.selected_range(),
        }
    }

    /// Capture a render-ready snapshot of the diff panel. Cloning is cheap: the
    /// diff content lives behind `Arc`s and blame data is only copied while the
    /// blame view is active.
    pub fn build_snapshot(
        &self,
        colors: AppColors,
        loading: bool,
        selected_commit: Option<CommitInfo>,
        app: WeakEntity<super::app::GitForgeApp>,
    ) -> DiffSnapshot {
        let ctx = self.viewer.render_ctx();
        DiffSnapshot {
            colors,
            loading,
            selected_commit,
            diff_state: self.diff_state.clone(),
            view_mode: ctx.view_mode,
            code_view_file: ctx.code_view_file,
            code_view_content: ctx.code_view_content,
            blame: ctx.blame,
            selection: ctx.selection,
            highlight: ctx.highlight,
            scroll_handle: ctx.scroll_handle,
            code_scroll_handle: ctx.code_scroll_handle,
            app,
        }
    }
}

fn render_diff_panel(snap: &DiffSnapshot) -> Div {
    let colors = &snap.colors;
    let entity = snap.app.clone();
    let loading = snap.loading;

    let surface = rgba_to_hsla(colors.surface);
    let border = rgba_to_hsla(colors.border);
    let muted = rgba_to_hsla(colors.text_muted);
    let text_color = rgba_to_hsla(colors.text);

    {
        match (snap.selected_commit.as_ref(), &snap.diff_state) {
            (Some(commit), Some(diff_state)) => {
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
                let file_stats = diff_state.file_stats.clone();
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
                    let path = file_diff_path_label(fd);

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

                    let stats = file_stats.get(fi).copied().unwrap_or_default();
                    let added_count = stats.added;
                    let removed_count = stats.removed;

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

                // Resolve the file to show without deep-cloning: the selected
                // index if valid, otherwise the first file.
                let resolved_idx = selected_file
                    .filter(|&idx| idx < file_diffs.len())
                    .or_else(|| (!file_diffs.is_empty()).then_some(0));

                let resolved_diff = resolved_idx.map(|idx| &file_diffs[idx]);
                let render_ctx = DiffViewerRenderCtx {
                    view_mode: snap.view_mode.clone(),
                    scroll_handle: snap.scroll_handle.clone(),
                    code_scroll_handle: snap.code_scroll_handle.clone(),
                    highlight: snap.highlight.clone(),
                    code_view_file: snap.code_view_file.clone(),
                    code_view_content: snap.code_view_content.clone(),
                    blame: snap.blame.clone(),
                    selection: snap.selection.clone(),
                };

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

                let diff_content =
                    if resolved_diff.is_none() && snap.view_mode == DiffViewMode::Diff {
                        render_diff_empty_state(&colors)
                    } else {
                        render_diff_viewer(
                            &render_ctx,
                            resolved_diff,
                            &colors,
                            DiffViewerHeader::CommitHistory {
                                entity: entity.clone(),
                            },
                            entity.clone(),
                            on_click,
                            "diff-lines",
                            "diff-line",
                        )
                    };

                diff_panel_root(surface).child(commit_detail).child(
                    div()
                        .flex_1()
                        .flex()
                        .flex_row()
                        .overflow_hidden()
                        .child(file_list)
                        .child(diff_content),
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

/// Layout style applied to the cached diff mirror view so it fills the
/// right-hand panel slot.
pub fn diff_view_cache_style() -> StyleRefinement {
    StyleRefinement::default()
        .size_full()
        .min_w(px(RIGHT_MIN_WIDTH))
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
