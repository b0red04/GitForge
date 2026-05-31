use gpui::*;
use gitforge_ui::{AppColors, rgba_to_hsla};
use gitforge_git::{RepoStatus, FileStatus, FileEntry};
use gitforge_diff::{FileDiff, DiffLineType};
use std::ops::Range;
use std::collections::HashMap;

use super::layout::{FILE_LIST_WIDTH, RIGHT_MIN_WIDTH};

const STATUS_FILE_WIDTH: f32 = FILE_LIST_WIDTH;
const STATUS_DIFF_LINE_HEIGHT: f32 = 20.0;
const STATUS_DIFF_LINE_NUM_WIDTH: f32 = 50.0;

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
    diff_for_selected: Option<FileDiff>,
    scroll_handle: UniformListScrollHandle,
    view_mode: StatusViewMode,
    commit_message: String,
    commit_message_focus: FocusHandle,
    diff_sel_anchor: Option<usize>,
    diff_sel_end: Option<usize>,
    ai_message_alternatives: Vec<String>,
    ai_file_summaries: HashMap<String, String>,
    ai_file_summary_visible: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub enum StatusViewMode {
    Status,
    Diff,
    Code,
    Commit,
}

#[allow(dead_code)]
impl StatusPanel {
    pub fn new(cx: &mut App) -> Self {
        Self {
            status: None,
            selection: None,
            diff_for_selected: None,
            scroll_handle: UniformListScrollHandle::default(),
            view_mode: StatusViewMode::Status,
            commit_message: String::new(),
            commit_message_focus: cx.focus_handle(),
            diff_sel_anchor: None,
            diff_sel_end: None,
            ai_message_alternatives: Vec::new(),
            ai_file_summaries: HashMap::new(),
            ai_file_summary_visible: None,
        }
    }

    pub fn set_status(&mut self, status: RepoStatus) {
        self.status = Some(status);
        self.selection = None;
        self.diff_for_selected = None;
        self.view_mode = StatusViewMode::Status;
        self.diff_sel_anchor = None;
        self.diff_sel_end = None;
    }

    pub fn clear(&mut self) {
        self.status = None;
        self.selection = None;
        self.diff_for_selected = None;
        self.view_mode = StatusViewMode::Status;
        self.diff_sel_anchor = None;
        self.diff_sel_end = None;
    }

    pub fn select_file(&mut self, section: StatusFileSection, file_idx: usize) {
        self.selection = Some(StatusSelection { section, file_idx });
        self.view_mode = StatusViewMode::Diff;
        self.diff_sel_anchor = None;
        self.diff_sel_end = None;
    }

    pub fn set_diff(&mut self, diff: FileDiff) {
        self.diff_for_selected = Some(diff);
    }

    pub fn show_commit(&mut self) {
        self.view_mode = StatusViewMode::Commit;
    }

    pub fn cancel_commit(&mut self) {
        self.view_mode = StatusViewMode::Status;
    }

    pub fn commit_message(&self) -> &str {
        &self.commit_message
    }

    pub fn commit_message_mut(&mut self) -> &mut String {
        &mut self.commit_message
    }

    pub fn take_commit_message(&mut self) -> String {
        let msg = self.commit_message.clone();
        self.commit_message.clear();
        self.view_mode = StatusViewMode::Status;
        self.ai_message_alternatives.clear();
        msg
    }

    pub fn set_ai_alternatives(&mut self, messages: Vec<String>) {
        self.ai_message_alternatives = messages;
    }

    pub fn ai_alternatives(&self) -> &[String] {
        &self.ai_message_alternatives
    }

    pub fn set_file_summary(&mut self, path: String, summary: String) {
        self.ai_file_summaries.insert(path, summary);
    }

    pub fn file_summary(&self, path: &str) -> Option<&str> {
        self.ai_file_summaries.get(path).map(|s| s.as_str())
    }

    pub fn show_file_summary(&mut self, path: Option<String>) {
        self.ai_file_summary_visible = path;
    }

    pub fn visible_summary(&self) -> Option<&str> {
        self.ai_file_summary_visible.as_ref()
            .and_then(|p| self.ai_file_summaries.get(p).map(|s| s.as_str()))
    }

    pub fn select_diff_line(&mut self, line_idx: usize, extend: bool) {
        if extend {
            if self.diff_sel_anchor.is_some() {
                self.diff_sel_end = Some(line_idx);
            } else {
                self.diff_sel_anchor = Some(line_idx);
                self.diff_sel_end = Some(line_idx);
            }
        } else {
            self.diff_sel_anchor = Some(line_idx);
            self.diff_sel_end = Some(line_idx);
        }
    }

