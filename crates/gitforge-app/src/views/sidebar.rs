use gitforge_git::{CommitInfo, RefInfo, RefKind, RepoState, WorktreeInfo};
use gitforge_ui::{
    AppColors, TextInput, TextInputEvent, TextInputRenderOpts, WidgetColors, collapsible_header,
    entity_on_click, entity_on_click_stop_propagation, ghost_button, parse_key_event,
    render_text_input, rgba_to_hsla,
};
use gpui::*;
use std::collections::{HashMap, HashSet};

use super::layout::SIDEBAR_WIDTH;
use super::settings::AppSettings;
const ROW_HEIGHT: f32 = 24.0;
const TAG_VISIBLE_LIMIT: usize = 6;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContextMenuAction {
    Checkout(String),
    DeleteBranch(String),
    RenameBranch(String),
    MergeBranch(String),
    CreateBranchFrom(String),
    DeleteTag(String),
    CheckoutRemote(String),
    FilterToBranch(String),
    RemoteHeader(String),
    FetchRemote(String),
    RemoveRemote(String),
    WorktreesHeader,
    PruneWorktrees,
    None,
}

/// Snapshot of the sidebar's expand/collapse state. Used both for per-tab
/// restoration (via `TabSnapshot`) and as the unit the owner reads/writes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SidebarExpansion {
    pub branches: bool,
    pub remotes: bool,
    pub tags: bool,
    pub worktrees: bool,
    pub pull_requests: bool,
    pub expanded_remotes: HashSet<String>,
}

pub struct SidebarState {
    pub branches_expanded: bool,
    pub remotes_expanded: bool,
    pub tags_expanded: bool,
    pub worktrees_expanded: bool,
    pub pull_requests_expanded: bool,
    pub expanded_remotes: HashSet<String>,
    pub filter_input: TextInput,
    pub context_menu: ContextMenuAction,
    pub context_menu_pos: (f32, f32),
}

impl SidebarState {
    pub fn new(cx: &mut App) -> Self {
        Self {
            branches_expanded: true,
            remotes_expanded: true,
            tags_expanded: true,
            worktrees_expanded: true,
            pull_requests_expanded: true,
            expanded_remotes: HashSet::new(),
            filter_input: TextInput::new("Filter...", cx),
            context_menu: ContextMenuAction::None,
            context_menu_pos: (0.0, 0.0),
        }
    }

    pub fn dismiss_context_menu(&mut self) {
        self.context_menu = ContextMenuAction::None;
    }

    pub fn set_context_menu(&mut self, action: ContextMenuAction, pos: (f32, f32)) {
        self.context_menu = action;
        self.context_menu_pos = pos;
    }

    pub fn toggle_branches(&mut self) {
        self.branches_expanded = !self.branches_expanded;
    }

    pub fn toggle_remotes(&mut self) {
        self.remotes_expanded = !self.remotes_expanded;
    }

    pub fn toggle_tags(&mut self) {
        self.tags_expanded = !self.tags_expanded;
    }

    pub fn toggle_worktrees(&mut self) {
        self.worktrees_expanded = !self.worktrees_expanded;
    }

    pub fn toggle_pull_requests(&mut self) {
        self.pull_requests_expanded = !self.pull_requests_expanded;
    }

    pub fn toggle_remote(&mut self, remote: String) {
        if self.expanded_remotes.contains(&remote) {
            self.expanded_remotes.remove(&remote);
        } else {
            self.expanded_remotes.insert(remote);
        }
    }

    pub fn update_filter(&mut self, typed_char: Option<&str>) {
        self.filter_input.edit(typed_char);
    }

    pub fn clear_filter(&mut self) {
        self.filter_input.clear();
    }

    /// Capture the full expansion state (all sections + per-remote) for
    /// per-tab snapshotting. Unlike the persisted slice, this includes
    /// `worktrees` and `expanded_remotes`.
    pub fn expansion(&self) -> SidebarExpansion {
        SidebarExpansion {
            branches: self.branches_expanded,
            remotes: self.remotes_expanded,
            tags: self.tags_expanded,
            worktrees: self.worktrees_expanded,
            pull_requests: self.pull_requests_expanded,
            expanded_remotes: self.expanded_remotes.clone(),
        }
    }

    pub fn apply_expansion(&mut self, exp: &SidebarExpansion) {
        self.branches_expanded = exp.branches;
        self.remotes_expanded = exp.remotes;
        self.tags_expanded = exp.tags;
        self.worktrees_expanded = exp.worktrees;
        self.pull_requests_expanded = exp.pull_requests;
        self.expanded_remotes = exp.expanded_remotes.clone();
    }

