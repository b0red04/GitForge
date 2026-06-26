use gitforge_ui::{AppColors, rgba_to_hsla};
use gpui::prelude::FluentBuilder;
use gpui::*;

use super::app::GitForgeApp;
use super::layout;
use super::repo_session::{move_repo_tab_to_end, reorder_repo_tab};

/// Drag payload carrying the id of the repository tab being dragged. Used as
/// the GPUI drag value (`on_drag`/`on_drop`/`on_drag_move` are keyed on this
/// type) so each tab can identify the tab under the cursor without manual
/// hit-testing.
#[derive(Clone, Copy)]
pub(crate) struct TabDragPayload(pub(crate) u64);

/// Floating "ghost" view rendered under the cursor while a tab is being
/// dragged. Built by the `on_drag` constructor and owned by GPUI for the
/// duration of the drag.
pub(crate) struct TabDragPreview {
    name: SharedString,
    bg: Hsla,
    border: Hsla,
    text: Hsla,
}

impl TabDragPreview {
    pub(crate) fn new(name: SharedString, colors: &AppColors) -> Self {
        Self {
            name,
            bg: rgba_to_hsla(colors.surface_high),
            border: rgba_to_hsla(colors.accent),
            text: rgba_to_hsla(colors.text),
        }
    }
}

impl Render for TabDragPreview {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .px_2()
            .h(px(28.0))
            .flex()
            .items_center()
            .border_1()
            .border_color(self.border)
            .rounded(px(4.0))
            .bg(self.bg)
            .shadow(vec![BoxShadow {
                color: black().opacity(0.4),
                offset: point(px(0.0), px(4.0)),
                blur_radius: px(12.0),
                spread_radius: px(0.0),
            }])
            .text_sm()
            .text_color(self.text)
            .child(self.name.clone())
    }
}

pub struct RepoTabView {
    pub id: u64,
    pub name: String,
    pub loading: bool,
    pub has_error: bool,
}

/// Width of the insertion caret drawn at the current drop position.
const DROP_CARET_WIDTH: f32 = 2.0;
const TAB_LABEL_MAX_WIDTH: f32 = 180.0;

