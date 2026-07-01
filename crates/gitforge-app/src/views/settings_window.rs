use super::app::GitForgeApp;
use super::layout::{TITLEBAR_HEIGHT, WINDOW_CORNER_RADIUS};
use super::settings::{AppSettings, RepoBehaviorSettings};
use super::window_chrome::{apply_top_corner_radius, seal_rounded_corners};
use gitforge_ui::{
    AppColors, Appearance, TextInput, TextInputDisplay, TextInputEvent, TextInputRenderOpts, Theme,
    ThemeEntry, modifier_keys_prevent_typing, parse_key_event, render_static_text_input,
    render_text_input, rgba_to_hsla, typed_character, window_control_button,
};
use gpui::prelude::FluentBuilder;
use gpui::*;
use std::path::{Path, PathBuf};

actions!(settings_window, [CloseSettingsWindow, PasteApiKey]);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsSection {
    General,
    ExternalTools,
    Sidebar,
    Graph,
    Ai,
    Repositories,
    Accounts,
    About,
}

impl SettingsSection {
    pub const ALL: [SettingsSection; 8] = [
        SettingsSection::General,
        SettingsSection::ExternalTools,
        SettingsSection::Sidebar,
        SettingsSection::Graph,
        SettingsSection::Ai,
        SettingsSection::Repositories,
        SettingsSection::Accounts,
        SettingsSection::About,
    ];

    fn label(self) -> &'static str {
        match self {
            SettingsSection::General => "General",
            SettingsSection::ExternalTools => "External Tools",
            SettingsSection::Sidebar => "Sidebar",
            SettingsSection::Graph => "Graph",
            SettingsSection::Ai => "AI",
            SettingsSection::Repositories => "Repositories",
            SettingsSection::Accounts => "Accounts",
            SettingsSection::About => "About",
        }
    }

    fn matches_search(self, query: &str) -> bool {
        if query.is_empty() {
            return true;
        }
        self.label().to_lowercase().contains(query)
            || self.search_keywords().iter().any(|k| k.contains(query))
    }

    fn search_keywords(self) -> Vec<String> {
        let words: &[&str] = match self {
            SettingsSection::General => &[
                "theme",
                "appearance",
                "dark",
                "light",
                "tokyo",
                "everforest",
                "ayu",
                "catppuccin",
                "gruvbox",
                "kanagawa",
                "nord",
                "one",
                "matrix",
            ],
            SettingsSection::ExternalTools => &["editor", "terminal", "diff", "merge", "tool"],
            SettingsSection::Sidebar => &["branches", "remotes", "tags", "expand"],
            SettingsSection::Graph => &[
                "checkpoint",
                "refs",
                "graph",
                "commit",
                "limit",
                "column",
                "sha",
                "time",
                "author",
                "visible",
                "hide",
                "show",
            ],
            SettingsSection::Ai => &[
                "provider",
                "model",
                "ollama",
                "openai",
                "anthropic",
                "zai",
                "glm",
                "coding",
                "api",
                "tone",
                "options",
                "variation",
                "temperature",
                "diff",
                "preset",
                "commit",
            ],
            SettingsSection::Repositories => &["recent", "tabs", "repo", "open", "closed"],
            SettingsSection::Accounts => &["github", "gitlab", "codeberg", "hosting", "pat"],
            SettingsSection::About => &["version", "update", "about", "release"],
        };
        words.iter().map(|s| s.to_string()).collect()
    }
}

#[derive(Clone, Debug)]
pub struct SettingsDraft {
    pub theme: String,
    pub editor_command: String,
    pub terminal_command: String,
    pub diff_tool: String,
    pub merge_tool: String,
    pub sidebar_branches_expanded: bool,
    pub sidebar_remotes_expanded: bool,
    pub sidebar_tags_expanded: bool,
    pub sidebar_pull_requests_expanded: bool,
    pub show_checkpoint_refs: bool,
    pub commit_limit: usize,
    pub commit_limit_text: String,
    pub graph_show_graph_column: bool,
    pub graph_show_sha_column: bool,
    pub graph_show_time_column: bool,
    pub graph_show_author_column: bool,
    pub ai_provider: String,
    pub ai_model: String,
    pub ai_conventional_commits: bool,
    pub ai_tone: String,
    pub ai_ollama_url: String,
    pub ai_zai_endpoint: String,
    pub ai_message_options_count: u8,
    pub ai_variation_mode: String,
    pub ai_default_alternative: String,
    pub ai_summary_max_chars: u32,
    pub ai_body_wrap_at: u32,
    pub ai_max_diff_chars: usize,
    pub ai_temperature: f32,
    /// In-session text for custom temperature entry (not persisted directly).
    pub ai_temperature_text: String,
    /// In-session text for custom summary line limit (not persisted directly).
    pub ai_summary_text: String,
    pub repo_path: Option<PathBuf>,
    pub repo_periodic_fetch_enabled: bool,
    pub repo_fetch_interval_minutes: u64,
    pub repo_fetch_interval_text: String,
    pub repo_auto_push_on_commit: bool,
    pub auto_update: bool,
}

#[derive(Clone, Debug, Default)]
pub struct AiSettingsUiState {
    pub api_key_configured: bool,
    pub available_models: Vec<String>,
    pub models_loading: bool,
    pub models_error: Option<String>,
}

impl SettingsDraft {
    pub fn from_settings(settings: &AppSettings) -> Self {
        let repo_defaults = RepoBehaviorSettings::default();
        Self {
            theme: settings.theme.clone(),
            editor_command: settings.tools.editor_command.clone(),
            terminal_command: settings.tools.terminal_command.clone(),
            diff_tool: settings.tools.diff_tool.clone(),
            merge_tool: settings.tools.merge_tool.clone(),
            sidebar_branches_expanded: settings.sidebar_branches_expanded,
            sidebar_remotes_expanded: settings.sidebar_remotes_expanded,
            sidebar_tags_expanded: settings.sidebar_tags_expanded,
            sidebar_pull_requests_expanded: settings.sidebar_pull_requests_expanded,
            show_checkpoint_refs: settings.show_checkpoint_refs,
            commit_limit: settings.commit_limit,
            commit_limit_text: settings.commit_limit.to_string(),
            graph_show_graph_column: settings.graph_show_graph_column,
            graph_show_sha_column: settings.graph_show_sha_column,
            graph_show_time_column: settings.graph_show_time_column,
            graph_show_author_column: settings.graph_show_author_column,
            ai_provider: settings.ai.provider.clone(),
            ai_model: settings.ai.model.clone(),
            ai_conventional_commits: settings.ai.conventional_commits,
            ai_tone: settings.ai.tone.clone(),
            ai_ollama_url: settings.ai.ollama_url.clone(),
            ai_zai_endpoint: settings.ai.zai_endpoint.clone(),
            ai_message_options_count: settings.ai.message_options_count,
            ai_variation_mode: settings.ai.variation_mode.clone(),
            ai_default_alternative: settings.ai.default_alternative.clone(),
            ai_summary_max_chars: settings.ai.summary_max_chars,
            ai_body_wrap_at: settings.ai.body_wrap_at,
            ai_max_diff_chars: settings.ai.max_diff_chars,
            ai_temperature: settings.ai.temperature,
            ai_temperature_text: format!("{:.2}", settings.ai.temperature),
            ai_summary_text: if settings.ai.summary_max_chars == 0 {
                String::new()
            } else {
                settings.ai.summary_max_chars.to_string()
            },
            repo_path: None,
            repo_periodic_fetch_enabled: repo_defaults.periodic_fetch_enabled,
            repo_fetch_interval_minutes: repo_defaults.fetch_interval_minutes,
            repo_fetch_interval_text: repo_defaults.fetch_interval_minutes.to_string(),
            repo_auto_push_on_commit: repo_defaults.auto_push_on_commit,
            auto_update: settings.auto_update,
        }
    }

    pub fn sync_repo_settings(&mut self, path: Option<PathBuf>, settings: RepoBehaviorSettings) {
        self.repo_path = path;
        self.repo_periodic_fetch_enabled = settings.periodic_fetch_enabled;
        self.repo_fetch_interval_minutes = settings.fetch_interval_minutes.max(1);
        self.repo_fetch_interval_text = self.repo_fetch_interval_minutes.to_string();
        self.repo_auto_push_on_commit = settings.auto_push_on_commit;
    }

    pub fn apply_to(&self, settings: &mut AppSettings) {
        settings.theme = self.theme.clone();
        settings.tools.editor_command = self.editor_command.clone();
        settings.tools.terminal_command = self.terminal_command.clone();
        settings.tools.diff_tool = self.diff_tool.clone();
        settings.tools.merge_tool = self.merge_tool.clone();
        settings.sidebar_branches_expanded = self.sidebar_branches_expanded;
        settings.sidebar_remotes_expanded = self.sidebar_remotes_expanded;
        settings.sidebar_tags_expanded = self.sidebar_tags_expanded;
        settings.sidebar_pull_requests_expanded = self.sidebar_pull_requests_expanded;
        settings.show_checkpoint_refs = self.show_checkpoint_refs;
        if let Ok(parsed) = self.commit_limit_text.parse::<usize>() {
            settings.commit_limit = parsed.max(1);
        }
        settings.graph_show_graph_column = self.graph_show_graph_column;
        settings.graph_show_sha_column = self.graph_show_sha_column;
        settings.graph_show_time_column = self.graph_show_time_column;
        settings.graph_show_author_column = self.graph_show_author_column;
        settings.ai.provider = self.ai_provider.clone();
        settings.ai.model = self.ai_model.clone();
        settings.ai.conventional_commits = self.ai_conventional_commits;
        settings.ai.tone = self.ai_tone.clone();
        settings.ai.ollama_url = self.ai_ollama_url.clone();
        settings.ai.zai_endpoint = self.ai_zai_endpoint.clone();
        settings.ai.message_options_count =
            gitforge_ai::clamp_options_count(self.ai_message_options_count);
        settings.ai.variation_mode = self.ai_variation_mode.clone();
        settings.ai.default_alternative = self.ai_default_alternative.clone();
        settings.ai.summary_max_chars = self.ai_summary_max_chars;
        settings.ai.body_wrap_at = self.ai_body_wrap_at;
        settings.ai.max_diff_chars = self.ai_max_diff_chars;
        if let Ok(parsed) = self.ai_temperature_text.parse::<f32>() {
            settings.ai.temperature = gitforge_ai::clamp_temperature(parsed);
        } else {
            settings.ai.temperature = gitforge_ai::clamp_temperature(self.ai_temperature);
        }
        if let Ok(parsed) = self.ai_summary_text.parse::<u32>() {
            settings.ai.summary_max_chars = parsed;
        } else if self.ai_summary_text.trim().is_empty() {
            settings.ai.summary_max_chars = self.ai_summary_max_chars;
        }

        if let Some(path) = self.repo_path.as_ref() {
            let repo = settings.repo_settings_for_path_mut(path);
            repo.periodic_fetch_enabled = self.repo_periodic_fetch_enabled;
            repo.fetch_interval_minutes = self.repo_fetch_interval_minutes.max(1);
            repo.auto_push_on_commit = self.repo_auto_push_on_commit;
        }
        settings.auto_update = self.auto_update;
    }
}

fn edit_repo_fetch_interval_field(draft: &mut SettingsDraft, ch: Option<&str>) {
    if let Some(c) = ch {
        if c.chars().all(|c| c.is_ascii_digit()) {
            draft.repo_fetch_interval_text.push_str(c);
        }
    } else {
        draft.repo_fetch_interval_text.pop();
    }
    if let Ok(parsed) = draft.repo_fetch_interval_text.parse::<u64>() {
        draft.repo_fetch_interval_minutes = parsed.max(1);
    }
}

fn edit_ai_temperature_field(draft: &mut SettingsDraft, ch: Option<&str>) {
    if let Some(c) = ch {
        draft.ai_temperature_text.push_str(c);
    } else {
        draft.ai_temperature_text.pop();
    }
    if let Ok(parsed) = draft.ai_temperature_text.parse::<f32>() {
        draft.ai_temperature = gitforge_ai::clamp_temperature(parsed);
    }
}

