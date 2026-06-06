use gitforge_git::RepoState;
use gitforge_ui::{AppColors, rgba_to_hsla};
use gpui::*;

use super::layout::{STATUS_BAR_HEIGHT, TOOLBAR_HEIGHT, WINDOW_CORNER_RADIUS};
use super::window_chrome::{apply_bottom_corner_radius, seal_rounded_corners};

fn toolbar_button(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    border: Hsla,
    text_color: Hsla,
    hover_bg: Hsla,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    div()
        .id(id.into())
        .px_2()
        .py_0()
        .rounded(px(3.0))
        .border_1()
        .border_color(border)
        .cursor_pointer()
        .text_xs()
        .text_color(text_color)
        .hover(move |s| s.bg(hover_bg))
        .child(label.into())
        .on_click(on_click)
}

pub fn render_toolbar(
    repo_state: Option<&RepoState>,
    colors: &AppColors,
    show_status_tab: bool,
    more_open: bool,
    entity: WeakEntity<super::app::GitForgeApp>,
) -> Div {
    let surface = rgba_to_hsla(colors.surface);
    let border = rgba_to_hsla(colors.border);
    let accent = rgba_to_hsla(colors.accent);
    let text_color = rgba_to_hsla(colors.text);
    let hover_bg = rgba_to_hsla(colors.surface_high);

    let mut toolbar = div()
        .relative()
        .w_full()
        .h(px(TOOLBAR_HEIGHT))
        .bg(surface)
        .border_b_1()
        .border_color(border)
        .flex()
        .items_center()
        .px_3()
        .gap_2();

    if repo_state.is_some() {
        let history_bg = if !show_status_tab { hover_bg } else { surface };
        let status_bg = if show_status_tab { hover_bg } else { surface };

        let ent_history = entity.clone();
        let ent_status = entity.clone();
        let ent_fetch = entity.clone();
        let ent_pull = entity.clone();
        let ent_push = entity.clone();
        let ent_branch = entity.clone();
        let ent_stash = entity.clone();
        let ent_pop = entity.clone();
        let ent_more = entity.clone();
        let ent_undo = entity.clone();

        toolbar = toolbar
            .child(
                toolbar_button(
                    "tab-history",
                    "History",
                    border,
                    text_color,
                    hover_bg,
                    move |_ev, _window, cx| {
                        if let Some(e) = ent_history.upgrade() {
                            e.update(cx, |this, cx| {
                                this.view_mode = super::app::MainViewMode::CommitHistory;
                                this.close_toolbar_more(cx);
                                cx.notify();
                            });
                        }
                    },
                )
                .bg(history_bg),
            )
            .child(
                toolbar_button(
                    "tab-status",
                    "Changes",
                    border,
                    text_color,
                    hover_bg,
                    move |_ev, _window, cx| {
                        if let Some(e) = ent_status.upgrade() {
                            e.update(cx, |this, cx| {
                                this.view_mode = super::app::MainViewMode::Status;
                                this.close_toolbar_more(cx);
                                this.load_status(cx);
                            });
                        }
                    },
                )
                .bg(status_bg),
            )
            .child(toolbar_button(
                "btn-fetch",
                "Fetch",
                border,
                accent,
                hover_bg,
                move |_ev, _window, cx| {
                    if let Some(e) = ent_fetch.upgrade() {
                        e.update(cx, |this, cx| {
                            this.fetch_all(cx);
                        });
                    }
                },
            ))
            .child(toolbar_button(
                "btn-pull",
                "Pull",
                border,
                accent,
                hover_bg,
                move |_ev, _window, cx| {
                    if let Some(e) = ent_pull.upgrade() {
                        e.update(cx, |this, cx| {
                            this.open_pull_dialog(cx);
                        });
                    }
                },
            ))
            .child(toolbar_button(
                "btn-push",
                "Push",
                border,
                accent,
                hover_bg,
                move |_ev, _window, cx| {
                    if let Some(e) = ent_push.upgrade() {
                        e.update(cx, |this, cx| {
                            this.open_push_dialog(cx);
                        });
                    }
                },
            ))
            .child(toolbar_button(
                "btn-new-branch",
                "Branch",
                border,
                accent,
                hover_bg,
                move |_ev, _window, cx| {
                    if let Some(e) = ent_branch.upgrade() {
                        e.update(cx, |this, cx| {
                            this.open_create_branch_dialog(None, cx);
                        });
                    }
                },
            ))
            .child(toolbar_button(
                "btn-stash-push",
                "Stash",
                border,
                text_color,
                hover_bg,
                move |_ev, _window, cx| {
                    if let Some(e) = ent_stash.upgrade() {
                        e.update(cx, |this, cx| {
                            this.open_stash_push_dialog(cx);
                        });
                    }
                },
            ))
            .child(toolbar_button(
                "btn-stash-pop",
                "Pop",
                border,
                text_color,
                hover_bg,
                move |_ev, _window, cx| {
                    if let Some(e) = ent_pop.upgrade() {
                        e.update(cx, |this, cx| {
                            this.stash_pop(cx);
                        });
                    }
                },
            ))
            .child(
                toolbar_button(
                    "btn-more",
                    if more_open { "More ▴" } else { "More ▾" },
                    border,
                    text_color,
                    hover_bg,
                    move |_ev, _window, cx| {
                        if let Some(e) = ent_more.upgrade() {
                            e.update(cx, |this, cx| {
                                this.toggle_toolbar_more(cx);
                            });
                        }
                    },
                )
                .bg(if more_open { hover_bg } else { surface }),
            )
            .child(toolbar_button(
                "undo-commit-btn",
                "Undo",
                border,
                rgba_to_hsla(colors.warning),
                hover_bg,
                move |_ev, _window, cx| {
                    if let Some(e) = ent_undo.upgrade() {
                        e.update(cx, |this, cx| {
                            this.soft_reset(cx);
                        });
                    }
                },
            ));

        if more_open {
            toolbar = toolbar.child(render_more_menu(colors, border, hover_bg, entity));
        }
    }

    toolbar
}

