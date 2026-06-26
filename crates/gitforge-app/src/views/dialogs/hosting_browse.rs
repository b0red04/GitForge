use gitforge_ui::{
    AppColors, DialogColors, dialog_label, dialog_overlay, dialog_surface, rgba_to_hsla,
};
use gpui::*;

use crate::views::app::AppDialog;

pub fn dialog_title_for(dialog: &AppDialog) -> String {
    match dialog {
        AppDialog::CloneFromHosting { provider } => format!("Clone from {provider}"),
        AppDialog::SearchHosting { provider } => format!("Search on {provider}"),
        _ => "Browse Repositories".to_string(),
    }
}

pub fn render(
    dialog: &AppDialog,
    colors: &AppColors,
    entity: WeakEntity<crate::views::app::GitForgeApp>,
    hosting_repos: &[gitforge_hosting::RemoteRepo],
    hosting_repos_loading: bool,
) -> Stateful<Div> {
    let dc = DialogColors::from_app(colors);
    let dialog_title = dialog_title_for(dialog);
    let ent_cancel = entity.clone();

    let mut content = dialog_surface(px(500.0), dc)
        .max_h(px(500.0))
        .gap_2()
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(dialog_title_gpui(&dialog_title, dc))
                .child(
                    div()
                        .id("hosting-cancel")
                        .px_2()
                        .py_0()
                        .border_1()
                        .border_color(dc.border)
                        .rounded(px(3.0))
                        .cursor_pointer()
                        .text_xs()
                        .text_color(dc.muted)
                        .child("Close")
                        .on_click(move |_ev, _window, cx| {
                            if let Some(e) = ent_cancel.upgrade() {
                                e.update(cx, |this, cx| {
                                    this.cancel_dialog(cx);
                                });
                            }
                        }),
                ),
        );

    if hosting_repos_loading {
        content = content.child(dialog_label("Loading repositories...", dc));
    } else if hosting_repos.is_empty() {
        content = content.child(dialog_label("No repositories found", dc));
    } else {
        let mut list = div().flex().flex_col().gap_1();
        for (i, repo) in hosting_repos.iter().enumerate() {
            list = list.child(render_repo_row(repo, i, colors, dc, entity.clone()));
        }
        content = content.child(list);
    }

    dialog_overlay(dc).child(content)
}

/// One clickable repository card. Shared between the `CloneFromHosting` /
/// `SearchHosting` list and the unified `AddRepo` dialog. Clicking calls
/// `clone_hosting_repo(url, name, cx)` on the app.
pub(crate) fn render_repo_row(
    repo: &gitforge_hosting::RemoteRepo,
    index: usize,
    colors: &AppColors,
    dc: DialogColors,
    entity: WeakEntity<crate::views::app::GitForgeApp>,
) -> Stateful<Div> {
    let clone_url = repo.clone_url.clone();
    let repo_name = repo.name.clone();
    let vis = if repo.is_private { "private" } else { "public" };
    let stars = repo.stars;
    let desc = repo.description.as_deref().unwrap_or("");

    div()
        .id(ElementId::NamedInteger("hosting-repo".into(), index as u64))
        .px_2()
        .py_1()
        .border_1()
        .border_color(dc.border)
        .rounded(px(3.0))
        .cursor_pointer()
        .hover(|s| s.bg(rgba_to_hsla(colors.surface_high)))
        .on_click(move |_ev, _window, cx| {
            if let Some(e) = entity.upgrade() {
                let url = clone_url.clone();
                let name = repo_name.clone();
                e.update(cx, |this, cx| {
                    this.clone_hosting_repo(url, name, cx);
                });
            }
        })
        .child(
            div()
                .flex()
                .flex_col()
                .gap_0()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(dc.text)
                                .child(repo.name.clone()),
                        )
                        .child(div().text_xs().text_color(dc.muted).child(vis))
                        .child(
                            div()
                                .text_xs()
                                .text_color(dc.accent)
                                .child(format!("*{}", stars)),
                        ),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(dc.muted)
                        .overflow_hidden()
                        .child(desc.to_string()),
                ),
        )
}

fn dialog_title_gpui(title: &str, colors: DialogColors) -> Div {
    div()
        .text_sm()
        .font_weight(FontWeight::BOLD)
        .text_color(colors.text)
        .child(title.to_string())
}

pub fn confirm_search(
    app: &mut crate::views::app::GitForgeApp,
    input: &str,
    provider: String,
    cx: &mut Context<crate::views::app::GitForgeApp>,
) {
    if input.is_empty() {
        return;
    }
    app.search_hosting_repos(input.to_string(), provider, cx);
}
