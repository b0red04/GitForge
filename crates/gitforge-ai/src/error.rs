use std::fmt;

use gitforge_remote::{HttpRemoteError, RemoteNet};

pub type AiResult<T> = Result<T, AiError>;

#[derive(Debug)]
pub enum AiError {
    Http(HttpRemoteError),
    EmptyResponse { provider: String },
    ApiKeyNotConfigured(String),
    Config(anyhow::Error),
    UnknownProvider(String),
    ModelListingUnsupported(String),
}

impl fmt::Display for AiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.user_message())
    }
}

impl std::error::Error for AiError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Http(e) => Some(e),
            Self::Config(e) => Some(e.as_ref()),
            _ => None,
        }
    }
}

impl From<HttpRemoteError> for AiError {
    fn from(e: HttpRemoteError) -> Self {
        Self::Http(e)
    }
}

impl From<anyhow::Error> for AiError {
    fn from(err: anyhow::Error) -> Self {
        Self::Config(err)
    }
}

impl AiError {
    pub fn config(err: impl Into<anyhow::Error>) -> Self {
        Self::Config(err.into())
    }

    pub fn api_key_not_configured(provider: &str) -> Self {
        Self::ApiKeyNotConfigured(format!(
            "API key not configured for provider \"{provider}\""
        ))
    }

    pub fn empty_response(provider: impl Into<String>) -> Self {
        Self::EmptyResponse {
            provider: provider.into(),
        }
    }

    pub fn unknown_provider(name: impl Into<String>) -> Self {
        Self::UnknownProvider(name.into())
    }

    pub fn model_listing_unsupported(provider: impl Into<String>) -> Self {
        Self::ModelListingUnsupported(provider.into())
    }

    /// Single-line, credential-redacted message for toasts and banners.
    pub fn user_message(&self) -> String {
        match self {
            Self::Http(e) => e.user_message(),
            Self::EmptyResponse { provider } => format!("{provider} returned no content"),
            Self::ApiKeyNotConfigured(msg) => gitforge_remote::redact_for_display(msg),
            Self::Config(err) => gitforge_remote::redact_for_display(&err.to_string()),
            Self::UnknownProvider(provider) => format!("Unknown AI provider: {provider}"),
            Self::ModelListingUnsupported(provider) => {
                format!("Provider does not support model listing: {provider}")
            }
        }
    }

    pub fn is_retryable(&self) -> bool {
        matches!(self, Self::Http(e) if e.is_retryable())
    }
}

pub trait AiNet<T> {
    fn ai_context(self, context: impl Into<String>) -> AiResult<T>;
}

impl<T> AiNet<T> for std::result::Result<T, reqwest::Error> {
    fn ai_context(self, context: impl Into<String>) -> AiResult<T> {
        self.remote_context(context).map_err(AiError::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gitforge_remote::HttpRemoteError;

    #[test]
    fn unknown_provider_user_message_includes_context() {
        let msg = AiError::unknown_provider("foo").user_message();
        assert_eq!(msg, "Unknown AI provider: foo");
    }

    #[test]
    fn model_listing_unsupported_user_message() {
        let msg = AiError::model_listing_unsupported("anthropic").user_message();
        assert_eq!(
            msg,
            "Provider does not support model listing: anthropic"
        );
    }

    #[test]
    fn display_delegates_to_user_message() {
        let err = AiError::Http(HttpRemoteError::Auth {
            context: "auth failed".into(),
            body: "GET https://sk-key@host/api failed".into(),
        });
        assert_eq!(err.to_string(), err.user_message());
        assert!(!err.to_string().contains("sk-key@"));
    }

    #[test]
    fn is_retryable_delegates_to_http() {
        assert!(AiError::Http(HttpRemoteError::RateLimited {
            context: "x".into(),
            retry_after: None,
            body: String::new(),
        })
        .is_retryable());
        assert!(!AiError::empty_response("openai").is_retryable());
    }
}
