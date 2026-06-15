use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub theme: String,
    pub last_repo_path: Option<String>,
    #[serde(default)]
    pub open_repo_paths: Vec<String>,
    #[serde(default)]
    pub recent_repo_paths: Vec<String>,
    #[serde(default)]
    pub active_repo_path: Option<String>,
    pub sidebar_branches_expanded: bool,
    pub sidebar_remotes_expanded: bool,
    pub sidebar_tags_expanded: bool,
    #[serde(default = "default_true")]
    pub sidebar_pull_requests_expanded: bool,
    pub window_width: f32,
    pub window_height: f32,
    pub ai: AiSettings,
    #[serde(default)]
    pub show_checkpoint_refs: bool,
    #[serde(default = "default_commit_limit")]
    pub commit_limit: usize,
    #[serde(default = "default_true")]
    pub graph_show_graph_column: bool,
    #[serde(default = "default_true")]
    pub graph_show_sha_column: bool,
    #[serde(default = "default_true")]
    pub graph_show_time_column: bool,
    #[serde(default = "default_true")]
    pub graph_show_author_column: bool,
    #[serde(default)]
    pub tools: ToolSettings,
    #[serde(default)]
    pub custom_commands: Vec<CustomCommand>,
    #[serde(default)]
    pub repo_settings: HashMap<String, RepoBehaviorSettings>,
    #[serde(default = "default_true")]
    pub auto_update: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoBehaviorSettings {
    #[serde(default)]
    pub periodic_fetch_enabled: bool,
    #[serde(default = "default_fetch_interval_minutes")]
    pub fetch_interval_minutes: u64,
    #[serde(default)]
    pub auto_push_on_commit: bool,
}

fn default_fetch_interval_minutes() -> u64 {
    15
}