fn edit_ai_summary_max_chars_field(draft: &mut SettingsDraft, ch: Option<&str>) {
    if let Some(c) = ch {
        draft.ai_summary_text.push_str(c);
    } else {
        draft.ai_summary_text.pop();
    }
    if let Ok(parsed) = draft.ai_summary_text.parse::<u32>() {
        draft.ai_summary_max_chars = parsed;
    }
}

fn edit_commit_limit_field(draft: &mut SettingsDraft, ch: Option<&str>) {
    if let Some(c) = ch {
        if c.chars().all(|c| c.is_ascii_digit()) {
            draft.commit_limit_text.push_str(c);
        }
    } else {
        draft.commit_limit_text.pop();
    }
    if let Ok(parsed) = draft.commit_limit_text.parse::<usize>() {
        draft.commit_limit = parsed.max(1);
    }
}

fn max_diff_label(chars: usize) -> &'static str {
    match chars {
        8192 => "8k",
        16384 => "16k",
        32768 => "32k",
        _ => "unlimited",
    }
}

fn max_diff_from_label(label: &str) -> usize {
    match label {
        "8k" => 8192,
        "16k" => 16384,
        "32k" => 32768,
        _ => 0,
    }
}

fn body_wrap_label(wrap: u32) -> &'static str {
    if wrap == 0 { "none" } else { "72" }
}

fn summary_dropdown_value(chars: u32) -> String {
    match chars {
        0 => "auto".to_string(),
        50 => "50".to_string(),
        72 => "72".to_string(),
        _ => "custom".to_string(),
    }
}

