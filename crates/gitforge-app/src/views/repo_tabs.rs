use gitforge_ui::{AppColors, rgba_to_hsla};
use gpui::*;

pub struct RepoTabView {
    pub id: u64,
    pub name: String,
    pub loading: bool,
    pub has_error: bool,
}

pub fn render_repo_tab_bar(
    tabs: &[RepoTabView],
    active_tab_id: Option<u64>,
    colors: &AppColors,
    entity: WeakEntity<super::app::GitForgeApp>,
) -> impl IntoElement {
    let surface = rgba_to_hsla(colors.surface);
    let surface_high = rgba_to_hsla(colors.surface_high);
    let border = rgba_to_hsla(colors.border);
    let text = rgba_to_hsla(colors.text);
    let muted = rgba_to_hsla(colors.text_muted);
    let accent = rgba_to_hsla(colors.accent);
    let error = rgba_to_hsla(colors.error);

    let mut row = div()
        .id("repo-tab-bar")
        .w_full()
        .h(px(30.0))
        .flex_shrink_0()
        .flex()
        .flex_row()
        .items_end()
        .overflow_x_scroll()
        .bg(surface)
        .border_b_1()
        .border_color(border)
        .px_2()
        .gap_1();

    for tab in tabs {
        let is_active = Some(tab.id) == active_tab_id;
        let bg = if is_active { surface_high } else { surface };
        let label_color = if tab.has_error { error } else { text };
        let tab_id = tab.id;
        let tab_id_for_close = tab.id;
        let name = tab.name.clone();
        let ent_activate = entity.clone();
        let ent_close = entity.clone();

        let mut tab_el = div()
            .id(ElementId::NamedInteger("repo-tab".into(), tab.id))
            .flex_none()
            .h(px(28.0))
            .flex()
            .items_center()
            .gap_1()
            .px_2()
            .border_1()
            .border_color(if is_active { accent } else { border })
            .rounded_t(px(4.0))
            .bg(bg)
            .cursor_pointer()
            .hover(move |s| s.bg(surface_high))
            .on_click(move |_ev, _window, cx| {
                if let Some(e) = ent_activate.upgrade() {
                    e.update(cx, |app, cx| app.activate_repo_tab(tab_id, cx));
                }
            })
            .child(
                div()
                    .text_sm()
                    .text_color(label_color)
                    .max_w(px(180.0))
                    .overflow_hidden()
                    .text_ellipsis()
                    .child(name),
            );

        if tab.loading {
            tab_el = tab_el.child(div().text_xs().text_color(muted).child("..."));
        }

        tab_el = tab_el.child(
            div()
                .id(ElementId::NamedInteger("repo-tab-close".into(), tab.id))
                .flex()
                .items_center()
                .justify_center()
                .size(px(16.0))
                .rounded(px(3.0))
                .hover(move |s| s.bg(border))
                .child(svg().size(px(12.0)).path("icons/x.svg").text_color(muted))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(move |_ev, _window, cx| {
                    cx.stop_propagation();
                    if let Some(e) = ent_close.upgrade() {
                        e.update(cx, |app, cx| app.close_repo_tab(tab_id_for_close, cx));
                    }
                }),
        );

        row = row.child(tab_el);
    }

    row
}