pub fn render_repo_tab_bar(
    tabs: &[RepoTabView],
    active_tab_id: Option<u64>,
    colors: &AppColors,
    window: &Window,
    entity: WeakEntity<GitForgeApp>,
    drag_source: Option<u64>,
    drop_caret: Option<usize>,
) -> impl IntoElement {
    let scale = window.scale_factor();
    let label_max_width = layout::snap_px(TAB_LABEL_MAX_WIDTH, scale);
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

    for (index, tab) in tabs.iter().enumerate() {
        // Insertion caret before this tab.
        if drop_caret == Some(index) {
            row = row.child(render_drop_caret(index, accent));
        }

        let is_active = Some(tab.id) == active_tab_id;
        let is_dragging = Some(tab.id) == drag_source;
        let bg = if is_active { surface_high } else { surface };
        let label_color = if tab.has_error { error } else { text };
        let tab_id = tab.id;
        let tab_id_for_close = tab.id;
        let name = tab.name.clone();
        let ent_activate = entity.clone();
        let ent_close = entity.clone();

        // DnD handler entities.
        let ent_drag = entity.clone();
        let ent_drag_move = entity.clone();
        let ent_drop = entity.clone();
        let name_for_preview: SharedString = tab.name.clone().into();
        let colors_drag = colors.clone();

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
            .when_some(if is_dragging { Some(0.4) } else { None }, |el, opacity| {
                el.opacity(opacity)
            })
            .cursor_pointer()
            .hover(move |s| s.bg(surface_high))
            .on_click(move |_ev, _window, cx| {
                if let Some(e) = ent_activate.upgrade() {
                    e.update(cx, |app, cx| app.activate_repo_tab(tab_id, cx));
                }
            })
            // Begin a tab reorder drag. GPUI only starts the drag after the
            // cursor moves past its threshold, so a plain click still reaches
            // `on_click` above to activate the tab.
            .on_drag(TabDragPayload(tab.id), move |_value, _pt, _window, cx| {
                if let Some(e) = ent_drag.upgrade() {
                    e.update(cx, |app, _cx| {
                        app.repo_session.tab_drag_source = Some(tab_id);
                    });
                }
                cx.new(|_| TabDragPreview::new(name_for_preview.clone(), &colors_drag))
            })
            .on_drag_move::<TabDragPayload>(move |ev, _window, cx| {
                let before = ev.event.position.x < ev.bounds.center().x;
                if let Some(e) = ent_drag_move.upgrade() {
                    e.update(cx, |app, cx| {
                        if app.repo_session.tab_drop_target != Some((tab_id, before)) {
                            app.repo_session.tab_drop_target = Some((tab_id, before));
                            cx.notify();
                        }
                    });
                }
            })
            .on_drop::<TabDragPayload>(move |payload, _window, cx| {
                if let Some(e) = ent_drop.upgrade() {
                    e.update(cx, |app, cx| {
                        let before = app
                            .repo_session
                            .tab_drop_target
                            .map(|(_, b)| b)
                            .unwrap_or(false);
                        reorder_repo_tab(
                            &mut app.repo_session.open_repo_tabs,
                            payload.0,
                            tab_id,
                            before,
                        );
                        app.repo_session.clear_tab_drag();
                        app.save_settings();
                        cx.notify();
                    });
                }
            })
            .child(
                div()
                    .flex_shrink_0()
                    .min_w(px(0.0))
                    .text_sm()
                    .text_color(label_color)
                    .max_w(label_max_width)
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
                .flex_shrink_0()
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

    // Trailing caret (drop at end) + trailing drop zone that accepts a drop to
    // move the dragged tab to the end of the bar.
    if drop_caret == Some(tabs.len()) {
        row = row.child(render_drop_caret(tabs.len(), accent));
    }

    // "+" affordance to add a new repository. Opens the unified Add Repository
    // dialog (Local folder picker + connected-account repo browser). Sits flush
    // against the last tab (the `flex_1` tail below absorbs the remaining
    // width). Visible in the empty state too, so the user can open their first
    // repo from the bar.
    let ent_add = entity.clone();
    row = row.child(
        div()
            .id("repo-tab-bar-add")
            .flex_none()
            .h(px(28.0))
            .w(px(28.0))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(3.0))
            .cursor_pointer()
            .hover(move |s| s.bg(surface_high))
            .child(
                svg()
                    .size(px(16.0))
                    .path("icons/plus.svg")
                    .text_color(muted),
            )
            .on_click(move |_ev, _window, cx| {
                if let Some(e) = ent_add.upgrade() {
                    e.update(cx, |app, cx| app.open_add_repo_dialog(cx));
                }
            }),
    );

    let ent_tail_move = entity.clone();
    let ent_tail_drop = entity.clone();
    row = row.child(
        div()
            .id("repo-tab-bar-tail")
            .flex_1()
            .h(px(28.0))
            .on_drag_move::<TabDragPayload>(move |_ev, _window, cx| {
                if let Some(e) = ent_tail_move.upgrade() {
                    e.update(cx, |app, cx| {
                        if app.repo_session.tab_drop_target.is_some() {
                            app.repo_session.tab_drop_target = None;
                            cx.notify();
                        }
                    });
                }
            })
            .on_drop::<TabDragPayload>(move |payload, _window, cx| {
                if let Some(e) = ent_tail_drop.upgrade() {
                    e.update(cx, |app, cx| {
                        move_repo_tab_to_end(&mut app.repo_session.open_repo_tabs, payload.0);
                        app.repo_session.clear_tab_drag();
                        app.save_settings();
                        cx.notify();
                    });
                }
            }),
    );

    row
}

fn render_drop_caret(index: usize, accent: Hsla) -> impl IntoElement {
    div()
        .id(ElementId::NamedInteger(
            "repo-tab-drop-caret".into(),
            index as u64,
        ))
        .flex_none()
        .w(px(DROP_CARET_WIDTH))
        .h(px(24.0))
        .my(px(2.0))
        .rounded(px(1.0))
        .bg(accent)
}