fn commit_preset_control(
    colors: &AppColors,
    entity: WeakEntity<SettingsWindow>,
) -> impl IntoElement {
    let border = rgba_to_hsla(colors.border);
    let text = rgba_to_hsla(colors.text);
    let surface = rgba_to_hsla(colors.surface);

    let presets = [
        ("Quick", "quick"),
        ("Standard", "standard"),
        ("Detailed", "detailed"),
    ];
    let mut row = div().flex().gap_1();
    for (label, id) in presets {
        let ent = entity.clone();
        row = row.child(
            div()
                .id(ElementId::Name(format!("commit-preset-{}", id).into()))
                .px_2()
                .py_1()
                .border_1()
                .border_color(border)
                .rounded(px(3.0))
                .bg(surface)
                .cursor_pointer()
                .text_xs()
                .text_color(text)
                .child(label)
                .on_click(move |_ev, _window, cx| {
                    if let Some(e) = ent.upgrade() {
                        e.update(cx, |this, cx| {
                            this.patch_draft(
                                |draft| match id {
                                    "quick" => {
                                        draft.ai_message_options_count = 1;
                                        draft.ai_tone = "concise".to_string();
                                    }
                                    "standard" => {
                                        draft.ai_message_options_count = 3;
                                        draft.ai_variation_mode = "mixed".to_string();
                                        draft.ai_tone = "balanced".to_string();
                                        draft.ai_default_alternative = "first".to_string();
                                    }
                                    "detailed" => {
                                        draft.ai_message_options_count = 1;
                                        draft.ai_tone = "verbose".to_string();
                                    }
                                    _ => {}
                                },
                                cx,
                            );
                        });
                    }
                }),
        );
    }
    row
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SettingsTextField {
    Editor,
    Terminal,
    DiffTool,
    MergeTool,
    AiModel,
    AiOllamaUrl,
    AiApiKey,
    AiTemperature,
    AiSummaryMaxChars,
    CommitLimit,
    RepoFetchInterval,
    Search,
    HostingPat,
}

pub struct SettingsWindow {
    main_app: WeakEntity<GitForgeApp>,
    colors: AppColors,
    pub draft: SettingsDraft,
    repo_data: SettingsRepoData,
    accounts: Vec<gitforge_hosting::HostingAccount>,
    active_section: SettingsSection,
    search_input: TextInput,
    focused_field: SettingsTextField,
    input_focus: FocusHandle,
    api_key_input: TextInput,
    api_key_configured: bool,
    pending_account_provider: Option<String>,
    pat_input: TextInput,
    pat_error: Option<String>,
    focus_handle: FocusHandle,
    available_models: Vec<String>,
    models_loading: bool,
    models_error: Option<String>,
}

impl SettingsWindow {
    pub fn new(
        main_app: WeakEntity<GitForgeApp>,
        colors: AppColors,
        draft: SettingsDraft,
        initial_section: SettingsSection,
        cx: &mut App,
    ) -> Self {
        Self {
            main_app,
            colors,
            draft,
            repo_data: SettingsRepoData {
                open_tabs: Vec::new(),
                active_path: None,
                active_settings: RepoBehaviorSettings::default(),
                recent_paths: Vec::new(),
                closed_paths: Vec::new(),
            },
            accounts: Vec::new(),
            active_section: initial_section,
            search_input: TextInput::new("Search settings...", cx),
            focused_field: SettingsTextField::Editor,
            input_focus: cx.focus_handle(),
            api_key_input: TextInput::new("API key", cx)
                .with_display(TextInputDisplay::MaskedBullets),
            api_key_configured: false,
            pending_account_provider: None,
            pat_input: TextInput::new("Personal Access Token", cx)
                .with_display(TextInputDisplay::MaskedWithCursor),
            pat_error: None,
            focus_handle: cx.focus_handle(),
            available_models: Vec::new(),
            models_loading: false,
            models_error: None,
        }
    }

    pub fn bootstrap_ai(&mut self, cx: &mut Context<Self>) {
        self.refresh_api_key_state();
        self.fetch_models_if_applicable(cx);
    }

    fn ai_ui_state(&self) -> AiSettingsUiState {
        AiSettingsUiState {
            api_key_configured: self.api_key_configured,
            available_models: self.available_models.clone(),
            models_loading: self.models_loading,
            models_error: self.models_error.clone(),
        }
    }

    fn refresh_api_key_state(&mut self) {
        let provider = self.draft.ai_provider.as_str();
        self.api_key_configured =
            provider_needs_api_key(provider) && gitforge_ai::has_api_key(provider);
    }

    fn save_api_key(&mut self, cx: &mut Context<Self>) {
        let provider = self.draft.ai_provider.clone();
        let key = self.api_key_input.text().trim().to_string();
        if key.is_empty() {
            self.models_error = Some("Enter an API key before saving.".into());
            cx.notify();
            return;
        }
        if let Err(e) = gitforge_ai::store_api_key(&provider, &key) {
            self.models_error = Some(format!("Failed to store API key: {e}"));
            self.api_key_configured = false;
            cx.notify();
            return;
        }
        self.api_key_input.clear();
        self.refresh_api_key_state();
        if !self.api_key_configured {
            let path = dirs::config_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("."))
                .join("gitforge")
                .join("ai-credentials.json");
            self.models_error = Some(format!(
                "API key could not be read back after saving. Check permissions on {}.",
                path.display()
            ));
            cx.notify();
            return;
        }
        self.models_error = None;
        self.fetch_models_if_applicable(cx);
        cx.notify();
    }

    fn clear_api_key(&mut self, cx: &mut Context<Self>) {
        let provider = self.draft.ai_provider.clone();
        if provider_needs_api_key(&provider) {
            let _ = gitforge_ai::delete_api_key(&provider);
        }
        self.api_key_input.clear();
        self.api_key_configured = false;
        self.available_models.clear();
        self.models_error = None;
        cx.notify();
    }

    fn on_provider_changed(&mut self, cx: &mut Context<Self>) {
        self.api_key_input.clear();
        self.available_models.clear();
        self.models_error = None;
        self.models_loading = false;
        self.refresh_api_key_state();
        self.fetch_models_if_applicable(cx);
        cx.notify();
    }

    fn fetch_models_if_applicable(&mut self, cx: &mut Context<Self>) {
        let provider = self.draft.ai_provider.clone();
        if !provider_supports_model_list(&provider) {
            return;
        }
        if provider_needs_api_key(&provider) && !gitforge_ai::has_api_key(&provider) {
            return;
        }
        self.fetch_models(cx);
    }

    fn fetch_models(&mut self, cx: &mut Context<Self>) {
        let provider = self.draft.ai_provider.clone();
        let ollama_url = self.draft.ai_ollama_url.clone();
        let zai_endpoint = gitforge_ai::parse_zai_endpoint(&self.draft.ai_zai_endpoint);

        let api_key = if provider_needs_api_key(&provider) {
            match gitforge_ai::get_api_key(&provider) {
                Ok(key) => Some(key),
                Err(e) => {
                    self.api_key_configured = false;
                    self.models_error = Some(format!(
                        "API key not available for {}: {e}. Enter your key and click Save.",
                        provider
                    ));
                    cx.notify();
                    return;
                }
            }
        } else {
            None
        };

        self.models_loading = true;
        self.models_error = None;
        cx.notify();

        cx.spawn(async move |this, cx| {
            let result = gitforge_ai::list_models_for_provider(
                &provider,
                &ollama_url,
                zai_endpoint,
                api_key,
            )
            .await;

            this.update(cx, |this, cx| {
                this.models_loading = false;
                match result {
                    Ok(models) => {
                        this.models_error = None;
                        this.available_models = models;
                        if this.draft.ai_model.is_empty() && !this.available_models.is_empty() {
                            this.draft.ai_model = this.available_models[0].clone();
                            this.commit(cx);
                        }
                    }
                    Err(e) => {
                        this.available_models.clear();
                        this.models_error = Some(e.user_message());
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub fn refresh_snapshot(
        &mut self,
        repo_data: SettingsRepoData,
        accounts: Vec<gitforge_hosting::HostingAccount>,
    ) {
        self.draft.sync_repo_settings(
            repo_data.active_path.clone(),
            repo_data.active_settings.clone(),
        );
        self.repo_data = repo_data;
        self.accounts = accounts;
    }

    pub fn set_section(&mut self, section: SettingsSection, cx: &mut Context<Self>) {
        let entering_ai =
            section == SettingsSection::Ai && self.active_section != SettingsSection::Ai;
        self.active_section = section;
        if entering_ai && !self.models_loading {
            self.fetch_models_if_applicable(cx);
        }
        cx.notify();
    }

    pub fn sync_colors(&mut self, colors: AppColors, cx: &mut Context<Self>) {
        self.colors = colors;
        cx.notify();
    }

    fn sync_colors_from_draft(&mut self, cx: &mut Context<Self>) {
        if let Ok(theme) = Theme::load_by_name(&self.draft.theme) {
            self.colors = AppColors::from_theme(&theme);
            cx.notify();
        }
    }

    fn commit(&mut self, cx: &mut Context<Self>) {
        let draft = self.draft.clone();
        if let Some(main) = self.main_app.upgrade() {
            main.update(cx, |app, cx| {
                app.apply_settings_from_window(&draft, cx);
            });
        }
    }

    fn patch_draft<F: FnOnce(&mut SettingsDraft)>(&mut self, f: F, cx: &mut Context<Self>) {
        f(&mut self.draft);
        self.sync_colors_from_draft(cx);
        self.commit(cx);
    }

    fn handle_close(
        &mut self,
        _: &CloseSettingsWindow,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(main) = self.main_app.upgrade() {
            main.update(cx, |app, cx| {
                app.clear_settings_window_handle();
                cx.notify();
            });
        }
        window.remove_window();
    }

    fn edit_focused_field(&mut self, ch: Option<&str>, cx: &mut Context<Self>) {
        if self.focused_field == SettingsTextField::AiApiKey {
            self.edit_api_key_field(ch, cx);
            return;
        }

        let field = self.focused_field;
        self.patch_draft(
            |draft| {
                let target = match field {
                    SettingsTextField::Editor => &mut draft.editor_command,
                    SettingsTextField::Terminal => &mut draft.terminal_command,
                    SettingsTextField::DiffTool => &mut draft.diff_tool,
                    SettingsTextField::MergeTool => &mut draft.merge_tool,
                    SettingsTextField::AiModel => &mut draft.ai_model,
                    SettingsTextField::AiOllamaUrl => &mut draft.ai_ollama_url,
                    SettingsTextField::AiTemperature => {
                        edit_ai_temperature_field(draft, ch);
                        return;
                    }
                    SettingsTextField::AiSummaryMaxChars => {
                        edit_ai_summary_max_chars_field(draft, ch);
                        return;
                    }
                    SettingsTextField::CommitLimit => {
                        edit_commit_limit_field(draft, ch);
                        return;
                    }
                    SettingsTextField::RepoFetchInterval => {
                        edit_repo_fetch_interval_field(draft, ch);
                        return;
                    }
                    SettingsTextField::AiApiKey
                    | SettingsTextField::Search
                    | SettingsTextField::HostingPat => &mut draft.editor_command,
                };
                if field == SettingsTextField::Search
                    || field == SettingsTextField::AiApiKey
                    || field == SettingsTextField::HostingPat
                {
                    return;
                }
                if let Some(c) = ch {
                    target.push_str(c);
                } else {
                    target.pop();
                }
            },
            cx,
        );
    }

    fn edit_search(&mut self, ch: Option<&str>, cx: &mut Context<Self>) {
        self.search_input.edit(ch);
        cx.notify();
    }

    fn edit_api_key_field(&mut self, ch: Option<&str>, cx: &mut Context<Self>) {
        self.api_key_input.edit(ch);
        cx.notify();
    }

    fn start_add_account(&mut self, provider: String, cx: &mut Context<Self>) {
        self.pending_account_provider = Some(provider);
        self.pat_input.clear();
        self.pat_error = None;
        self.focused_field = SettingsTextField::HostingPat;
        cx.notify();
    }

    fn cancel_add_account(&mut self, cx: &mut Context<Self>) {
        self.pending_account_provider = None;
        self.pat_input.clear();
        self.pat_error = None;
        cx.notify();
    }

    fn edit_pat_field(&mut self, ch: Option<&str>, cx: &mut Context<Self>) {
        self.pat_input.edit(ch);
        cx.notify();
    }

    fn append_pat_from_clipboard(&mut self, cx: &mut Context<Self>) {
        let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) else {
            return;
        };
        let text = text.replace(['\n', '\r'], "");
        if !text.is_empty() {
            self.pat_input.edit(Some(&text));
            cx.notify();
        }
    }

    fn save_hosting_account(&mut self, cx: &mut Context<Self>) {
        let Some(provider) = self.pending_account_provider.clone() else {
            return;
        };
        let token = self.pat_input.text().trim().to_string();
        if token.is_empty() {
            self.pat_error = Some("Enter a Personal Access Token before saving.".into());
            cx.notify();
            return;
        }
        if let Some(main) = self.main_app.upgrade() {
            main.update(cx, |app, cx| {
                app.add_hosting_account(provider, token, cx);
            });
        }
        self.pending_account_provider = None;
        self.pat_input.clear();
        self.pat_error = None;
        cx.notify();
    }

    fn append_api_key_from_clipboard(&mut self, cx: &mut Context<Self>) {
        let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) else {
            return;
        };
        let text = text.replace(['\n', '\r'], "");
        if !text.is_empty() {
            self.api_key_input.edit(Some(&text));
            cx.notify();
        }
    }

    fn handle_paste_api_key(&mut self, window: &Window, cx: &mut Context<Self>) {
        if self.pat_input.focus_handle().is_focused(window) {
            self.focused_field = SettingsTextField::HostingPat;
            self.append_pat_from_clipboard(cx);
            return;
        }
        if !self.api_key_input.focus_handle().is_focused(window) {
            return;
        }
        self.focused_field = SettingsTextField::AiApiKey;
        self.append_api_key_from_clipboard(cx);
    }

    fn handle_key_input(&mut self, ev: &KeyDownEvent, window: &Window, cx: &mut Context<Self>) {
        if self.pat_input.focus_handle().is_focused(window) {
            self.focused_field = SettingsTextField::HostingPat;
            if is_paste_keystroke(ev) {
                self.append_pat_from_clipboard(cx);
                return;
            }
            match ev.keystroke.key.as_str() {
                "enter" => self.save_hosting_account(cx),
                "backspace" => self.edit_pat_field(None, cx),
                "escape" => self.cancel_add_account(cx),
                _ => {
                    if let Some(ch) = typed_character(ev) {
                        if !modifier_keys_prevent_typing(&ev.keystroke.modifiers) {
                            self.edit_pat_field(Some(&ch), cx);
                        }
                    }
                }
            }
            return;
        }

        if self.api_key_input.focus_handle().is_focused(window) {
            self.focused_field = SettingsTextField::AiApiKey;
            if is_paste_keystroke(ev) {
                self.append_api_key_from_clipboard(cx);
                return;
            }
            match ev.keystroke.key.as_str() {
                "enter" => self.save_api_key(cx),
                "backspace" => self.edit_api_key_field(None, cx),
                "escape" => {
                    self.api_key_input.clear();
                    cx.notify();
                }
                _ => {
                    if let Some(ch) = typed_character(ev) {
                        if !modifier_keys_prevent_typing(&ev.keystroke.modifiers) {
                            self.edit_api_key_field(Some(&ch), cx);
                        }
                    }
                }
            }
            return;
        }

        if !self.input_focus.is_focused(window) {
            return;
        }

        if self.focused_field == SettingsTextField::Search {
            match ev.keystroke.key.as_str() {
                "backspace" => self.edit_search(None, cx),
                _ => {
                    if let Some(ch) = typed_character(ev) {
                        if !modifier_keys_prevent_typing(&ev.keystroke.modifiers) {
                            self.edit_search(Some(&ch), cx);
                        }
                    }
                }
            }
            return;
        }

        match ev.keystroke.key.as_str() {
            "backspace" => self.edit_focused_field(None, cx),
            _ => {
                if let Some(ch) = typed_character(ev) {
                    if !modifier_keys_prevent_typing(&ev.keystroke.modifiers) {
                        self.edit_focused_field(Some(&ch), cx);
                    }
                }
            }
        }
    }

    fn paste_api_key(&mut self, _: &PasteApiKey, window: &mut Window, cx: &mut Context<Self>) {
        self.handle_paste_api_key(window, cx);
    }
}

fn is_paste_keystroke(ev: &KeyDownEvent) -> bool {
    let key = ev.keystroke.key.as_str();
    if !key.eq_ignore_ascii_case("v") {
        return false;
    }
    let m = &ev.keystroke.modifiers;
    m.control || m.platform
}

fn provider_needs_api_key(provider: &str) -> bool {
    matches!(provider, "openai" | "anthropic" | "zai")
}

fn provider_supports_model_list(provider: &str) -> bool {
    matches!(provider, "openai" | "zai" | "ollama")
}

impl Focusable for SettingsWindow {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

fn render_settings_window_controls(
    window: &Window,
    icon_color: Hsla,
    icon_hover: Hsla,
    hover_bg: Hsla,
) -> Option<impl IntoElement> {
    if !matches!(window.window_decorations(), Decorations::Client { .. }) {
        return None;
    }

    let controls = window.window_controls();
    let max_icon = if window.is_maximized() {
        "icons/generic_restore.svg"
    } else {
        "icons/generic_maximize.svg"
    };

    let mut row = div()
        .id("settings-titlebar-window-controls")
        .flex()
        .flex_row()
        .items_center()
        .gap_3()
        .px_3()
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation());

    if controls.minimize {
        row = row.child(window_control_button(
            "settings-titlebar-minimize",
            "icons/generic_minimize.svg",
            icon_color,
            icon_hover,
            hover_bg,
            |_ev, window, _cx| window.minimize_window(),
        ));
    }

    if controls.maximize {
        row = row.child(window_control_button(
            "settings-titlebar-maximize",
            max_icon,
            icon_color,
            icon_hover,
            hover_bg,
            |_ev, window, _cx| window.zoom_window(),
        ));
    }

    row = row.child(window_control_button(
        "settings-titlebar-close",
        "icons/generic_close.svg",
        icon_color,
        icon_hover,
        hover_bg,
        |_ev, window, cx| window.dispatch_action(Box::new(CloseSettingsWindow), cx),
    ));

    Some(row)
}

fn render_settings_titlebar(colors: &AppColors, window: &Window) -> impl IntoElement {
    let decorations = window.window_decorations();
    let controls = window.window_controls();
    let titlebar_bg = if window.is_window_active() {
        rgba_to_hsla(colors.surface)
    } else {
        rgba_to_hsla(colors.surface_high)
    };
    let muted = rgba_to_hsla(colors.text_muted);
    let text = rgba_to_hsla(colors.text);
    let icon_hover = text;
    let hover_bg = rgba_to_hsla(colors.surface_high);
    let rounding = px(WINDOW_CORNER_RADIUS);
    let tiling = match decorations {
        Decorations::Server => Tiling::default(),
        Decorations::Client { tiling } => tiling,
    };

    let title = div()
        .flex()
        .items_center()
        .gap_2()
        .min_w(px(0.0))
        .overflow_hidden()
        .child(
            div()
                .text_sm()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(text)
                .child("GitForge"),
        )
        .child(div().text_sm().text_color(muted).child("/"))
        .child(div().text_sm().text_color(muted).child("Settings"));

    let mut bar = div()
        .id("settings-titlebar")
        .relative()
        .w_full()
        .h(px(TITLEBAR_HEIGHT))
        .flex_shrink_0()
        .window_control_area(WindowControlArea::Drag)
        .flex()
        .flex_row()
        .items_center()
        .bg(titlebar_bg)
        .on_mouse_down(MouseButton::Left, |_ev, window, _| {
            window.start_window_move();
        })
        .on_click(|event, window, _| {
            if event.click_count() == 2 {
                window.zoom_window();
            }
        });

    if matches!(decorations, Decorations::Client { .. }) && controls.window_menu {
        bar = bar.on_mouse_down(MouseButton::Right, |ev, window, _| {
            window.show_window_menu(ev.position);
        });
    }

    if matches!(decorations, Decorations::Client { .. }) {
        bar = seal_rounded_corners(apply_top_corner_radius(bar, rounding, tiling), titlebar_bg);
    }

    bar = bar.pl(px(8.0)).child(title.flex_1().min_w(px(0.0)));

    if !window.is_fullscreen() {
        if let Some(controls) = render_settings_window_controls(window, muted, icon_hover, hover_bg)
        {
            bar = bar.child(controls);
        }
    }

    bar
}

impl Render for SettingsWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let query = self.search_input.text().to_lowercase();
        let visible_sections: Vec<SettingsSection> = SettingsSection::ALL
            .into_iter()
            .filter(|s| s.matches_search(&query))
            .collect();

        let display_section = if visible_sections.iter().any(|s| *s == self.active_section) {
            self.active_section
        } else {
            visible_sections
                .first()
                .copied()
                .unwrap_or(self.active_section)
        };

        let surface = rgba_to_hsla(self.colors.surface);
        let surface_high = rgba_to_hsla(self.colors.surface_high);
        let border = rgba_to_hsla(self.colors.border);
        let text = rgba_to_hsla(self.colors.text);
        let muted = rgba_to_hsla(self.colors.text_muted);
        let accent = rgba_to_hsla(self.colors.accent);
        let hover = rgba_to_hsla(self.colors.sidebar_hover);

        let entity = cx.entity().downgrade();
        let draft = self.draft.clone();
        let focused = self.focused_field;
        let input_focus = self.input_focus.clone();
        let api_key_focus = self.api_key_input.focus_handle().clone();
        let pat_focus = self.pat_input.focus_handle().clone();
        let input_focused = input_focus.is_focused(window);
        let api_key_focused = api_key_focus.is_focused(window);
        let pat_focused = pat_focus.is_focused(window);
        let repo_data = self.repo_data.clone();
        let accounts = self.accounts.clone();
        let pending_account_provider = self.pending_account_provider.clone();
        let pat_error = self.pat_error.clone();
        let ent_keys = entity.clone();

        let sidebar = render_sidebar(
            &visible_sections,
            display_section,
            &self.search_input,
            focused == SettingsTextField::Search
                && self.search_input.focus_handle().is_focused(window),
            &self.colors,
            surface,
            surface_high,
            border,
            text,
            muted,
            accent,
            hover,
            entity.clone(),
            window,
        );

        let ai_ui = self.ai_ui_state();
        let content = render_content(
            display_section,
            &draft,
            &ai_ui,
            focused,
            input_focused,
            api_key_focused,
            pat_focused,
            &self.colors,
            self.main_app.clone(),
            entity,
            &input_focus,
            &self.api_key_input,
            &self.pat_input,
            window,
            Some(repo_data),
            accounts,
            pending_account_provider.as_deref(),
            pat_error.as_deref(),
        );

        let settings_content = div()
            .relative()
            .flex_1()
            .min_h(px(0.0))
            .flex()
            .flex_row()
            .overflow_hidden()
            .child(sidebar)
            .child(
                div()
                    .flex_1()
                    .h_full()
                    .flex()
                    .flex_col()
                    .overflow_hidden()
                    .child(content),
            )
            .child(
                div()
                    .absolute()
                    .bottom_2()
                    .left_3()
                    .text_xs()
                    .text_color(muted)
                    .child("Escape Close"),
            );

        let inner = div()
            .id("settings-window-content")
            .flex()
            .flex_col()
            .size_full()
            .overflow_hidden()
            .child(render_settings_titlebar(&self.colors, window))
            .child(super::titlebar::render_titlebar_divider(&self.colors))
            .child(settings_content);

        div()
            .id("settings-root")
            .size_full()
            .bg(gpui::transparent_black())
            .text_color(text)
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::handle_close))
            .on_action(cx.listener(Self::paste_api_key))
            .on_key_down({
                let ent = ent_keys;
                let input_fh = input_focus.clone();
                let api_key_fh = api_key_focus.clone();
                let pat_fh = pat_focus.clone();
                move |ev: &KeyDownEvent, window, cx| {
                    if !input_fh.is_focused(window)
                        && !api_key_fh.is_focused(window)
                        && !pat_fh.is_focused(window)
                    {
                        return;
                    }
                    if let Some(e) = ent.upgrade() {
                        e.update(cx, |this, cx| this.handle_key_input(ev, window, cx));
                    }
                }
            })
            .overflow_hidden()
            .child(super::window_chrome::render_window_chrome(
                inner,
                &self.colors,
                window,
            ))
    }
}

fn render_sidebar(
    sections: &[SettingsSection],
    active: SettingsSection,
    search_input: &TextInput,
    search_focused: bool,
    colors: &AppColors,
    surface: Hsla,
    surface_high: Hsla,
    border: Hsla,
    text: Hsla,
    muted: Hsla,
    accent: Hsla,
    hover: Hsla,
    entity: WeakEntity<SettingsWindow>,
    window: &Window,
) -> impl IntoElement {
    let ent_search = entity.clone();
    let mut nav = div().flex().flex_col().gap_0();
    for section in sections {
        let is_active = *section == active;
        let ent = entity.clone();
        let sec = *section;
        let bg = if is_active { surface_high } else { surface };
        nav = nav.child(
            div()
                .id(ElementId::Name(
                    format!("settings-nav-{}", section.label()).into(),
                ))
                .px_3()
                .py_2()
                .bg(bg)
                .border_l_2()
                .border_color(if is_active {
                    accent
                } else {
                    gpui::transparent_black()
                })
                .cursor_pointer()
                .hover(|s| s.bg(hover))
                .flex()
                .items_center()
                .gap_2()
                .on_click(move |_ev, _window, cx| {
                    if let Some(e) = ent.upgrade() {
                        e.update(cx, |this, cx| {
                            this.set_section(sec, cx);
                        });
                    }
                })
                .child(div().text_xs().text_color(muted).child("›"))
                .child(
                    div()
                        .text_sm()
                        .text_color(if is_active { text } else { muted })
                        .child(section.label()),
                ),
        );
    }

    div()
        .id("settings-sidebar")
        .w(px(220.0))
        .h_full()
        .flex()
        .flex_col()
        .bg(surface)
        .border_r_1()
        .border_color(border)
        .child(
            div().p_3().border_b_1().border_color(border).child(
                render_text_input(
                    search_input,
                    colors,
                    window,
                    &TextInputRenderOpts::new(ElementId::Name("settings-search".into()))
                        .text_xs()
                        .force_focused(search_focused)
                        .background(surface_high),
                    |_| {},
                )
                .on_key_down(move |ev, _window, cx| {
                    if let Some(e) = ent_search.upgrade() {
                        e.update(cx, |this, cx| {
                            this.focused_field = SettingsTextField::Search;
                            match parse_key_event(ev) {
                                TextInputEvent::Backspace => this.edit_search(None, cx),
                                TextInputEvent::Typed(c) => this.edit_search(Some(&c), cx),
                                _ => {}
                            }
                        });
                    }
                }),
            ),
        )
        .child(
            div()
                .id("settings-nav-scroll")
                .flex_1()
                .flex()
                .flex_col()
                .overflow_y_scroll()
                .child(nav),
        )
}

#[derive(Clone)]
pub struct SettingsRepoData {
    pub open_tabs: Vec<(u64, PathBuf)>,
    pub active_path: Option<PathBuf>,
    pub active_settings: RepoBehaviorSettings,
    pub recent_paths: Vec<String>,
    pub closed_paths: Vec<PathBuf>,
}

fn render_content(
    section: SettingsSection,
    draft: &SettingsDraft,
    ai_ui: &AiSettingsUiState,
    focused: SettingsTextField,
    input_focused: bool,
    api_key_focused: bool,
    pat_focused: bool,
    colors: &AppColors,
    main_app: WeakEntity<GitForgeApp>,
    entity: WeakEntity<SettingsWindow>,
    input_focus: &FocusHandle,
    api_key_input: &TextInput,
    pat_input: &TextInput,
    window: &Window,
    repo_data: Option<SettingsRepoData>,
    accounts: Vec<gitforge_hosting::HostingAccount>,
    pending_account_provider: Option<&str>,
    pat_error: Option<&str>,
) -> impl IntoElement {
    let border = rgba_to_hsla(colors.border);
    let text = rgba_to_hsla(colors.text);
    let muted = rgba_to_hsla(colors.text_muted);
    let accent = rgba_to_hsla(colors.accent);
    let bg = rgba_to_hsla(colors.background);

    let ent_json = entity.clone();

    let header = div()
        .flex()
        .items_center()
        .justify_between()
        .px_6()
        .py_3()
        .border_b_1()
        .border_color(border)
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(text)
                        .child("User"),
                )
                .child(div().text_sm().text_color(muted).child("·"))
                .child(div().text_sm().text_color(accent).child("Settings")),
        )
        .child(
            div()
                .id("settings-edit-json")
                .px_3()
                .py_1()
                .border_1()
                .border_color(border)
                .rounded(px(4.0))
                .cursor_pointer()
                .text_xs()
                .text_color(text)
                .hover(|s| s.bg(rgba_to_hsla(colors.surface_high)))
                .on_click(move |_ev, _window, cx| {
                    let path = AppSettings::settings_path();
                    let _ = std::process::Command::new("xdg-open").arg(&path).spawn();
                    if let Some(e) = ent_json.upgrade() {
                        e.update(cx, |_, cx| cx.notify());
                    }
                })
                .child("Edit in settings.json"),
        );

    let section_title = div()
        .px_6()
        .pt_6()
        .pb_2()
        .child(
            div()
                .text_xl()
                .font_weight(FontWeight::BOLD)
                .text_color(text)
                .child(section.label()),
        )
        .child(
            div()
                .text_sm()
                .text_color(muted)
                .child(format!("{} Settings", section.label())),
        );

    let section_body = div().id("settings-section-body").flex().flex_col();
    let section_body = match section {
        SettingsSection::General => {
            render_general_section(section_body, draft, colors, entity.clone())
        }
        SettingsSection::ExternalTools => render_tools_section(
            section_body,
            draft,
            focused,
            input_focused,
            colors,
            entity.clone(),
            input_focus,
        ),
        SettingsSection::Sidebar => {
            render_sidebar_section(section_body, draft, colors, entity.clone())
        }
        SettingsSection::Graph => render_graph_section(
            section_body,
            draft,
            focused,
            input_focused,
            colors,
            entity.clone(),
            input_focus,
        ),
        SettingsSection::Ai => render_ai_section(
            section_body,
            draft,
            ai_ui,
            focused,
            input_focused,
            api_key_focused,
            colors,
            entity.clone(),
            input_focus,
            api_key_input,
            window,
        ),
        SettingsSection::Repositories => render_repositories_section(
            section_body,
            repo_data.unwrap_or(SettingsRepoData {
                open_tabs: Vec::new(),
                active_path: None,
                active_settings: RepoBehaviorSettings::default(),
                recent_paths: Vec::new(),
                closed_paths: Vec::new(),
            }),
            draft,
            focused,
            input_focused,
            colors,
            main_app.clone(),
            entity.clone(),
            input_focus,
        ),
        SettingsSection::Accounts => render_accounts_section(
            section_body,
            accounts,
            colors,
            main_app.clone(),
            entity.clone(),
            pending_account_provider,
            pat_input,
            pat_focused,
            pat_error,
            window,
        ),
        SettingsSection::About => render_about_section(section_body, draft, colors, entity.clone()),
    };

    div()
        .flex_1()
        .h_full()
        .flex()
        .flex_col()
        .bg(bg)
        .overflow_hidden()
        .child(header)
        .child(
            div()
                .id("settings-content-scroll")
                .flex_1()
                .flex()
                .flex_col()
                .overflow_y_scroll()
                .child(section_title)
                .child(div().px_6().pb_6().child(section_body)),
        )
}