    /// Load the persisted subset of the expansion state from settings. Only
    /// branches/remotes/tags/pull_requests are persisted; `worktrees` and
    /// `expanded_remotes` are per-tab only and untouched here.
    pub fn apply_persisted_from_settings(&mut self, settings: &AppSettings) {
        self.branches_expanded = settings.sidebar_branches_expanded;
        self.remotes_expanded = settings.sidebar_remotes_expanded;
        self.tags_expanded = settings.sidebar_tags_expanded;
        self.pull_requests_expanded = settings.sidebar_pull_requests_expanded;
    }

    /// Write the persisted subset back into settings.
    pub fn write_persisted_to_settings(&self, settings: &mut AppSettings) {
        settings.sidebar_branches_expanded = self.branches_expanded;
        settings.sidebar_remotes_expanded = self.remotes_expanded;
        settings.sidebar_tags_expanded = self.tags_expanded;
        settings.sidebar_pull_requests_expanded = self.pull_requests_expanded;
    }

    pub fn seed_expanded_remotes(&mut self, repo_state: &RepoState) {
        if !self.expanded_remotes.is_empty() {
            return;
        }
        for rf in &repo_state.references {
            if rf.kind == RefKind::RemoteBranch {
                let remote = rf
                    .remote_name
                    .clone()
                    .unwrap_or_else(|| "origin".to_string());
                self.expanded_remotes.insert(remote);
            }
        }
    }
}