    pub fn diff_selected_range(&self) -> Option<Range<usize>> {
        match (self.diff_sel_anchor, self.diff_sel_end) {
            (Some(a), Some(b)) => {
                let start = a.min(b);
                let end = a.max(b) + 1;
                Some(start..end)
            }
            _ => None,
        }
    }

    pub fn diff_selected_indices(&self) -> Vec<usize> {
        self.diff_selected_range()
            .map(|r| r.collect())
            .unwrap_or_default()
    }

    pub fn current_diff(&self) -> Option<&FileDiff> {
        self.diff_for_selected.as_ref()
    }

    pub fn current_section(&self) -> Option<StatusFileSection> {
        self.selection.as_ref().map(|s| s.section)
    }

    pub fn render(
        &self,
        colors: &AppColors,
        entity: WeakEntity<super::app::GitForgeApp>,
        window: &mut Window,
        ai_generating: bool,
    ) -> Div {
        let surface = rgba_to_hsla(colors.surface);
        let border = rgba_to_hsla(colors.border);
        let muted = rgba_to_hsla(colors.text_muted);

        let header = div()
            .px_3().py_2()
            .border_b_1().border_color(border)
            .flex().items_center()
            .child(div().text_xs().font_weight(FontWeight::BOLD).text_color(muted).child("CHANGES"));

        let mut panel = match &self.status {
            Some(status) if status.has_changes() || self.view_mode == StatusViewMode::Commit => {
                match self.view_mode {
                    StatusViewMode::Status | StatusViewMode::Commit => {
                        let file_list = self.render_file_list(status, colors, entity.clone());
                        if self.view_mode == StatusViewMode::Commit {
                            let editor = self.render_commit_editor(colors, entity.clone(), window, ai_generating);
                            let p = status_panel_shell(surface)
                                .child(header)
                                .child(
                                    div().flex_1().flex().flex_row()
                                        .child(file_list)
                                        .child(editor)
                                );
                            return p;
                        }
                        let placeholder = self.render_placeholder(colors);
                        status_panel_shell(surface)
                            .child(header)
                            .child(
                                div().flex_1().flex().flex_row()
                                    .child(file_list)
                                    .child(placeholder)
                            )
                    }
                    StatusViewMode::Diff => {
                        let file_list = self.render_file_list(status, colors, entity.clone());
                        let diff_content = self.render_selected_diff(colors, entity.clone());
                        status_panel_shell(surface)
                            .child(header)
                            .child(
                                div().flex_1().flex().flex_row()
                                    .child(file_list)
                                    .child(diff_content)
                            )
                    }
                    StatusViewMode::Code => {
                        return self.render_placeholder(colors);
                    }
                }
            }
            _ => {
                status_panel_shell(surface)
                    .child(header)
                    .child(
                        div().flex_1().flex().items_center().justify_center()
                            .child(div().text_sm().text_color(muted).child("No uncommitted changes"))
                    )
            }
        };

        if let Some(summary) = self.visible_summary() {
            let popup = render_ai_summary_popup(summary, colors, entity);
            panel = panel.child(popup);
        }

        panel
    }

