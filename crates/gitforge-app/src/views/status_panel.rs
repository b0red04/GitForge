use gitforge_diff::FileDiff;
use gitforge_git::{DiffStat, FileEntry, FileStatus, RepoState, RepoStatus};
use gitforge_ui::{AppColors, rgba_to_hsla};
use gpui::*;
use std::ops::Range;
use std::path::Path;
use std::rc::Rc;

use super::commit_editor::CommitEditor;
use super::diff_viewer::{DiffViewer, DiffViewerHeader, render_diff_viewer};
use super::layout::{FILE_LIST_WIDTH, RIGHT_MIN_WIDTH};

const STATUS_FILE_WIDTH: f32 = FILE_LIST_WIDTH;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusFileSection {
    Staged,
    Unstaged,
    Untracked,
    Conflicted,
}

#[derive(Debug, Clone)]
pub struct StatusSelection {
    pub section: StatusFileSection,
    pub file_idx: usize,
}

pub struct StatusPanel {
    status: Option<RepoStatus>,
    selection: Option<StatusSelection>,
    viewer: DiffViewer,
    view_mode: StatusViewMode,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub enum StatusViewMode {
    Status,
    Diff,
    Commit,
    GraphStaging,
}

#[allow(dead_code)]
impl StatusPanel {
    pub fn new() -> Self {
        Self {
            status: None,
            selection: None,
            viewer: DiffViewer::new(),
            view_mode: StatusViewMode::Status,
        }
    }

    pub fn set_status(&mut self, status: RepoStatus, preserve_graph_staging: bool) {
        self.status = Some(status);
        self.viewer.clear_diff();

        if preserve_graph_staging {
            self.view_mode = StatusViewMode::GraphStaging;
        } else {
            self.selection = None;
            self.view_mode = StatusViewMode::Status;
        }
    }

    pub fn is_graph_staging(&self) -> bool {
        self.view_mode == StatusViewMode::GraphStaging
    }

    pub fn enter_graph_staging(&mut self) {
        self.view_mode = StatusViewMode::GraphStaging;
        self.selection = None;
        self.viewer.clear_diff();
    }

    pub fn exit_graph_staging(&mut self) {
        if self.view_mode == StatusViewMode::GraphStaging {
            self.view_mode = StatusViewMode::Status;
        }
    }

    pub fn clear(&mut self) {
        self.status = None;
        self.selection = None;
        self.view_mode = StatusViewMode::Status;
        self.viewer.clear_diff();
    }

    pub fn select_file(&mut self, section: StatusFileSection, file_idx: usize) {
        self.selection = Some(StatusSelection { section, file_idx });
        self.view_mode = StatusViewMode::Diff;
        self.viewer.clear_selection();
    }

    pub fn set_diff(&mut self, diff: FileDiff) {
        self.viewer.set_diff(diff);
    }

    pub fn show_commit(&mut self) {
        self.view_mode = StatusViewMode::Commit;
    }

    pub fn cancel_commit(&mut self) {
        if self.view_mode != StatusViewMode::GraphStaging {
            self.view_mode = StatusViewMode::Status;
        }
    }

    pub fn reset_after_commit(&mut self) {
        if self.view_mode != StatusViewMode::GraphStaging {
            self.view_mode = StatusViewMode::Status;
        }
    }

    pub fn restore_from_snapshot(
        &mut self,
        selection: Option<StatusSelection>,
        view_mode: StatusViewMode,
    ) {
        self.selection = selection;
        self.view_mode = view_mode;
    }

    pub fn select_diff_line(&mut self, line_idx: usize, extend: bool) {
        self.viewer.select_line(line_idx, extend);
    }

    pub fn diff_selected_range(&self) -> Option<Range<usize>> {
        self.viewer.selected_range()
    }

    pub fn diff_selected_indices(&self) -> Vec<usize> {
        self.viewer.selected_indices()
    }