pub(crate) fn render_sidebar(
    repo_state: Option<&RepoState>,
    colors: &AppColors,
    loading: bool,
    state: &SidebarState,
    entity: WeakEntity<super::app::GitForgeApp>,
    window: &mut Window,
    pull_requests: &[gitforge_hosting::PullRequest],
    pull_requests_loading: bool,
    pull_request_hint: Option<super::ops::pr_ops::PullRequestSidebarHint>,
) -> Div {
    let sidebar_bg = rgba_to_hsla(colors.sidebar_background);
    let border = rgba_to_hsla(colors.border);
    let muted = rgba_to_hsla(colors.text_muted);
    let _text_color = rgba_to_hsla(colors.text);
    let _accent = rgba_to_hsla(colors.accent);

    let mut sidebar = div()
        .w(px(SIDEBAR_WIDTH))
        .h_full()
        .bg(sidebar_bg)
        .border_r_1()
        .border_color(border)
        .flex()
        .flex_col();

    sidebar = sidebar.child(
        div()
            .px_2()
            .py_1()
            .border_b_1()
            .border_color(border)
            .flex()
            .items_center()
            .gap_1()
            .child(
                div()
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .text_color(muted)
                    .child("REPOSITORY"),
            ),
    );

    match repo_state {
        Some(repo) => {
            let filter = state.filter_input.text().to_lowercase();

            let branches: Vec<&RefInfo> = repo
                .references
                .iter()
                .filter(|r| r.kind == RefKind::Branch)
                .filter(|r| filter.is_empty() || r.name.to_lowercase().contains(&filter))
                .collect();
            let remote_branches: Vec<&RefInfo> = repo
                .references
                .iter()
                .filter(|r| r.kind == RefKind::RemoteBranch)
                .filter(|r| filter.is_empty() || r.name.to_lowercase().contains(&filter))
                .collect();
            let mut tags: Vec<&RefInfo> = repo
                .references
                .iter()
                .filter(|r| r.kind == RefKind::Tag)
                .filter(|r| filter.is_empty() || r.name.to_lowercase().contains(&filter))
                .collect();
            sort_tags_newest_first(&mut tags, &repo.commits);

            sidebar = sidebar.child(render_search_bar(
                colors,
                &state.filter_input,
                entity.clone(),
                window,
            ));

            let branches_expanded = state.branches_expanded;
            sidebar = sidebar.child(render_collapsible_section_with_action(
                format!("BRANCHES ({})", branches.len()),
                branches_expanded,
                colors,
                "sidebar-branches".to_string(),
                entity.clone(),
                SectionToggle::Branches,
                "sidebar-create-branch",
                entity_on_click_stop_propagation(entity.clone(), |this, cx| {
                    this.open_create_branch_dialog(None, cx);
                }),
                None,
            ));

            if branches_expanded {
                for rf in &branches {
                    sidebar = sidebar.child(render_ref_item(rf, colors, "branch", entity.clone()));
                }
                if branches.is_empty() && filter.is_empty() {
                    sidebar = sidebar.child(render_empty_hint("No branches", muted));
                }
            }

            if !remote_branches.is_empty() || !filter.is_empty() {
                let remotes_expanded = state.remotes_expanded;
                sidebar = sidebar.child(render_collapsible_section(
                    format!("REMOTES ({})", remote_branches.len()),
                    remotes_expanded,
                    colors,
                    "sidebar-remotes".to_string(),
                    entity.clone(),
                    SectionToggle::Remotes,
                ));

                if remotes_expanded {
                    let remote_groups = group_by_remote(&remote_branches);
                    let mut sorted_remotes: Vec<_> = remote_groups.keys().collect();
                    sorted_remotes.sort();

                    for remote_name in sorted_remotes {
                        let remote_refs = &remote_groups[remote_name];
                        let remote_expanded = state.expanded_remotes.contains(remote_name);

                        sidebar = sidebar.child(render_remote_group_header(
                            remote_name,
                            remote_refs.len(),
                            remote_expanded,
                            colors,
                            entity.clone(),
                        ));

                        if remote_expanded {
                            for rf in remote_refs {
                                sidebar = sidebar.child(render_ref_item(
                                    rf,
                                    colors,
                                    "remote-branch",
                                    entity.clone(),
                                ));
                            }
                        }
                    }
                    if remote_branches.is_empty() && filter.is_empty() {
                        sidebar = sidebar.child(render_empty_hint("No remote branches", muted));
                    }
                }
            }

            sidebar = sidebar.child(render_add_remote_button(colors, entity.clone()));

            let filtered_prs: Vec<&gitforge_hosting::PullRequest> = pull_requests
                .iter()
                .filter(|pr| {
                    filter.is_empty()
                        || pr.title.to_lowercase().contains(&filter)
                        || pr.number.to_string().contains(&filter)
                })
                .collect();

            sidebar = sidebar.child(render_pull_requests_header(
                pull_requests.len(),
                state.pull_requests_expanded,
                colors,
                entity.clone(),
            ));

            if state.pull_requests_expanded {
                if pull_requests_loading {
                    sidebar = sidebar.child(render_empty_hint("Loading...", muted));
                } else if let Some(hint) = pull_request_hint {
                    sidebar = sidebar.child(render_pull_request_hint(hint, muted));
                } else if filtered_prs.is_empty() {
                    sidebar = sidebar.child(render_empty_hint("No open pull requests", muted));
                } else {
                    for pr in &filtered_prs {
                        sidebar = sidebar.child(render_pr_item(pr, colors, entity.clone()));
                    }
                }
            }

            let tags_expanded = state.tags_expanded;
            sidebar = sidebar.child(render_collapsible_section_with_action(
                format!("TAGS ({})", tags.len()),
                tags_expanded,
                colors,
                "sidebar-tags".to_string(),
                entity.clone(),
                SectionToggle::Tags,
                "sidebar-create-tag",
                entity_on_click_stop_propagation(entity.clone(), |this, cx| {
                    this.open_create_tag_dialog(None, cx);
                }),
                None,
            ));

            if tags_expanded {
                if tags.is_empty() && filter.is_empty() {
                    sidebar = sidebar.child(render_empty_hint("No tags", muted));
                } else {
                    let mut tag_list = div()
                        .id("sidebar-tags-scroll")
                        .max_h(px(ROW_HEIGHT * TAG_VISIBLE_LIMIT as f32))
                        .overflow_y_scroll();
                    for rf in &tags {
                        tag_list =
                            tag_list.child(render_ref_item(rf, colors, "tag", entity.clone()));
                    }
                    sidebar = sidebar.child(tag_list);
                }
            }

            let worktrees = &repo.worktrees;
            let wt_count = worktrees.len();
            let worktrees_expanded = state.worktrees_expanded;
            sidebar = sidebar.child(render_collapsible_section_with_action(
                format!("WORKTREES ({})", wt_count),
                worktrees_expanded,
                colors,
                "sidebar-worktrees".to_string(),
                entity.clone(),
                SectionToggle::Worktrees,
                "sidebar-create-worktree",
                entity_on_click_stop_propagation(entity.clone(), |this, cx| {
                    this.open_create_worktree_dialog(cx);
                }),
                Some(ContextMenuAction::WorktreesHeader),
            ));

            if worktrees_expanded {
                if worktrees.is_empty() {
                    sidebar = sidebar.child(render_empty_hint("No worktrees", muted));
                } else {
                    for wt in worktrees {
                        sidebar = sidebar.child(render_worktree_item(wt, colors, entity.clone()));
                    }
                }
            }

            if !filter.is_empty()
                && branches.is_empty()
                && remote_branches.is_empty()
                && tags.is_empty()
                && filtered_prs.is_empty()
            {
                sidebar = sidebar.child(
                    div()
                        .p_2()
                        .child(div().text_xs().text_color(muted).child("No matches")),
                );
            }
        }
        None => {
            sidebar = sidebar.child(render_search_bar(
                colors,
                &state.filter_input,
                entity.clone(),
                window,
            ));
            sidebar = sidebar
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .pb_2()
                        .border_b_1()
                        .border_color(border)
                        .child(div().text_sm().text_color(muted).child("BRANCHES")),
                )
                .child(div().flex_1().flex().items_center().justify_center().child(
                    div().text_sm().text_color(muted).child(if loading {
                        "Loading..."
                    } else {
                        "No repository open"
                    }),
                ));
        }
    }

    sidebar
}