    fn render_file_list(
        &self,
        status: &RepoStatus,
        colors: &AppColors,
        entity: WeakEntity<super::app::GitForgeApp>,
    ) -> Stateful<Div> {
        let border = rgba_to_hsla(colors.border);
        let muted = rgba_to_hsla(colors.text_muted);
        let accent = rgba_to_hsla(colors.accent);

        let mut list = div()
            .id(ElementId::Name("status-file-list".into()))
            .w(px(STATUS_FILE_WIDTH))
            .h_full()
            .border_r_1().border_color(border)
            .flex().flex_col()
            .overflow_y_scroll();

        if !status.staged.is_empty() {
            let unstage_ent = entity.clone();
            list = list.child(
                div().px_2().py_1()
                    .border_b_1().border_color(border)
                    .flex().items_center()
                    .child(
                        div().text_xs().font_weight(FontWeight::BOLD).text_color(muted)
                            .child(format!("STAGED CHANGES ({})", status.staged.len()))
                    )
                    .child(div().flex_1())
                    .child(
                        div()
                            .id("unstage-all-btn")
                            .px_1().rounded(px(2.0))
                            .border_1().border_color(border)
                            .cursor_pointer()
                            .text_xs().text_color(accent)
                            .child("-All")
                            .on_click(move |_ev, _window, cx| {
                                if let Some(e) = unstage_ent.upgrade() {
                                    e.update(cx, |this, cx| {
                                        this.unstage_all(cx);
                                    });
                                }
                            })
                    )
            );
            for (i, entry) in status.staged.iter().enumerate() {
                let is_sel = self.selection.as_ref()
                    .map_or(false, |s| s.section == StatusFileSection::Staged && s.file_idx == i);
                list = list.child(render_status_file_entry(
                    entry, is_sel, StatusFileSection::Staged, i, colors, entity.clone(),
                ));
            }
        }

        if !status.unstaged.is_empty() {
            let stage_ent = entity.clone();
            list = list.child(
                div().px_2().py_1().mt_1()
                    .border_b_1().border_color(border)
                    .flex().items_center()
                    .child(
                        div().text_xs().font_weight(FontWeight::BOLD).text_color(muted)
                            .child(format!("UNSTAGED CHANGES ({})", status.unstaged.len()))
                    )
                    .child(div().flex_1())
                    .child(
                        div()
                            .id("stage-all-unstaged-btn")
                            .px_1().rounded(px(2.0))
                            .border_1().border_color(border)
                            .cursor_pointer()
                            .text_xs().text_color(accent)
                            .child("+All")
                            .on_click(move |_ev, _window, cx| {
                                if let Some(e) = stage_ent.upgrade() {
                                    e.update(cx, |this, cx| {
                                        this.stage_all(cx);
                                    });
                                }
                            })
                    )
            );
            for (i, entry) in status.unstaged.iter().enumerate() {
                let is_sel = self.selection.as_ref()
                    .map_or(false, |s| s.section == StatusFileSection::Unstaged && s.file_idx == i);
                list = list.child(render_status_file_entry(
                    entry, is_sel, StatusFileSection::Unstaged, i, colors, entity.clone(),
                ));
            }
        }

        if !status.untracked.is_empty() {
            let stage_ent = entity.clone();
            list = list.child(
                div().px_2().py_1().mt_1()
                    .border_b_1().border_color(border)
                    .flex().items_center()
                    .child(
                        div().text_xs().font_weight(FontWeight::BOLD).text_color(muted)
                            .child(format!("UNTRACKED FILES ({})", status.untracked.len()))
                    )
                    .child(div().flex_1())
                    .child(
                        div()
                            .id("stage-all-untracked-btn")
                            .px_1().rounded(px(2.0))
                            .border_1().border_color(border)
                            .cursor_pointer()
                            .text_xs().text_color(accent)
                            .child("+All")
                            .on_click(move |_ev, _window, cx| {
                                if let Some(e) = stage_ent.upgrade() {
                                    e.update(cx, |this, cx| {
                                        this.stage_all(cx);
                                    });
                                }
                            })
                    )
            );
            for (i, entry) in status.untracked.iter().enumerate() {
                let is_sel = self.selection.as_ref()
                    .map_or(false, |s| s.section == StatusFileSection::Untracked && s.file_idx == i);
                list = list.child(render_status_file_entry(
                    entry, is_sel, StatusFileSection::Untracked, i, colors, entity.clone(),
                ));
            }
        }

        if !status.conflicted.is_empty() {
            list = list.child(
                div().px_2().py_1().mt_1()
                    .border_b_1().border_color(border)
                    .flex().items_center().gap_1()
                    .child(div().text_xs().font_weight(FontWeight::BOLD).text_color(rgba_to_hsla(colors.warning))
                        .child(format!("CONFLICTS ({})", status.conflicted.len())))
            );
            for (i, entry) in status.conflicted.iter().enumerate() {
                let is_sel = self.selection.as_ref()
                    .map_or(false, |s| s.section == StatusFileSection::Conflicted && s.file_idx == i);
                list = list.child(render_status_file_entry(
                    entry, is_sel, StatusFileSection::Conflicted, i, colors, entity.clone(),
                ));
            }
        }

        let commit_ent = entity.clone();
        let can_commit = !status.staged.is_empty();
        list = list.child(
            div().p_2().border_t_1().border_color(border)
                .child({
                    let commit_ent2 = commit_ent.clone();
                    let btn_bg = if can_commit { rgba_to_hsla(colors.accent) } else { muted };
                    div()
                        .id("commit-btn")
                        .w_full()
                        .py_1()
                        .rounded(px(4.0))
                        .flex().items_center().justify_center()
                        .bg(btn_bg)
                        .cursor_pointer()
                        .text_xs().text_color(rgba_to_hsla(colors.background))
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
                })
        );

        list
    }

