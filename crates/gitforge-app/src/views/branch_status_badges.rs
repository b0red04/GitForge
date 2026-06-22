use gpui::*;

use super::app::GitForgeApp;

pub fn render_conflict_badge(warning: Hsla) -> impl IntoElement {
    svg()
        .flex_none()
        .flex_shrink_0()
        .size(px(15.0))
        .path("icons/git_merge_conflict.svg")
        .text_color(warning)
}

fn provider_icon_path(provider_id: &str) -> Option<&'static str> {
    match provider_id {
        "github" => Some("icons/github.svg"),
        "gitlab" => Some("icons/gitlab.svg"),
        _ => None,
    }
}

pub fn render_pr_link(
    number: u64,
    provider_id: Option<&str>,
    color: Hsla,
    hover_bg: Hsla,
    html_url: String,
    entity: WeakEntity<GitForgeApp>,
) -> Stateful<Div> {
    let entity_click = entity.clone();
    let url = html_url;

    let mut link = div()
        .id(ElementId::Name(format!("titlebar-pr-{number}").into()))
        .flex()
        .flex_none()
        .flex_shrink_0()
        .items_center()
        .gap_1()
        .h(px(24.0))
        .px_2()
        .rounded(px(4.0))
        .cursor_pointer()
        .hover(move |s| s.bg(hover_bg))
        .on_click(move |_ev, _window, cx| {
            if let Some(e) = entity_click.upgrade() {
                e.update(cx, |this, _cx| {
                    this.open_in_browser(url.clone());
                });
            }
        })
        .on_mouse_move(|_, _, cx| cx.stop_propagation())
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation());

    if let Some(provider) = provider_id.and_then(|id| provider_icon_path(id)) {
        link = link.child(
            svg()
                .flex_none()
                .size(px(14.0))
                .path(provider)
                .text_color(color),
        );
    }

    link.child(
        div()
            .text_sm()
            .text_color(color)
            .flex_none()
            .child(format!("#{number}")),
    )
}