    pub fn current_diff(&self) -> Option<&FileDiff> {
        self.viewer.current_diff()
    }

    pub fn current_section(&self) -> Option<StatusFileSection> {
        self.selection.as_ref().map(|s| s.section)
    }

    pub fn status_selection(&self) -> Option<&StatusSelection> {
        self.selection.as_ref()
    }

    pub fn view_mode(&self) -> StatusViewMode {
        self.view_mode.clone()
    }

    pub fn render(
        &self,
        colors: &AppColors,
        entity: WeakEntity<super::app::GitForgeApp>,
        window: &mut Window,
        ai_generating: bool,
        editor: &CommitEditor,
    ) -> Div {
        let surface = rgba_to_hsla(colors.surface);
        let border = rgba_to_hsla(colors.border);
        let muted = rgba_to_hsla(colors.text_muted);

        let header = div()
            .px_3()
            .py_2()
            .border_b_1()
            .border_color(border)
            .flex()
            .items_center()
            .child(
                div()
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .text_color(muted)
                    .child("CHANGES"),
            );

        let panel = match &self.status {
            Some(status)
                if status.has_changes()
                    || self.view_mode == StatusViewMode::Commit
                    || self.view_mode == StatusViewMode::GraphStaging =>
            {
                match self.view_mode {
                    StatusViewMode::Status
                    | StatusViewMode::Commit
                    | StatusViewMode::GraphStaging => {
                        let file_list = self.render_file_list(status, colors, entity.clone());
                        if self.view_mode == StatusViewMode::Commit {
                            let editor_el =
                                editor.render(colors, entity.clone(), window, ai_generating, false);
                            let p = status_panel_shell(surface).child(header).child(
                                div()
                                    .flex_1()
                                    .flex()
                                    .flex_row()
                                    .child(file_list)
                                    .child(editor_el),
                            );
                            return p;
                        }
                        let placeholder = self.render_placeholder(colors);
                        status_panel_shell(surface).child(header).child(
                            div()
                                .flex_1()
                                .flex()
                                .flex_row()
                                .child(file_list)
                                .child(placeholder),
                        )
                    }
                    StatusViewMode::Diff => {
                        let file_list = self.render_file_list(status, colors, entity.clone());
                        let diff_content = self.render_selected_diff(colors, entity.clone());
                        status_panel_shell(surface).child(header).child(
                            div()
                                .flex_1()
                                .flex()
                                .flex_row()
                                .child(file_list)
                                .child(diff_content),
                        )
                    }
                }
            }
            _ => status_panel_shell(surface).child(header).child(
                div().flex_1().flex().items_center().justify_center().child(
                    div()
                        .text_sm()
                        .text_color(muted)
                        .child("No uncommitted changes"),
                ),
            ),
        };

        panel
    }