fn sort_tags_newest_first<'a>(tags: &mut Vec<&'a RefInfo>, commits: &[CommitInfo]) {
    let dates: HashMap<&str, chrono::DateTime<chrono::Utc>> = commits
        .iter()
        .map(|c| (c.id.as_str(), c.author_date))
        .collect();

    tags.sort_by(|a, b| {
        let date_a = dates.get(a.target_commit_id.as_str());
        let date_b = dates.get(b.target_commit_id.as_str());
        match (date_a, date_b) {
            (Some(da), Some(db)) => db.cmp(da),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => b.name.cmp(&a.name),
        }
    });
}

fn render_search_bar(
    colors: &AppColors,
    filter_input: &TextInput,
    entity: WeakEntity<super::app::GitForgeApp>,
    window: &mut Window,
) -> impl IntoElement {
    let border = rgba_to_hsla(colors.border);
    let surface = rgba_to_hsla(colors.surface);
    let ent = entity.clone();
    let opts = TextInputRenderOpts::new(ElementId::Name("sidebar-filter".into()))
        .text_xs()
        .background(surface);

    let input = render_text_input(filter_input, colors, window, &opts, |_| {}).on_key_down(
        move |ev, _window, cx| {
            if let Some(e) = ent.upgrade() {
                e.update(cx, |this, cx| match parse_key_event(ev) {
                    TextInputEvent::Backspace => this.update_sidebar_filter(None, cx),
                    TextInputEvent::Escape => this.clear_sidebar_filter(cx),
                    TextInputEvent::Typed(c) => this.update_sidebar_filter(Some(&c), cx),
                    _ => {}
                });
            }
        },
    );

    div()
        .px_2()
        .py_1()
        .border_b_1()
        .border_color(border)
        .child(input)
}

#[allow(dead_code)]
enum SectionToggle {
    Branches,
    Remotes,
    Tags,
    Worktrees,
    Remote(String),
}

fn render_collapsible_section(
    title: String,
    expanded: bool,
    colors: &AppColors,
    id: String,
    entity: WeakEntity<super::app::GitForgeApp>,
    toggle: SectionToggle,
) -> Stateful<Div> {
    collapsible_header(
        ElementId::Name(id.into()),
        &title,
        expanded,
        None,
        entity_on_click(entity, move |this, cx| match &toggle {
            SectionToggle::Branches => this.toggle_sidebar_branches(cx),
            SectionToggle::Remotes => this.toggle_sidebar_remotes(cx),
            SectionToggle::Tags => this.toggle_sidebar_tags(cx),
            SectionToggle::Worktrees => this.toggle_sidebar_worktrees(cx),
            SectionToggle::Remote(name) => this.toggle_sidebar_remote(name.clone(), cx),
        }),
        WidgetColors::from_app(colors),
    )
}

fn render_collapsible_section_with_action(
    title: String,
    expanded: bool,
    colors: &AppColors,
    id: String,
    entity: WeakEntity<super::app::GitForgeApp>,
    toggle: SectionToggle,
    action_id: &'static str,
    on_action: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    header_context_menu: Option<ContextMenuAction>,
) -> Stateful<Div> {
    let widget_colors = WidgetColors::from_app(colors);
    let accent = rgba_to_hsla(colors.accent);
    let arrow = if expanded { "▾" } else { "▸" };
    let ent_toggle = entity.clone();
    let ent_context = entity.clone();

    let mut header = div()
        .id(ElementId::Name(id.into()))
        .px_2()
        .py_1()
        .border_b_1()
        .border_color(widget_colors.border)
        .bg(widget_colors.surface_high)
        .flex()
        .items_center()
        .gap_1()
        .cursor_pointer()
        .on_click(move |_ev, _window, cx| {
            if let Some(e) = ent_toggle.upgrade() {
                e.update(cx, |this, cx| match &toggle {
                    SectionToggle::Branches => this.toggle_sidebar_branches(cx),
                    SectionToggle::Remotes => this.toggle_sidebar_remotes(cx),
                    SectionToggle::Tags => this.toggle_sidebar_tags(cx),
                    SectionToggle::Worktrees => this.toggle_sidebar_worktrees(cx),
                    SectionToggle::Remote(name) => this.toggle_sidebar_remote(name.clone(), cx),
                });
            }
        })
        .child(div().text_xs().text_color(widget_colors.muted).child(arrow))
        .child(
            div()
                .text_xs()
                .font_weight(FontWeight::BOLD)
                .text_color(widget_colors.muted)
                .child(title),
        )
        .child(div().flex_1())
        .child(ghost_button(action_id, "+", accent, on_action));

    if let Some(menu_action) = header_context_menu {
        header = header.on_mouse_down(
            MouseButton::Right,
            move |ev: &MouseDownEvent, _window, cx| {
                let pos = ev.position;
                let x: f32 = pos.x.into();
                let y: f32 = pos.y.into();
                if let Some(e) = ent_context.upgrade() {
                    e.update(cx, |this, cx| {
                        this.repo_session
                            .sidebar_state
                            .set_context_menu(menu_action.clone(), (x, y));
                        cx.notify();
                    });
                }
                cx.stop_propagation();
            },
        );
    }

    header
}

