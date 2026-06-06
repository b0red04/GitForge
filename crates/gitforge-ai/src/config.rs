use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ZaiEndpoint {
    #[default]
    General,
    Coding,
}

impl ZaiEndpoint {
    pub const GENERAL_URL: &'static str = "https://api.z.ai/api/paas/v4";
    pub const CODING_URL: &'static str = "https://api.z.ai/api/coding/paas/v4";

    pub fn base_url(self) -> &'static str {
        match self {
            Self::General => Self::GENERAL_URL,
            Self::Coding => Self::CODING_URL,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::General => "general",
            Self::Coding => "coding",
        }
    }
}

impl FromStr for ZaiEndpoint {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "coding" => Ok(Self::Coding),
            "general" => Ok(Self::General),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProviderConfig {
    pub model: String,
    pub ollama_url: String,
    pub zai_endpoint: ZaiEndpoint,
    pub temperature: f32,
}

impl ProviderConfig {
    pub fn model_or_default(&self, provider: &str) -> String {
        if !self.model.is_empty() {
            return self.model.clone();
        }
        default_model_for_provider(provider).to_string()
    }
}

impl Default for ProviderConfig {
    fn default() -> Self {
        Self {
            model: String::new(),
            ollama_url: "http://localhost:11434".to_string(),
            zai_endpoint: ZaiEndpoint::default(),
            temperature: default_temperature(),
        }
    }
}

/// Settings that control commit message generation prompts and parsing.
#[derive(Debug, Clone)]
pub struct CommitMessageConfig {
    pub tone: String,
    pub conventional_commits: bool,
    pub message_options_count: u8,
    pub variation_mode: String,
    pub default_alternative: String,
    pub summary_max_chars: u32,
    pub body_wrap_at: u32,
    pub max_diff_chars: usize,
}

impl Default for CommitMessageConfig {
    fn default() -> Self {
        Self {
            tone: "balanced".to_string(),
            conventional_commits: false,
            message_options_count: default_message_options_count(),
            variation_mode: default_variation_mode(),
            default_alternative: default_default_alternative(),
            summary_max_chars: 0,
            body_wrap_at: default_body_wrap_at(),
            max_diff_chars: 0,
        }
    }
}

impl CommitMessageConfig {
    pub fn options_count(&self) -> usize {
        clamp_options_count(self.message_options_count) as usize
    }

    pub fn normalized_variation_mode(&self) -> &str {
        normalize_variation_mode(&self.variation_mode)
    }

    pub fn normalized_default_alternative(&self) -> &str {
        normalize_default_alternative(&self.default_alternative)
    }
}

pub fn default_model_for_provider(provider: &str) -> &'static str {
    match provider {
        "zai" => "glm-5.1",
        "ollama" => "codellama",
        "openai" => "gpt-4o-mini",
        "anthropic" => "claude-sonnet-4-20250514",
        _ => "",
    }
}

pub fn default_message_options_count() -> u8 {
    3
}

pub fn default_variation_mode() -> String {
    "mixed".to_string()
}

pub fn default_default_alternative() -> String {
    "first".to_string()
}

pub fn default_body_wrap_at() -> u32 {
    72
}

pub fn default_temperature() -> f32 {
    0.3
}

pub fn clamp_options_count(count: u8) -> u8 {
    count.clamp(1, 3)
}

pub fn clamp_temperature(temp: f32) -> f32 {
    temp.clamp(0.0, 1.0)
}

pub fn normalize_tone(tone: &str) -> &str {
    match tone {
        "verbose" => "detailed",
        other => other,
    }
}

pub fn normalize_variation_mode(mode: &str) -> &str {
    match mode {
        "uniform" | "detailed" => mode,
        _ => "mixed",
    }
}

pub fn normalize_default_alternative(mode: &str) -> &str {
    match mode {
        "shortest" | "longest" => mode,
        _ => "first",
    }
}

/// Pick which generated message should fill the commit input.
pub fn pick_default_message(messages: &[String], mode: &str) -> usize {
    if messages.is_empty() {
        return 0;
    }
    match normalize_default_alternative(mode) {
        "shortest" => messages
            .iter()
            .enumerate()
            .min_by_key(|(_, m)| m.len())
            .map(|(i, _)| i)
            .unwrap_or(0),
        "longest" => messages
            .iter()
            .enumerate()
            .max_by_key(|(_, m)| m.len())
            .map(|(i, _)| i)
            .unwrap_or(0),
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zai_endpoint_urls() {
        assert_eq!(
            ZaiEndpoint::General.base_url(),
            "https://api.z.ai/api/paas/v4"
        );
        assert_eq!(
            ZaiEndpoint::Coding.base_url(),
            "https://api.z.ai/api/coding/paas/v4"
        );
    }

    #[test]
    fn zai_endpoint_from_str() {
        assert_eq!("coding".parse(), Ok(ZaiEndpoint::Coding));
        assert_eq!("general".parse(), Ok(ZaiEndpoint::General));
        assert!("invalid".parse::<ZaiEndpoint>().is_err());
    }

    #[test]
    fn normalize_verbose_tone() {
        assert_eq!(normalize_tone("verbose"), "detailed");
        assert_eq!(normalize_tone("concise"), "concise");
    }

    #[test]
    fn clamp_options_count_bounds() {
        assert_eq!(clamp_options_count(0), 1);
        assert_eq!(clamp_options_count(1), 1);
        assert_eq!(clamp_options_count(3), 3);
        assert_eq!(clamp_options_count(9), 3);
    }

    #[test]
    fn clamp_temperature_bounds() {
        assert_eq!(clamp_temperature(-1.0), 0.0);
        assert_eq!(clamp_temperature(0.3), 0.3);
        assert_eq!(clamp_temperature(2.0), 1.0);
    }

    #[test]
    fn pick_default_message_modes() {
        let messages = vec![
            "short".to_string(),
            "a much longer detailed message body".to_string(),
            "medium length".to_string(),
        ];
        assert_eq!(pick_default_message(&messages, "first"), 0);
        assert_eq!(pick_default_message(&messages, "shortest"), 0);
        assert_eq!(pick_default_message(&messages, "longest"), 1);
    }
}