    fn render_selected_diff(
        &self,
        colors: &AppColors,
        entity: WeakEntity<super::app::GitForgeApp>,
    ) -> Div {
        let surface = rgba_to_hsla(colors.surface);
        let muted = rgba_to_hsla(colors.text_muted);
        let border = rgba_to_hsla(colors.border);
        let accent = rgba_to_hsla(colors.accent);
        let selection_bg = rgba_to_hsla(colors.selection_bg);

        let Some(diff) = &self.diff_for_selected else {
            return div()
                .flex_1().h_full().bg(surface)
                .flex().items_center().justify_center()
                .child(div().text_sm().text_color(muted).child("Select a file to view diff"));
        };

        let path_label = diff.new_path.as_deref()
            .or(diff.old_path.as_deref())
            .unwrap_or("(unknown)");

        let sel = self.selection.clone();
        let section_label = match sel.as_ref().map(|s| s.section) {
            Some(StatusFileSection::Staged) => "Staged",
            Some(StatusFileSection::Unstaged) => "Unstaged",
            Some(StatusFileSection::Conflicted) => "Conflicted",
            _ => "Changes",
        };

        let sel_range = self.diff_selected_range();
        let has_selection = sel_range.is_some();

        let mut file_header = div()
            .px_3().py_2()
            .border_b_1().border_color(border)
            .flex().items_center().gap_2()
            .child(
                div().text_sm().font_family("monospace").text_color(rgba_to_hsla(colors.text))
                    .child(path_label.to_string())
            )
            .child(div().flex_1());

        if has_selection {
            let is_staged = matches!(sel.as_ref().map(|s| s.section), Some(StatusFileSection::Staged));
            let label = if is_staged { "Unstage Lines" } else { "Stage Lines" };
            let lines_ent = entity.clone();
            file_header = file_header.child(
                div()
                    .id("stage-lines-btn")
                    .px_2().py_0()
                    .rounded(px(3.0))
                    .bg(accent)
                    .cursor_pointer()
                    .text_xs().text_color(rgba_to_hsla(colors.background))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(label.to_string())
                    .on_click(move |_ev, _window, cx| {
                        if let Some(e) = lines_ent.upgrade() {
                            e.update(cx, |this, cx| {
                                if is_staged {
                                    this.unstage_selected_lines(cx);
                                } else {
                                    this.stage_selected_lines(cx);
                                }
                            });
                        }
                    })
            );
        }

        file_header = file_header.child(
            div().text_xs().text_color(muted).child(section_label)
        );

        let total_lines = diff.lines.len();
        let lines_data = diff.lines.clone();
        let cl = colors.clone();

        let diff_lines = uniform_list(
            "status-diff-lines",
            total_lines,
            move |visible_range: Range<usize>, _window: &mut Window, _cx: &mut App| {
                let mut rows = Vec::with_capacity(visible_range.len());
                let added_bg = rgba_to_hsla(cl.diff_added_bg);
                let removed_bg = rgba_to_hsla(cl.diff_removed_bg);
                let added_fg = rgba_to_hsla(cl.diff_added);
                let removed_fg = rgba_to_hsla(cl.diff_removed);
                let hunk_header_bg = rgba_to_hsla(cl.diff_hunk_header);
                let text_color = rgba_to_hsla(cl.text);
                let muted = rgba_to_hsla(cl.text_muted);
                let bdr = rgba_to_hsla(cl.border);
                let surf = rgba_to_hsla(cl.surface);

                for line_i in visible_range {
                    let Some(line) = lines_data.get(line_i) else { continue };

                    let is_conflict_marker = line.content.starts_with("<<<<<<< ")
                        || line.content.starts_with("=======\n")
                        || line.content.starts_with("=======\r")
                        || line.content == "======="
                        || line.content.starts_with(">>>>>>> ");

                    let (base_bg, line_fg, prefix) = if is_conflict_marker {
                        (rgba_to_hsla(cl.warning).alpha(0.15), rgba_to_hsla(cl.warning), "\u{26a0}")
                    } else {
                        match line.line_type {
                            DiffLineType::Added => (added_bg, added_fg, "+"),
                            DiffLineType::Removed => (removed_bg, removed_fg, "-"),
                            DiffLineType::HunkHeader => (hunk_header_bg, muted, " "),
                            DiffLineType::Context => (surf, text_color, " "),
                            DiffLineType::NoNewlineAtEof => (surf, muted, "\\"),
                        }
                    };

                    let is_selected = sel_range.as_ref().map_or(false, |r| r.contains(&line_i));
                    let row_bg = if is_selected { selection_bg } else { base_bg };

                    let old_num = line.old_line.map(|n| format!("{:>4}", n)).unwrap_or_else(|| "    ".to_string());
                    let new_num = line.new_line.map(|n| format!("{:>4}", n)).unwrap_or_else(|| "    ".to_string());
                    let display: String = line.content.chars().take(200).collect();

                    let click_ent = entity.clone();
                    let row = div()
                        .id(ElementId::Name(format!("sdl-{line_i}").into()))
                        .h(px(STATUS_DIFF_LINE_HEIGHT))
                        .flex().flex_row().items_center()
                        .bg(row_bg)
                        .cursor_pointer()
                        .on_mouse_down(MouseButton::Left, move |ev: &MouseDownEvent, _window, cx| {
                            if let Some(e) = click_ent.upgrade() {
                                let extend = ev.modifiers.shift;
                                e.update(cx, |this, cx| {
                                    this.select_status_diff_line(line_i, extend, cx);
                                });
                            }
                        })
                        .child(
                            div().w(px(STATUS_DIFF_LINE_NUM_WIDTH)).h_full()
                                .flex().flex_row().items_center().bg(row_bg)
                                .border_r_1().border_color(bdr)
                                .child(div().w(px(STATUS_DIFF_LINE_NUM_WIDTH / 2.0)).text_xs().font_family("monospace").text_color(muted).pl_2().child(old_num))
                                .child(div().w(px(STATUS_DIFF_LINE_NUM_WIDTH / 2.0)).text_xs().font_family("monospace").text_color(muted).pl_1().child(new_num))
                        )
                        .child(
                            div().w(px(14.0)).h_full().flex().items_center().justify_center()
                                .text_xs().font_family("monospace").text_color(line_fg)
                                .child(prefix.to_string())
                        )
                        .child(
                            div().flex_1().text_xs().font_family("monospace").text_color(line_fg).pr_3().overflow_hidden()
                                .child(display)
                        );

                    rows.push(row.into_any_element());
                }
                rows
            },
        )
        .track_scroll(self.scroll_handle.clone());

        div()
            .flex_1().h_full().bg(surface)
            .flex().flex_col()
            .child(file_header)
            .child(div().flex_1().child(diff_lines))
    }