fn setting_row(
    label: &str,
    description: &str,
    control: impl IntoElement,
    border: Hsla,
    text: Hsla,
    muted: Hsla,
) -> impl IntoElement {
    div()
        .w_full()
        .py_3()
        .border_b_1()
        .border_color(border)
        .flex()
        .items_center()
        .gap_4()
        .child(
            div()
                .flex_1()
                .min_w(px(0.0))
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(text)
                        .child(label.to_string()),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(muted)
                        .child(description.to_string()),
                ),
        )
        .child(control)
}

fn setting_row_without_border(
    label: &str,
    description: &str,
    control: impl IntoElement,
    text: Hsla,
    muted: Hsla,
) -> impl IntoElement {
    div()
        .w_full()
        .py_3()
        .flex()
        .items_start()
        .gap_4()
        .child(
            div()
                .flex_1()
                .min_w(px(0.0))
                .flex()
                .flex_col()
                .gap_1()
                .pt_1()
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(text)
                        .child(label.to_string()),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(muted)
                        .child(description.to_string()),
                ),
        )
        .child(control)
}

fn pill_toggle(
    on: bool,
    ent: WeakEntity<SettingsWindow>,
    toggle_id: &'static str,
    colors: &AppColors,
) -> impl IntoElement {
    let border = rgba_to_hsla(colors.border);
    let accent = rgba_to_hsla(colors.accent);
    let bg_off = rgba_to_hsla(colors.surface_high);
    let ent2 = ent.clone();
    div()
        .id(ElementId::Name(toggle_id.into()))
        .w(px(40.0))
        .h(px(22.0))
        .rounded_full()
        .bg(if on { accent } else { bg_off })
        .border_1()
        .border_color(if on { accent } else { border })
        .cursor_pointer()
        .flex()
        .items_center()
        .px_1()
        .child(
            div()
                .w(px(16.0))
                .h(px(16.0))
                .rounded_full()
                .bg(gpui::white())
                .ml(if on { px(18.0) } else { px(0.0) }),
        )
        .on_click(move |_ev, _window, cx| {
            if let Some(e) = ent2.upgrade() {
                e.update(cx, |this, cx| {
                    this.patch_draft(
                        |draft| match toggle_id {
                            "branches" => {
                                draft.sidebar_branches_expanded = !draft.sidebar_branches_expanded
                            }
                            "remotes" => {
                                draft.sidebar_remotes_expanded = !draft.sidebar_remotes_expanded
                            }
                            "tags" => draft.sidebar_tags_expanded = !draft.sidebar_tags_expanded,
                            "pull-requests" => {
                                draft.sidebar_pull_requests_expanded =
                                    !draft.sidebar_pull_requests_expanded
                            }
                            "checkpoints" => {
                                draft.show_checkpoint_refs = !draft.show_checkpoint_refs
                            }
                            "graph-col-graph" => {
                                draft.graph_show_graph_column = !draft.graph_show_graph_column
                            }
                            "graph-col-sha" => {
                                draft.graph_show_sha_column = !draft.graph_show_sha_column
                            }
                            "graph-col-time" => {
                                draft.graph_show_time_column = !draft.graph_show_time_column
                            }
                            "graph-col-author" => {
                                draft.graph_show_author_column = !draft.graph_show_author_column
                            }
                            "conventional" => {
                                draft.ai_conventional_commits = !draft.ai_conventional_commits
                            }
                            "repo-periodic-fetch" => {
                                draft.repo_periodic_fetch_enabled =
                                    !draft.repo_periodic_fetch_enabled
                            }
                            "repo-auto-push" => {
                                draft.repo_auto_push_on_commit = !draft.repo_auto_push_on_commit
                            }
                            "auto-update" => {
                                if gitforge_update::auto_update_supported() {
                                    draft.auto_update = !draft.auto_update;
                                }
                            }
                            _ => {}
                        },
                        cx,
                    );
                });
            }
        })
}

