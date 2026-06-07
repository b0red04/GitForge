use gitforge_git::RepoState;
use gitforge_ui::{AppColors, rgba_to_hsla};
use gpui::*;

use super::commands::{MenuEntries, MenuEntry, TitlebarMenu, titlebar_menu_entries};
use super::layout::{TITLEBAR_HEIGHT, WINDOW_CORNER_RADIUS};
use super::window_chrome::{apply_top_corner_radius, seal_rounded_corners};

struct TitlebarRepoContext {
    repo_name: String,
    branch_name: String,
    is_detached: bool,
}

fn repo_context(repo_state: Option<&RepoState>) -> Option<TitlebarRepoContext> {
    let repo = repo_state?;
    let repo_name = repo
        .path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("repository")
        .to_string();
    let branch_name = repo
        .head_branch
        .as_deref()
        .unwrap_or("(detached)")
        .to_string();

    Some(TitlebarRepoContext {
        repo_name,
        branch_name,
        is_detached: repo.head_branch.is_none(),
    })
}

fn titlebar_icon(icon_path: &'static str, color: Hsla) -> Svg {
    svg()
        .flex_none()
        .size(px(14.0))
        .path(icon_path)
        .text_color(color)
}

fn breadcrumb_segment(
    icon_path: &'static str,
    label: impl Into<SharedString>,
    color: Hsla,
    max_width: Pixels,
) -> Div {
    div()
        .flex()
        .items_center()
        .gap_1()
        .min_w(px(0.0))
        .max_w(max_width)
        .overflow_hidden()
        .child(titlebar_icon(icon_path, color))
        .child(
            div()
                .text_sm()
                .text_color(color)
                .min_w(px(0.0))
                .overflow_hidden()
                .text_ellipsis()
                .child(label.into()),
        )
}

fn no_repo_prompt(muted: Hsla) -> Div {
    div()
        .text_sm()
        .text_color(muted)
        .overflow_hidden()
        .text_ellipsis()
        .child("Press Ctrl+O to open a repository")
}

fn repo_breadcrumb(repo: TitlebarRepoContext, muted: Hsla) -> Div {
    let branch_icon = if repo.is_detached {
        "icons/git-commit.svg"
    } else {
        "icons/git-branch.svg"
    };

    div()
        .flex()
        .flex_row()
        .items_center()
        .gap_2()
        .min_w(px(0.0))
        .overflow_hidden()
        .child(breadcrumb_segment(
            "icons/git-commit.svg",
            repo.repo_name,
            muted,
            px(180.0),
        ))
        .child(div().text_sm().text_color(muted).flex_none().child("/"))
        .child(breadcrumb_segment(
            branch_icon,
            repo.branch_name,
            muted,
            px(260.0),
        ))
}

fn window_control_button(
    id: impl Into<ElementId>,
    icon_path: &'static str,
    icon_color: Hsla,
    icon_hover: Hsla,
    hover_bg: Hsla,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    div()
        .id(id.into())
        .group("")
        .flex()
        .items_center()
        .justify_center()
        .w(px(20.0))
        .h(px(20.0))
        .rounded(px(10.0))
        .cursor_pointer()
        .hover(|s| s.bg(hover_bg))
        .active(|s| s.bg(hover_bg))
        .child(
            svg()
                .flex_none()
                .size(px(16.0))
                .path(icon_path)
                .text_color(icon_color)
                .group_hover("", |s| s.text_color(icon_hover)),
        )
        .on_click(on_click)
        .on_mouse_move(|_, _, cx| cx.stop_propagation())
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
}

fn render_window_controls(
    window: &Window,
    icon_color: Hsla,
    icon_hover: Hsla,
    hover_bg: Hsla,
) -> Option<impl IntoElement> {
    if !matches!(window.window_decorations(), Decorations::Client { .. }) {
        return None;
    }

    let controls = window.window_controls();
    let is_maximized = window.is_maximized();
    let max_icon = if is_maximized {
        "icons/generic_restore.svg"
    } else {
        "icons/generic_maximize.svg"
    };

    let mut row = div()
        .id("titlebar-window-controls")
        .flex()
        .flex_row()
        .items_center()
        .gap_3()
        .px_3()
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation());

    if controls.minimize {
        row = row.child(window_control_button(
            "titlebar-minimize",
            "icons/generic_minimize.svg",
            icon_color,
            icon_hover,
            hover_bg,
            |_ev, window, _cx| window.minimize_window(),
        ));
    }

    if controls.maximize {
        row = row.child(window_control_button(
            "titlebar-maximize",
            max_icon,
            icon_color,
            icon_hover,
            hover_bg,
            |_ev, window, _cx| window.zoom_window(),
        ));
    }

    row = row.child(window_control_button(
        "titlebar-close",
        "icons/generic_close.svg",
        icon_color,
        icon_hover,
        hover_bg,
        |_ev, window, _cx| window.remove_window(),
    ));

    Some(row)
}

