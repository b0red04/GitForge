use serde::{Deserialize, Serialize};
use std::path::Path;

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
    #[serde(default = "default_syntax_operator")]
    pub syntax_operator: String,
    #[serde(default = "default_syntax_property")]
    pub syntax_property: String,
    #[serde(default = "default_syntax_tag")]
    pub syntax_tag: String,
    #[serde(default = "default_syntax_attribute")]
    pub syntax_attribute: String,
    #[serde(default = "default_syntax_constant")]
    pub syntax_constant: String,
    #[serde(default = "default_syntax_module")]
    pub syntax_module: String,
    #[serde(default = "default_syntax_punctuation")]
    pub syntax_punctuation: String,
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
fn default_syntax_keyword() -> String { "#ff7b72".into() }
fn default_syntax_function() -> String { "#d2a8ff".into() }
fn default_syntax_string() -> String { "#a5d6ff".into() }
fn default_syntax_number() -> String { "#79c0ff".into() }
fn default_syntax_comment() -> String { "#8b949e".into() }
fn default_syntax_type() -> String { "#ffa657".into() }
fn default_syntax_variable() -> String { "#e6edf3".into() }
fn default_syntax_operator() -> String { "#79c0ff".into() }
fn default_syntax_property() -> String { "#79c0ff".into() }
fn default_syntax_tag() -> String { "#7ee787".into() }
fn default_syntax_attribute() -> String { "#79c0ff".into() }
fn default_syntax_constant() -> String { "#79c0ff".into() }
fn default_syntax_module() -> String { "#ffa657".into() }
fn default_syntax_punctuation() -> String { "#e6edf3".into() }

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
        Self::load_from_str(include_str!("../../../assets/themes/default-dark.json"))
            .expect("default dark theme should be valid")
    }

    pub fn default_light() -> Self {
        Self::load_from_str(include_str!("../../../assets/themes/default-light.json"))
            .expect("default light theme should be valid")
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
}
