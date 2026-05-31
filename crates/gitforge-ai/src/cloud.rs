use anyhow::{Result, bail};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use crate::provider::AiProvider;

pub struct OpenAiProvider {
    api_key: String,
    model: String,
}

impl OpenAiProvider {
    pub fn new(api_key: &str, model: &str) -> Self {
        Self {
            api_key: api_key.to_string(),
            model: model.to_string(),
        }
    }

    pub fn with_keyring(key_name: &str, model: &str) -> Result<Self> {
        let entry = keyring::Entry::new("gitforge-ai", key_name)?;
        let api_key = entry.get_password()?;
        Ok(Self::new(&api_key, model))
    }
}

#[derive(Serialize)]
struct OpenAiRequest {
    model: String,
    messages: Vec<OpenAiMessage>,
    temperature: f32,
}

#[derive(Serialize, Deserialize)]
struct OpenAiMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct OpenAiResponse {
    choices: Vec<OpenAiChoice>,
}

#[derive(Deserialize)]
struct OpenAiChoice {
    message: OpenAiMessage,
}

#[async_trait]
impl AiProvider for OpenAiProvider {
    fn name(&self) -> &str {
        "openai"
    }

    async fn generate(&self, prompt: &str, system: Option<&str>) -> Result<String> {
        let mut messages = Vec::new();
        if let Some(sys) = system {
            messages.push(OpenAiMessage {
                role: "system".to_string(),
                content: sys.to_string(),
            });
        }
        messages.push(OpenAiMessage {
            role: "user".to_string(),
            content: prompt.to_string(),
        });

        let body = OpenAiRequest {
            model: self.model.clone(),
            messages,
            temperature: 0.3,
        };

        let client = reqwest::Client::new();
        let resp = client.post("https://api.openai.com/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&body)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            bail!("OpenAI API error ({}): {}", status, text);
        }

        let data: OpenAiResponse = resp.json().await?;
        match data.choices.first() {
            Some(choice) => Ok(choice.message.content.trim().to_string()),
            None => bail!("OpenAI returned no choices"),
        }
    }
}

pub struct AnthropicProvider {
    api_key: String,
    model: String,
}

impl AnthropicProvider {
    pub fn new(api_key: &str, model: &str) -> Self {
        Self {
            api_key: api_key.to_string(),
            model: model.to_string(),
        }
    }

    pub fn with_keyring(key_name: &str, model: &str) -> Result<Self> {
        let entry = keyring::Entry::new("gitforge-ai", key_name)?;
        let api_key = entry.get_password()?;
        Ok(Self::new(&api_key, model))
    }
}

#[derive(Serialize)]
struct AnthropicRequest {
    model: String,
    max_tokens: u32,
    system: Option<String>,
    messages: Vec<AnthropicMessage>,
}

#[derive(Serialize)]
struct AnthropicMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct AnthropicResponse {
    content: Vec<AnthropicContent>,
}

#[derive(Deserialize)]
struct AnthropicContent {
    text: String,
}

#[async_trait]
impl AiProvider for AnthropicProvider {
    fn name(&self) -> &str {
        "anthropic"
    }

    async fn generate(&self, prompt: &str, system: Option<&str>) -> Result<String> {
        let body = AnthropicRequest {
            model: self.model.clone(),
            max_tokens: 1024,
            system: system.map(|s| s.to_string()),
            messages: vec![AnthropicMessage {
                role: "user".to_string(),
                content: prompt.to_string(),
            }],
        };

        let client = reqwest::Client::new();
        let resp = client.post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            bail!("Anthropic API error ({}): {}", status, text);
        }

        let data: AnthropicResponse = resp.json().await?;
        match data.content.first() {
            Some(block) => Ok(block.text.trim().to_string()),
            None => bail!("Anthropic returned no content"),
        }
    }
}
