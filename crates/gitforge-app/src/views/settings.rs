use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub theme: String,
    pub last_repo_path: Option<String>,
    pub sidebar_branches_expanded: bool,
    pub sidebar_remotes_expanded: bool,
    pub sidebar_tags_expanded: bool,
    pub window_width: f32,
    pub window_height: f32,
    pub ai: AiSettings,
    /// Include non-standard refs (e.g. `refs/t3/checkpoints/`) in commit history.
    #[serde(default)]
    pub show_checkpoint_refs: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiSettings {
    pub provider: String,
    pub model: String,
    pub conventional_commits: bool,
    pub tone: String,
    pub ollama_url: String,
}

impl Default for AiSettings {
    fn default() -> Self {
        Self {
            provider: "disabled".to_string(),
            model: String::new(),
            conventional_commits: false,
            tone: "balanced".to_string(),
            ollama_url: "http://localhost:11434".to_string(),
        }
    }
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            theme: "default-dark".to_string(),
            last_repo_path: None,
            sidebar_branches_expanded: true,
            sidebar_remotes_expanded: true,
            sidebar_tags_expanded: true,
            window_width: 1280.0,
            window_height: 800.0,
            ai: AiSettings::default(),
            show_checkpoint_refs: false,
        }
    }
}

impl AppSettings {
    pub fn config_dir() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("gitforge")
    }

    pub fn settings_path() -> PathBuf {
        Self::config_dir().join("settings.json")
    }

    pub fn load() -> Self {
        let path = Self::settings_path();
        match std::fs::read_to_string(&path) {
            Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    pub fn save(&self) {
        let dir = Self::config_dir();
        let _ = std::fs::create_dir_all(&dir);
        let path = Self::settings_path();
        if let Ok(content) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(path, content);
        }
    }
}