fn render_ref_item(
    rf: &RefInfo,
    colors: &AppColors,
    _kind: &str,
    entity: WeakEntity<super::app::GitForgeApp>,
) -> Stateful<Div> {
    let text_color = if rf.is_head {
        rgba_to_hsla(colors.accent)
    } else {
        rgba_to_hsla(colors.text_muted)
    };
    let bg = rgba_to_hsla(colors.sidebar_background);

    let display_name = if rf.kind == RefKind::RemoteBranch {
        rf.name
            .strip_prefix(&format!("{}/", rf.remote_name.as_deref().unwrap_or("")))
            .unwrap_or(&rf.name)
            .to_string()
    } else {
        rf.name.clone()
    };

    let prefix = if rf.is_head { "* " } else { "  " };
    let target_id = rf.target_commit_id.clone();

    let kind_str = match rf.kind {
        RefKind::Branch => "branch",
        RefKind::RemoteBranch => "remote",
        RefKind::Tag => "tag",
        _ => "other",
    };
    let elem_id = format!("sidebar-ref-{}-{}", kind_str, rf.name);

    let name_for_checkout3 = rf.name.clone();
    let name_for_delete2 = rf.name.clone();

    let ent_navigate = entity.clone();
    let ent_context = entity.clone();

    let is_branch = rf.kind == RefKind::Branch;
    let is_remote = rf.kind == RefKind::RemoteBranch;
    let is_tag = rf.kind == RefKind::Tag;
    let is_head = rf.is_head;

    let row = div()
        .id(ElementId::Name(elem_id.into()))
        .w_full()
        .h(px(ROW_HEIGHT))
        .px_2()
        .flex()
        .items_center()
        .bg(bg)
        .cursor_pointer()
        .hover(|s| s.bg(rgba_to_hsla(colors.sidebar_hover)))
        .on_click(move |_ev, _window, cx| {
            if let Some(e) = ent_navigate.upgrade() {
                let id = target_id.clone();
                e.update(cx, |this, cx| {
                    this.navigate_to_ref(id, cx);
                });
            }
        })
        .on_mouse_down(
            MouseButton::Right,
            move |ev: &MouseDownEvent, _window, cx| {
                let pos = ev.position;
                let x: f32 = pos.x.into();
                let y: f32 = pos.y.into();
                let action = if is_branch && !is_head {
                    ContextMenuAction::Checkout(name_for_checkout3.clone())
                } else if is_remote && !is_head {
                    ContextMenuAction::CheckoutRemote(name_for_checkout3.clone())
                } else if is_tag {
                    ContextMenuAction::DeleteTag(name_for_delete2.clone())
                } else {
                    ContextMenuAction::None
                };
                if action != ContextMenuAction::None {
                    if let Some(e) = ent_context.upgrade() {
                        e.update(cx, |this, cx| {
                            this.repo_session
                                .sidebar_state
                                .set_context_menu(action, (x, y));
                            cx.notify();
                        });
                    }
                    cx.stop_propagation();
                }
            },
        )
        .child(
            div()
                .text_sm()
                .text_color(text_color)
                .child(format!("{}{}", prefix, display_name)),
        );

    row
}

fn render_empty_hint(text: &str, muted: Hsla) -> Div {
    let t = text.to_string();
    div()
        .px_2()
        .py_1()
        .child(div().text_xs().text_color(muted).italic().child(t))
}

fn group_by_remote<'a>(
    refs: &[&'a RefInfo],
) -> std::collections::HashMap<String, Vec<&'a RefInfo>> {
    let mut groups: std::collections::HashMap<String, Vec<&RefInfo>> =
        std::collections::HashMap::new();
    for rf in refs {
        let remote = rf
            .remote_name
            .clone()
            .unwrap_or_else(|| "origin".to_string());
        groups.entry(remote).or_default().push(*rf);
    }
    groups
}

