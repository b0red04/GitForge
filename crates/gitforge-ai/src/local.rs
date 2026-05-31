use anyhow::{Result, bail};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use crate::provider::AiProvider;

pub struct OllamaProvider {
    base_url: String,
    model: String,
}

impl OllamaProvider {
    pub fn new(base_url: &str, model: &str) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            model: model.to_string(),
        }
    }

    pub fn default_local() -> Self {
        Self::new("http://localhost:11434", "codellama")
    }
}

#[derive(Serialize)]
struct OllamaRequest {
    model: String,
    prompt: String,
    system: Option<String>,
    stream: bool,
}

#[derive(Deserialize)]
struct OllamaResponse {
    response: String,
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
        };

        let client = reqwest::Client::new();
        let resp = client.post(&url)
            .json(&body)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            bail!("Ollama API error ({}): {}", status, text);
        }

        let data: OllamaResponse = resp.json().await?;
        Ok(data.response.trim().to_string())
    }
}