fn text_field_control(
    value: &str,
    field: SettingsTextField,
    is_active: bool,
    placeholder: &str,
    colors: &AppColors,
    entity: WeakEntity<SettingsWindow>,
    input_focus: &FocusHandle,
) -> impl IntoElement {
    let ent_focus = entity.clone();
    let fh_click = input_focus.clone();

    render_static_text_input(
        value,
        placeholder,
        input_focus,
        is_active,
        TextInputDisplay::Plain,
        false,
        colors,
        TextInputRenderOpts::new(ElementId::Name(format!("settings-field-{field:?}").into()))
            .text_xs()
            .width(px(200.0))
            .text_ellipsis()
            .overflow_hidden()
            .force_focused(is_active),
        {
            let fh = input_focus.clone();
            move |window| {
                window.focus(&fh);
            }
        },
    )
    .on_click(move |_ev, window, cx| {
        if let Some(e) = ent_focus.upgrade() {
            e.update(cx, |this, cx| {
                this.focused_field = field;
                cx.notify();
            });
        }
        window.focus(&fh_click);
    })
}

fn dropdown_control(
    value: &str,
    options: &[&str],
    setting_key: &'static str,
    colors: &AppColors,
    entity: WeakEntity<SettingsWindow>,
) -> impl IntoElement {
    let border = rgba_to_hsla(colors.border);
    let text = rgba_to_hsla(colors.text);
    let surface = rgba_to_hsla(colors.surface);
    let accent = rgba_to_hsla(colors.accent);

    let mut buttons = Vec::new();
    for opt in options {
        let is_active = *opt == value;
        let ent = entity.clone();
        let val = opt.to_string();
        let key = setting_key;
        buttons.push(
            div()
                .id(ElementId::Name(
                    format!("settings-dd-{}-{}", key, opt).into(),
                ))
                .px_2()
                .py_1()
                .border_1()
                .border_color(if is_active { accent } else { border })
                .rounded(px(3.0))
                .bg(if is_active { accent } else { surface })
                .cursor_pointer()
                .text_xs()
                .text_color(if is_active {
                    rgba_to_hsla(colors.background)
                } else {
                    text
                })
                .child((*opt).to_string())
                .on_click(move |_ev, _window, cx| {
                    if let Some(e) = ent.upgrade() {
                        e.update(cx, |this, cx| {
                            this.patch_draft(
                                |draft| match key {
                                    "ai-provider" => {
                                        draft.ai_provider = val.clone();
                                        if draft.ai_model.is_empty() {
                                            draft.ai_model = default_model_for_provider(&val);
                                        }
                                    }
                                    "ai-tone" => draft.ai_tone = val.clone(),
                                    "ai-model" => draft.ai_model = val.clone(),
                                    "zai-endpoint" => draft.ai_zai_endpoint = val.clone(),
                                    "ai-options-count" => {
                                        draft.ai_message_options_count =
                                            val.parse().unwrap_or(3).clamp(1, 3);
                                    }
                                    "ai-variation" => draft.ai_variation_mode = val.clone(),
                                    "ai-default-alt" => draft.ai_default_alternative = val.clone(),
                                    "ai-summary-limit" => {
                                        draft.ai_summary_max_chars = match val.as_str() {
                                            "auto" => 0,
                                            "50" => 50,
                                            "72" => 72,
                                            _ => draft.ai_summary_max_chars,
                                        };
                                        if draft.ai_summary_max_chars > 0 {
                                            draft.ai_summary_text =
                                                draft.ai_summary_max_chars.to_string();
                                        } else {
                                            draft.ai_summary_text.clear();
                                        }
                                    }
                                    "ai-body-wrap" => {
                                        draft.ai_body_wrap_at = if val == "none" { 0 } else { 72 };
                                    }
                                    "ai-max-diff" => {
                                        draft.ai_max_diff_chars = max_diff_from_label(&val);
                                    }
                                    "ai-temperature" => {
                                        draft.ai_temperature = match val.as_str() {
                                            "low" => 0.1,
                                            "high" => 0.7,
                                            "custom" => draft.ai_temperature,
                                            _ => 0.3,
                                        };
                                        if val != "custom" {
                                            draft.ai_temperature_text =
                                                format!("{:.2}", draft.ai_temperature);
                                        }
                                    }
                                    _ => {}
                                },
                                cx,
                            );
                            if key == "zai-endpoint" {
                                this.fetch_models_if_applicable(cx);
                            }
                        });
                    }
                }),
        );
    }

    div().flex().gap_1().children(buttons)
}

fn models_dropdown_control(
    value: &str,
    models: &[String],
    colors: &AppColors,
    entity: WeakEntity<SettingsWindow>,
) -> impl IntoElement {
    let border = rgba_to_hsla(colors.border);
    let text = rgba_to_hsla(colors.text);
    let surface = rgba_to_hsla(colors.surface);
    let accent = rgba_to_hsla(colors.accent);

    let mut row = div().flex().flex_wrap().gap_1().max_w(px(420.0));
    for model in models {
        let is_active = model == value;
        let ent = entity.clone();
        let val = model.clone();
        row = row.child(
            div()
                .id(ElementId::Name(format!("settings-model-{model}").into()))
                .px_2()
                .py_1()
                .border_1()
                .border_color(if is_active { accent } else { border })
                .rounded(px(3.0))
                .bg(if is_active { accent } else { surface })
                .cursor_pointer()
                .text_xs()
                .text_color(if is_active {
                    rgba_to_hsla(colors.background)
                } else {
                    text
                })
                .child(model.clone())
                .on_click(move |_ev, _window, cx| {
                    if let Some(e) = ent.upgrade() {
                        e.update(cx, |this, cx| {
                            this.patch_draft(|draft| draft.ai_model = val.clone(), cx);
                        });
                    }
                }),
        );
    }
    row
}

fn model_row_control(
    inner: impl IntoElement,
    accent: Hsla,
    entity: WeakEntity<SettingsWindow>,
    show_refresh: bool,
    loading: bool,
) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap_2()
        .child(inner)
        .when(show_refresh && !loading, |row| {
            let ent = entity;
            row.child(
                div()
                    .id("settings-ai-refresh-models")
                    .cursor_pointer()
                    .child(
                        svg()
                            .size(px(14.0))
                            .path("icons/refresh.svg")
                            .text_color(accent),
                    )
                    .on_click(move |_ev, _window, cx| {
                        if let Some(e) = ent.upgrade() {
                            e.update(cx, |this, cx| this.fetch_models(cx));
                        }
                    }),
            )
        })
}

fn api_key_field_control(
    api_key_input: &TextInput,
    configured: bool,
    is_active: bool,
    colors: &AppColors,
    entity: WeakEntity<SettingsWindow>,
    window: &Window,
) -> impl IntoElement {
    let border = rgba_to_hsla(colors.border);
    let accent = rgba_to_hsla(colors.accent);
    let muted = rgba_to_hsla(colors.text_muted);
    let ent_focus = entity.clone();
    let ent_keys = entity.clone();
    let ent_save = entity.clone();
    let ent_clear = entity.clone();
    let fh = api_key_input.focus_handle().clone();

    div()
        .flex()
        .items_center()
        .gap_2()
        .child(
            render_text_input(
                api_key_input,
                colors,
                window,
                &TextInputRenderOpts::new(ElementId::Name("settings-field-AiApiKey".into()))
                    .text_xs()
                    .width(px(220.0))
                    .text_ellipsis()
                    .overflow_hidden()
                    .configured(configured)
                    .force_focused(is_active),
                |_| {},
            )
            .on_click(move |_ev, window, cx| {
                if let Some(e) = ent_focus.upgrade() {
                    e.update(cx, |this, cx| {
                        this.focused_field = SettingsTextField::AiApiKey;
                        cx.notify();
                    });
                }
                window.focus(&fh);
            })
            .on_key_down(move |ev, window, cx| {
                if let Some(e) = ent_keys.upgrade() {
                    e.update(cx, |this, cx| this.handle_key_input(ev, window, cx));
                }
                cx.stop_propagation();
            }),
        )
        .child(
            div()
                .id("settings-ai-key-save")
                .px_2()
                .py_1()
                .border_1()
                .border_color(accent)
                .rounded(px(4.0))
                .cursor_pointer()
                .on_click(move |_ev, _window, cx| {
                    if let Some(e) = ent_save.upgrade() {
                        e.update(cx, |this, cx| this.save_api_key(cx));
                    }
                })
                .child(div().text_xs().text_color(accent).child("Save")),
        )
        .when(configured, |row| {
            row.child(
                div()
                    .id("settings-ai-key-clear")
                    .px_2()
                    .py_1()
                    .border_1()
                    .border_color(border)
                    .rounded(px(4.0))
                    .cursor_pointer()
                    .on_click(move |_ev, _window, cx| {
                        if let Some(e) = ent_clear.upgrade() {
                            e.update(cx, |this, cx| this.clear_api_key(cx));
                        }
                    })
                    .child(div().text_xs().text_color(muted).child("Clear")),
            )
        })
}