pub(super) fn render_context_menu_overlay(
    action: &ContextMenuAction,
    pos: (f32, f32),
    colors: &AppColors,
    entity: WeakEntity<super::app::GitForgeApp>,
) -> Stateful<Div> {
    let surface = rgba_to_hsla(colors.surface);
    let border = rgba_to_hsla(colors.border);
    let text_color = rgba_to_hsla(colors.text);
    let _muted = rgba_to_hsla(colors.text_muted);
    let warning = rgba_to_hsla(colors.warning);

    let items = match action {
        ContextMenuAction::Checkout(name) => vec![
            ("Checkout", ContextMenuAction::Checkout(name.clone())),
            ("Merge", ContextMenuAction::MergeBranch(name.clone())),
            (
                "Create Branch From...",
                ContextMenuAction::CreateBranchFrom(name.clone()),
            ),
            ("Rename", ContextMenuAction::RenameBranch(name.clone())),
            ("Delete", ContextMenuAction::DeleteBranch(name.clone())),
            (
                "Filter Graph",
                ContextMenuAction::FilterToBranch(name.clone()),
            ),
        ],
        ContextMenuAction::CheckoutRemote(name) => {
            vec![("Checkout", ContextMenuAction::CheckoutRemote(name.clone()))]
        }
        ContextMenuAction::DeleteTag(name) => {
            vec![("Delete Tag", ContextMenuAction::DeleteTag(name.clone()))]
        }
        ContextMenuAction::RemoteHeader(name) => vec![
            ("Fetch", ContextMenuAction::FetchRemote(name.clone())),
            (
                "Remove remote",
                ContextMenuAction::RemoveRemote(name.clone()),
            ),
        ],
        ContextMenuAction::WorktreesHeader => {
            vec![("Prune stale", ContextMenuAction::PruneWorktrees)]
        }
        _ => vec![],
    };

    let menu_dismiss_ent = entity.clone();
    let mut menu = div()
        .id("context-menu")
        .occlude()
        .bg(surface)
        .border_1()
        .border_color(border)
        .rounded(px(4.0))
        .min_w(px(160.0))
        .shadow(vec![BoxShadow {
            color: black().opacity(0.35),
            offset: point(px(0.0), px(4.0)),
            blur_radius: px(12.0),
            spread_radius: px(0.0),
        }])
        .on_mouse_move(|_, _, cx| cx.stop_propagation())
        .on_mouse_down(MouseButton::Left, |_ev, _window, cx| {
            cx.stop_propagation();
        })
        .on_mouse_down(MouseButton::Right, |_ev, _window, cx| {
            cx.stop_propagation();
        })
        .on_click(move |_ev, _window, cx| {
            cx.stop_propagation();
            if let Some(e) = menu_dismiss_ent.upgrade() {
                e.update(cx, |this, cx| {
                    this.repo_session.sidebar_state.dismiss_context_menu();
                    cx.notify();
                });
            }
        });

    for (idx, (label, menu_action)) in items.into_iter().enumerate() {
        let item_color = match &menu_action {
            ContextMenuAction::DeleteBranch(_)
            | ContextMenuAction::DeleteTag(_)
            | ContextMenuAction::RemoveRemote(_) => warning,
            _ => text_color,
        };
        let item_ent = entity.clone();
        let item_id = format!("ctx-menu-item-{}", idx);
        menu = menu.child(
            div()
                .id(ElementId::Name(item_id.into()))
                .px_3()
                .py_1()
                .cursor_pointer()
                .text_xs()
                .text_color(item_color)
                .hover(|s| s.bg(rgba_to_hsla(colors.sidebar_hover)))
                .on_mouse_move(|_, _, cx| cx.stop_propagation())
                .child(label.to_string())
                .on_click(move |_ev, _window, cx| {
                    cx.stop_propagation();
                    if let Some(e) = item_ent.upgrade() {
                        e.update(cx, |this, cx| {
                            match &menu_action {
                                ContextMenuAction::Checkout(n) => {
                                    this.checkout_branch(n.clone(), cx)
                                }
                                ContextMenuAction::DeleteBranch(n) => {
                                    this.open_delete_branch_dialog(n.clone(), false, cx)
                                }
                                ContextMenuAction::RenameBranch(n) => {
                                    this.open_rename_branch_dialog(n.clone(), cx)
                                }
                                ContextMenuAction::MergeBranch(n) => {
                                    this.merge_branch(n.clone(), false, cx)
                                }
                                ContextMenuAction::CreateBranchFrom(n) => {
                                    this.open_create_branch_dialog(Some(n.clone()), cx)
                                }
                                ContextMenuAction::DeleteTag(n) => this.delete_tag(n.clone(), cx),
                                ContextMenuAction::CheckoutRemote(n) => {
                                    this.checkout_remote_branch(n.clone(), cx)
                                }
                                ContextMenuAction::FilterToBranch(n) => {
                                    this.set_branch_filter(Some(n.clone()), cx)
                                }
                                ContextMenuAction::FetchRemote(n) => {
                                    this.fetch_remote(n.clone(), cx)
                                }
                                ContextMenuAction::RemoveRemote(n) => {
                                    this.remove_remote(n.clone(), cx)
                                }
                                ContextMenuAction::PruneWorktrees => this.prune_worktrees(cx),
                                _ => {}
                            }
                            this.repo_session.sidebar_state.dismiss_context_menu();
                            cx.notify();
                        });
                    }
                }),
        );
    }

    let overlay_dismiss_ent = entity;
    div()
        .id("context-menu-overlay")
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .occlude()
        .on_mouse_move(|_, _, cx| cx.stop_propagation())
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation())
        .on_click(move |_ev, _window, cx| {
            cx.stop_propagation();
            if let Some(e) = overlay_dismiss_ent.upgrade() {
                e.update(cx, |this, cx| {
                    this.repo_session.sidebar_state.dismiss_context_menu();
                    cx.notify();
                });
            }
        })
        .child(
            anchored()
                .position(point(px(pos.0), px(pos.1)))
                .position_mode(AnchoredPositionMode::Window)
                .child(menu),
        )
}