    pub fn render_graph_staging(
        &self,
        repo_state: Option<&RepoState>,
        colors: &AppColors,
        entity: WeakEntity<super::app::GitForgeApp>,
        window: &mut Window,
        ai_generating: bool,
        editor: &CommitEditor,
    ) -> Div {
        let surface = rgba_to_hsla(colors.surface);
        let border = rgba_to_hsla(colors.border);
        let muted = rgba_to_hsla(colors.text_muted);
        let text_color = rgba_to_hsla(colors.text);

        let branch = repo_state
            .and_then(|rs| rs.head_branch.as_deref())
            .or_else(|| self.status.as_ref().and_then(|s| s.head_branch.as_deref()))
            .unwrap_or("HEAD");

        let Some(status) = &self.status else {
            return status_panel_shell(surface).child(
                div().flex_1().flex().items_center().justify_center().child(
                    div()
                        .text_sm()
                        .text_color(muted)
                        .child("No uncommitted changes"),
                ),
            );
        };

        if !status.has_changes() {
            return status_panel_shell(surface).child(
                div().flex_1().flex().items_center().justify_center().child(
                    div()
                        .text_sm()
                        .text_color(muted)
                        .child("No uncommitted changes"),
                ),
            );
        }

        let file_count = status.changed_file_count();
        let header_label = format!("{file_count} file changes on {branch}");

        let file_list = div()
            .id(ElementId::Name("graph-staging-file-list".into()))
            .flex_1()
            .min_h(px(0.0))
            .overflow_y_scroll()
            .flex()
            .flex_col();
        let file_list =
            self.populate_file_sections(file_list, status, colors, entity.clone(), false, false);

        let editor_el = editor.render(colors, entity.clone(), window, ai_generating, true);

        let can_commit = !status.staged.is_empty();
        let commit_label = if can_commit {
            format!("Commit {} files", status.staged.len())
        } else {
            "Stage changes to commit".to_string()
        };
        let commit_ent = entity.clone();
        let btn_bg = if can_commit {
            rgba_to_hsla(colors.accent)
        } else {
            muted
        };

        let primary_action = div()
            .px_3()
            .py_2()
            .border_t_1()
            .border_color(border)
            .flex_shrink_0()
            .child(
                div()
                    .id("graph-staging-commit-btn")
                    .w_full()
                    .py_2()
                    .rounded(px(4.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(btn_bg)
                    .cursor_pointer()
                    .text_sm()
                    .text_color(rgba_to_hsla(colors.background))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(commit_label)
                    .on_click(move |_ev, _window, cx| {
                        if let Some(e) = commit_ent.upgrade() {
                            e.update(cx, |this, cx| {
                                if can_commit {
                                    this.perform_commit(false, cx);
                                } else {
                                    this.stage_all(cx);
                                }
                            });
                        }
                    }),
            );

        status_panel_shell(surface)
            .child(
                div()
                    .px_3()
                    .py_2()
                    .border_b_1()
                    .border_color(border)
                    .flex_shrink_0()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(text_color)
                            .child(header_label),
                    ),
            )
            .child(file_list)
            .child(editor_el.flex_shrink_0())
            .child(primary_action)
    }

    fn populate_file_sections(
        &self,
        mut list: Stateful<Div>,
        status: &RepoStatus,
        colors: &AppColors,
        entity: WeakEntity<super::app::GitForgeApp>,
        open_diff_on_click: bool,
        include_sidebar_commit_button: bool,
    ) -> Stateful<Div> {
        let border = rgba_to_hsla(colors.border);
        let muted = rgba_to_hsla(colors.text_muted);
        let accent = rgba_to_hsla(colors.accent);

        if !status.staged.is_empty() {
            let unstage_ent = entity.clone();
            list = list.child(
                div()
                    .px_2()
                    .py_1()
                    .border_b_1()
                    .border_color(border)
                    .flex()
                    .items_center()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .text_color(muted)
                            .child(format!("STAGED CHANGES ({})", status.staged.len())),
                    )
                    .child(div().flex_1())
                    .child(
                        div()
                            .id("unstage-all-btn")
                            .px_1()
                            .rounded(px(2.0))
                            .border_1()
                            .border_color(border)
                            .cursor_pointer()
                            .text_xs()
                            .text_color(accent)
                            .child("-All")
                            .on_click(move |_ev, _window, cx| {
                                if let Some(e) = unstage_ent.upgrade() {
                                    e.update(cx, |this, cx| {
                                        this.unstage_all(cx);
                                    });
                                }
                            }),
                    ),
            );
            for (i, entry) in status.staged.iter().enumerate() {
                let is_sel = self
                    .selection
                    .as_ref()
                    .is_some_and(|s| s.section == StatusFileSection::Staged && s.file_idx == i);
                list = list.child(render_status_file_entry(
                    entry,
                    is_sel,
                    StatusFileSection::Staged,
                    i,
                    colors,
                    entity.clone(),
                    open_diff_on_click,
                ));
            }
        }

        if !status.unstaged.is_empty() {
            let stage_ent = entity.clone();
            list = list.child(
                div()
                    .px_2()
                    .py_1()
                    .mt_1()
                    .border_b_1()
                    .border_color(border)
                    .flex()
                    .items_center()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .text_color(muted)
                            .child(format!("UNSTAGED CHANGES ({})", status.unstaged.len())),
                    )
                    .child(div().flex_1())
                    .child(
                        div()
                            .id("stage-all-unstaged-btn")
                            .px_1()
                            .rounded(px(2.0))
                            .border_1()
                            .border_color(border)
                            .cursor_pointer()
                            .text_xs()
                            .text_color(accent)
                            .child("+All")
                            .on_click(move |_ev, _window, cx| {
                                if let Some(e) = stage_ent.upgrade() {
                                    e.update(cx, |this, cx| {
                                        this.stage_all(cx);
                                    });
                                }
                            }),
                    ),
            );
            for (i, entry) in status.unstaged.iter().enumerate() {
                let is_sel = self
                    .selection
                    .as_ref()
                    .is_some_and(|s| s.section == StatusFileSection::Unstaged && s.file_idx == i);
                list = list.child(render_status_file_entry(
                    entry,
                    is_sel,
                    StatusFileSection::Unstaged,
                    i,
                    colors,
                    entity.clone(),
                    open_diff_on_click,
                ));
            }
        }