fn pat_field_control(
    pat_input: &TextInput,
    is_active: bool,
    colors: &AppColors,
    entity: WeakEntity<SettingsWindow>,
    window: &Window,
) -> impl IntoElement {
    let border = rgba_to_hsla(colors.border);
    let accent = rgba_to_hsla(colors.accent);
    let muted = rgba_to_hsla(colors.text_muted);
    let ent_focus = entity.clone();
    let ent_keys = entity.clone();
    let ent_save = entity.clone();
    let ent_cancel = entity.clone();
    let fh = pat_input.focus_handle().clone();

    div()
        .flex()
        .items_center()
        .gap_2()
        .child(
            render_text_input(
                pat_input,
                colors,
                window,
                &TextInputRenderOpts::new(ElementId::Name("settings-field-HostingPat".into()))
                    .text_xs()
                    .width(px(280.0))
                    .text_ellipsis()
                    .overflow_hidden()
                    .force_focused(is_active),
                |_| {},
            )
            .on_click(move |_ev, window, cx| {
                if let Some(e) = ent_focus.upgrade() {
                    e.update(cx, |this, cx| {
                        this.focused_field = SettingsTextField::HostingPat;
                        cx.notify();
                    });
                }
                window.focus(&fh);
            })
            .on_key_down(move |ev, window, cx| {
                if let Some(e) = ent_keys.upgrade() {
                    e.update(cx, |this, cx| this.handle_key_input(ev, window, cx));
                }
                cx.stop_propagation();
            }),
        )
        .child(
            div()
                .id("settings-pat-save")
                .px_2()
                .py_1()
                .border_1()
                .border_color(accent)
                .rounded(px(4.0))
                .cursor_pointer()
                .on_click(move |_ev, _window, cx| {
                    if let Some(e) = ent_save.upgrade() {
                        e.update(cx, |this, cx| this.save_hosting_account(cx));
                    }
                })
                .child(div().text_xs().text_color(accent).child("Save")),
        )
        .child(
            div()
                .id("settings-pat-cancel")
                .px_2()
                .py_1()
                .border_1()
                .border_color(border)
                .rounded(px(4.0))
                .cursor_pointer()
                .on_click(move |_ev, _window, cx| {
                    if let Some(e) = ent_cancel.upgrade() {
                        e.update(cx, |this, cx| this.cancel_add_account(cx));
                    }
                })
                .child(div().text_xs().text_color(muted).child("Cancel")),
        )
}

fn provider_display_name(provider: &str) -> &str {
    gitforge_hosting::urls::provider_label(provider)
}

fn pat_scope_lines(provider: &str) -> &'static [&'static str] {
    match provider {
        "github" => &[
            "Enable these scopes on a classic personal access token:",
            "• repo — list/clone private and public repos, fork, open pull requests, read branches",
            "• public_repo — use instead of repo if you only work with public repositories",
            "• read:user — read profile and avatar (optional; sign-in works with repo alone)",
        ],
        "gitlab" => &[
            "Enable these scopes on a personal access token:",
            "• api — full API access for private repos, fork, and merge requests",
            "• read_api — browse and search repositories only (no fork or merge requests)",
            "• read_user — read profile and avatar",
        ],
        "codeberg" => &[
            "Enable these scopes on an access token:",
            "• read:user — read profile and avatar",
            "• read:repository — list and search repositories",
            "• write:repository — fork repositories and create pull requests",
        ],
        _ => &["Use a token with repository read and write API access."],
    }
}

fn render_pat_scope_guidance(provider: &str, muted: Hsla, accent: Hsla) -> Div {
    let title = format!("{} token scopes", provider_display_name(provider));
    let mut block = div()
        .flex()
        .flex_col()
        .gap_1()
        .px_2()
        .py_2()
        .rounded(px(4.0))
        .bg(muted.opacity(0.08))
        .child(
            div()
                .text_xs()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(accent)
                .child(title),
        );

    for line in pat_scope_lines(provider) {
        block = block.child(div().text_xs().text_color(muted).child(*line));
    }

    block
}

fn default_model_for_provider(provider: &str) -> String {
    gitforge_ai::default_model_for_provider(provider).to_string()
}

fn theme_picker_control(
    current: &str,
    themes: &[ThemeEntry],
    colors: &AppColors,
    entity: WeakEntity<SettingsWindow>,
) -> impl IntoElement {
    let border = rgba_to_hsla(colors.border);
    let text = rgba_to_hsla(colors.text);
    let surface = rgba_to_hsla(colors.surface);
    let accent = rgba_to_hsla(colors.accent);
    let muted = rgba_to_hsla(colors.text_muted);

    let mut dark_themes = Vec::new();
    let mut light_themes = Vec::new();
    for entry in themes {
        match entry.appearance {
            Appearance::Dark => dark_themes.push(entry),
            Appearance::Light => light_themes.push(entry),
        }
    }

    let render_group = |label: &str, group: &[&ThemeEntry]| {
        let mut pills = Vec::new();
        for entry in group {
            let is_active = entry.name == current;
            let ent = entity.clone();
            let theme_id = entry.name.clone();
            let display = entry.display_name.clone();
            pills.push(
                div()
                    .id(ElementId::Name(
                        format!("settings-theme-{}", entry.name).into(),
                    ))
                    .px_2()
                    .py_1()
                    .border_1()
                    .border_color(if is_active { accent } else { border })
                    .rounded(px(3.0))
                    .bg(if is_active { accent } else { surface })
                    .cursor_pointer()
                    .text_xs()
                    .text_color(if is_active {
                        rgba_to_hsla(colors.background)
                    } else {
                        text
                    })
                    .child(display)
                    .on_click(move |_ev, _window, cx| {
                        if let Some(e) = ent.upgrade() {
                            e.update(cx, |this, cx| {
                                this.patch_draft(|draft| draft.theme = theme_id.clone(), cx);
                            });
                        }
                    }),
            );
        }

        div()
            .flex()
            .flex_col()
            .gap_1()
            .child(
                div()
                    .text_xs()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(muted)
                    .child(label.to_string()),
            )
            .child(div().flex().flex_wrap().gap_1().children(pills))
    };

    div()
        .flex()
        .flex_col()
        .gap_2()
        .max_w(px(440.0))
        .child(render_group("Dark", &dark_themes))
        .child(div().w_full().h(px(1.0)).my_2().bg(border))
        .child(render_group("Light", &light_themes))
}

fn render_general_section(
    body: Stateful<Div>,
    draft: &SettingsDraft,
    colors: &AppColors,
    entity: WeakEntity<SettingsWindow>,
) -> Stateful<Div> {
    let text = rgba_to_hsla(colors.text);
    let muted = rgba_to_hsla(colors.text_muted);

    let themes = Theme::discover_themes();
    let theme_control = theme_picker_control(&draft.theme, &themes, colors, entity);

    body.child(setting_row_without_border(
        "Theme",
        "Choose the application color scheme.",
        theme_control,
        text,
        muted,
    ))
}

fn render_about_section(
    mut body: Stateful<Div>,
    draft: &SettingsDraft,
    colors: &AppColors,
    entity: WeakEntity<SettingsWindow>,
) -> Stateful<Div> {
    let border = rgba_to_hsla(colors.border);
    let text = rgba_to_hsla(colors.text);
    let muted = rgba_to_hsla(colors.text_muted);
    let accent = rgba_to_hsla(colors.accent);
    let hover_bg = rgba_to_hsla(colors.surface_high);
    let version = env!("CARGO_PKG_VERSION");

    body = body.child(setting_row(
        "Version",
        "Installed GitForge release.",
        div()
            .text_sm()
            .text_color(text)
            .child(format!("GitForge {version}")),
        border,
        text,
        muted,
    ));

    let update_supported = gitforge_update::auto_update_supported();
    let update_description = if let Some(reason) = gitforge_update::update_block_reason() {
        reason.message().to_string()
    } else {
        "Automatically check for updates and install them in the background.".to_string()
    };

    body = body.child(setting_row(
        "Automatic Updates",
        &update_description,
        pill_toggle(
            draft.auto_update && update_supported,
            entity.clone(),
            "auto-update",
            colors,
        ),
        border,
        text,
        muted,
    ));

    body.child(setting_row(
        "Check for Updates",
        "Look for a newer release on GitHub.",
        div()
            .id("settings-check-for-updates")
            .px_3()
            .py_1()
            .rounded(px(4.0))
            .border_1()
            .border_color(border)
            .cursor_pointer()
            .text_xs()
            .text_color(accent)
            .hover(|s| s.bg(hover_bg))
            .child("Check for Updates")
            .on_click(move |_ev, window, cx| {
                gitforge_update::check(&gitforge_update::Check, window, cx);
            }),
        border,
        text,
        muted,
    ))
}

fn render_tools_section(
    mut body: Stateful<Div>,
    draft: &SettingsDraft,
    focused: SettingsTextField,
    input_focused: bool,
    colors: &AppColors,
    entity: WeakEntity<SettingsWindow>,
    input_focus: &FocusHandle,
) -> Stateful<Div> {
    let border = rgba_to_hsla(colors.border);
    let text = rgba_to_hsla(colors.text);
    let muted = rgba_to_hsla(colors.text_muted);

    body = body.child(setting_row(
        "Editor",
        "Command used to open files in an external editor.",
        text_field_control(
            &draft.editor_command,
            SettingsTextField::Editor,
            focused == SettingsTextField::Editor && input_focused,
            "editor command",
            colors,
            entity.clone(),
            input_focus,
        ),
        border,
        text,
        muted,
    ));
    body = body.child(setting_row(
        "Terminal",
        "Command used to open a terminal in the repository directory.",
        text_field_control(
            &draft.terminal_command,
            SettingsTextField::Terminal,
            focused == SettingsTextField::Terminal && input_focused,
            "terminal command",
            colors,
            entity.clone(),
            input_focus,
        ),
        border,
        text,
        muted,
    ));
    body = body.child(setting_row(
        "Diff Tool",
        "External program for viewing diffs (optional).",
        text_field_control(
            &draft.diff_tool,
            SettingsTextField::DiffTool,
            focused == SettingsTextField::DiffTool && input_focused,
            "diff tool",
            colors,
            entity.clone(),
            input_focus,
        ),
        border,
        text,
        muted,
    ));
    body.child(setting_row(
        "Merge Tool",
        "External program for resolving merge conflicts (optional).",
        text_field_control(
            &draft.merge_tool,
            SettingsTextField::MergeTool,
            focused == SettingsTextField::MergeTool && input_focused,
            "merge tool",
            colors,
            entity.clone(),
            input_focus,
        ),
        border,
        text,
        muted,
    ))
}

fn render_sidebar_section(
    mut body: Stateful<Div>,
    draft: &SettingsDraft,
    colors: &AppColors,
    entity: WeakEntity<SettingsWindow>,
) -> Stateful<Div> {
    let border = rgba_to_hsla(colors.border);
    let text = rgba_to_hsla(colors.text);
    let muted = rgba_to_hsla(colors.text_muted);

    body = body.child(setting_row(
        "Expand Branches",
        "Expand the branches section in the sidebar by default.",
        pill_toggle(
            draft.sidebar_branches_expanded,
            entity.clone(),
            "branches",
            colors,
        ),
        border,
        text,
        muted,
    ));
    body = body.child(setting_row(
        "Expand Remotes",
        "Expand the remotes section in the sidebar by default.",
        pill_toggle(
            draft.sidebar_remotes_expanded,
            entity.clone(),
            "remotes",
            colors,
        ),
        border,
        text,
        muted,
    ));
    body.child(setting_row(
        "Expand Tags",
        "Expand the tags section in the sidebar by default.",
        pill_toggle(draft.sidebar_tags_expanded, entity.clone(), "tags", colors),
        border,
        text,
        muted,
    ))
    .child(setting_row(
        "Expand Pull Requests",
        "Expand the pull requests section in the sidebar by default.",
        pill_toggle(
            draft.sidebar_pull_requests_expanded,
            entity.clone(),
            "pull-requests",
            colors,
        ),
        border,
        text,
        muted,
    ))
}