fn render_remote_group_header(
    remote_name: &str,
    count: usize,
    expanded: bool,
    colors: &AppColors,
    entity: WeakEntity<super::app::GitForgeApp>,
) -> Stateful<Div> {
    let border = rgba_to_hsla(colors.border);
    let muted = rgba_to_hsla(colors.text_muted);
    let surface_high = rgba_to_hsla(colors.surface_high);
    let arrow = if expanded { "▾" } else { "▸" };
    let title = format!("{} ({})", remote_name, count);
    let id = format!("sidebar-remote-{}", remote_name);

    let name_toggle = remote_name.to_string();
    let name_context = remote_name.to_string();
    let ent_toggle = entity.clone();
    let ent_context = entity.clone();

    div()
        .id(ElementId::Name(id.into()))
        .px_2()
        .py_1()
        .border_b_1()
        .border_color(border)
        .bg(surface_high)
        .flex()
        .items_center()
        .gap_1()
        .cursor_pointer()
        .on_click(move |_ev, _window, cx| {
            if let Some(e) = ent_toggle.upgrade() {
                let name = name_toggle.clone();
                e.update(cx, |this, cx| {
                    this.toggle_sidebar_remote(name, cx);
                });
            }
        })
        .on_mouse_down(
            MouseButton::Right,
            move |ev: &MouseDownEvent, _window, cx| {
                let pos = ev.position;
                let x: f32 = pos.x.into();
                let y: f32 = pos.y.into();
                if let Some(e) = ent_context.upgrade() {
                    let name = name_context.clone();
                    e.update(cx, |this, cx| {
                        this.repo_session
                            .sidebar_state
                            .set_context_menu(ContextMenuAction::RemoteHeader(name), (x, y));
                        cx.notify();
                    });
                }
                cx.stop_propagation();
            },
        )
        .child(div().text_xs().text_color(muted).child(arrow))
        .child(
            div()
                .text_xs()
                .font_weight(FontWeight::BOLD)
                .text_color(muted)
                .child(title),
        )
}

fn render_pull_requests_header(
    count: usize,
    expanded: bool,
    colors: &AppColors,
    entity: WeakEntity<super::app::GitForgeApp>,
) -> Stateful<Div> {
    let border = rgba_to_hsla(colors.border);
    let muted = rgba_to_hsla(colors.text_muted);
    let accent = rgba_to_hsla(colors.accent);
    let surface_high = rgba_to_hsla(colors.surface_high);
    let arrow = if expanded { "▾" } else { "▸" };
    let ent_toggle = entity.clone();
    let ent_create = entity;

    div()
        .id("sidebar-pull-requests")
        .px_2()
        .py_1()
        .border_b_1()
        .border_color(border)
        .bg(surface_high)
        .flex()
        .items_center()
        .gap_1()
        .cursor_pointer()
        .on_click(move |_ev, _window, cx| {
            if let Some(e) = ent_toggle.upgrade() {
                e.update(cx, |this, cx| {
                    this.toggle_sidebar_pull_requests(cx);
                });
            }
        })
        .child(div().text_xs().text_color(muted).child(arrow))
        .child(
            div()
                .text_xs()
                .font_weight(FontWeight::BOLD)
                .text_color(muted)
                .child(format!("PULL REQUESTS ({count})")),
        )
        .child(div().flex_1())
        .child(ghost_button(
            "sidebar-create-pr",
            "+",
            accent,
            entity_on_click_stop_propagation(ent_create, |this, cx| this.open_create_pr_dialog(cx)),
        ))
}

fn render_pull_request_hint(hint: super::ops::pr_ops::PullRequestSidebarHint, muted: Hsla) -> Div {
    let text = match hint {
        super::ops::pr_ops::PullRequestSidebarHint::NoOrigin => "No supported origin remote",
        super::ops::pr_ops::PullRequestSidebarHint::UnsupportedProvider => {
            "No supported origin remote"
        }
        super::ops::pr_ops::PullRequestSidebarHint::NoAccount => {
            "Connect a GitHub, GitLab, or Codeberg account"
        }
    };
    render_empty_hint(text, muted)
}

