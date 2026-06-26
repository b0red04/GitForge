use gitforge_diff::{DiffLineType, FileDiff};
use gitforge_git::BlameLine;
use gitforge_git::CommitInfo;
use gitforge_ui::{AppColors, ShellWidth, WidgetColors, empty_state, panel_shell, rgba_to_hsla};
use gpui::*;
use std::ops::Range;
use std::rc::Rc;
use std::sync::Arc;

use super::diff_view::SharedHighlightState;
use super::diff_viewer::{
    DiffViewer, DiffViewerHeader, file_diff_path_label, is_lfs_pointer, render_diff_viewer,
};
use super::layout::RIGHT_MIN_WIDTH;
use super::path_display::{format_parent_path, split_path_display};

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
///
/// The snapshot is also the single derivation point for [`DiffViewKey`]: the
/// mirror's stored key is always [`DiffSnapshot::key`], never a separate
/// hand-maintained projection. Adding a visible field to the snapshot requires
/// adding it to `key()` so the cache refreshes — they live in the same file, a
/// few lines apart.
#[derive(Clone)]
pub struct DiffSnapshot {
    pub theme: String,
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
    /// Whether the large diff overlay is open. Drives the active state of the
    /// file-list header toggle button.
    pub overlay_open: bool,
    pub app: WeakEntity<super::app::GitForgeApp>,
}

impl DiffSnapshot {
    /// Derive the cache fingerprint from this snapshot. This is the
    /// **authoritative** key: [`DiffViewMirror`] always stores the key
    /// returned here, never a separately-built one, so the stored key and the
    /// stored snapshot can never drift apart.
    ///
    /// The cheap pre-check in [`DiffPanel::build_key`] is an independent
    /// optimization that avoids building the full snapshot on cache hits.
    /// It must produce the same value as this method for the same panel
    /// state; if it drifts, the worst case is an unnecessary rebuild, never
    /// stale rendering (the stored key is always correct).
    pub fn key(&self) -> DiffViewKey {
        DiffViewKey {
            theme: self.theme.clone(),
            loading: self.loading,
            selected_commit_id: self.selected_commit.as_ref().map(|c| c.id.clone()),
            diff_commit_id: self.diff_state.as_ref().map(|d| d.commit_id.clone()),
            selected_file_idx: self.diff_state.as_ref().and_then(|d| d.selected_file_idx),
            view_mode_tag: self.view_mode.tag(),
            code_view_file: self.code_view_file.clone(),
            blame_file: if self.view_mode == DiffViewMode::Blame {
                self.blame.as_ref().map(|b| b.file_path.clone())
            } else {
                None
            },
            selection: self.selection.clone(),
            overlay_open: self.overlay_open,
        }
    }
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
    pub overlay_open: bool,
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

    pub fn update_snapshot(&mut self, snapshot: DiffSnapshot) {
        self.key = snapshot.key();
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
        let selected_diff = state
            .selected_file_idx
            .and_then(|idx| state.file_diffs.get(idx))
            .cloned();
        self.diff_state = Some(state);
        if let Some(diff) = selected_diff {
            self.viewer.set_diff(diff);
        } else {
            self.viewer.set_diff_mode();
        }
    }

    pub fn clear(&mut self) {
        self.diff_state = None;
        self.viewer.clear_diff();
    }

