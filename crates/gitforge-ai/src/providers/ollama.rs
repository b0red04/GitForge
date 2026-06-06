use anyhow::{Result, bail};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

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

pub async fn list_ollama_models(base_url: &str) -> Result<Vec<String>> {
    let base = base_url.trim_end_matches('/');
    let url = format!("{base}/api/tags");

    let client = reqwest::Client::new();
    let resp = client.get(&url).send().await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        bail!("Ollama models API error ({status}): {text}");
    }

    let data: OllamaTagsResponse = resp.json().await?;
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

    async fn generate(&self, prompt: &str, system: Option<&str>) -> Result<String> {
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

        let client = reqwest::Client::new();
        let resp = client.post(&url).json(&body).send().await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            bail!("Ollama API error ({}): {}", status, text);
        }

        let data: OllamaResponse = resp.json().await?;
        Ok(data.response.trim().to_string())
    }
}