        if !status.untracked.is_empty() {
            let stage_ent = entity.clone();
            list = list.child(
                div()
                    .px_2()
                    .py_1()
                    .mt_1()
                    .border_b_1()
                    .border_color(border)
                    .flex()
                    .items_center()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .text_color(muted)
                            .child(format!("Untracked ({})", status.untracked.len())),
                    )
                    .child(div().flex_1())
                    .child(
                        div()
                            .id("stage-all-untracked-btn")
                            .px_1()
                            .rounded(px(2.0))
                            .border_1()
                            .border_color(border)
                            .cursor_pointer()
                            .text_xs()
                            .text_color(accent)
                            .child("+All")
                            .on_click(move |_ev, _window, cx| {
                                if let Some(e) = stage_ent.upgrade() {
                                    e.update(cx, |this, cx| {
                                        this.stage_all(cx);
                                    });
                                }
                            }),
                    ),
            );
            for (i, entry) in status.untracked.iter().enumerate() {
                let is_sel = self
                    .selection
                    .as_ref()
                    .is_some_and(|s| s.section == StatusFileSection::Untracked && s.file_idx == i);
                list = list.child(render_status_file_entry(
                    entry,
                    is_sel,
                    StatusFileSection::Untracked,
                    i,
                    colors,
                    entity.clone(),
                    open_diff_on_click,
                ));
            }
        }

