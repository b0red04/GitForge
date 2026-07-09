use gitforge_ui::{AppColors, Theme};
use gpui::*;

use super::super::settings_window::{SettingsDraft, SettingsSection, SettingsWindow};
use crate::views::app::GitForgeApp;

impl GitForgeApp {
    pub fn set_theme(&mut self, name: &str, cx: &mut Context<Self>) {
        match Theme::load_by_name(name) {
            Ok(theme) => {
                self.colors = AppColors::from_theme(&theme);
                self.settings.theme = name.to_string();
                self.settings.save();
                self.push_settings_window_theme(cx);
                cx.notify();
            }
            Err(e) => {
                tracing::warn!("Failed to load theme '{}': {}", name, e);
            }
        }
    }

    pub(crate) fn push_settings_window_theme(&mut self, cx: &mut Context<Self>) {
        let Some(handle) = self.settings_window else {
            return;
        };
        let colors = self.colors.clone();
        let theme = self.settings.theme.clone();
        cx.spawn(async move |_, cx| {
            cx.update(|cx| {
                handle
                    .update(cx, |settings, _, cx| {
                        settings.draft.theme = theme;
                        settings.sync_colors(colors, cx);
                    })
                    .ok();
            })
            .ok();
        })
        .detach();
    }

    pub(crate) fn cycle_theme(&mut self, cx: &mut Context<Self>) {
        let themes = Theme::discover_themes();
        if themes.is_empty() {
            return;
        }
        let next = themes
            .iter()
            .position(|t| t.name == self.settings.theme)
            .map(|idx| (idx + 1) % themes.len())
            .unwrap_or(0);
        self.set_theme(&themes[next].name, cx);
    }

    pub fn hosting_accounts_snapshot(&self) -> Vec<gitforge_hosting::HostingAccount> {
        self.hosting_accounts.clone()
    }

    pub fn clear_settings_window_handle(&mut self) {
        self.settings_window = None;
    }

    pub(crate) fn notify_settings_window(&mut self, cx: &mut Context<Self>) {
        let Some(handle) = self.settings_window else {
            return;
        };
        let repo_data = self.settings_repo_data();
        let accounts = self.hosting_accounts_snapshot();
        let colors = self.colors.clone();
        let theme = self.settings.theme.clone();
        cx.spawn(async move |_, cx| {
            cx.update(|cx| {
                handle
                    .update(cx, |settings, _, cx| {
                        settings.draft.theme = theme;
                        settings.sync_colors(colors, cx);
                        settings.refresh_snapshot(repo_data, accounts);
                        cx.notify();
                    })
                    .ok();
            })
            .ok();
        })
        .detach();
    }

    pub fn open_settings_window(
        &mut self,
        section: Option<SettingsSection>,
        cx: &mut Context<Self>,
    ) {
        let initial_section = section.unwrap_or(SettingsSection::General);

        if let Some(handle) = self.settings_window {
            let draft = SettingsDraft::from_settings(&self.settings);
            let colors = self.colors.clone();
            let repo_data = self.settings_repo_data();
            let accounts = self.hosting_accounts_snapshot();
            if handle
                .update(cx, |settings, window, cx| {
                    window.activate_window();
                    settings.draft = draft;
                    settings.sync_colors(colors, cx);
                    settings.refresh_snapshot(repo_data, accounts);
                    settings.set_section(initial_section, cx);
                    settings.bootstrap_ai(cx);
                })
                .is_ok()
            {
                return;
            }
            self.settings_window = None;
        }

        let draft = SettingsDraft::from_settings(&self.settings);
        let colors = self.colors.clone();
        let repo_data = self.settings_repo_data();
        let accounts = self.hosting_accounts_snapshot();
        let main = cx.entity().downgrade();
        let window_bounds =
            WindowBounds::Windowed(Bounds::centered(None, size(px(900.0), px(700.0)), cx));

        match cx.open_window(
            WindowOptions {
                window_bounds: Some(window_bounds),
                titlebar: None,
                window_decorations: Some(WindowDecorations::Client),
                window_background: WindowBackgroundAppearance::Transparent,
                app_id: Some("dev.gitforge.GitForge".into()),
                focus: true,
                ..Default::default()
            },
            |window, cx| {
                cx.bind_keys([
                    KeyBinding::new(
                        "escape",
                        super::super::settings_window::CloseSettingsWindow,
                        None,
                    ),
                    KeyBinding::new("ctrl-v", super::super::settings_window::PasteApiKey, None),
                    KeyBinding::new("cmd-v", super::super::settings_window::PasteApiKey, None),
                ]);
                let view =
                    cx.new(|cx| SettingsWindow::new(main, colors, draft, initial_section, cx));
                view.update(cx, |settings, cx| settings.bootstrap_ai(cx));
                view.focus_handle(cx).focus(window);
                view
            },
        ) {
            Ok(handle) => {
                let _ = handle.update(cx, |settings, _, cx| {
                    settings.refresh_snapshot(repo_data, accounts);
                    settings.bootstrap_ai(cx);
                    cx.notify();
                });
                self.settings_window = Some(handle);
            }
            Err(e) => tracing::error!("Failed to open settings window: {}", e),
        }
        cx.notify();
    }

    pub fn apply_settings_from_window(&mut self, draft: &SettingsDraft, cx: &mut Context<Self>) {
        let prev_checkpoint = self.settings.show_checkpoint_refs;
        let prev_commit_limit = self.settings.commit_limit;
        let prev_graph_col = self.settings.graph_show_graph_column;
        let prev_sha_col = self.settings.graph_show_sha_column;
        let prev_time_col = self.settings.graph_show_time_column;
        let prev_author_col = self.settings.graph_show_author_column;
        let prev_periodic = self.active_repo_behavior_settings();
        let prev_auto_update = self.settings.auto_update;
        draft.apply_to(&mut self.settings);
        self.repo_session
            .sidebar_state
            .apply_persisted_from_settings(&self.settings);
        self.set_theme(&draft.theme, cx);
        self.save_settings();
        let columns_changed = draft.graph_show_graph_column != prev_graph_col
            || draft.graph_show_sha_column != prev_sha_col
            || draft.graph_show_time_column != prev_time_col
            || draft.graph_show_author_column != prev_author_col;
        let cur_periodic = self.active_repo_behavior_settings();
        let periodic_changed = prev_periodic.periodic_fetch_enabled
            != cur_periodic.periodic_fetch_enabled
            || prev_periodic.fetch_interval_minutes != cur_periodic.fetch_interval_minutes;
        if draft.show_checkpoint_refs != prev_checkpoint || draft.commit_limit != prev_commit_limit
        {
            self.refresh_repository(cx);
        } else if columns_changed {
            cx.notify();
        }
        if periodic_changed {
            self.restart_periodic_fetch(cx);
        }
        if draft.auto_update != prev_auto_update {
            gitforge_update::set_auto_update_enabled(self.settings.auto_update, cx);
        }
        cx.notify();
    }
}
