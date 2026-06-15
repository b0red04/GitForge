use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Theme {
    pub name: String,
    pub appearance: Appearance,
    pub colors: ThemeColors,
    pub fonts: ThemeFonts,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Appearance {
    Dark,
    Light,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeColors {
    pub background: String,
    pub surface: String,
    pub surface_high: String,
    pub border: String,
    pub border_focused: String,
    pub text: String,
    #[serde(default = "default_muted_text")]
    pub text_muted: String,
    pub accent: String,
    #[serde(default = "default_accent_secondary")]
    pub accent_secondary: String,
    pub error: String,
    pub warning: String,
    pub success: String,

    pub sidebar_background: String,
    pub sidebar_text: String,
    pub sidebar_selected: String,
    pub sidebar_hover: String,

    pub commit_hash: String,
    pub ref_branch: String,
    pub ref_tag: String,
    pub ref_remote: String,
    pub ref_head: String,

    pub diff_added: String,
    pub diff_added_bg: String,
    pub diff_removed: String,
    pub diff_removed_bg: String,
    pub diff_hunk_header: String,

    pub graph_lane_1: String,
    pub graph_lane_2: String,
    pub graph_lane_3: String,
    pub graph_lane_4: String,
    pub graph_lane_5: String,
    pub graph_lane_6: String,
    pub graph_lane_7: String,
    pub graph_lane_8: String,

    pub scroll_bar: String,
    pub scroll_bar_hover: String,
    pub selection: String,
    pub selection_bg: String,

    #[serde(default = "default_syntax_keyword")]
    pub syntax_keyword: String,
    #[serde(default = "default_syntax_function")]
    pub syntax_function: String,
    #[serde(default = "default_syntax_string")]
    pub syntax_string: String,
    #[serde(default = "default_syntax_number")]
    pub syntax_number: String,
    #[serde(default = "default_syntax_comment")]
    pub syntax_comment: String,
    #[serde(default = "default_syntax_type")]
    pub syntax_type: String,
    #[serde(default = "default_syntax_variable")]
    pub syntax_variable: String,
    #[serde(default = "default_syntax_property")]
    pub syntax_property: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeFonts {
    #[serde(default = "default_ui_font")]
    pub ui: String,
    #[serde(default = "default_mono_font")]
    pub mono: String,
    #[serde(default = "default_ui_size")]
    pub ui_size: f32,
    #[serde(default = "default_mono_size")]
    pub mono_size: f32,
}

fn default_muted_text() -> String {
    "#8b949e".into()
}
fn default_accent_secondary() -> String {
    "#f0a030".into()
}
fn default_ui_font() -> String {
    "Inter".into()
}
fn default_mono_font() -> String {
    "JetBrains Mono".into()
}
fn default_ui_size() -> f32 {
    13.0
}
fn default_mono_size() -> f32 {
    13.0
}
fn default_syntax_keyword() -> String {
    "#ff7b72".into()
}
fn default_syntax_function() -> String {
    "#d2a8ff".into()
}
fn default_syntax_string() -> String {
    "#a5d6ff".into()
}
fn default_syntax_number() -> String {
    "#79c0ff".into()
}
fn default_syntax_comment() -> String {
    "#8b949e".into()
}
fn default_syntax_type() -> String {
    "#ffa657".into()
}
fn default_syntax_variable() -> String {
    "#e6edf3".into()
}
fn default_syntax_property() -> String {
    "#79c0ff".into()
}

impl Theme {
    pub fn load_from_file(path: &Path) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let theme: Theme = serde_json::from_str(&content)?;
        Ok(theme)
    }

    pub fn load_from_str(content: &str) -> anyhow::Result<Self> {
        let theme: Theme = serde_json::from_str(content)?;
        Ok(theme)
    }

    pub fn default_dark() -> Self {
        load_bundled("default-dark").expect("default-dark should be bundled")
    }

    pub fn default_light() -> Self {
        load_bundled("default-light").expect("default-light should be bundled")
    }

    pub fn graph_lane_color(&self, lane: usize) -> &str {
        let colors = [
            &self.colors.graph_lane_1,
            &self.colors.graph_lane_2,
            &self.colors.graph_lane_3,
            &self.colors.graph_lane_4,
            &self.colors.graph_lane_5,
            &self.colors.graph_lane_6,
            &self.colors.graph_lane_7,
            &self.colors.graph_lane_8,
        ];
        colors[lane % colors.len()]
    }

    pub fn load_by_name(name: &str) -> anyhow::Result<Self> {
        if let Some(theme) = load_bundled(name) {
            return Ok(theme);
        }

        if let Some(theme) = load_user_theme(name) {
            return Ok(theme);
        }

        if let Some(theme) = load_system_theme(name) {
            return Ok(theme);
        }

        anyhow::bail!("Theme '{}' not found", name)
    }

    pub fn discover_themes() -> Vec<ThemeEntry> {
        let mut themes = bundled_theme_entries();

        let bundled_names: std::collections::HashSet<String> =
            themes.iter().map(|t| t.name.clone()).collect();

        if let Ok(entries) = std::fs::read_dir(user_themes_dir()) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) != Some("json") {
                    continue;
                }
                let stem = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("unknown");
                if bundled_names.contains(stem) {
                    continue;
                }
                if let Ok(theme) = Self::load_from_file(&path) {
                    themes.push(ThemeEntry {
                        name: stem.to_string(),
                        display_name: theme.name.clone(),
                        appearance: theme.appearance,
                    });
                }
            }
        }

        themes
    }
}

include!(concat!(env!("OUT_DIR"), "/bundled_themes.rs"));

fn user_themes_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("gitforge")
        .join("themes")
}

fn system_themes_dir() -> PathBuf {
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        PathBuf::from("/usr/share/gitforge/themes")
    }
    #[cfg(not(all(unix, not(target_os = "macos"))))]
    {
        dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("gitforge")
            .join("themes")
    }
}

fn load_user_theme(name: &str) -> Option<Theme> {
    let path = user_themes_dir().join(format!("{name}.json"));
    if path.exists() {
        Theme::load_from_file(&path).ok()
    } else {
        None
    }
}

fn load_system_theme(name: &str) -> Option<Theme> {
    let path = system_themes_dir().join(format!("{name}.json"));
    if path.exists() {
        Theme::load_from_file(&path).ok()
    } else {
        None
    }
}

#[derive(Debug, Clone)]
pub struct ThemeEntry {
    pub name: String,
    pub display_name: String,
    pub appearance: Appearance,
}