impl Default for RepoBehaviorSettings {
    fn default() -> Self {
        Self {
            periodic_fetch_enabled: false,
            fetch_interval_minutes: default_fetch_interval_minutes(),
            auto_push_on_commit: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiSettings {
    pub provider: String,
    pub model: String,
    pub conventional_commits: bool,
    pub tone: String,
    pub ollama_url: String,
    /// Z.ai API surface: "general" or "coding" (GLM Coding Plan).
    #[serde(default = "default_zai_endpoint")]
    pub zai_endpoint: String,
    #[serde(default = "gitforge_ai::default_message_options_count")]
    pub message_options_count: u8,
    #[serde(default = "gitforge_ai::default_variation_mode")]
    pub variation_mode: String,
    #[serde(default = "gitforge_ai::default_default_alternative")]
    pub default_alternative: String,
    #[serde(default)]
    pub summary_max_chars: u32,
    #[serde(default = "gitforge_ai::default_body_wrap_at")]
    pub body_wrap_at: u32,
    #[serde(default)]
    pub max_diff_chars: usize,
    #[serde(default = "gitforge_ai::default_temperature")]
    pub temperature: f32,
}

impl AiSettings {
    pub fn commit_message_config(&self) -> gitforge_ai::CommitMessageConfig {
        gitforge_ai::CommitMessageConfig {
            tone: self.tone.clone(),
            conventional_commits: self.conventional_commits,
            message_options_count: gitforge_ai::clamp_options_count(self.message_options_count),
            variation_mode: self.variation_mode.clone(),
            default_alternative: self.default_alternative.clone(),
            summary_max_chars: self.summary_max_chars,
            body_wrap_at: self.body_wrap_at,
            max_diff_chars: self.max_diff_chars,
        }
    }

    pub fn provider_config(&self) -> gitforge_ai::ProviderConfig {
        gitforge_ai::ProviderConfig {
            model: self.model.clone(),
            ollama_url: self.ollama_url.clone(),
            zai_endpoint: gitforge_ai::parse_zai_endpoint(&self.zai_endpoint),
            temperature: gitforge_ai::clamp_temperature(self.temperature),
        }
    }
}

fn default_zai_endpoint() -> String {
    "general".to_string()
}

fn default_commit_limit() -> usize {
    1000
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSettings {
    #[serde(default = "default_editor_command")]
    pub editor_command: String,
    #[serde(default = "default_terminal_command")]
    pub terminal_command: String,
    #[serde(default)]
    pub diff_tool: String,
    #[serde(default)]
    pub merge_tool: String,
}

fn default_editor_command() -> String {
    std::env::var("EDITOR")
        .or_else(|_| std::env::var("VISUAL"))
        .unwrap_or_else(|_| "xdg-open".into())
}

fn default_terminal_command() -> String {
    std::env::var("TERMINAL").unwrap_or_else(|_| {
        for cmd in &[
            "alacritty",
            "kitty",
            "wezterm",
            "gnome-terminal",
            "konsole",
            "xterm",
        ] {
            if which_exists(cmd) {
                return cmd.to_string();
            }
        }
        "xterm".into()
    })
}

fn which_exists(cmd: &str) -> bool {
    std::process::Command::new("which")
        .arg(cmd)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

impl Default for ToolSettings {
    fn default() -> Self {
        Self {
            editor_command: default_editor_command(),
            terminal_command: default_terminal_command(),
            diff_tool: String::new(),
            merge_tool: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomCommand {
    pub name: String,
    pub command: String,
    #[serde(default)]
    pub description: String,
}

impl Default for AiSettings {
    fn default() -> Self {
        Self {
            provider: "disabled".to_string(),
            model: String::new(),
            conventional_commits: false,
            tone: "balanced".to_string(),
            ollama_url: "http://localhost:11434".to_string(),
            zai_endpoint: default_zai_endpoint(),
            message_options_count: gitforge_ai::default_message_options_count(),
            variation_mode: gitforge_ai::default_variation_mode(),
            default_alternative: gitforge_ai::default_default_alternative(),
            summary_max_chars: 0,
            body_wrap_at: gitforge_ai::default_body_wrap_at(),
            max_diff_chars: 0,
            temperature: gitforge_ai::default_temperature(),
        }
    }
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            theme: "default-dark".to_string(),
            last_repo_path: None,
            open_repo_paths: Vec::new(),
            recent_repo_paths: Vec::new(),
            active_repo_path: None,
            sidebar_branches_expanded: true,
            sidebar_remotes_expanded: true,
            sidebar_tags_expanded: true,
            sidebar_pull_requests_expanded: true,
            window_width: 1280.0,
            window_height: 800.0,
            ai: AiSettings::default(),
            show_checkpoint_refs: false,
            commit_limit: default_commit_limit(),
            graph_show_graph_column: true,
            graph_show_sha_column: true,
            graph_show_time_column: true,
            graph_show_author_column: true,
            tools: ToolSettings::default(),
            custom_commands: Vec::new(),
            repo_settings: HashMap::new(),
            auto_update: true,
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
        let mut settings = match std::fs::read_to_string(&path) {
            Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
            Err(_) => Self::default(),
        };

        if settings.open_repo_paths.is_empty()
            && let Some(path) = settings.last_repo_path.clone()
        {
            settings.open_repo_paths.push(path);
        }

        if settings.active_repo_path.is_none() {
            settings.active_repo_path = settings.open_repo_paths.first().cloned();
        }

        settings
    }

    pub fn save(&self) {
        let dir = Self::config_dir();
        let _ = std::fs::create_dir_all(&dir);
        let path = Self::settings_path();
        if let Ok(content) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(path, content);
        }
    }

    pub fn repo_settings_key(path: &Path) -> String {
        std::fs::canonicalize(path)
            .unwrap_or_else(|_| path.to_path_buf())
            .to_string_lossy()
            .to_string()
    }

    pub fn repo_settings_for_path(&self, path: &Path) -> RepoBehaviorSettings {
        self.repo_settings
            .get(&Self::repo_settings_key(path))
            .cloned()
            .unwrap_or_default()
    }

    pub fn repo_settings_for_path_mut(&mut self, path: &Path) -> &mut RepoBehaviorSettings {
        let key = Self::repo_settings_key(path);
        self.repo_settings.entry(key).or_default()
    }
}
