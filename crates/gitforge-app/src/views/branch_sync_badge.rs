use gpui::*;

pub fn render_sync_indicators(ahead: u32, behind: u32, color: Hsla) -> Div {
    if ahead == 0 && behind == 0 {
        return div();
    }

    let mut container = div().flex().flex_none().items_center().gap(px(4.0));

    if ahead > 0 {
        container = container.child(
            div()
                .flex()
                .items_center()
                .gap(px(1.0))
                .child(div().text_xs().text_color(color).child(format!("{ahead}")))
                .child(
                    svg()
                        .size(px(10.0))
                        .path("icons/arrow-up.svg")
                        .text_color(color),
                ),
        );
    }

    if behind > 0 {
        container = container.child(
            div()
                .flex()
                .items_center()
                .gap(px(1.0))
                .child(div().text_xs().text_color(color).child(format!("{behind}")))
                .child(
                    svg()
                        .size(px(10.0))
                        .path("icons/arrow-down.svg")
                        .text_color(color),
                ),
        );
    }

    container
}
