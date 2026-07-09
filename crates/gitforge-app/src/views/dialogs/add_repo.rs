use gitforge_ui::{
    AppColors, DialogColors, TextInput, TextInputEvent, TextInputRenderOpts,
    attach_dialog_input_keys, dialog_label, dialog_overlay, dialog_surface, render_text_input,
    rgba_to_hsla,
};
use gpui::*;

use crate::views::app::GitForgeApp;
use crate::views::dialogs::hosting_browse::render_repo_row;

/// Active tab in the unified AddRepo dialog. Lives on `GitForgeApp` so the
/// renderer can switch tabs without rebuilding the `AppDialog` payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AddRepoTab {
    Local,
    /// Provider id (e.g. "github", "gitlab", "codeberg"). The app keeps at
    /// most one account per provider, so this uniquely identifies a tab.
    Account(String),
}

/// Pretty, capitalised label for an account tab: `GitHub — alice`.
fn account_tab_label(provider: &str, username: &str) -> String {
    let name = match provider {
        "github" => "GitHub",
        "gitlab" => "GitLab",
        "codeberg" => "Codeberg",
        other => other,
    };
    format!("{name} — {username}")
}

#[allow(clippy::too_many_arguments)]
pub fn render(
    colors: &AppColors,
    entity: WeakEntity<GitForgeApp>,
    window: &mut Window,
    accounts: &[gitforge_hosting::HostingAccount],
    active_tab: &AddRepoTab,
    url_input: &TextInput,
    repos: &[gitforge_hosting::RemoteRepo],
    repos_loading: bool,
) -> Stateful<Div> {
    let dc = DialogColors::from_app(colors);
    let surface_high = rgba_to_hsla(colors.surface_high);

    let ent_close = entity.clone();
    let title_row = div()
        .flex()
        .items_center()
        .justify_between()
        .child(
            div()
                .text_sm()
                .font_weight(FontWeight::BOLD)
                .text_color(dc.text)
                .child("Add Repository"),
        )
        .child(
            div()
                .id("add-repo-close")
                .flex()
                .items_center()
                .justify_center()
                .size(px(20.0))
                .rounded(px(3.0))
                .cursor_pointer()
                .hover(move |s| s.bg(surface_high))
                .child(
                    svg()
                        .size(px(14.0))
                        .path("icons/x.svg")
                        .text_color(dc.muted),
                )
                .on_click(move |_ev, _window, cx| {
                    if let Some(e) = ent_close.upgrade() {
                        e.update(cx, |this, cx| this.cancel_dialog(cx));
                    }
                }),
        );

    // Tab strip: "Local" + one per connected account. All connected
    // providers are listed; the active tab gets an accent border + label.
    let mut tab_strip = div().flex().flex_row().gap_1().w_full();
    tab_strip = tab_strip.child(render_tab_button(
        "add-repo-tab-local",
        "Local",
        matches!(active_tab, AddRepoTab::Local),
        dc,
        surface_high,
        entity.clone(),
        AddRepoTab::Local,
    ));
    for (i, account) in accounts.iter().enumerate() {
        let label = account_tab_label(&account.provider, &account.username);
        let tab = AddRepoTab::Account(account.provider.clone());
        tab_strip = tab_strip.child(render_tab_button(
            ElementId::NamedInteger("add-repo-tab-account".into(), i as u64),
            label,
            active_tab == &tab,
            dc,
            surface_high,
            entity.clone(),
            tab,
        ));
    }

    let body = match active_tab {
        AddRepoTab::Local => render_local_tab(
            colors,
            dc,
            surface_high,
            entity.clone(),
            window,
            url_input,
            accounts.is_empty(),
        ),
        AddRepoTab::Account(_) => {
            render_account_tab(colors, dc, entity.clone(), repos, repos_loading)
        }
    };

    let content = dialog_surface(px(540.0), dc)
        .max_h(px(540.0))
        .gap_2()
        .child(title_row)
        .child(tab_strip)
        .child(body);

    dialog_overlay(dc).child(content)
}

fn render_tab_button(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    active: bool,
    dc: DialogColors,
    hover_bg: Hsla,
    entity: WeakEntity<GitForgeApp>,
    tab: AddRepoTab,
) -> Stateful<Div> {
    div()
        .id(id.into())
        .px_2()
        .py_1()
        .rounded(px(3.0))
        .border_1()
        .border_color(if active { dc.accent } else { dc.border })
        .bg(if active { hover_bg } else { dc.surface })
        .cursor_pointer()
        .text_xs()
        .text_color(if active { dc.accent } else { dc.muted })
        .hover(move |s| s.bg(hover_bg))
        .on_click(move |_ev, _window, cx| {
            if let Some(e) = entity.upgrade() {
                let tab = tab.clone();
                e.update(cx, |app, cx| app.switch_add_repo_tab(tab, cx));
            }
        })
        .child(label.into())
}