fn render_graph_section(
    mut body: Stateful<Div>,
    draft: &SettingsDraft,
    focused: SettingsTextField,
    input_focused: bool,
    colors: &AppColors,
    entity: WeakEntity<SettingsWindow>,
    input_focus: &FocusHandle,
) -> Stateful<Div> {
    let border = rgba_to_hsla(colors.border);
    let text = rgba_to_hsla(colors.text);
    let muted = rgba_to_hsla(colors.text_muted);

    body = body.child(setting_row(
        "Show Checkpoint Refs",
        "Display checkpoint references in the commit graph.",
        pill_toggle(
            draft.show_checkpoint_refs,
            entity.clone(),
            "checkpoints",
            colors,
        ),
        border,
        text,
        muted,
    ));
    body = body.child(setting_row(
        "Show Graph Column",
        "Display the lane visualization column with arcs and commit circles.",
        pill_toggle(
            draft.graph_show_graph_column,
            entity.clone(),
            "graph-col-graph",
            colors,
        ),
        border,
        text,
        muted,
    ));
    body = body.child(setting_row(
        "Show SHA Column",
        "Display the short commit hash column.",
        pill_toggle(
            draft.graph_show_sha_column,
            entity.clone(),
            "graph-col-sha",
            colors,
        ),
        border,
        text,
        muted,
    ));
    body = body.child(setting_row(
        "Show Time Column",
        "Display the relative time column (e.g. \"5m ago\").",
        pill_toggle(
            draft.graph_show_time_column,
            entity.clone(),
            "graph-col-time",
            colors,
        ),
        border,
        text,
        muted,
    ));
    body = body.child(setting_row(
        "Show Author Column",
        "Display the commit author name column.",
        pill_toggle(
            draft.graph_show_author_column,
            entity.clone(),
            "graph-col-author",
            colors,
        ),
        border,
        text,
        muted,
    ));
    body.child(setting_row(
        "Commit Limit",
        "Maximum number of commits to load when opening a repository.",
        text_field_control(
            &draft.commit_limit_text,
            SettingsTextField::CommitLimit,
            focused == SettingsTextField::CommitLimit && input_focused,
            "1000",
            colors,
            entity,
            input_focus,
        ),
        border,
        text,
        muted,
    ))
}

fn render_ai_section(
    mut body: Stateful<Div>,
    draft: &SettingsDraft,
    ai_ui: &AiSettingsUiState,
    focused: SettingsTextField,
    input_focused: bool,
    api_key_focused: bool,
    colors: &AppColors,
    entity: WeakEntity<SettingsWindow>,
    input_focus: &FocusHandle,
    api_key_input: &TextInput,
    window: &Window,
) -> Stateful<Div> {
    let border = rgba_to_hsla(colors.border);
    let text = rgba_to_hsla(colors.text);
    let muted = rgba_to_hsla(colors.text_muted);
    let accent = rgba_to_hsla(colors.accent);

    let providers = ["disabled", "zai", "ollama", "openai", "anthropic"];
    let current = if providers.contains(&draft.ai_provider.as_str()) {
        draft.ai_provider.as_str()
    } else {
        "disabled"
    };

    let mut provider_dd = div().flex().flex_wrap().gap_1();
    for p in providers {
        let is_active = p == current;
        let ent = entity.clone();
        let val = p.to_string();
        provider_dd = provider_dd.child(
            div()
                .id(ElementId::Name(format!("settings-ai-prov-{}", p).into()))
                .px_2()
                .py_1()
                .border_1()
                .border_color(if is_active { accent } else { border })
                .rounded(px(3.0))
                .bg(if is_active {
                    accent
                } else {
                    rgba_to_hsla(colors.surface)
                })
                .cursor_pointer()
                .on_click(move |_ev, _window, cx| {
                    if let Some(e) = ent.upgrade() {
                        e.update(cx, |this, cx| {
                            this.patch_draft(
                                |draft| {
                                    draft.ai_provider = val.clone();
                                    if draft.ai_model.is_empty() {
                                        draft.ai_model = default_model_for_provider(&val);
                                    }
                                },
                                cx,
                            );
                            this.on_provider_changed(cx);
                        });
                    }
                })
                .child(
                    div()
                        .text_xs()
                        .text_color(if is_active {
                            rgba_to_hsla(colors.background)
                        } else {
                            text
                        })
                        .child(p),
                ),
        );
    }

    let show_api_key = provider_needs_api_key(current);
    let zai_endpoint = if draft.ai_zai_endpoint == "coding" {
        "coding"
    } else {
        "general"
    };

    body = body.child(setting_row(
        "Provider",
        "AI backend used for commit messages and summaries.",
        provider_dd,
        border,
        text,
        muted,
    ));

    if current == "zai" {
        body = body.child(setting_row(
            "Z.ai Endpoint",
            "General API for broad use; Coding Plan uses the subscription coding endpoint (see Z.ai docs).",
            dropdown_control(
                zai_endpoint,
                &["general", "coding"],
                "zai-endpoint",
                colors,
                entity.clone(),
            ),
            border,
            text,
            muted,
        ));
    }

    if show_api_key {
        body = body.child(setting_row(
            "API Key",
            "Stored in the system keyring. Enter a new key to replace the saved one.",
            api_key_field_control(
                api_key_input,
                ai_ui.api_key_configured,
                focused == SettingsTextField::AiApiKey && api_key_focused,
                colors,
                entity.clone(),
                window,
            ),
            border,
            text,
            muted,
        ));
    }

    if current != "disabled" {
        let show_refresh = provider_supports_model_list(current);
        let has_models = !ai_ui.available_models.is_empty();
        body = body.child(setting_row(
            "Model",
            "Model used for commit message generation. Pick from the list or type a custom name.",
            div()
                .flex()
                .flex_col()
                .gap_2()
                .when(has_models, |c| {
                    c.child(models_dropdown_control(
                        &draft.ai_model,
                        &ai_ui.available_models,
                        colors,
                        entity.clone(),
                    ))
                })
                .child(model_row_control(
                    text_field_control(
                        &draft.ai_model,
                        SettingsTextField::AiModel,
                        focused == SettingsTextField::AiModel && input_focused,
                        "model name",
                        colors,
                        entity.clone(),
                        input_focus,
                    ),
                    accent,
                    entity.clone(),
                    show_refresh,
                    ai_ui.models_loading,
                )),
            border,
            text,
            muted,
        ));

        if ai_ui.models_loading {
            body = body.child(
                div()
                    .px_6()
                    .pb_1()
                    .text_xs()
                    .text_color(muted)
                    .child("Loading models…"),
            );
        } else if let Some(err) = &ai_ui.models_error {
            body = body.child(
                div()
                    .px_6()
                    .pb_1()
                    .text_xs()
                    .text_color(rgba_to_hsla(colors.error))
                    .child(err.clone()),
            );
        }
    }

    if current == "ollama" {
        body = body.child(setting_row(
            "Ollama URL",
            "Base URL for the local Ollama server.",
            text_field_control(
                &draft.ai_ollama_url,
                SettingsTextField::AiOllamaUrl,
                focused == SettingsTextField::AiOllamaUrl && input_focused,
                "http://localhost:11434",
                colors,
                entity.clone(),
                input_focus,
            ),
            border,
            text,
            muted,
        ));
    }

    body = body.child(
        div()
            .px_6()
            .pt_4()
            .pb_1()
            .text_xs()
            .font_weight(FontWeight::BOLD)
            .text_color(muted)
            .child("COMMIT MESSAGE GENERATION"),
    );

    body = body.child(setting_row(
        "Presets",
        "Quick shortcuts; you can still adjust individual options below.",
        commit_preset_control(colors, entity.clone()),
        border,
        text,
        muted,
    ));

    let options_count = draft.ai_message_options_count.clamp(1, 3).to_string();
    body = body.child(setting_row(
        "Options Count",
        "How many commit message alternatives to generate (1–3).",
        dropdown_control(
            &options_count,
            &["1", "2", "3"],
            "ai-options-count",
            colors,
            entity.clone(),
        ),
        border,
        text,
        muted,
    ));

    let multi_options = draft.ai_message_options_count > 1;
    if multi_options {
        body = body.child(setting_row(
            "Variation",
            "How multiple options should differ: mixed styles, same tone, or all detailed.",
            dropdown_control(
                &draft.ai_variation_mode,
                &["mixed", "uniform", "detailed"],
                "ai-variation",
                colors,
                entity.clone(),
            ),
            border,
            text,
            muted,
        ));
        body = body.child(setting_row(
            "Default Selection",
            "Which generated option fills the commit message input.",
            dropdown_control(
                &draft.ai_default_alternative,
                &["first", "shortest", "longest"],
                "ai-default-alt",
                colors,
                entity.clone(),
            ),
            border,
            text,
            muted,
        ));
    } else {
        body = body.child(setting_row(
            "Variation",
            "How multiple options should differ: mixed styles, same tone, or all detailed.",
            div()
                .text_xs()
                .text_color(muted)
                .child("Only applies when generating more than one option."),
            border,
            text,
            muted,
        ));
        body = body.child(setting_row(
            "Default Selection",
            "Which generated option fills the commit message input.",
            div()
                .text_xs()
                .text_color(muted)
                .child("Only applies when generating more than one option."),
            border,
            text,
            muted,
        ));
    }

    body = body.child(setting_row(
        "Commit Message Style",
        "Tone for single messages and uniform variation mode.",
        dropdown_control(
            &draft.ai_tone,
            &["balanced", "concise", "verbose"],
            "ai-tone",
            colors,
            entity.clone(),
        ),
        border,
        text,
        muted,
    ));

    body = body.child(setting_row(
        "Conventional Commits",
        "Prefer conventional commit message format when generating.",
        pill_toggle(
            draft.ai_conventional_commits,
            entity.clone(),
            "conventional",
            colors,
        ),
        border,
        text,
        muted,
    ));

    let summary_value = summary_dropdown_value(draft.ai_summary_max_chars);
    body = body.child(setting_row(
        "Summary Line Limit",
        "Maximum characters for the first line; custom field overrides preset.",
        div()
            .flex()
            .flex_col()
            .gap_1()
            .child(dropdown_control(
                &summary_value,
                &["auto", "50", "72", "custom"],
                "ai-summary-limit",
                colors,
                entity.clone(),
            ))
            .child(text_field_control(
                &draft.ai_summary_text,
                SettingsTextField::AiSummaryMaxChars,
                focused == SettingsTextField::AiSummaryMaxChars && input_focused,
                "custom chars",
                colors,
                entity.clone(),
                input_focus,
            )),
        border,
        text,
        muted,
    ));

    body = body.child(setting_row(
        "Body Wrap",
        "Wrap commit message body text at a column width.",
        dropdown_control(
            body_wrap_label(draft.ai_body_wrap_at),
            &["72", "none"],
            "ai-body-wrap",
            colors,
            entity.clone(),
        ),
        border,
        text,
        muted,
    ));

    body = body.child(setting_row(
        "Max Diff Size",
        "Limit staged diff size sent to the model; larger diffs are truncated.",
        dropdown_control(
            max_diff_label(draft.ai_max_diff_chars),
            &["unlimited", "8k", "16k", "32k"],
            "ai-max-diff",
            colors,
            entity.clone(),
        ),
        border,
        text,
        muted,
    ));

    let temp_preset = if (draft.ai_temperature - 0.1).abs() < f32::EPSILON {
        "low"
    } else if (draft.ai_temperature - 0.7).abs() < f32::EPSILON {
        "high"
    } else if (draft.ai_temperature - 0.3).abs() < f32::EPSILON {
        "medium"
    } else {
        "custom"
    };

    body.child(setting_row(
        "Temperature",
        "Model creativity; lower is more focused, higher is more varied.",
        div()
            .flex()
            .flex_col()
            .gap_1()
            .child(dropdown_control(
                temp_preset,
                &["low", "medium", "high", "custom"],
                "ai-temperature",
                colors,
                entity.clone(),
            ))
            .child(text_field_control(
                &draft.ai_temperature_text,
                SettingsTextField::AiTemperature,
                focused == SettingsTextField::AiTemperature && input_focused,
                "0.00–1.00",
                colors,
                entity,
                input_focus,
            )),
        border,
        text,
        muted,
    ))
}