    fn render_commit_editor(
        &self,
        colors: &AppColors,
        entity: WeakEntity<super::app::GitForgeApp>,
        window: &mut Window,
        ai_generating: bool,
    ) -> Stateful<Div> {
        let surface = rgba_to_hsla(colors.surface);
        let border = rgba_to_hsla(colors.border);
        let muted = rgba_to_hsla(colors.text_muted);
        let text_color = rgba_to_hsla(colors.text);
        let accent = rgba_to_hsla(colors.accent);
        let bg = rgba_to_hsla(colors.background);

        let is_focused = self.commit_message_focus.is_focused(window);
        let display_text = if self.commit_message.is_empty() && !is_focused {
            String::from("Enter commit message...")
        } else {
            let mut t = self.commit_message.clone();
            if is_focused && !t.ends_with('\n') {
                t.push('\u{2502}');
            }
            t
        };
        let display_color = if self.commit_message.is_empty() && !is_focused {
            muted
        } else {
            text_color
        };
        let border_color = if is_focused { accent } else { border };
        let fh = self.commit_message_focus.clone();

        let ent1 = entity.clone();
        let ent2 = entity.clone();
        let ent3 = entity.clone();
        let ent4 = entity.clone();
        let has_message = !self.commit_message.trim().is_empty();

        let generate_label = if ai_generating { "Generating..." } else { "Generate" };
        let generate_color = if ai_generating { muted } else { accent };

        let mut editor = div()
            .id("commit-editor-panel")
            .flex_1().h_full().bg(surface)
            .flex().flex_col()
            .child(
                div().px_3().py_2()
                    .border_b_1().border_color(border)
                    .text_xs().font_weight(FontWeight::BOLD).text_color(muted)
                    .child("COMMIT")
            );

        if self.ai_message_alternatives.len() > 1 {
            let mut alt_row = div().px_3().py_1().border_b_1().border_color(border)
                .flex().flex_wrap().gap_1();
            for (i, alt) in self.ai_message_alternatives.iter().enumerate() {
                let ent_alt = entity.clone();
                let first_line = alt.lines().next().unwrap_or(alt);
                let label = if first_line.len() > 40 {
                    format!("{}...", &first_line[..37])
                } else {
                    first_line.to_string()
                };
                let is_selected = self.commit_message.lines().next().unwrap_or("") == alt.lines().next().unwrap_or("");
                let pill_bg = if is_selected { accent } else { surface };
                let pill_tc = if is_selected { rgba_to_hsla(colors.background) } else { text_color };
                let pill_bc = if is_selected { accent } else { border };
                alt_row = alt_row.child(
                    div()
                        .id(ElementId::Name(format!("ai-alt-{}", i).into()))
                        .px_2().py(px(1.0))
                        .border_1().border_color(pill_bc)
                        .rounded(px(3.0))
                        .bg(pill_bg)
                        .cursor_pointer()
                        .text_xs().text_color(pill_tc)
                        .child(label)
                        .on_click(move |_ev, _window, cx| {
                            if let Some(e) = ent_alt.upgrade() {
                                let idx = i;
                                e.update(cx, |this, cx| {
                                    this.select_ai_alternative(idx, cx);
                                });
                            }
                        })
                );
            }
            editor = editor.child(alt_row);
        }

        editor = editor            .child(
                div()
                    .id("commit-msg-input")
                    .track_focus(&self.commit_message_focus)
                    .m_3().p_2()
                    .min_h(px(120.0))
                    .border_1().border_color(border_color)
                    .rounded(px(4.0))
                    .bg(bg)
                    .on_click(move |_ev, window, _cx| {
                        window.focus(&fh);
                    })
                    .on_key_down(move |ev: &KeyDownEvent, _window, cx| {
                        let key = &ev.keystroke.key;
                        match key.as_str() {
                            "backspace" => {
                                if let Some(e) = ent1.upgrade() {
                                    e.update(cx, |this, cx| {
                                        this.edit_commit_message(None, cx);
                                    });
                                }
                            }
                            "enter" => {
                                if let Some(e) = ent1.upgrade() {
                                    let ch = ev.keystroke.key_char.clone();
                                    e.update(cx, |this, cx| {
                                        if let Some(c) = ch {
                                            this.edit_commit_message(Some(&c), cx);
                                        } else {
                                            this.edit_commit_message(Some("\n"), cx);
                                        }
                                    });
                                }
                            }
                            "escape" => {
                                if let Some(e) = ent1.upgrade() {
                                    e.update(cx, |this, cx| {
                                        this.cancel_commit_dialog(cx);
                                    });
                                }
                            }
                            _ => {
                                let ch = ev.keystroke.key_char.clone();
                                if let Some(typed) = ch {
                                    if !ev.keystroke.modifiers.platform {
                                        if let Some(e) = ent1.upgrade() {
                                            let c = typed;
                                            e.update(cx, |this, cx| {
                                                this.edit_commit_message(Some(&c), cx);
                                            });
                                        }
                                    }
                                }
                            }
                        }
                    })
                    .child(
                        div().text_sm().font_family("monospace").text_color(display_color)
                            .child(display_text)
                    )
            )
            .child(div().flex_1())
            .child(
                div().px_3().py_2().border_t_1().border_color(border)
                    .flex().gap_2()
                    .child({
                        let ent_gen = ent4.clone();
                        div()
                            .id("ai-generate-btn")
                            .px_3().py_1()
                            .rounded(px(4.0))
                            .border_1().border_color(generate_color)
                            .cursor_pointer()
                            .text_xs().text_color(generate_color)
                            .child(generate_label)
                            .on_click(move |_ev, _window, cx| {
                                if let Some(e) = ent_gen.upgrade() {
                                    e.update(cx, |this, cx| {
                                        this.generate_commit_message(cx);
                                    });
                                }
                            })
                    })
                    .child({
                        let btn_bg = if has_message { accent } else { muted };
                        div()
                            .id("submit-commit-btn")
                            .px_4().py_1()
                            .rounded(px(4.0))
                            .bg(btn_bg)
                            .cursor_pointer()
                            .text_xs().text_color(rgba_to_hsla(colors.background))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Commit")
                            .on_click(move |_ev, _window, cx| {
                                if has_message {
                                    if let Some(e) = ent2.upgrade() {
                                        e.update(cx, |this, cx| {
                                            this.perform_commit(false, cx);
                                        });
                                    }
                                }
                            })
                    })
                    .child({
                        div()
                            .id("amend-commit-btn")
                            .px_4().py_1()
                            .rounded(px(4.0))
                            .border_1().border_color(border)
                            .cursor_pointer()
                            .text_xs().text_color(text_color)
                            .child("Amend")
                            .on_click(move |_ev, _window, cx| {
                                if has_message {
                                    if let Some(e) = ent3.upgrade() {
                                        e.update(cx, |this, cx| {
                                            this.perform_commit(true, cx);
                                        });
                                    }
                                }
                            })
                    })
            );

        editor
    }

