use gitforge_remote::ensure_success;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::error::{AiNet, AiResult};
use crate::provider::AiProvider;

pub struct OllamaProvider {
    base_url: String,
    model: String,
    temperature: f32,
}

impl OllamaProvider {
    pub fn new(base_url: &str, model: &str, temperature: f32) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            model: model.to_string(),
            temperature,
        }
    }
}

#[derive(Serialize)]
struct OllamaRequest {
    model: String,
    prompt: String,
    system: Option<String>,
    stream: bool,
    options: OllamaOptions,
}

#[derive(Serialize)]
struct OllamaOptions {
    temperature: f32,
}

#[derive(Deserialize)]
struct OllamaResponse {
    response: String,
}

#[derive(Deserialize)]
struct OllamaTagsResponse {
    models: Vec<OllamaModelTag>,
}

#[derive(Deserialize)]
struct OllamaModelTag {
    name: String,
}

pub async fn list_ollama_models(base_url: &str) -> AiResult<Vec<String>> {
    let base = base_url.trim_end_matches('/');
    let url = format!("{base}/api/tags");
    let context = "Ollama models API request failed";

    let client = reqwest::Client::new();
    let resp = client.get(&url).send().await.ai_context(context)?;
    let resp = ensure_success(resp, context).await?;

    let data: OllamaTagsResponse = resp.json().await.ai_context(context)?;
    let mut names: Vec<String> = data.models.into_iter().map(|m| m.name).collect();
    names.sort();
    names.dedup();
    Ok(names)
}

#[async_trait]
impl AiProvider for OllamaProvider {
    fn name(&self) -> &str {
        "ollama"
    }

    async fn generate(&self, prompt: &str, system: Option<&str>) -> AiResult<String> {
        let url = format!("{}/api/generate", self.base_url);
        let body = OllamaRequest {
            model: self.model.clone(),
            prompt: prompt.to_string(),
            system: system.map(|s| s.to_string()),
            stream: false,
            options: OllamaOptions {
                temperature: self.temperature,
            },
        };

        let context = "Ollama API request failed";
        let client = reqwest::Client::new();
        let resp = client
            .post(&url)
            .json(&body)
            .send()
            .await
            .ai_context(context)?;
        let resp = ensure_success(resp, context).await?;

        let data: OllamaResponse = resp.json().await.ai_context(context)?;
        Ok(data.response.trim().to_string())
    }
}