fn render_repositories_section(
    mut body: Stateful<Div>,
    data: SettingsRepoData,
    draft: &SettingsDraft,
    focused: SettingsTextField,
    input_focused: bool,
    colors: &AppColors,
    main_app: WeakEntity<GitForgeApp>,
    entity: WeakEntity<SettingsWindow>,
    input_focus: &FocusHandle,
) -> Stateful<Div> {
    let border = rgba_to_hsla(colors.border);
    let text = rgba_to_hsla(colors.text);
    let muted = rgba_to_hsla(colors.text_muted);
    let accent = rgba_to_hsla(colors.accent);
    let hover = rgba_to_hsla(colors.sidebar_hover);

    body = body.child(
        div()
            .text_xs()
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(muted)
            .child("Active Repository Behavior"),
    );
    if let Some(path) = data.active_path.as_ref() {
        body = body
            .child(
                div()
                    .text_xs()
                    .text_color(muted)
                    .overflow_hidden()
                    .text_ellipsis()
                    .child(path.to_string_lossy().to_string()),
            )
            .child(setting_row(
                "Periodic Fetch",
                "Fetch all remotes periodically while this repository is active.",
                pill_toggle(
                    draft.repo_periodic_fetch_enabled,
                    entity.clone(),
                    "repo-periodic-fetch",
                    colors,
                ),
                border,
                text,
                muted,
            ))
            .child(setting_row(
                "Fetch Interval",
                "Minutes between periodic fetches for this repository.",
                text_field_control(
                    &draft.repo_fetch_interval_text,
                    SettingsTextField::RepoFetchInterval,
                    focused == SettingsTextField::RepoFetchInterval && input_focused,
                    "1",
                    colors,
                    entity.clone(),
                    input_focus,
                ),
                border,
                text,
                muted,
            ))
            .child(setting_row(
                "Auto Push on Commit",
                "Push the current branch to origin after a successful commit.",
                pill_toggle(
                    draft.repo_auto_push_on_commit,
                    entity.clone(),
                    "repo-auto-push",
                    colors,
                ),
                border,
                text,
                muted,
            ));
    } else {
        body = body.child(
            div()
                .text_xs()
                .text_color(muted)
                .child("Open a repository to customize repository-specific behavior."),
        );
    }

    body = body.child(
        div()
            .text_xs()
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(muted)
            .child("Open Tabs"),
    );
    if data.open_tabs.is_empty() {
        body = body.child(
            div()
                .text_xs()
                .text_color(muted)
                .child("No repositories open."),
        );
    } else {
        for (i, (tab_id, path)) in data.open_tabs.iter().enumerate() {
            let ent = main_app.clone();
            let id = *tab_id;
            let label = repo_path_label(path);
            let full = path.to_string_lossy().to_string();
            body = body.child(
                div()
                    .id(ElementId::NamedInteger(
                        "settings-open-tab".into(),
                        i as u64,
                    ))
                    .px_2()
                    .py_1()
                    .rounded(px(4.0))
                    .cursor_pointer()
                    .hover(|s| s.bg(hover))
                    .on_click(move |_ev, _window, cx| {
                        if let Some(app) = ent.upgrade() {
                            app.update(cx, |this, cx| {
                                this.activate_repo_tab_from_settings(id, cx);
                            });
                        }
                    })
                    .child(div().text_sm().text_color(text).child(label))
                    .child(
                        div()
                            .text_xs()
                            .text_color(muted)
                            .overflow_hidden()
                            .text_ellipsis()
                            .child(full),
                    ),
            );
        }
    }

    body = body.child(
        div()
            .text_xs()
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(muted)
            .child("Recent"),
    );
    if data.recent_paths.is_empty() {
        body = body.child(
            div()
                .text_xs()
                .text_color(muted)
                .child("No recent repositories."),
        );
    } else {
        for (i, path_str) in data.recent_paths.iter().enumerate() {
            let ent_open = main_app.clone();
            let ent_remove = main_app.clone();
            let path_buf = PathBuf::from(path_str);
            let path_remove = path_str.clone();
            let label = repo_path_label(&path_buf);
            body = body.child(
                div()
                    .id(ElementId::NamedInteger("settings-recent".into(), i as u64))
                    .px_2()
                    .py_1()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.0))
                            .child(div().text_sm().text_color(text).child(label))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(muted)
                                    .overflow_hidden()
                                    .text_ellipsis()
                                    .child(path_str.clone()),
                            ),
                    )
                    .child(
                        div()
                            .id(ElementId::NamedInteger(
                                "settings-recent-open".into(),
                                i as u64,
                            ))
                            .px_2()
                            .py_0()
                            .border_1()
                            .border_color(accent)
                            .rounded(px(3.0))
                            .cursor_pointer()
                            .on_click(move |_ev, _window, cx| {
                                if let Some(app) = ent_open.upgrade() {
                                    let p = path_buf.clone();
                                    app.update(cx, |this, cx| {
                                        this.open_repo_from_settings(p, cx);
                                    });
                                }
                            })
                            .child(div().text_xs().text_color(accent).child("Open")),
                    )
                    .child(
                        div()
                            .id(ElementId::NamedInteger(
                                "settings-recent-rm".into(),
                                i as u64,
                            ))
                            .px_2()
                            .py_0()
                            .border_1()
                            .border_color(border)
                            .rounded(px(3.0))
                            .cursor_pointer()
                            .on_click(move |_ev, _window, cx| {
                                if let Some(app) = ent_remove.upgrade() {
                                    let p = path_remove.clone();
                                    app.update(cx, |this, cx| {
                                        this.remove_recent_repo_path(p, cx);
                                    });
                                }
                            })
                            .child(div().text_xs().text_color(muted).child("Remove")),
                    ),
            );
        }
    }

    body = body.child(
        div()
            .text_xs()
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(muted)
            .child("Recently Closed"),
    );
    if data.closed_paths.is_empty() {
        body = body.child(
            div()
                .text_xs()
                .text_color(muted)
                .child("No recently closed tabs."),
        );
    } else {
        for (i, path) in data.closed_paths.iter().rev().enumerate() {
            let ent = main_app.clone();
            let path_buf = path.clone();
            let label = repo_path_label(path);
            let full = path.to_string_lossy().to_string();
            body = body.child(
                div()
                    .id(ElementId::NamedInteger("settings-closed".into(), i as u64))
                    .px_2()
                    .py_1()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.0))
                            .child(div().text_sm().text_color(text).child(label))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(muted)
                                    .overflow_hidden()
                                    .text_ellipsis()
                                    .child(full),
                            ),
                    )
                    .child(
                        div()
                            .id(ElementId::NamedInteger(
                                "settings-closed-reopen".into(),
                                i as u64,
                            ))
                            .px_2()
                            .py_0()
                            .border_1()
                            .border_color(accent)
                            .rounded(px(3.0))
                            .cursor_pointer()
                            .on_click(move |_ev, _window, cx| {
                                if let Some(app) = ent.upgrade() {
                                    let p = path_buf.clone();
                                    app.update(cx, |this, cx| {
                                        this.reopen_closed_repo_from_settings(p, cx);
                                    });
                                }
                            })
                            .child(div().text_xs().text_color(accent).child("Reopen")),
                    ),
            );
        }
    }

    body
}

fn render_accounts_section(
    mut body: Stateful<Div>,
    accounts: Vec<gitforge_hosting::HostingAccount>,
    colors: &AppColors,
    main_app: WeakEntity<GitForgeApp>,
    settings_entity: WeakEntity<SettingsWindow>,
    pending_account_provider: Option<&str>,
    pat_input: &TextInput,
    pat_focused: bool,
    pat_error: Option<&str>,
    window: &Window,
) -> Stateful<Div> {
    let border = rgba_to_hsla(colors.border);
    let text = rgba_to_hsla(colors.text);
    let muted = rgba_to_hsla(colors.text_muted);
    let accent = rgba_to_hsla(colors.accent);
    let warning = rgba_to_hsla(colors.warning);

    let ent_github = settings_entity.clone();
    let ent_gitlab = settings_entity.clone();
    let ent_codeberg = settings_entity.clone();
    let pat_fh = pat_input.focus_handle().clone();

    body = body
        .flex()
        .flex_col()
        .gap_3()
        .child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .text_xs()
                        .text_color(muted)
                        .child(
                            format!(
                                "Add an account with a Personal Access Token (PAT). Tokens are stored locally in {}.",
                                dirs::config_dir()
                                    .map(|d| d.join("gitforge").display().to_string())
                                    .unwrap_or_else(|| "~/.config/gitforge/".to_string()),
                            ),
                        ),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(muted)
                        .child(
                            "Choose a provider below. Required token scopes are listed when you add an account.",
                        ),
                ),
        )
        .child(
            div()
                .flex()
                .gap_2()
                .child(add_provider_button(
                    "GitHub",
                    accent,
                    border,
                    ent_github,
                    pat_fh.clone(),
                    "github",
                ))
                .child(add_provider_button(
                    "GitLab",
                    accent,
                    border,
                    ent_gitlab,
                    pat_fh.clone(),
                    "gitlab",
                ))
                .child(add_provider_button(
                    "Codeberg",
                    accent,
                    border,
                    ent_codeberg,
                    pat_fh,
                    "codeberg",
                )),
        );

    if let Some(provider) = pending_account_provider {
        let provider_label = provider_display_name(provider);
        let mut form = div()
            .flex()
            .flex_col()
            .gap_2()
            .px_2()
            .py_2()
            .border_1()
            .border_color(border)
            .rounded(px(4.0))
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(text)
                    .child(format!("Add {provider_label} account")),
            )
            .child(render_pat_scope_guidance(provider, muted, accent))
            .child(pat_field_control(
                pat_input,
                pat_focused,
                colors,
                settings_entity.clone(),
                window,
            ));
        if let Some(err) = pat_error {
            form = form.child(div().text_xs().text_color(warning).child(err.to_string()));
        }
        body = body.child(form);
    }

    if accounts.is_empty() {
        body = body.child(
            div()
                .text_xs()
                .text_color(muted)
                .child("No accounts configured."),
        );
    } else {
        let mut list = div().flex().flex_col().gap_1();
        for (i, account) in accounts.iter().enumerate() {
            let ent_remove = main_app.clone();
            let username = account.username.clone();
            let provider = account.provider.clone();
            let display = account.display_name.clone();
            let prov_lower = account.provider.clone();
            let provider_color = match account.provider.as_str() {
                "github" => accent,
                "gitlab" => rgba_to_hsla(colors.accent_secondary),
                "codeberg" => rgba_to_hsla(colors.success),
                _ => muted,
            };
            list = list.child(
                div()
                    .id(ElementId::NamedInteger("settings-account".into(), i as u64))
                    .px_2()
                    .py_1()
                    .border_1()
                    .border_color(border)
                    .rounded(px(4.0))
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(provider_color)
                                    .child(prov_lower),
                            )
                            .child(div().text_sm().text_color(text).child(display))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(muted)
                                    .child(format!("@{}", username)),
                            ),
                    )
                    .child(
                        div()
                            .id(ElementId::NamedInteger("settings-acct-rm".into(), i as u64))
                            .px_2()
                            .py_0()
                            .border_1()
                            .border_color(warning)
                            .rounded(px(3.0))
                            .cursor_pointer()
                            .on_click(move |_ev, _window, cx| {
                                if let Some(app) = ent_remove.upgrade() {
                                    let u = username.clone();
                                    let p = provider.clone();
                                    app.update(cx, |this, cx| {
                                        this.remove_hosting_account(u, p, cx);
                                    });
                                }
                            })
                            .child(div().text_xs().text_color(warning).child("Remove")),
                    ),
            );
        }
        body = body.child(list);
    }

    body
}

fn add_provider_button(
    label: &str,
    color: Hsla,
    border_color: Hsla,
    entity: WeakEntity<SettingsWindow>,
    pat_focus: FocusHandle,
    provider: &str,
) -> impl IntoElement {
    let provider = provider.to_string();
    div()
        .id(ElementId::Name(format!("settings-add-{}", provider).into()))
        .px_2()
        .py_1()
        .border_1()
        .border_color(border_color)
        .rounded(px(4.0))
        .cursor_pointer()
        .on_click(move |_ev, window, cx| {
            if let Some(settings) = entity.upgrade() {
                let p = provider.clone();
                let fh = pat_focus.clone();
                settings.update(cx, |this, cx| {
                    this.start_add_account(p, cx);
                });
                window.focus(&fh);
            }
        })
        .child(
            div()
                .text_xs()
                .text_color(color)
                .child(format!("+ {}", label)),
        )
}

fn repo_path_label(path: &Path) -> String {
    path.file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string_lossy().into_owned())
}
