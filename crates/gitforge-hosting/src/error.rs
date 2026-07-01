use std::fmt;

use gitforge_remote::{HttpRemoteError, RemoteNet};

pub type HostingResult<T> = Result<T, HostingError>;

#[derive(Debug)]
pub enum HostingError {
    Http(HttpRemoteError),
    TokenNotFound(String),
    Config(anyhow::Error),
    MissingProjectId { path: String },
}

impl fmt::Display for HostingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.user_message())
    }
}

impl std::error::Error for HostingError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Http(e) => Some(e),
            Self::Config(e) => Some(e.as_ref()),
            _ => None,
        }
    }
}

impl From<HttpRemoteError> for HostingError {
    fn from(e: HttpRemoteError) -> Self {
        Self::Http(e)
    }
}

impl From<anyhow::Error> for HostingError {
    fn from(err: anyhow::Error) -> Self {
        Self::Config(err)
    }
}

impl HostingError {
    pub fn config(err: impl Into<anyhow::Error>) -> Self {
        Self::Config(err.into())
    }

    pub fn token_not_found(token_key: &str) -> Self {
        Self::TokenNotFound(format!(
            "Hosting token not found for \"{token_key}\". Re-add the account in Settings → Accounts."
        ))
    }

    /// Single-line, credential-redacted message for toasts and banners.
    pub fn user_message(&self) -> String {
        match self {
            Self::Http(e) => e.user_message(),
            Self::TokenNotFound(msg) => gitforge_remote::redact_for_display(msg),
            Self::Config(err) => gitforge_remote::redact_for_display(&err.to_string()),
            Self::MissingProjectId { path } => {
                format!("Missing numeric id for GitLab project {path}")
            }
        }
    }

    pub fn is_retryable(&self) -> bool {
        matches!(self, Self::Http(e) if e.is_retryable())
    }
}

pub trait HostingNet<T> {
    fn hosting_context(self, context: impl Into<String>) -> HostingResult<T>;
}

impl<T> HostingNet<T> for std::result::Result<T, reqwest::Error> {
    fn hosting_context(self, context: impl Into<String>) -> HostingResult<T> {
        self.remote_context(context).map_err(HostingError::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gitforge_remote::HttpRemoteError;

    #[test]
    fn display_delegates_to_user_message() {
        let err = HostingError::Http(HttpRemoteError::Auth {
            context: "auth failed".into(),
            body: "GET https://token@host/api failed".into(),
        });
        assert_eq!(err.to_string(), err.user_message());
        assert!(!err.to_string().contains("token@"));
    }

    #[test]
    fn is_retryable_delegates_to_http() {
        assert!(HostingError::Http(HttpRemoteError::Server {
            context: "x".into(),
            status: 503,
            body: String::new(),
        })
        .is_retryable());
        assert!(!HostingError::token_not_found("key").is_retryable());
    }
}