    pub fn select_file(&mut self, file_idx: usize) {
        let selected_diff = if let Some(ds) = self.diff_state.as_mut() {
            let resolved_idx = if ds.file_diffs.get(file_idx).is_some() {
                Some(file_idx)
            } else if ds.file_diffs.is_empty() {
                None
            } else {
                Some(0)
            };
            ds.selected_file_idx = resolved_idx;
            resolved_idx.and_then(|idx| ds.file_diffs.get(idx)).cloned()
        } else {
            None
        };

        if let Some(diff) = selected_diff {
            self.viewer.set_diff(diff);
        } else {
            self.viewer.set_diff_mode();
        }
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
        overlay_open: bool,
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
            overlay_open,
        }
    }

    /// Capture a render-ready snapshot of the diff panel. Cloning is cheap: the
    /// diff content lives behind `Arc`s and blame data is only copied while the
    /// blame view is active. The `theme` label is stored alongside `colors` so
    /// that [`DiffSnapshot::key`] can derive the cache fingerprint without a
    /// reverse color→name lookup.
    pub fn build_snapshot(
        &self,
        theme: String,
        colors: AppColors,
        loading: bool,
        selected_commit: Option<CommitInfo>,
        overlay_open: bool,
        app: WeakEntity<super::app::GitForgeApp>,
    ) -> DiffSnapshot {
        let ctx = self.viewer.render_ctx();
        DiffSnapshot {
            theme,
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
            overlay_open,
            app,
        }
    }

    /// Render the line-level diff for the currently-selected file, for display
    /// inside the large diff overlay. Returns `None` when there is no diff
    /// state or no file selected; the caller shows an empty state in that case.
    ///
    /// This completes the [`DiffViewer`] wiring that [`render_diff_panel`] never
    /// used: the panel's viewer (scroll handle, highlight, selection, view mode)
    /// is finally painted, via the shared [`render_diff_viewer`] renderer.
    pub fn render_overlay_diff(
        &self,
        colors: &AppColors,
        entity: WeakEntity<super::app::GitForgeApp>,
    ) -> Option<Div> {
        let ds = self.diff_state.as_ref()?;
        let file_idx = ds
            .selected_file_idx
            .filter(|idx| *idx < ds.file_diffs.len())
            .unwrap_or(0);
        let diff = ds.file_diffs.get(file_idx)?;
        if diff.lines.is_empty() && !diff.is_binary && !is_lfs_pointer(diff) {
            return Some(empty_state(
                "No line-level diff for this file",
                WidgetColors::from_app(colors),
            ));
        }
        let ctx = self.viewer.render_ctx();
        // Line selection is intentionally a no-op in the commit-history overlay
        // for now (no stage/unstage-line actions apply to committed files).
        let on_select_line = Rc::new(|_line_i: usize, _extend: bool, _cx: &mut gpui::App| ());
        Some(render_diff_viewer(
            &ctx,
            Some(diff),
            colors,
            DiffViewerHeader::CommitHistory {
                entity: entity.clone(),
            },
            entity,
            on_select_line,
            "overlay-diff-lines",
            "odl",
        ))
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
                let commit_detail = render_commit_detail(commit, colors, border, text_color, muted);

                if diff_state.file_diffs.is_empty() {
                    return diff_panel_root(surface)
                        .child(commit_detail)
                        .child(empty_state(
                            "No changes in this commit",
                            WidgetColors::from_app(&colors),
                        ));
                }

                let selected_file = diff_state.selected_file_idx;
                let file_diffs = diff_state.file_diffs.clone();
                let file_stats = diff_state.file_stats.clone();
                let colors = colors.clone();
                let file_click_entity = entity.clone();

                let summary = format_change_summary(&file_diffs);
                let overlay_open = snap.overlay_open;
                let toggle_ent = entity.clone();
                let toggle_icon = if overlay_open {
                    "icons/generic_restore.svg"
                } else {
                    "icons/generic_maximize.svg"
                };
                let toggle_bg = if overlay_open {
                    rgba_to_hsla(colors.sidebar_selected)
                } else {
                    gpui::transparent_black()
                };
                let mut file_list = div()
                    .id(ElementId::Name("commit-file-list".into()))
                    .flex_1()
                    .min_h(px(0.0))
                    .overflow_y_scroll()
                    .flex()
                    .flex_col();

                file_list = file_list.child(
                    div()
                        .flex_shrink_0()
                        .flex()
                        .items_center()
                        .gap_2()
                        .px_3()
                        .py_2()
                        .border_b_1()
                        .border_color(border)
                        .text_xs()
                        .text_color(muted)
                        .child(div().flex_1().child(summary))
                        .child(
                            div()
                                .id(ElementId::Name("diff-overlay-toggle".into()))
                                .flex_shrink_0()
                                .flex()
                                .items_center()
                                .justify_center()
                                .size(px(18.0))
                                .rounded(px(3.0))
                                .bg(toggle_bg)
                                .hover(move |s| s.bg(border))
                                .cursor_pointer()
                                .child(svg().size(px(13.0)).path(toggle_icon).text_color(muted))
                                .on_click(move |_ev, _window, cx| {
                                    if let Some(e) = toggle_ent.upgrade() {
                                        e.update(cx, |this, cx| this.toggle_diff_overlay(cx));
                                    }
                                }),
                        ),
                );

                for (fi, fd) in file_diffs.iter().enumerate() {
                    let path = file_diff_path_label(fd);
                    let is_sel = selected_file == Some(fi);
                    let bg = if is_sel {
                        rgba_to_hsla(colors.sidebar_selected)
                    } else {
                        rgba_to_hsla(colors.surface)
                    };
                    let path_muted = rgba_to_hsla(colors.text_muted);
                    let is_deleted = fd.new_path.is_none();
                    let name_color = if is_deleted {
                        path_muted
                    } else if is_sel {
                        text_color
                    } else {
                        rgba_to_hsla(colors.text)
                    };

                    let stats = file_stats.get(fi).copied().unwrap_or_default();
                    let added_count = stats.added;
                    let removed_count = stats.removed;
                    let stats_color_added = rgba_to_hsla(colors.diff_added);
                    let stats_color_removed = rgba_to_hsla(colors.diff_removed);

                    let (file_name, parent_path) = split_path_display(path);
                    let mut path_label = div()
                        .flex_1()
                        .min_w(px(0.0))
                        .flex()
                        .flex_row()
                        .items_center()
                        .overflow_hidden();
                    if let Some(parent) = parent_path {
                        let prefix = format_parent_path(&parent);
                        path_label = path_label.child(
                            div()
                                .min_w(px(0.0))
                                .flex_shrink()
                                .overflow_hidden()
                                .text_ellipsis()
                                .text_xs()
                                .text_color(path_muted)
                                .child(format!("{prefix}/")),
                        );
                    }
                    path_label = path_label.child(
                        div()
                            .flex_shrink_0()
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(name_color)
                            .child(file_name),
                    );

                    let click_ent = file_click_entity.clone();
                    file_list = file_list.child(
                        div()
                            .id(ElementId::Name(format!("diff-file-{fi}").into()))
                            .px_3()
                            .py_1p5()
                            .bg(bg)
                            .cursor_pointer()
                            .on_click(move |ev, _window, cx| {
                                if let Some(e) = click_ent.upgrade() {
                                    if ev.click_count() >= 2 {
                                        e.update(cx, |this, cx| {
                                            this.open_diff_overlay_for_file(fi, cx);
                                        });
                                    } else {
                                        e.update(cx, |this, cx| {
                                            this.select_diff_file(fi, cx);
                                        });
                                    }
                                }
                            })
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(render_diff_file_status_icon(fd, &colors))
                                    .child(path_label)
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

                diff_panel_root(surface)
                    .child(commit_detail)
                    .child(file_list)
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
    panel_shell(
        ShellWidth::Flexible {
            min_w: px(RIGHT_MIN_WIDTH),
        },
        surface,
        false,
        false,
    )
}

fn author_initials(name: &str) -> String {
    name.split_whitespace()
        .filter_map(|w| w.chars().next())
        .take(2)
        .collect::<String>()
        .to_uppercase()
}

fn format_change_summary(file_diffs: &[FileDiff]) -> String {
    let mut added = 0usize;
    let mut deleted = 0usize;
    let mut modified = 0usize;
    for fd in file_diffs {
        match (fd.old_path.is_some(), fd.new_path.is_some()) {
            (false, true) => added += 1,
            (true, false) => deleted += 1,
            _ => modified += 1,
        }
    }
    let mut parts = Vec::new();
    if modified > 0 {
        parts.push(format!("{modified} modified"));
    }
    if added > 0 {
        parts.push(format!("{added} added"));
    }
    if deleted > 0 {
        parts.push(format!("{deleted} deleted"));
    }
    parts.join(", ")
}

fn render_diff_file_status_icon(diff: &FileDiff, colors: &AppColors) -> Div {
    let (label, bg) = match (diff.old_path.is_some(), diff.new_path.is_some()) {
        (false, true) => ("+", rgba_to_hsla(colors.diff_added)),
        (true, false) => ("−", rgba_to_hsla(colors.diff_removed)),
        _ => ("M", rgba_to_hsla(colors.warning)),
    };

    div()
        .w(px(16.0))
        .h(px(16.0))
        .flex_shrink_0()
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(2.0))
        .bg(bg)
        .text_xs()
        .font_weight(FontWeight::BOLD)
        .text_color(rgba_to_hsla(colors.background))
        .child(label.to_string())
}

fn render_commit_detail(
    commit: &gitforge_git::CommitInfo,
    colors: &AppColors,
    border: Hsla,
    text_color: Hsla,
    muted: Hsla,
) -> Div {
    let accent = rgba_to_hsla(colors.accent);

    let parents = match commit.parent_ids.len() {
        0 => String::new(),
        1 => format!(
            " · parent: {}",
            &commit.parent_ids[0][..6.min(commit.parent_ids[0].len())]
        ),
        n => format!(" · {n} parents"),
    };
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

    detail
}
