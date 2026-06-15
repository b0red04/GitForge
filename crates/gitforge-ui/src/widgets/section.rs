use crate::widgets::WidgetColors;
use gpui::*;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum HeaderPadding {
    Compact,
    Normal,
    Loose,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum HeaderBorder {
    Bottom,
    Top,
    None,
}

pub fn section_header(
    id: impl Into<ElementId>,
    title: &str,
    padding: HeaderPadding,
    border: HeaderBorder,
    count: Option<usize>,
    colors: WidgetColors,
) -> Stateful<Div> {
    let mut header = div().id(id.into()).flex().items_center().gap_1();

    header = match padding {
        HeaderPadding::Compact => header.px_2().py_1(),
        HeaderPadding::Normal => header.px_2().py_2(),
        HeaderPadding::Loose => header.px_3().py_2(),
    };

    if matches!(border, HeaderBorder::Bottom) {
        header = header.border_b_1().border_color(colors.border);
    } else if matches!(border, HeaderBorder::Top) {
        header = header.border_t_1().border_color(colors.border);
    }

    header = header.child(
        div()
            .text_xs()
            .font_weight(FontWeight::BOLD)
            .text_color(colors.muted)
            .child(title.to_string()),
    );

    if let Some(n) = count {
        header = header.child(
            div()
                .text_xs()
                .text_color(colors.muted)
                .child(format!(" ({})", n)),
        );
    }

    header
}

pub fn collapsible_header(
    id: impl Into<ElementId>,
    title: &str,
    expanded: bool,
    count: Option<usize>,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    colors: WidgetColors,
) -> Stateful<Div> {
    let mut header = div()
        .id(id.into())
        .px_2()
        .py_1()
        .border_b_1()
        .border_color(colors.border)
        .bg(colors.surface_high)
        .flex()
        .items_center()
        .gap_1()
        .cursor_pointer();

    let arrow = if expanded { "\u{25be}" } else { "\u{25b8}" };
    header = header.child(div().text_xs().text_color(colors.muted).child(arrow));

    header = header.child(
        div()
            .text_xs()
            .font_weight(FontWeight::BOLD)
            .text_color(colors.muted)
            .child(title.to_string()),
    );

    if let Some(n) = count {
        header = header.child(
            div()
                .text_xs()
                .text_color(colors.muted)
                .child(format!(" ({})", n)),
        );
    }

    header.on_click(on_click)
}