        if !status.conflicted.is_empty() {
            list = list.child(
                div()
                    .px_2()
                    .py_1()
                    .mt_1()
                    .border_b_1()
                    .border_color(border)
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgba_to_hsla(colors.warning))
                            .child(format!("CONFLICTS ({})", status.conflicted.len())),
                    ),
            );
            for (i, entry) in status.conflicted.iter().enumerate() {
                let is_sel = self
                    .selection
                    .as_ref()
                    .is_some_and(|s| s.section == StatusFileSection::Conflicted && s.file_idx == i);
                list = list.child(render_status_file_entry(
                    entry,
                    is_sel,
                    StatusFileSection::Conflicted,
                    i,
                    colors,
                    entity.clone(),
                    open_diff_on_click,
                ));
            }
        }

        if include_sidebar_commit_button {
            let commit_ent = entity.clone();
            let can_commit = !status.staged.is_empty();
            list = list.child(div().p_2().border_t_1().border_color(border).child({
                let commit_ent2 = commit_ent.clone();
                let btn_bg = if can_commit {
                    rgba_to_hsla(colors.accent)
                } else {
                    muted
                };
                div()
                    .id("commit-btn")
                    .w_full()
                    .py_1()
                    .rounded(px(4.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(btn_bg)
                    .cursor_pointer()
                    .text_xs()
                    .text_color(rgba_to_hsla(colors.background))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("Commit")
                    .on_click(move |_ev, _window, cx| {
                        if can_commit {
                            if let Some(e) = commit_ent2.upgrade() {
                                e.update(cx, |this, cx| {
                                    this.show_commit_dialog(cx);
                                });
                            }
                        }
                    })
            }));
        }

        list
    }

    fn render_file_list(
        &self,
        status: &RepoStatus,
        colors: &AppColors,
        entity: WeakEntity<super::app::GitForgeApp>,
    ) -> Stateful<Div> {
        let border = rgba_to_hsla(colors.border);

        let list = div()
            .id(ElementId::Name("status-file-list".into()))
            .w(px(STATUS_FILE_WIDTH))
            .h_full()
            .border_r_1()
            .border_color(border)
            .flex()
            .flex_col()
            .overflow_y_scroll();

        self.populate_file_sections(list, status, colors, entity, true, true)
    }

    fn render_selected_diff(
        &self,
        colors: &AppColors,
        entity: WeakEntity<super::app::GitForgeApp>,
    ) -> Div {
        let diff = self.viewer.current_diff();
        let sel = self.selection.clone();
        let section_label = match sel.as_ref().map(|s| s.section) {
            Some(StatusFileSection::Staged) => "Staged",
            Some(StatusFileSection::Unstaged) => "Unstaged",
            Some(StatusFileSection::Conflicted) => "Conflicted",
            _ => "Changes",
        };
        let is_staged = matches!(
            sel.as_ref().map(|s| s.section),
            Some(StatusFileSection::Staged)
        );
        let has_selection = self.viewer.selected_range().is_some();

        let on_click = {
            let ent = entity.clone();
            Rc::new(move |line_i: usize, extend: bool, cx: &mut App| {
                if let Some(e) = ent.upgrade() {
                    e.update(cx, |this, cx| {
                        this.select_status_diff_line(line_i, extend, cx);
                    });
                }
            })
        };

        render_diff_viewer(
            &self.viewer.render_ctx(),
            diff,
            colors,
            DiffViewerHeader::WorkingTree {
                section_label,
                is_staged,
                has_line_selection: has_selection,
                entity: entity.clone(),
            },
            entity,
            on_click,
            "status-diff-lines",
            "sdl",
        )
    }

    fn render_placeholder(&self, colors: &AppColors) -> Div {
        let surface = rgba_to_hsla(colors.surface);
        let muted = rgba_to_hsla(colors.text_muted);
        div()
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
                    .child("Select a file to view changes"),
            )
    }
}

fn status_panel_shell(surface: Hsla) -> Div {
    div()
        .w_full()
        .h_full()
        .min_w(px(RIGHT_MIN_WIDTH))
        .bg(surface)
        .flex()
        .flex_col()
}

