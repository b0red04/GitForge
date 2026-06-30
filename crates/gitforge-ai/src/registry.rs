use std::str::FromStr;

use crate::config::{ProviderConfig, ZaiEndpoint};
use crate::error::{AiError, AiResult};
use crate::get_api_key;
use crate::provider::AiProvider;
use crate::providers::{
    AnthropicProvider, OllamaProvider, list_ollama_models, list_openai_compatible_models,
    openai_provider, zai_models_base_url, zai_provider,
};

#[derive(Debug, Clone, Copy)]
pub struct ProviderDescriptor {
    pub id: &'static str,
    pub label: &'static str,
    pub needs_api_key: bool,
    pub default_model: &'static str,
}

pub const AI_PROVIDERS: &[ProviderDescriptor] = &[
    ProviderDescriptor {
        id: "zai",
        label: "Z.ai",
        needs_api_key: true,
        default_model: "glm-5.1",
    },
    ProviderDescriptor {
        id: "ollama",
        label: "Ollama",
        needs_api_key: false,
        default_model: "codellama",
    },
    ProviderDescriptor {
        id: "openai",
        label: "OpenAI",
        needs_api_key: true,
        default_model: "gpt-4o-mini",
    },
    ProviderDescriptor {
        id: "anthropic",
        label: "Anthropic",
        needs_api_key: true,
        default_model: "claude-sonnet-4-20250514",
    },
];

pub fn provider_descriptor(id: &str) -> Option<&'static ProviderDescriptor> {
    AI_PROVIDERS.iter().find(|p| p.id == id)
}

pub fn create_provider(name: &str, config: &ProviderConfig) -> AiResult<Box<dyn AiProvider>> {
    let model = config.model_or_default(name);

    match name {
        "ollama" => Ok(Box::new(OllamaProvider::new(
            &config.ollama_url,
            &model,
            config.temperature,
        ))),
        "openai" => {
            let api_key = get_api_key("openai")?;
            Ok(Box::new(openai_provider(
                &api_key,
                &model,
                config.temperature,
            )))
        }
        "anthropic" => {
            let api_key = get_api_key("anthropic")?;
            Ok(Box::new(AnthropicProvider::new(
                &api_key,
                &model,
                config.temperature,
            )))
        }
        "zai" => {
            let api_key = get_api_key("zai")?;
            Ok(Box::new(zai_provider(
                &api_key,
                &model,
                config.zai_endpoint,
                config.temperature,
            )))
        }
        other => Err(AiError::unknown_provider(other)),
    }
}

pub async fn list_models_for_provider(
    provider: &str,
    ollama_url: &str,
    zai_endpoint: ZaiEndpoint,
    api_key: Option<String>,
) -> AiResult<Vec<String>> {
    match provider {
        "openai" => {
            let api_key = api_key.ok_or_else(|| AiError::api_key_not_configured("openai"))?;
            list_openai_compatible_models(crate::providers::openai::OPENAI_BASE_URL, &api_key).await
        }
        "zai" => {
            let api_key = api_key.ok_or_else(|| AiError::api_key_not_configured("zai"))?;
            list_openai_compatible_models(zai_models_base_url(zai_endpoint), &api_key).await
        }
        "ollama" => list_ollama_models(ollama_url).await,
        "anthropic" => Ok(Vec::new()),
        other => Err(AiError::UnknownProvider(format!(
            "Provider does not support model listing: {other}"
        ))),
    }
}

pub fn parse_zai_endpoint(s: &str) -> ZaiEndpoint {
    ZaiEndpoint::from_str(s).unwrap_or_default()
}