    fn render_placeholder(&self, colors: &AppColors) -> Div {
        let surface = rgba_to_hsla(colors.surface);
        let muted = rgba_to_hsla(colors.text_muted);
        div()
            .flex_1().h_full().bg(surface)
            .flex().items_center().justify_center()
            .child(div().text_sm().text_color(muted).child("Select a file to view changes"))
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
) -> Stateful<Div> {
    let surface = rgba_to_hsla(colors.surface);
    let selected_bg = rgba_to_hsla(colors.sidebar_selected);
    let bg = if is_selected { selected_bg } else { surface };
     let text_color = if is_selected { rgba_to_hsla(colors.text) } else { rgba_to_hsla(colors.text_muted) };

     let (status_char, status_color) = status_badge(&entry.status, colors);

    let ent = entity.clone();
    let path_owned = entry.path.clone();

    let action_ent = entity.clone();
    let action_path = entry.path.clone();
    let action_section = section;

    let (action_label, action_color) = match section {
        StatusFileSection::Staged => ("\u{2212}", rgba_to_hsla(colors.diff_removed)),
        _ => ("+", rgba_to_hsla(colors.diff_added)),
    };

    let show_discard = matches!(section, StatusFileSection::Unstaged);
    let show_remove = matches!(section, StatusFileSection::Untracked);
    let show_gitignore = matches!(section, StatusFileSection::Untracked);
    let show_conflict_actions = matches!(section, StatusFileSection::Conflicted);

    let mut entry_row = div()
        .id(ElementId::Name(format!("status-file-{:?}-{}", section, idx).into()))
        .w_full()
        .px_2().py_1()
        .bg(bg)
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

    let show_ai_summary = matches!(section, StatusFileSection::Staged | StatusFileSection::Unstaged);
    let ai_ent = entity.clone();
    let ai_path = entry.path.clone();

    let mut inner_row = div().flex().items_center().gap_1()
        .child(
            div()
                .w(px(16.0)).h(px(16.0))
                .flex().items_center().justify_center()
                .rounded(px(2.0))
                .bg(status_color)
                .text_xs().font_weight(FontWeight::BOLD)
                .text_color(rgba_to_hsla(colors.background))
                .child(status_char.clone())
        )
        .child(
            div().flex_1().text_xs().text_color(text_color)
                .overflow_hidden().text_ellipsis()
                .child(entry.path.clone())
        );

    if show_ai_summary {
        inner_row = inner_row.child(
            div()
                .id(ElementId::Name(format!("ai-summary-{:?}-{}", section, idx).into()))
                .px(px(2.0)).py(px(0.0))
                .rounded(px(2.0))
                .cursor_pointer()
                .text_xs().text_color(rgba_to_hsla(colors.accent))
                .child("AI")
                .on_click(move |_ev, _window, cx| {
                    if let Some(e) = ai_ent.upgrade() {
                        let p = ai_path.clone();
                        e.update(cx, |this, cx| {
                            this.summarize_file_diff(p, cx);
                        });
                    }
                })
        );
    }

    inner_row = inner_row.child(
        div()
            .id(ElementId::Name(format!("action-{:?}-{}", section, idx).into()))
            .w(px(18.0)).h(px(18.0))
            .flex().items_center().justify_center()
            .rounded(px(2.0))
            .border_1().border_color(action_color)
            .cursor_pointer()
            .text_xs().font_weight(FontWeight::BOLD)
            .text_color(action_color)
            .child(action_label.to_string())
            .on_click(move |_ev, _window, cx| {
                if let Some(e) = action_ent.upgrade() {
                    let p = action_path.clone();
                    e.update(cx, |this, cx| {
                        match action_section {
                            StatusFileSection::Staged => this.unstage_file(p, cx),
                            StatusFileSection::Conflicted => this.stage_file(p, cx),
                            _ => this.stage_file(p, cx),
                        }
                    });
                }
            })
    );

    entry_row = entry_row.child(inner_row);

    if show_discard {
        let discard_ent = entity.clone();
        let discard_path = entry.path.clone();
        entry_row = entry_row.child(
            div()
                .id(ElementId::Name(format!("discard-{:?}-{}", section, idx).into()))
                .w(px(18.0)).h(px(18.0))
                .flex().items_center().justify_center()
                .rounded(px(2.0))
                .border_1().border_color(rgba_to_hsla(colors.diff_removed))
                .cursor_pointer()
                .text_xs().text_color(rgba_to_hsla(colors.diff_removed))
                .child("\u{00d7}")
                .on_click(move |_ev, _window, cx| {
                    if let Some(e) = discard_ent.upgrade() {
                        let p = discard_path.clone();
                        e.update(cx, |this, cx| {
                            this.discard_file(p, cx);
                        });
                    }
                })
        );
    }

    if show_remove {
        let remove_ent = entity.clone();
        let remove_path = entry.path.clone();
        entry_row = entry_row.child(
            div()
                .id(ElementId::Name(format!("remove-{:?}-{}", section, idx).into()))
                .w(px(18.0)).h(px(18.0))
                .flex().items_center().justify_center()
                .rounded(px(2.0))
                .border_1().border_color(rgba_to_hsla(colors.diff_removed))
                .cursor_pointer()
                .text_xs().text_color(rgba_to_hsla(colors.diff_removed))
                .child("\u{00d7}")
                .on_click(move |_ev, _window, cx| {
                    if let Some(e) = remove_ent.upgrade() {
                        let p = remove_path.clone();
                        e.update(cx, |this, cx| {
                            this.remove_untracked_file(p, cx);
                        });
                    }
                })
        );
    }

    if show_gitignore {
        let gitignore_ent = entity.clone();
        let gitignore_path = entry.path.clone();
        entry_row = entry_row.child(
            div()
                .id(ElementId::Name(format!("gitignore-{:?}-{}", section, idx).into()))
                .px_1()
                .rounded(px(2.0))
                .border_1().border_color(rgba_to_hsla(colors.border))
                .cursor_pointer()
                .text_xs().text_color(rgba_to_hsla(colors.text_muted))
                .child("ign")
                .on_click(move |_ev, _window, cx| {
                    if let Some(e) = gitignore_ent.upgrade() {
                        let p = gitignore_path.clone();
                        e.update(cx, |this, cx| {
                            this.add_to_gitignore(p, cx);
                        });
                    }
                })
        );
    }

    if show_conflict_actions {
        let ours_ent = entity.clone();
        let theirs_ent = entity.clone();
        let conflict_path = entry.path.clone();
        let conflict_path2 = entry.path.clone();
        entry_row = entry_row.child(
            div().flex().gap_0().ml_1()
                .child(
                    div()
                        .id(ElementId::Name(format!("use-ours-{}", idx).into()))
                        .px_1()
                        .rounded(px(2.0))
                        .border_1().border_color(rgba_to_hsla(colors.accent))
                        .cursor_pointer()
                        .text_xs().text_color(rgba_to_hsla(colors.accent))
                        .child("Ours")
                        .on_click(move |_ev, _window, cx| {
                            if let Some(e) = ours_ent.upgrade() {
                                let p = conflict_path.clone();
                                e.update(cx, |this, cx| {
                                    this.resolve_conflict_ours(p, cx);
                                });
                            }
                        })
                )
                .child(
                    div()
                        .id(ElementId::Name(format!("use-theirs-{}", idx).into()))
                        .px_1()
                        .rounded(px(2.0))
                        .border_1().border_color(rgba_to_hsla(colors.warning))
                        .cursor_pointer()
                        .text_xs().text_color(rgba_to_hsla(colors.warning))
                        .child("Theirs")
                        .on_click(move |_ev, _window, cx| {
                            if let Some(e) = theirs_ent.upgrade() {
                                let p = conflict_path2.clone();
                                e.update(cx, |this, cx| {
                                    this.resolve_conflict_theirs(p, cx);
                                });
                            }
                        })
                )
        );
    }

    entry_row
}

fn render_ai_summary_popup(
    summary: &str,
    colors: &AppColors,
    entity: WeakEntity<super::app::GitForgeApp>,
) -> Stateful<Div> {
    let surface = rgba_to_hsla(colors.surface);
    let _border = rgba_to_hsla(colors.border);
    let text_color = rgba_to_hsla(colors.text);
    let accent = rgba_to_hsla(colors.accent);
    let muted = rgba_to_hsla(colors.text_muted);
    let ent = entity.clone();

    div()
        .id("ai-summary-popup")
        .absolute()
        .left(px(0.0)).top(px(24.0))
        .w(px(280.0))
        .bg(surface)
        .border_1().border_color(accent)
        .rounded(px(4.0))
        .p_2()
        .child(
            div().flex().items_center().gap_1().mb_1()
                .child(div().text_xs().font_weight(FontWeight::BOLD).text_color(accent).child("AI Summary"))
                .child(div().flex_1())
                .child(
                    div()
                        .id("ai-summary-close")
                        .px_1().cursor_pointer()
                        .text_xs().text_color(muted)
                        .child("x")
                        .on_click(move |_ev, _window, cx| {
                            if let Some(e) = ent.upgrade() {
                                e.update(cx, |this, cx| {
                                    this.dismiss_file_summary(cx);
                                });
                            }
                        })
                )
        )
        .child(
            div().text_xs().text_color(text_color).child(summary.to_string())
        )
}

fn status_badge(status: &FileStatus, colors: &AppColors) -> (String, Hsla) {
    match status {
        FileStatus::Modified => ("M".to_string(), rgba_to_hsla(colors.warning)),
        FileStatus::Added => ("A".to_string(), rgba_to_hsla(colors.diff_added)),
        FileStatus::Deleted => ("D".to_string(), rgba_to_hsla(colors.diff_removed)),
        FileStatus::Renamed => ("R".to_string(), rgba_to_hsla(colors.accent)),
        FileStatus::Copied => ("C".to_string(), rgba_to_hsla(colors.accent)),
        FileStatus::Untracked => ("?".to_string(), rgba_to_hsla(colors.text_muted)),
        FileStatus::Conflicted => ("!".to_string(), rgba_to_hsla(colors.diff_removed)),
        FileStatus::Unmodified | FileStatus::Ignored => (" ".to_string(), rgba_to_hsla(colors.text_muted)),
    }
}