fn render_status_file_entry(
    entry: &FileEntry,
    is_selected: bool,
    section: StatusFileSection,
    idx: usize,
    colors: &AppColors,
    entity: WeakEntity<super::app::GitForgeApp>,
    open_diff_on_click: bool,
) -> Stateful<Div> {
    let surface = rgba_to_hsla(colors.surface);
    let selected_bg = rgba_to_hsla(colors.sidebar_selected);
    let bg = if is_selected { selected_bg } else { surface };
    let text_color = if is_selected {
        rgba_to_hsla(colors.text)
    } else {
        rgba_to_hsla(colors.text)
    };
    let path_muted = rgba_to_hsla(colors.text_muted);
    let is_deleted = entry.status == FileStatus::Deleted;

    let ent = entity.clone();
    let path_owned = entry.path.clone();

    let action_ent = entity.clone();
    let action_path = entry.path.clone();
    let action_section = section;
    let is_staged_row = matches!(section, StatusFileSection::Staged);

    let show_discard = matches!(section, StatusFileSection::Unstaged);
    let show_remove = matches!(section, StatusFileSection::Untracked);
    let show_conflict_actions = matches!(section, StatusFileSection::Conflicted);

    let mut entry_row = div()
        .id(ElementId::Name(
            format!("status-file-{:?}-{}", section, idx).into(),
        ))
        .w_full()
        .px_2()
        .py_1()
        .bg(bg);
    if open_diff_on_click {
        entry_row = entry_row
            .cursor_pointer()
            .hover(|s| s.bg(rgba_to_hsla(colors.sidebar_hover)))
            .on_click(move |_ev, _window, cx| {
                if let Some(e) = ent.upgrade() {
                    let p = path_owned.clone();
                    e.update(cx, |this, cx| {
                        this.select_status_file(section, idx, p, cx);
                    });
                }
            });
    }

    let (file_name, parent_path) = split_path_display(&entry.path);
    let name_color = if is_deleted { path_muted } else { text_color };

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

    let mut inner_row = div()
        .flex()
        .items_center()
        .gap_2()
        .min_w(px(0.0))
        .child(render_git_status_icon(&entry.status, colors))
        .child(path_label)
        .child(render_line_diff_stat(entry.diff_stat, colors));

    let border = rgba_to_hsla(colors.border);
    let accent = rgba_to_hsla(colors.accent);

    if show_discard {
        let discard_ent = entity.clone();
        let discard_path = entry.path.clone();
        inner_row = inner_row.child(
            div()
                .id(ElementId::Name(
                    format!("discard-{:?}-{}", section, idx).into(),
                ))
                .w(px(18.0))
                .h(px(18.0))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(2.0))
                .border_1()
                .border_color(rgba_to_hsla(colors.diff_removed))
                .cursor_pointer()
                .text_xs()
                .text_color(rgba_to_hsla(colors.diff_removed))
                .child("\u{00d7}")
                .on_click(move |_ev, _window, cx| {
                    if let Some(e) = discard_ent.upgrade() {
                        let p = discard_path.clone();
                        e.update(cx, |this, cx| {
                            this.discard_file(p, cx);
                        });
                    }
                }),
        );
    }

    if show_remove {
        let remove_ent = entity.clone();
        let remove_path = entry.path.clone();
        inner_row = inner_row.child(
            div()
                .id(ElementId::Name(
                    format!("remove-{:?}-{}", section, idx).into(),
                ))
                .w(px(18.0))
                .h(px(18.0))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(2.0))
                .border_1()
                .border_color(rgba_to_hsla(colors.diff_removed))
                .cursor_pointer()
                .text_xs()
                .text_color(rgba_to_hsla(colors.diff_removed))
                .child("\u{00d7}")
                .on_click(move |_ev, _window, cx| {
                    if let Some(e) = remove_ent.upgrade() {
                        let p = remove_path.clone();
                        e.update(cx, |this, cx| {
                            this.remove_untracked_file(p, cx);
                        });
                    }
                }),
        );
    }

    let checkbox_bg = if is_staged_row { accent } else { surface };
    inner_row = inner_row.child(
        div()
            .id(ElementId::Name(
                format!("stage-checkbox-{:?}-{}", section, idx).into(),
            ))
            .w(px(14.0))
            .h(px(14.0))
            .flex_shrink_0()
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(2.0))
            .border_1()
            .border_color(if is_staged_row { accent } else { border })
            .bg(checkbox_bg)
            .cursor_pointer()
            .on_click(move |_ev, _window, cx| {
                if let Some(e) = action_ent.upgrade() {
                    let p = action_path.clone();
                    e.update(cx, |this, cx| match action_section {
                        StatusFileSection::Staged => this.unstage_file(p, cx),
                        StatusFileSection::Conflicted => this.stage_file(p, cx),
                        _ => this.stage_file(p, cx),
                    });
                }
            }),
    );

    entry_row = entry_row.child(inner_row);

    if show_conflict_actions {
        let ours_ent = entity.clone();
        let theirs_ent = entity.clone();
        let conflict_path = entry.path.clone();
        let conflict_path2 = entry.path.clone();
        entry_row = entry_row.child(
            div()
                .flex()
                .gap_0()
                .ml_1()
                .child(
                    div()
                        .id(ElementId::Name(format!("use-ours-{}", idx).into()))
                        .px_1()
                        .rounded(px(2.0))
                        .border_1()
                        .border_color(rgba_to_hsla(colors.accent))
                        .cursor_pointer()
                        .text_xs()
                        .text_color(rgba_to_hsla(colors.accent))
                        .child("Ours")
                        .on_click(move |_ev, _window, cx| {
                            if let Some(e) = ours_ent.upgrade() {
                                let p = conflict_path.clone();
                                e.update(cx, |this, cx| {
                                    this.resolve_conflict_ours(p, cx);
                                });
                            }
                        }),
                )
                .child(
                    div()
                        .id(ElementId::Name(format!("use-theirs-{}", idx).into()))
                        .px_1()
                        .rounded(px(2.0))
                        .border_1()
                        .border_color(rgba_to_hsla(colors.warning))
                        .cursor_pointer()
                        .text_xs()
                        .text_color(rgba_to_hsla(colors.warning))
                        .child("Theirs")
                        .on_click(move |_ev, _window, cx| {
                            if let Some(e) = theirs_ent.upgrade() {
                                let p = conflict_path2.clone();
                                e.update(cx, |this, cx| {
                                    this.resolve_conflict_theirs(p, cx);
                                });
                            }
                        }),
                ),
        );
    }

    entry_row
}

