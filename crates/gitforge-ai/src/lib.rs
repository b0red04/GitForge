pub mod provider;
pub mod local;
pub mod cloud;
pub mod prompt;

pub use provider::AiProvider;
pub use local::OllamaProvider;
pub use cloud::{OpenAiProvider, AnthropicProvider};

use anyhow::Result;

pub fn store_api_key(provider: &str, key: &str) -> Result<()> {
    let entry = keyring::Entry::new("gitforge-ai", provider)?;
    entry.set_password(key)?;
    Ok(())
}

pub fn get_api_key(provider: &str) -> Result<String> {
    let entry = keyring::Entry::new("gitforge-ai", provider)?;
    Ok(entry.get_password()?)
}

pub fn delete_api_key(provider: &str) -> Result<()> {
    let entry = keyring::Entry::new("gitforge-ai", provider)?;
    entry.delete_credential()?;
    Ok(())
}

pub fn create_provider(name: &str, model: &str) -> Result<Box<dyn AiProvider>> {
    match name {
        "ollama" => Ok(Box::new(OllamaProvider::new("http://localhost:11434", model))),
        "openai" => {
            let api_key = get_api_key("openai")?;
            Ok(Box::new(OpenAiProvider::new(&api_key, model)))
        }
        "anthropic" => {
            let api_key = get_api_key("anthropic")?;
            Ok(Box::new(AnthropicProvider::new(&api_key, model)))
        }
        other => anyhow::bail!("Unknown AI provider: {}", other),
    }
}
