use gitforge_git::{RefInfo, RefKind, RepoState, WorktreeInfo};
use gitforge_ui::{AppColors, rgba_to_hsla};
use gpui::*;
use std::collections::HashSet;

use super::layout::SIDEBAR_WIDTH;
const ROW_HEIGHT: f32 = 24.0;

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
    None,
}

pub struct SidebarState {
    pub branches_expanded: bool,
    pub remotes_expanded: bool,
    pub tags_expanded: bool,
    pub worktrees_expanded: bool,
    pub expanded_remotes: HashSet<String>,
    pub search_filter: String,
    pub filter_focus: FocusHandle,
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
            expanded_remotes: HashSet::new(),
            search_filter: String::new(),
            filter_focus: cx.focus_handle(),
            context_menu: ContextMenuAction::None,
            context_menu_pos: (0.0, 0.0),
        }
    }

    pub fn dismiss_context_menu(&mut self) {
        self.context_menu = ContextMenuAction::None;
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

pub fn render_sidebar(
    repo_state: Option<&RepoState>,
    colors: &AppColors,
    loading: bool,
    state: &SidebarState,
    entity: WeakEntity<super::app::GitForgeApp>,
    window: &mut Window,
    hosting_accounts: &[gitforge_hosting::HostingAccount],
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
            let filter = state.search_filter.to_lowercase();

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
            let tags: Vec<&RefInfo> = repo
                .references
                .iter()
                .filter(|r| r.kind == RefKind::Tag)
                .filter(|r| filter.is_empty() || r.name.to_lowercase().contains(&filter))
                .collect();

            sidebar = sidebar.child(render_search_bar(
                colors,
                &state.search_filter,
                &state.filter_focus,
                entity.clone(),
                window,
            ));

            let branches_expanded = state.branches_expanded;
            sidebar = sidebar.child(render_collapsible_section(
                format!("BRANCHES ({})", branches.len()),
                branches_expanded,
                colors,
                "sidebar-branches".to_string(),
                entity.clone(),
                SectionToggle::Branches,
            ));

            if branches_expanded {
                for rf in &branches {
                    sidebar = sidebar.child(render_ref_item(rf, colors, "branch", entity.clone()));
                }
                if branches.is_empty() && filter.is_empty() {
                    sidebar = sidebar.child(render_empty_hint("No branches", muted));
                }
                sidebar = sidebar.child(render_create_branch_button(colors, entity.clone()));
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

            if !tags.is_empty() || !filter.is_empty() {
                let tags_expanded = state.tags_expanded;
                sidebar = sidebar.child(render_collapsible_section(
                    format!("TAGS ({})", tags.len()),
                    tags_expanded,
                    colors,
                    "sidebar-tags".to_string(),
                    entity.clone(),
                    SectionToggle::Tags,
                ));

                if tags_expanded {
                    for rf in &tags {
                        sidebar = sidebar.child(render_ref_item(rf, colors, "tag", entity.clone()));
                    }
                    if tags.is_empty() && filter.is_empty() {
                        sidebar = sidebar.child(render_empty_hint("No tags", muted));
                    }
                }
            }

            let worktrees = &repo.worktrees;
            let wt_count = worktrees.len();
            if wt_count > 0 {
                let worktrees_expanded = state.worktrees_expanded;
                sidebar = sidebar.child(render_collapsible_section(
                    format!("WORKTREES ({})", wt_count),
                    worktrees_expanded,
                    colors,
                    "sidebar-worktrees".to_string(),
                    entity.clone(),
                    SectionToggle::Worktrees,
                ));

                if worktrees_expanded {
                    for wt in worktrees {
                        sidebar = sidebar.child(render_worktree_item(wt, colors, entity.clone()));
                    }
                    sidebar = sidebar.child(render_create_worktree_button(colors, entity.clone()));
                    sidebar = sidebar.child(render_prune_worktrees_button(colors, entity.clone()));
                }
            }

            if !filter.is_empty()
                && branches.is_empty()
                && remote_branches.is_empty()
                && tags.is_empty()
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
                &state.search_filter,
                &state.filter_focus,
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

    if !hosting_accounts.is_empty() {
        let accent = rgba_to_hsla(colors.accent);
        let text_color = rgba_to_hsla(colors.text);
        let muted_color = rgba_to_hsla(colors.text_muted);
        let _border_color = rgba_to_hsla(colors.border);

        sidebar = sidebar.child(
            div()
                .px_2()
                .py_1()
                .border_t_1()
                .border_color(rgba_to_hsla(colors.border))
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .text_color(muted_color)
                        .child("ACCOUNTS"),
                )
                .child({
                    let ent = entity.clone();
                    div()
                        .id("sidebar-accounts-manage")
                        .px_1()
                        .cursor_pointer()
                        .text_xs()
                        .text_color(accent)
                        .child("Manage")
                        .on_click(move |_ev, _window, cx| {
                            if let Some(e) = ent.upgrade() {
                                e.update(cx, |this, cx| {
                                    this.open_manage_accounts_dialog(cx);
                                });
                            }
                        })
                }),
        );

        for account in hosting_accounts {
            let ent_open = entity.clone();
            let provider_click = account.provider.clone();
            let provider_id = account.provider.clone();
            let provider_label = account.provider.clone();
            let username = account.username.clone();
            let display = account.display_name.clone();

            let prov_color = match provider_label.as_str() {
                "github" => accent,
                "gitlab" => rgba_to_hsla(colors.accent_secondary),
                "codeberg" => rgba_to_hsla(colors.success),
                _ => muted_color,
            };

            sidebar = sidebar.child(
                div()
                    .id(ElementId::Name(
                        format!("sidebar-account-{}-{}", provider_id, username).into(),
                    ))
                    .px_2()
                    .py_0()
                    .flex()
                    .items_center()
                    .gap_2()
                    .cursor_pointer()
                    .hover(|s| s.bg(rgba_to_hsla(colors.sidebar_hover)))
                    .on_click(move |_ev, _window, cx| {
                        if let Some(e) = ent_open.upgrade() {
                            let p = provider_click.clone();
                            e.update(cx, |this, cx| {
                                this.open_search_hosting_dialog(p, cx);
                            });
                        }
                    })
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(prov_color)
                            .child(provider_label),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(text_color)
                            .overflow_hidden()
                            .child(display),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(muted_color)
                            .child(format!("@{}", username)),
                    ),
            );
        }
    }

    sidebar
}