fn render_more_menu(
    colors: &AppColors,
    border: Hsla,
    hover_bg: Hsla,
    entity: WeakEntity<super::app::GitForgeApp>,
) -> Div {
    let surface = rgba_to_hsla(colors.surface);
    let accent = rgba_to_hsla(colors.accent);
    let text_color = rgba_to_hsla(colors.text);

    let mut row = div()
        .absolute()
        .top(px(TOOLBAR_HEIGHT))
        .right(px(12.0))
        .bg(surface)
        .border_1()
        .border_color(border)
        .rounded(px(4.0))
        .p_2()
        .flex()
        .flex_col()
        .gap_1();

    let ent_clone = entity.clone();
    let ent_gh = entity.clone();
    let ent_gl = entity.clone();
    let ent_ssh = entity.clone();
    let ent_accounts = entity.clone();
    let ent_browser = entity.clone();
    let ent_ai = entity.clone();
    let ent_wt = entity.clone();

    row = row
        .child(more_item(
            "more-clone",
            "Clone",
            border,
            text_color,
            hover_bg,
            move |ent, cx| {
                if let Some(e) = ent.upgrade() {
                    e.update(cx, |this, cx| {
                        this.close_toolbar_more(cx);
                        this.open_clone_dialog(cx);
                    });
                }
            },
            ent_clone,
        ))
        .child(more_item(
            "more-github",
            "Clone from GitHub",
            border,
            accent,
            hover_bg,
            move |ent, cx| {
                if let Some(e) = ent.upgrade() {
                    e.update(cx, |this, cx| {
                        this.close_toolbar_more(cx);
                        this.open_clone_from_hosting_dialog("github".to_string(), cx);
                    });
                }
            },
            ent_gh,
        ))
        .child(more_item(
            "more-gitlab",
            "Clone from GitLab",
            border,
            accent,
            hover_bg,
            move |ent, cx| {
                if let Some(e) = ent.upgrade() {
                    e.update(cx, |this, cx| {
                        this.close_toolbar_more(cx);
                        this.open_clone_from_hosting_dialog("gitlab".to_string(), cx);
                    });
                }
            },
            ent_gl,
        ))
        .child(more_item(
            "more-ssh",
            "SSH Keys",
            border,
            text_color,
            hover_bg,
            move |ent, cx| {
                if let Some(e) = ent.upgrade() {
                    e.update(cx, |this, cx| {
                        this.close_toolbar_more(cx);
                        this.open_ssh_generate_key_dialog(cx);
                    });
                }
            },
            ent_ssh,
        ))
        .child(more_item(
            "more-accounts",
            "Accounts",
            border,
            accent,
            hover_bg,
            move |ent, cx| {
                if let Some(e) = ent.upgrade() {
                    e.update(cx, |this, cx| {
                        this.close_toolbar_more(cx);
                        this.open_manage_accounts_dialog(cx);
                    });
                }
            },
            ent_accounts,
        ))
        .child(more_item(
            "more-browser",
            "Open in Browser",
            border,
            accent,
            hover_bg,
            move |ent, cx| {
                if let Some(e) = ent.upgrade() {
                    e.update(cx, |this, cx| {
                        this.close_toolbar_more(cx);
                        this.open_repo_in_browser(cx);
                    });
                }
            },
            ent_browser,
        ))
        .child(more_item(
            "more-ai",
            "AI Settings",
            border,
            accent,
            hover_bg,
            move |ent, cx| {
                if let Some(e) = ent.upgrade() {
                    e.update(cx, |this, cx| {
                        this.close_toolbar_more(cx);
                        this.open_settings_window(
                            Some(crate::views::SettingsSection::Ai),
                            cx,
                        );
                    });
                }
            },
            ent_ai,
        ))
        .child(more_item(
            "more-worktree",
            "New Worktree",
            border,
            text_color,
            hover_bg,
            move |ent, cx| {
                if let Some(e) = ent.upgrade() {
                    e.update(cx, |this, cx| {
                        this.close_toolbar_more(cx);
                        this.open_create_worktree_dialog(cx);
                    });
                }
            },
            ent_wt,
        ));

    row
}

