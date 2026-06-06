use crate::providers::openai_compat::OpenAiCompatibleProvider;

pub const OPENAI_BASE_URL: &str = "https://api.openai.com/v1";

pub fn openai_provider(
    api_key: &str,
    model: &str,
    temperature: f32,
) -> OpenAiCompatibleProvider {
    OpenAiCompatibleProvider::new(OPENAI_BASE_URL, api_key, model, "openai", temperature)
}