fn hamburger_button(
    icon_color: Hsla,
    icon_hover: Hsla,
    hover_bg: Hsla,
    entity: WeakEntity<super::app::GitForgeApp>,
) -> Stateful<Div> {
    div()
        .id("titlebar-hamburger")
        .group("hamburger")
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(3.0))
        .w(px(28.0))
        .h(px(24.0))
        .rounded(px(4.0))
        .cursor_pointer()
        .hover(|s| s.bg(hover_bg))
        .active(|s| s.bg(hover_bg))
        .child(
            div()
                .w(px(14.0))
                .h(px(1.0))
                .bg(icon_color)
                .group_hover("hamburger", |s| s.bg(icon_hover)),
        )
        .child(
            div()
                .w(px(14.0))
                .h(px(1.0))
                .bg(icon_color)
                .group_hover("hamburger", |s| s.bg(icon_hover)),
        )
        .child(
            div()
                .w(px(14.0))
                .h(px(1.0))
                .bg(icon_color)
                .group_hover("hamburger", |s| s.bg(icon_hover)),
        )
        .on_click(move |_ev, _window, cx| {
            if let Some(e) = entity.upgrade() {
                e.update(cx, |app, cx| {
                    app.toggle_titlebar_menus(cx);
                });
            }
        })
        .on_mouse_move(|_, _, cx| cx.stop_propagation())
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
}

fn menu_label_button(
    menu: TitlebarMenu,
    active_menu: Option<TitlebarMenu>,
    text_color: Hsla,
    hover_bg: Hsla,
    active_bg: Hsla,
    entity: WeakEntity<super::app::GitForgeApp>,
) -> Stateful<Div> {
    let is_active = active_menu == Some(menu);
    let bg = if is_active {
        active_bg
    } else {
        gpui::transparent_black()
    };

    div()
        .id(menu.element_id())
        .flex_none()
        .px_2()
        .h(px(24.0))
        .flex()
        .items_center()
        .rounded(px(4.0))
        .bg(bg)
        .text_sm()
        .text_color(text_color)
        .cursor_pointer()
        .hover(move |s| s.bg(hover_bg))
        .on_click(move |_ev, _window, cx| {
            if let Some(e) = entity.upgrade() {
                e.update(cx, |app, cx| {
                    app.open_titlebar_menu(menu, cx);
                });
            }
        })
        .on_mouse_move(|_, _, cx| cx.stop_propagation())
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child(menu.label())
}

pub fn render_titlebar_menu_dropdown(
    menu: TitlebarMenu,
    colors: &AppColors,
    entity: WeakEntity<super::app::GitForgeApp>,
) -> Stateful<Div> {
    let surface = rgba_to_hsla(colors.surface);
    let border = rgba_to_hsla(colors.border);
    let text_color = rgba_to_hsla(colors.text);
    let muted = rgba_to_hsla(colors.text_muted);
    let hover_bg = rgba_to_hsla(colors.sidebar_hover);

    let mut dropdown = div()
        .id("titlebar-menu-dropdown")
        .absolute()
        .top(px(TITLEBAR_HEIGHT))
        .left(px(menu.dropdown_left()))
        .min_w(px(280.0))
        .max_h(px(480.0))
        .overflow_y_scroll()
        .bg(surface)
        .border_1()
        .border_color(border)
        .rounded(px(4.0))
        .py_1()
        .shadow(vec![BoxShadow {
            color: black().opacity(0.35),
            offset: point(px(0.0), px(4.0)),
            blur_radius: px(12.0),
            spread_radius: px(0.0),
        }])
        .on_mouse_move(|_, _, cx| cx.stop_propagation())
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_click(|_, _, cx| cx.stop_propagation());

    match titlebar_menu_entries(menu) {
        MenuEntries::WithSeparators(entries) => {
            for (idx, entry) in entries.iter().enumerate() {
                match entry {
                    MenuEntry::Separator => {
                        dropdown = dropdown.child(
                            div()
                                .id(ElementId::Name(format!("titlebar-menu-sep-{idx}").into()))
                                .my_1()
                                .mx_2()
                                .h(px(1.0))
                                .bg(border),
                        );
                    }
                    MenuEntry::Item(item) => {
                        let item_ent = entity.clone();
                        let action = item.action;
                        let label = item.label;
                        let keybinding = item.keybinding.unwrap_or_default();

                        dropdown = dropdown.child(
                            div()
                                .id(ElementId::Name(format!("titlebar-menu-item-{idx}").into()))
                                .h(px(28.0))
                                .px_3()
                                .flex()
                                .items_center()
                                .gap_3()
                                .cursor_pointer()
                                .hover(move |s| s.bg(hover_bg))
                                .on_click(move |_ev, window, cx| {
                                    if let Some(e) = item_ent.upgrade() {
                                        e.update(cx, |app, cx| {
                                            app.close_titlebar_menu(cx);
                                        });
                                    }
                                    window.dispatch_action(action.boxed_action(), cx);
                                })
                                .child(
                                    div()
                                        .flex_1()
                                        .text_sm()
                                        .text_color(text_color)
                                        .child(label),
                                )
                                .child(div().text_xs().text_color(muted).child(keybinding)),
                        );
                    }
                }
            }
        }
        MenuEntries::Flat(entries) => {
            for (idx, entry) in entries.iter().enumerate() {
                let item_ent = entity.clone();
                let action = entry.action;
                let label = entry.label;
                let keybinding = entry.keybinding.unwrap_or_default();

                dropdown = dropdown.child(
                    div()
                        .id(ElementId::Name(format!("titlebar-menu-item-{idx}").into()))
                        .h(px(28.0))
                        .px_3()
                        .flex()
                        .items_center()
                        .gap_3()
                        .cursor_pointer()
                        .hover(move |s| s.bg(hover_bg))
                        .on_click(move |_ev, window, cx| {
                            if let Some(e) = item_ent.upgrade() {
                                e.update(cx, |app, cx| {
                                    app.close_titlebar_menu(cx);
                                });
                            }
                            window.dispatch_action(action.boxed_action(), cx);
                        })
                        .child(div().flex_1().text_sm().text_color(text_color).child(label))
                        .child(div().text_xs().text_color(muted).child(keybinding)),
                );
            }
        }
    }

    dropdown
}

