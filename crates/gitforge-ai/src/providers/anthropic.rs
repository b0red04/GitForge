use gitforge_remote::ensure_success;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::error::{AiError, AiNet, AiResult};
use crate::provider::AiProvider;

pub struct AnthropicProvider {
    api_key: String,
    model: String,
    temperature: f32,
}

impl AnthropicProvider {
    pub fn new(api_key: &str, model: &str, temperature: f32) -> Self {
        Self {
            api_key: api_key.to_string(),
            model: model.to_string(),
            temperature,
        }
    }
}

#[derive(Serialize)]
struct AnthropicRequest {
    model: String,
    max_tokens: u32,
    temperature: f32,
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

    async fn generate(&self, prompt: &str, system: Option<&str>) -> AiResult<String> {
        let body = AnthropicRequest {
            model: self.model.clone(),
            max_tokens: 1024,
            temperature: self.temperature,
            system: system.map(|s| s.to_string()),
            messages: vec![AnthropicMessage {
                role: "user".to_string(),
                content: prompt.to_string(),
            }],
        };

        let context = "Anthropic API request failed";
        let client = reqwest::Client::new();
        let resp = client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
            .ai_context(context)?;
        let resp = ensure_success(resp, context).await?;

        let data: AnthropicResponse = resp.json().await.ai_context(context)?;
        match data.content.first() {
            Some(block) => Ok(block.text.trim().to_string()),
            None => Err(AiError::empty_response("anthropic")),
        }
    }
}