fn render_pr_item(
    pr: &gitforge_hosting::PullRequest,
    colors: &AppColors,
    entity: WeakEntity<super::app::GitForgeApp>,
) -> Stateful<Div> {
    let text_color = rgba_to_hsla(colors.text);
    let muted = rgba_to_hsla(colors.text_muted);
    let bg = rgba_to_hsla(colors.sidebar_background);
    let url = pr.html_url.clone();
    let mut label = format!("#{} {}", pr.number, pr.title);
    if pr.draft {
        label.push_str(" (draft)");
    }
    let branch_suffix = pr.head_branch.clone();

    let entity_click = entity.clone();
    let mut row = div()
        .id(ElementId::Name(format!("sidebar-pr-{}", pr.number).into()))
        .px_2()
        .h(px(ROW_HEIGHT))
        .flex()
        .items_center()
        .gap_1()
        .bg(bg)
        .cursor_pointer()
        .hover(|s| s.bg(rgba_to_hsla(colors.sidebar_hover)))
        .on_click(move |_ev, _window, cx| {
            if let Some(e) = entity_click.upgrade() {
                e.update(cx, |this, _cx| {
                    this.open_in_browser(url.clone());
                });
            }
        })
        .child(
            div()
                .text_xs()
                .text_color(text_color)
                .overflow_hidden()
                .text_ellipsis()
                .child(label),
        );

    if let Some(branch) = branch_suffix {
        row = row.child(
            div()
                .text_xs()
                .text_color(muted)
                .flex_shrink_0()
                .child(branch),
        );
    }

    row
}

fn render_add_remote_button(
    colors: &AppColors,
    entity: WeakEntity<super::app::GitForgeApp>,
) -> Stateful<Div> {
    let accent = rgba_to_hsla(colors.accent);
    let muted = rgba_to_hsla(colors.text_muted);
    let ent = entity.clone();

    div()
        .id("sidebar-add-remote")
        .w_full()
        .h(px(ROW_HEIGHT))
        .px_2()
        .flex()
        .items_center()
        .cursor_pointer()
        .hover(|s| s.bg(rgba_to_hsla(colors.sidebar_hover)))
        .on_click(move |_ev, _window, cx| {
            if let Some(e) = ent.upgrade() {
                e.update(cx, |this, cx| {
                    this.open_add_remote_dialog(cx);
                });
            }
        })
        .child(div().text_xs().text_color(accent).child("+ "))
        .child(div().text_xs().text_color(muted).child("Add Remote"))
}

fn render_worktree_item(
    wt: &WorktreeInfo,
    colors: &AppColors,
    entity: WeakEntity<super::app::GitForgeApp>,
) -> Stateful<Div> {
    let bg = rgba_to_hsla(colors.sidebar_background);
    let text_color = if wt.is_current {
        rgba_to_hsla(colors.accent)
    } else {
        rgba_to_hsla(colors.text)
    };
    let muted = rgba_to_hsla(colors.text_muted);
    let border = rgba_to_hsla(colors.border);

    let path_display = wt
        .path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_else(|| wt.path.to_str().unwrap_or("?"))
        .to_string();

    let branch_label = if let Some(ref branch) = wt.branch {
        format!("({})", branch)
    } else if wt.is_detached {
        "(detached)".to_string()
    } else {
        String::new()
    };

    let current_prefix = if wt.is_current { "● " } else { "  " };
    let wt_path = wt.path.clone();
    let wt_path_remove = wt.path.clone();
    let wt_is_current = wt.is_current;
    let ent_switch = entity.clone();
    let ent_remove = entity.clone();

    let elem_id = format!("sidebar-worktree-{}", path_display);

    div()
        .id(ElementId::Name(elem_id.into()))
        .w_full()
        .h(px(ROW_HEIGHT))
        .px_2()
        .flex()
        .items_center()
        .bg(bg)
        .cursor_pointer()
        .hover(|s| s.bg(rgba_to_hsla(colors.sidebar_hover)))
        .on_click(move |_ev, _window, cx| {
            if let Some(e) = ent_switch.upgrade() {
                if !wt_is_current {
                    let p = wt_path.clone();
                    e.update(cx, |this, cx| {
                        this.switch_worktree(p, cx);
                    });
                }
            }
        })
        .child(
            div()
                .text_xs()
                .text_color(text_color)
                .child(format!("{}{}", current_prefix, path_display)),
        )
        .child(div().flex_1())
        .child(div().text_xs().text_color(muted).child(branch_label))
        .child({
            let ent_rm = ent_remove.clone();
            let rm_path = wt_path_remove.clone();
            div()
                .id(ElementId::Name(
                    format!("wt-remove-{}", path_display).into(),
                ))
                .ml_1()
                .px_1()
                .py_0()
                .rounded(px(2.0))
                .border_1()
                .border_color(border)
                .text_xs()
                .text_color(rgba_to_hsla(colors.warning))
                .cursor_pointer()
                .hover(|s| s.bg(rgba_to_hsla(colors.sidebar_hover)))
                .child("×")
                .on_click(move |_ev, _window, cx| {
                    if let Some(e) = ent_rm.upgrade() {
                        let p = rm_path.to_str().unwrap_or("").to_string();
                        e.update(cx, |this, cx| {
                            this.open_remove_worktree_dialog(p, cx);
                        });
                    }
                })
        })
}