fn render_search_bar(
    colors: &AppColors,
    current_value: &str,
    focus_handle: &FocusHandle,
    entity: WeakEntity<super::app::GitForgeApp>,
    window: &mut Window,
) -> Stateful<Div> {
    let border = rgba_to_hsla(colors.border);
    let muted = rgba_to_hsla(colors.text_muted);
    let surface = rgba_to_hsla(colors.surface);
    let text_color = rgba_to_hsla(colors.text);
    let is_focused = focus_handle.is_focused(window);

    let display_text = if current_value.is_empty() && !is_focused {
        String::from("Filter...")
    } else {
        let mut t = current_value.to_string();
        if is_focused {
            t.push('\u{2502}');
        }
        t
    };
    let display_color = if current_value.is_empty() && !is_focused {
        muted
    } else {
        text_color
    };

    let ent = entity.clone();
    let border_color = if is_focused {
        rgba_to_hsla(colors.accent)
    } else {
        border
    };
    let fh = focus_handle.clone();

    div()
        .id(ElementId::Name("sidebar-filter".into()))
        .track_focus(focus_handle)
        .px_2()
        .py_1()
        .border_b_1()
        .border_color(border)
        .on_click(move |_ev, window, _cx| {
            window.focus(&fh);
        })
        .on_key_down(move |ev: &KeyDownEvent, _window, cx| {
            let key = &ev.keystroke.key;
            match key.as_str() {
                "backspace" => {
                    if let Some(e) = ent.upgrade() {
                        e.update(cx, |this, cx| {
                            this.update_sidebar_filter(None, cx);
                        });
                    }
                }
                "escape" => {
                    if let Some(e) = ent.upgrade() {
                        e.update(cx, |this, cx| {
                            this.clear_sidebar_filter(cx);
                        });
                    }
                }
                _ => {
                    let ch = ev.keystroke.key_char.clone();
                    if let Some(typed) = ch {
                        if !ev.keystroke.modifiers.platform {
                            if let Some(e) = ent.upgrade() {
                                let c = typed;
                                e.update(cx, |this, cx| {
                                    this.update_sidebar_filter(Some(&c), cx);
                                });
                            }
                        }
                    }
                }
            }
        })
        .child(
            div()
                .w_full()
                .px_2()
                .py_1()
                .rounded(px(3.0))
                .border_1()
                .border_color(border_color)
                .bg(surface)
                .flex()
                .items_center()
                .child(
                    div()
                        .text_xs()
                        .text_color(display_color)
                        .child(display_text),
                ),
        )
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
    let border = rgba_to_hsla(colors.border);
    let muted = rgba_to_hsla(colors.text_muted);
    let surface_high = rgba_to_hsla(colors.surface_high);
    let arrow = if expanded { "▾" } else { "▸" };

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
            if let Some(e) = entity.upgrade() {
                e.update(cx, |this, cx| match &toggle {
                    SectionToggle::Branches => this.toggle_sidebar_branches(cx),
                    SectionToggle::Remotes => this.toggle_sidebar_remotes(cx),
                    SectionToggle::Tags => this.toggle_sidebar_tags(cx),
                    SectionToggle::Worktrees => this.toggle_sidebar_worktrees(cx),
                    SectionToggle::Remote(name) => this.toggle_sidebar_remote(name.clone(), cx),
                });
            }
        })
        .child(div().text_xs().text_color(muted).child(arrow))
        .child(
            div()
                .text_xs()
                .font_weight(FontWeight::BOLD)
                .text_color(muted)
                .child(title),
        )
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
                } else if is_remote {
                    ContextMenuAction::CheckoutRemote(name_for_checkout3.clone())
                } else if is_tag {
                    ContextMenuAction::DeleteTag(name_for_delete2.clone())
                } else {
                    ContextMenuAction::None
                };
                if action != ContextMenuAction::None {
                    if let Some(e) = ent_context.upgrade() {
                        e.update(cx, |this, cx| {
                            this.repo_session.sidebar_state.context_menu = action;
                            this.repo_session.sidebar_state.context_menu_pos = (x, y);
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

fn render_create_branch_button(
    colors: &AppColors,
    entity: WeakEntity<super::app::GitForgeApp>,
) -> Stateful<Div> {
    let accent = rgba_to_hsla(colors.accent);
    let muted = rgba_to_hsla(colors.text_muted);
    let ent = entity.clone();

    div()
        .id("sidebar-create-branch")
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
                    this.open_create_branch_dialog(None, cx);
                });
            }
        })
        .child(div().text_xs().text_color(accent).child("+ "))
        .child(div().text_xs().text_color(muted).child("New Branch"))
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
        _ => vec![],
    };

    let dismiss_ent = entity.clone();
    let mut menu = div()
        .id("context-menu")
        .absolute()
        .top(px(pos.1))
        .left(px(pos.0))
        .bg(surface)
        .border_1()
        .border_color(border)
        .rounded(px(4.0))
        .min_w(px(160.0))
        .on_mouse_down(MouseButton::Left, |_ev, _window, cx| {
            cx.stop_propagation();
        })
        .on_mouse_down(MouseButton::Right, |_ev, _window, cx| {
            cx.stop_propagation();
        })
        .on_click(move |_ev, _window, cx| {
            cx.stop_propagation();
            if let Some(e) = dismiss_ent.upgrade() {
                e.update(cx, |this, cx| {
                    this.repo_session.sidebar_state.dismiss_context_menu();
                    cx.notify();
                });
            }
        });

    for (idx, (label, menu_action)) in items.into_iter().enumerate() {
        let item_color = match &menu_action {
            ContextMenuAction::DeleteBranch(_) | ContextMenuAction::DeleteTag(_) => warning,
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
                .child(label.to_string())
                .on_click(move |_ev, _window, cx| {
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
                                    this.checkout_branch(n.clone(), cx)
                                }
                                ContextMenuAction::FilterToBranch(n) => {
                                    this.set_branch_filter(Some(n.clone()), cx)
                                }
                                _ => {}
                            }
                            this.repo_session.sidebar_state.dismiss_context_menu();
                            cx.notify();
                        });
                    }
                }),
        );
    }

    menu
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
    let warning = rgba_to_hsla(colors.warning);
    let arrow = if expanded { "▾" } else { "▸" };
    let title = format!("{} ({})", remote_name, count);
    let id = format!("sidebar-remote-{}", remote_name);

    let name_toggle = remote_name.to_string();
    let name_fetch = remote_name.to_string();
    let name_remove = remote_name.to_string();
    let ent_toggle = entity.clone();
    let ent_fetch = entity.clone();
    let ent_remove = entity.clone();

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
        .child(div().text_xs().text_color(muted).child(arrow))
        .child(
            div()
                .text_xs()
                .font_weight(FontWeight::BOLD)
                .text_color(muted)
                .child(title),
        )
        .child(div().flex_1())
        .child(
            div()
                .id(ElementId::Name(
                    format!("remote-fetch-{}", remote_name).into(),
                ))
                .px_1()
                .py_0()
                .rounded(px(2.0))
                .border_1()
                .border_color(border)
                .text_xs()
                .text_color(muted)
                .cursor_pointer()
                .hover(|s| s.bg(rgba_to_hsla(colors.sidebar_hover)))
                .child("F")
                .on_click(move |_ev, _window, cx| {
                    if let Some(e) = ent_fetch.upgrade() {
                        let name = name_fetch.clone();
                        e.update(cx, |this, cx| {
                            this.fetch_remote(name, cx);
                        });
                    }
                }),
        )
        .child(
            div()
                .id(ElementId::Name(
                    format!("remote-remove-{}", remote_name).into(),
                ))
                .px_1()
                .py_0()
                .rounded(px(2.0))
                .border_1()
                .border_color(border)
                .text_xs()
                .text_color(warning)
                .cursor_pointer()
                .hover(|s| s.bg(rgba_to_hsla(colors.sidebar_hover)))
                .child("×")
                .on_click(move |_ev, _window, cx| {
                    if let Some(e) = ent_remove.upgrade() {
                        let name = name_remove.clone();
                        e.update(cx, |this, cx| {
                            this.remove_remote(name, cx);
                        });
                    }
                }),
        )
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

fn render_create_worktree_button(
    colors: &AppColors,
    entity: WeakEntity<super::app::GitForgeApp>,
) -> Stateful<Div> {
    let accent = rgba_to_hsla(colors.accent);
    let muted = rgba_to_hsla(colors.text_muted);
    let ent = entity.clone();

    div()
        .id("sidebar-create-worktree")
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
                    this.open_create_worktree_dialog(cx);
                });
            }
        })
        .child(div().text_xs().text_color(accent).child("+ "))
        .child(div().text_xs().text_color(muted).child("New Worktree"))
}

fn render_prune_worktrees_button(
    colors: &AppColors,
    entity: WeakEntity<super::app::GitForgeApp>,
) -> Stateful<Div> {
    let muted = rgba_to_hsla(colors.text_muted);
    let _border = rgba_to_hsla(colors.border);
    let ent = entity.clone();

    div()
        .id("sidebar-prune-worktrees")
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
                    this.prune_worktrees(cx);
                });
            }
        })
        .child(div().text_xs().text_color(muted).child("Prune stale"))
}