fn more_item(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    _border: Hsla,
    text_color: Hsla,
    hover_bg: Hsla,
    action: impl Fn(WeakEntity<super::app::GitForgeApp>, &mut App) + 'static,
    entity: WeakEntity<super::app::GitForgeApp>,
) -> Stateful<Div> {
    div()
        .id(id.into())
        .px_3()
        .py_1()
        .rounded(px(3.0))
        .cursor_pointer()
        .text_xs()
        .text_color(text_color)
        .hover(move |s| s.bg(hover_bg))
        .child(label.into())
        .on_click(move |_ev, _window, cx| {
            action(entity.clone(), cx);
        })
}

pub fn render_status_bar(
    remote_status: &str,
    colors: &AppColors,
    window: &Window,
) -> impl IntoElement {
    let surface = rgba_to_hsla(colors.surface);
    let border = rgba_to_hsla(colors.border);
    let muted = rgba_to_hsla(colors.text_muted);
    let accent = rgba_to_hsla(colors.accent);
    let warning = rgba_to_hsla(colors.warning);

    let status_color = if remote_status.contains("failed") || remote_status.contains("error") {
        warning
    } else if remote_status.is_empty() {
        muted
    } else {
        accent
    };

    let status_text = if remote_status.is_empty() {
        "Ready".to_string()
    } else {
        remote_status.to_string()
    };

    let rounding = px(WINDOW_CORNER_RADIUS);
    let tiling = match window.window_decorations() {
        Decorations::Server => Tiling::default(),
        Decorations::Client { tiling } => tiling,
    };

    let hints = div().text_xs().text_color(muted).child(
        "Ctrl+O Open  ↑↓ Navigate  Ctrl+Shift+F Fetch  Ctrl+Shift+U Pull  Ctrl+Shift+H Push",
    );

    let base = div()
        .w_full()
        .h(px(STATUS_BAR_HEIGHT))
        .flex_shrink_0()
        .bg(surface)
        .border_t_1()
        .border_color(border)
        .flex()
        .items_center()
        .px_3()
        .gap_3();

    let bar = if matches!(window.window_decorations(), Decorations::Client { .. }) {
        seal_rounded_corners(
            apply_bottom_corner_radius(base.id("status-bar"), rounding, tiling),
            surface,
        )
    } else {
        base.id("status-bar")
    };

    bar.child(div().text_xs().text_color(status_color).child(status_text))
        .child(div().flex_1())
        .child(hints)
}
