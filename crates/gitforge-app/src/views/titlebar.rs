use gitforge_git::RepoState;
use gitforge_ui::{AppColors, rgba_to_hsla};
use gpui::*;

use super::commands::{TitlebarMenu, titlebar_menu_entries};
use super::layout::{TITLEBAR_HEIGHT, WINDOW_CORNER_RADIUS};
use super::window_chrome::{apply_top_corner_radius, seal_rounded_corners};

fn breadcrumb_text(repo_state: Option<&RepoState>) -> String {
    match repo_state {
        Some(repo) => {
            let repo_name = repo
                .path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("repository");
            let branch = repo.head_branch.as_deref().unwrap_or("(detached)");
            format!("{repo_name} › {branch}")
        }
        None => "Press Ctrl+O to open a repository".into(),
    }
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

fn render_titlebar_menu_dropdown(
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
        .min_w(px(220.0))
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

    for (idx, entry) in titlebar_menu_entries(menu).iter().enumerate() {
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
                .on_click(move |_ev, _window, cx| {
                    if let Some(e) = item_ent.upgrade() {
                        e.update(cx, |app, cx| {
                            app.execute_app_command(action, cx);
                        });
                    }
                })
                .child(div().flex_1().text_sm().text_color(text_color).child(label))
                .child(div().text_xs().text_color(muted).child(keybinding)),
        );
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
    let accent = rgba_to_hsla(colors.accent);
    let muted = rgba_to_hsla(colors.text_muted);
    let icon_color = rgba_to_hsla(colors.text_muted);
    let icon_hover = rgba_to_hsla(colors.text);
    let hover_bg = rgba_to_hsla(colors.surface_high);

    let breadcrumb = breadcrumb_text(repo_state);
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

    left_cluster = left_cluster
        .child(
            div()
                .text_sm()
                .font_weight(FontWeight::BOLD)
                .text_color(accent)
                .flex_shrink_0()
                .child("GitForge"),
        )
        .child(
            div()
                .text_sm()
                .text_color(muted)
                .overflow_hidden()
                .text_ellipsis()
                .child(breadcrumb),
        );

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

    if let Some(menu) = active_menu {
        bar = bar.child(render_titlebar_menu_dropdown(menu, colors, entity));
    }

    bar
}
