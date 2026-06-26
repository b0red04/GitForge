pub mod config;
pub mod prompt;
pub mod provider;
pub mod providers;
pub mod registry;
mod secrets;

pub use config::{
    CommitMessageConfig, ProviderConfig, ZaiEndpoint, clamp_options_count, clamp_temperature,
    default_body_wrap_at, default_default_alternative, default_message_options_count,
    default_model_for_provider, default_temperature, default_variation_mode, normalize_tone,
    pick_default_message,
};
pub use prompt::{sanitize_branch_name, truncate_diff};
pub use provider::AiProvider;
pub use providers::{
    AnthropicProvider, OllamaProvider, OpenAiCompatibleProvider, list_ollama_models,
    list_openai_compatible_models,
};
pub use registry::{
    AI_PROVIDERS, ProviderDescriptor, create_provider, list_models_for_provider,
    parse_zai_endpoint, provider_descriptor,
};

use anyhow::Result;

pub fn store_api_key(provider: &str, key: &str) -> Result<()> {
    secrets::store_api_key(provider, key)
}

pub fn get_api_key(provider: &str) -> Result<String> {
    secrets::get_api_key(provider)
}

pub fn has_api_key(provider: &str) -> bool {
    secrets::has_api_key(provider)
}

pub fn delete_api_key(provider: &str) -> Result<()> {
    secrets::delete_api_key(provider)
}
