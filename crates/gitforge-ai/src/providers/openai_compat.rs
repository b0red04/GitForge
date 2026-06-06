use anyhow::{Result, bail};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::provider::AiProvider;

pub struct OpenAiCompatibleProvider {
    base_url: String,
    api_key: String,
    model: String,
    provider_id: String,
    temperature: f32,
}

impl OpenAiCompatibleProvider {
    pub fn new(
        base_url: &str,
        api_key: &str,
        model: &str,
        provider_id: &str,
        temperature: f32,
    ) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key: api_key.to_string(),
            model: model.to_string(),
            provider_id: provider_id.to_string(),
            temperature,
        }
    }
}

#[derive(Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    temperature: f32,
}

#[derive(Serialize, Deserialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Deserialize)]
struct ChatChoice {
    message: ChatMessage,
}

#[derive(Deserialize)]
struct ModelsResponse {
    data: Vec<ModelEntry>,
}

#[derive(Deserialize)]
struct ModelEntry {
    id: String,
}

pub async fn list_openai_compatible_models(base_url: &str, api_key: &str) -> Result<Vec<String>> {
    let base = base_url.trim_end_matches('/');
    let url = format!("{base}/models");

    let client = reqwest::Client::new();
    let resp = client
        .get(&url)
        .header("Authorization", format!("Bearer {api_key}"))
        .header("Accept-Language", "en-US,en")
        .send()
        .await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        bail!("Models API error ({status}): {text}");
    }

    let data: ModelsResponse = resp.json().await?;
    let mut ids: Vec<String> = data.data.into_iter().map(|m| m.id).collect();
    ids.sort();
    ids.dedup();
    Ok(ids)
}

#[async_trait]
impl AiProvider for OpenAiCompatibleProvider {
    fn name(&self) -> &str {
        &self.provider_id
    }

    async fn generate(&self, prompt: &str, system: Option<&str>) -> Result<String> {
        let mut messages = Vec::new();
        if let Some(sys) = system {
            messages.push(ChatMessage {
                role: "system".to_string(),
                content: sys.to_string(),
            });
        }
        messages.push(ChatMessage {
            role: "user".to_string(),
            content: prompt.to_string(),
        });

        let body = ChatRequest {
            model: self.model.clone(),
            messages,
            temperature: self.temperature,
        };

        let url = format!("{}/chat/completions", self.base_url);
        let client = reqwest::Client::new();
        let resp = client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Accept-Language", "en-US,en")
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            bail!("{} API error ({}): {}", self.provider_id, status, text);
        }

        let data: ChatResponse = resp.json().await?;
        match data.choices.first() {
            Some(choice) => Ok(choice.message.content.trim().to_string()),
            None => bail!("{} returned no choices", self.provider_id),
        }
    }
}