fn render_local_tab(
    colors: &AppColors,
    dc: DialogColors,
    surface_high: Hsla,
    entity: WeakEntity<GitForgeApp>,
    window: &mut Window,
    url_input: &TextInput,
    no_accounts: bool,
) -> Stateful<Div> {
    let ent_folder = entity.clone();
    let open_folder = div()
        .id("add-repo-open-folder")
        .w_full()
        .py_2()
        .rounded(px(3.0))
        .border_1()
        .border_color(dc.accent)
        .text_sm()
        .text_color(dc.accent)
        .flex()
        .items_center()
        .justify_center()
        .gap_2()
        .cursor_pointer()
        .hover(move |s| s.bg(surface_high))
        .child(
            svg()
                .size(px(16.0))
                .path("icons/folder.svg")
                .text_color(dc.accent),
        )
        .child(SharedString::from("Open Folder…"))
        .on_click(move |_ev, _window, cx| {
            if let Some(e) = ent_folder.upgrade() {
                e.update(cx, |app, cx| app.add_repo_open_local_folder(cx));
            }
        });

    // URL input. Reuses the shared `dialog_input`, so key handling mirrors
    // the standalone CloneRepo dialog: Enter clones, Escape closes.
    let url_field = attach_dialog_input_keys(
        render_text_input(
            url_input,
            colors,
            window,
            &TextInputRenderOpts::new(ElementId::Name("add-repo-url-input".into()))
                .placeholder("URL destination-path"),
            |_| {},
        ),
        entity.clone(),
        |this, cx, _window, event| match event {
            TextInputEvent::Enter { .. } => this.add_repo_clone_from_url(cx),
            TextInputEvent::Escape => this.cancel_dialog(cx),
            TextInputEvent::Backspace => this.edit_dialog_input(None, cx),
            TextInputEvent::Typed(c) => this.edit_dialog_input(Some(&c), cx),
            _ => {}
        },
    );

    let ent_clone = entity.clone();
    let clone_button = div()
        .id("add-repo-clone")
        .px_3()
        .py_1()
        .flex_none()
        .rounded(px(3.0))
        .border_1()
        .border_color(dc.accent)
        .text_sm()
        .text_color(dc.accent)
        .cursor_pointer()
        .hover(move |s| s.bg(surface_high))
        .child(SharedString::from("Clone"))
        .on_click(move |_ev, _window, cx| {
            if let Some(e) = ent_clone.upgrade() {
                e.update(cx, |app, cx| app.add_repo_clone_from_url(cx));
            }
        });

    let url_row = div()
        .flex()
        .flex_row()
        .gap_1()
        .child(url_field.flex_1())
        .child(clone_button);

    let mut body = div()
        .id("add-repo-local-body")
        .flex()
        .flex_col()
        .gap_2()
        .child(open_folder)
        .child(dialog_label("Or clone from URL:", dc))
        .child(url_row);

    if no_accounts {
        let ent_settings = entity.clone();
        body = body
            .child(dialog_label(
                "No connected accounts. Connect one to browse remote repos.",
                dc,
            ))
            .child(
                div()
                    .id("add-repo-settings")
                    .flex_none()
                    .px_2()
                    .py_1()
                    .rounded(px(3.0))
                    .border_1()
                    .border_color(dc.border)
                    .text_xs()
                    .text_color(dc.muted)
                    .cursor_pointer()
                    .hover(move |s| s.bg(surface_high))
                    .child(SharedString::from("Open Settings"))
                    .on_click(move |_ev, _window, cx| {
                        if let Some(e) = ent_settings.upgrade() {
                            e.update(cx, |app, cx| app.add_repo_open_settings(cx));
                        }
                    }),
            );
    }

    body
}

fn render_account_tab(
    colors: &AppColors,
    dc: DialogColors,
    entity: WeakEntity<GitForgeApp>,
    repos: &[gitforge_hosting::RemoteRepo],
    repos_loading: bool,
) -> Stateful<Div> {
    if repos_loading {
        return div()
            .id("add-repo-account-loading")
            .child(dialog_label("Loading repositories...", dc));
    }
    if repos.is_empty() {
        return div()
            .id("add-repo-account-empty")
            .child(dialog_label("No repositories found.", dc));
    }
    let mut list = div()
        .id("add-repo-repo-list")
        .flex()
        .flex_col()
        .gap_1()
        .max_h(px(400.0))
        .overflow_y_scroll();
    for (i, repo) in repos.iter().enumerate() {
        list = list.child(render_repo_row(repo, i, colors, dc, entity.clone()));
    }
    list
}
