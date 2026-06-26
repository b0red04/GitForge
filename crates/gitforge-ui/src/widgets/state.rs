use crate::widgets::WidgetColors;
use gpui::*;

pub fn empty_state(message: &str, colors: WidgetColors) -> Div {
    div().flex_1().flex().items_center().justify_center().child(
        div()
            .text_sm()
            .text_color(colors.muted)
            .child(message.to_string()),
    )
}

pub fn empty_state_with_bg(message: &str, bg: Hsla, colors: WidgetColors) -> Div {
    div()
        .flex_1()
        .flex()
        .items_center()
        .justify_center()
        .bg(bg)
        .child(
            div()
                .text_sm()
                .text_color(colors.muted)
                .child(message.to_string()),
        )
}
