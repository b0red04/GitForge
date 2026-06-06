use crate::config::ZaiEndpoint;
use crate::providers::openai_compat::OpenAiCompatibleProvider;

pub fn zai_provider(
    api_key: &str,
    model: &str,
    endpoint: ZaiEndpoint,
    temperature: f32,
) -> OpenAiCompatibleProvider {
    OpenAiCompatibleProvider::new(
        endpoint.base_url(),
        api_key,
        model,
        "zai",
        temperature,
    )
}

pub fn zai_models_base_url(endpoint: ZaiEndpoint) -> &'static str {
    endpoint.base_url()
}