pub fn render_titlebar(
    repo_state: Option<&RepoState>,
    colors: &AppColors,
    window: &Window,
    entity: WeakEntity<super::app::GitForgeApp>,
    menus_visible: bool,
    active_menu: Option<TitlebarMenu>,
) -> impl IntoElement {
    let decorations = window.window_decorations();
    let controls = window.window_controls();

    let titlebar_bg = if window.is_window_active() {
        rgba_to_hsla(colors.surface)
    } else {
        rgba_to_hsla(colors.surface_high)
    };
    let muted = rgba_to_hsla(colors.text_muted);
    let icon_color = rgba_to_hsla(colors.text_muted);
    let icon_hover = rgba_to_hsla(colors.text);
    let hover_bg = rgba_to_hsla(colors.surface_high);

    let repo_context = repo_context(repo_state);
    let rounding = px(WINDOW_CORNER_RADIUS);
    let tiling = match decorations {
        Decorations::Server => Tiling::default(),
        Decorations::Client { tiling } => tiling,
    };

    let mut left_cluster = div()
        .flex()
        .flex_row()
        .items_center()
        .gap_2()
        .overflow_hidden()
        .child(hamburger_button(
            icon_color,
            icon_hover,
            hover_bg,
            entity.clone(),
        ));

    if menus_visible {
        for menu in TitlebarMenu::ALL {
            left_cluster = left_cluster.child(menu_label_button(
                menu,
                active_menu,
                icon_hover,
                hover_bg,
                rgba_to_hsla(colors.selection_bg),
                entity.clone(),
            ));
        }
    }

    left_cluster = if let Some(repo) = repo_context {
        left_cluster.child(repo_breadcrumb(repo, muted))
    } else {
        left_cluster.child(no_repo_prompt(muted))
    };

    let mut bar = div()
        .id("titlebar")
        .relative()
        .w_full()
        .h(px(TITLEBAR_HEIGHT))
        .flex_shrink_0()
        .window_control_area(WindowControlArea::Drag)
        .flex()
        .flex_row()
        .items_center()
        .bg(titlebar_bg)
        .on_mouse_down(MouseButton::Left, |_ev, window, _| {
            window.start_window_move();
        })
        .on_click(|event, window, _| {
            if event.click_count() == 2 {
                window.zoom_window();
            }
        });

    if matches!(decorations, Decorations::Client { .. }) && controls.window_menu {
        bar = bar.on_mouse_down(MouseButton::Right, |ev, window, _| {
            window.show_window_menu(ev.position);
        });
    }

    if matches!(decorations, Decorations::Client { .. }) {
        bar = seal_rounded_corners(apply_top_corner_radius(bar, rounding, tiling), titlebar_bg);
    }

    bar = bar.pl(px(8.0)).child(left_cluster.flex_1().min_w(px(0.0)));

    if !window.is_fullscreen() {
        if let Some(controls) = render_window_controls(window, icon_color, icon_hover, hover_bg) {
            bar = bar.child(controls);
        }
    }

    bar
}

pub fn render_titlebar_divider(colors: &AppColors) -> impl IntoElement {
    div()
        .id("titlebar-divider")
        .w_full()
        .h(px(1.0))
        .flex_shrink_0()
        .bg(rgba_to_hsla(colors.border))
}