fn split_path_display(path: &str) -> (String, Option<String>) {
    let path = Path::new(path);
    let file_name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string_lossy().into_owned());
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map(|p| p.to_string_lossy().replace('\\', "/"));
    (file_name, parent)
}

fn format_parent_path(parent: &str) -> String {
    const MAX: usize = 36;
    if parent.len() <= MAX {
        return format!("...{parent}");
    }
    format!("...{}", &parent[parent.len() - (MAX - 3)..])
}

/// Zed-style status glyph: modified = amber M, new/untracked = green +, etc.
fn render_git_status_icon(status: &FileStatus, colors: &AppColors) -> Div {
    let (label, bg) = match status {
        FileStatus::Untracked | FileStatus::Added => ("+", rgba_to_hsla(colors.diff_added)),
        FileStatus::Modified | FileStatus::Renamed | FileStatus::Copied => {
            ("M", rgba_to_hsla(colors.warning))
        }
        FileStatus::Deleted => ("−", rgba_to_hsla(colors.diff_removed)),
        FileStatus::Conflicted => ("!", rgba_to_hsla(colors.error)),
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

fn render_line_diff_stat(stat: Option<DiffStat>, colors: &AppColors) -> Div {
    let Some(stat) = stat else {
        return div().flex_shrink_0();
    };
    if stat.added == 0 && stat.deleted == 0 {
        return div().flex_shrink_0();
    }

    let added_color = rgba_to_hsla(colors.diff_added);
    let removed_color = rgba_to_hsla(colors.diff_removed);

    let mut row = div().flex().items_center().gap_1().flex_shrink_0();

    if stat.added > 0 {
        row = row.child(
            div()
                .text_xs()
                .font_family("monospace")
                .text_color(added_color)
                .child(format!("+{}", stat.added)),
        );
    }
    if stat.deleted > 0 {
        row = row.child(
            div()
                .text_xs()
                .font_family("monospace")
                .text_color(removed_color)
                .child(format!("-{}", stat.deleted)),
        );
    }

    row
}
