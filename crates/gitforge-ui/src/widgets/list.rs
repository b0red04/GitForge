use crate::widgets::WidgetColors;
use gpui::*;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum RowPadding {
    Normal,
    Loose,
}

pub fn list_row<H: Fn(&ClickEvent, &mut Window, &mut App) + 'static>(
    id: impl Into<ElementId>,
    height: Option<Pixels>,
    padding: RowPadding,
    selected: bool,
    hover_bg: Hsla,
    on_click: Option<H>,
    colors: WidgetColors,
) -> Stateful<Div> {
    let mut row = div().id(id.into()).w_full().flex().items_center().cursor_pointer();

    row = match padding {
        RowPadding::Normal => row.px_2(),
        RowPadding::Loose => row.px_3(),
    };

    if let Some(h) = height {
        row = row.h(h);
    }

    let bg = if selected {
        colors.sidebar_selected
    } else {
        colors.sidebar_background
    };
    row = row.bg(bg).hover(move |s| s.bg(hover_bg));

    if let Some(handler) = on_click {
        row.on_click(handler)
    } else {
        row
    }
}

pub fn virtual_list(
    id: &'static str,
    item_count: usize,
    scroll_handle: Option<UniformListScrollHandle>,
    render_row: impl Fn(usize) -> AnyElement + 'static,
) -> UniformList {
    let list = uniform_list(id, item_count, move |visible_range, _window, _cx| {
        let mut rows = Vec::with_capacity(visible_range.len());
        for i in visible_range {
            rows.push(render_row(i));
        }
        rows
    });

    if let Some(handle) = scroll_handle {
        list.track_scroll(handle)
    } else {
        list
    }
}
