use gitforge_git::RepoState;
use gitforge_ui::{AppColors, rgba_to_hsla};
use gpui::*;

use super::layout::TOOLBAR_HEIGHT;

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
        .justify_center()
        .px_3()
        .gap_2();

    if repo_state.is_some() {
        let ent_fetch = entity.clone();
        let ent_pull = entity.clone();
        let ent_push = entity.clone();
        let ent_stash = entity.clone();
        let ent_pop = entity.clone();
        let ent_branch = entity.clone();
        let ent_create_pr = entity.clone();
        let ent_squash = entity.clone();

        toolbar = toolbar
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
                            this.pull_current(cx);
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
                            this.push_current(cx);
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
                "btn-create-pr",
                "Create PR",
                border,
                accent,
                hover_bg,
                move |_ev, _window, cx| {
                    if let Some(e) = ent_create_pr.upgrade() {
                        e.update(cx, |this, cx| {
                            this.open_create_pr_dialog(cx);
                        });
                    }
                },
            ))
            .child(toolbar_button(
                "btn-squash",
                "Squash",
                border,
                accent,
                hover_bg,
                move |_ev, _window, cx| {
                    if let Some(e) = ent_squash.upgrade() {
                        e.update(cx, |this, cx| {
                            this.open_squash_wizard(cx);
                        });
                    }
                },
            ));
    }

    toolbar
}
