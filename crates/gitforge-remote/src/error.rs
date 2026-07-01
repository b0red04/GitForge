use reqwest::header::HeaderMap;
use reqwest::StatusCode;

use crate::redact::redact_for_display;

pub type HttpResult<T> = Result<T, HttpRemoteError>;

#[derive(Debug, thiserror::Error)]
pub enum HttpRemoteError {
    #[error("{context}")]
    Auth {
        context: String,
        body: String,
    },

    #[error("{context}")]
    NotFound {
        context: String,
        body: String,
    },

    #[error("{context}")]
    RateLimited {
        context: String,
        retry_after: Option<u64>,
        body: String,
    },

    #[error("{context}")]
    Api {
        context: String,
        status: u16,
        body: String,
    },

    #[error("{context}")]
    Server {
        context: String,
        status: u16,
        body: String,
    },

    #[error("Network error during {context}: {detail}")]
    Network {
        context: String,
        #[source]
        detail: reqwest::Error,
    },
}

impl HttpRemoteError {
    pub fn network(context: impl Into<String>, detail: reqwest::Error) -> Self {
        Self::Network {
            context: context.into(),
            detail,
        }
    }

    /// Single-line, credential-redacted message for toasts and banners.
    pub fn user_message(&self) -> String {
        match self {
            Self::Auth { context, body } => redact_for_display(&format!("{context}: {body}")),
            Self::NotFound { context, body } => redact_for_display(&format!("{context}: {body}")),
            Self::RateLimited {
                context,
                body,
                retry_after,
            } => {
                let mut msg = redact_for_display(&format!("{context}: {body}"));
                if let Some(secs) = retry_after {
                    msg = format!("{msg} (retry in {secs}s)");
                }
                msg
            }
            Self::Api {
                context,
                status,
                body,
            } => redact_for_display(&format!("{context}: {status} - {body}")),
            Self::Server {
                context,
                status,
                body,
            } => redact_for_display(&format!("{context}: {status} - {body}")),
            Self::Network { context, detail } => {
                redact_for_display(&format!("Network error during {context}: {detail}"))
            }
        }
    }

    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            Self::RateLimited { .. } | Self::Server { .. } | Self::Network { .. }
        )
    }
}

pub fn http_response_to_error(
    context: impl Into<String>,
    status: StatusCode,
    headers: &HeaderMap,
    body: String,
) -> HttpRemoteError {
    let context = context.into();
    let code = status.as_u16();
    match code {
        401 | 403 => HttpRemoteError::Auth { context, body },
        404 => HttpRemoteError::NotFound { context, body },
        429 => HttpRemoteError::RateLimited {
            context,
            retry_after: parse_retry_after(headers),
            body,
        },
        400..=499 => HttpRemoteError::Api {
            context,
            status: code,
            body,
        },
        500..=599 => HttpRemoteError::Server {
            context,
            status: code,
            body,
        },
        _ => HttpRemoteError::Api {
            context,
            status: code,
            body,
        },
    }
}

fn parse_retry_after(headers: &HeaderMap) -> Option<u64> {
    let raw = headers
        .get(reqwest::header::RETRY_AFTER)?
        .to_str()
        .ok()?;
    if let Ok(seconds) = raw.parse::<u64>() {
        return Some(seconds);
    }
    retry_after_from_http_date(raw)
}

fn retry_after_from_http_date(raw: &str) -> Option<u64> {
    use chrono::Utc;
    let retry_at = chrono::DateTime::parse_from_rfc2822(raw.trim())
        .or_else(|_| chrono::DateTime::parse_from_rfc3339(raw.trim()))
        .ok()?
        .with_timezone(&Utc);
    let seconds = retry_at.signed_duration_since(Utc::now()).num_seconds();
    Some(seconds.max(0) as u64)
}

pub trait RemoteNet<T> {
    fn remote_context(self, context: impl Into<String>) -> HttpResult<T>;
}

impl<T> RemoteNet<T> for std::result::Result<T, reqwest::Error> {
    fn remote_context(self, context: impl Into<String>) -> HttpResult<T> {
        self.map_err(|detail| HttpRemoteError::network(context, detail))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_status_codes_to_variants() {
        let headers = HeaderMap::new();
        assert!(matches!(
            http_response_to_error("ctx", StatusCode::UNAUTHORIZED, &headers, String::new()),
            HttpRemoteError::Auth { .. }
        ));
        assert!(matches!(
            http_response_to_error("ctx", StatusCode::FORBIDDEN, &headers, String::new()),
            HttpRemoteError::Auth { .. }
        ));
        assert!(matches!(
            http_response_to_error("ctx", StatusCode::NOT_FOUND, &headers, String::new()),
            HttpRemoteError::NotFound { .. }
        ));
        assert!(matches!(
            http_response_to_error("ctx", StatusCode::UNPROCESSABLE_ENTITY, &headers, String::new()),
            HttpRemoteError::Api { status: 422, .. }
        ));
        assert!(matches!(
            http_response_to_error("ctx", StatusCode::INTERNAL_SERVER_ERROR, &headers, String::new()),
            HttpRemoteError::Server { status: 500, .. }
        ));
    }

    #[test]
    fn maps_rate_limit_with_retry_after() {
        let mut headers = HeaderMap::new();
        headers.insert(reqwest::header::RETRY_AFTER, "42".parse().unwrap());
        let err = http_response_to_error("ctx", StatusCode::TOO_MANY_REQUESTS, &headers, String::new());
        match err {
            HttpRemoteError::RateLimited { retry_after, .. } => assert_eq!(retry_after, Some(42)),
            other => panic!("expected RateLimited, got {other:?}"),
        }
    }

    #[test]
    fn maps_rate_limit_with_http_date_retry_after() {
        let mut headers = HeaderMap::new();
        let retry_at = chrono::Utc::now() + chrono::Duration::seconds(45);
        let http_date = retry_at.format("%a, %d %b %Y %H:%M:%S GMT").to_string();
        headers.insert(
            reqwest::header::RETRY_AFTER,
            http_date.parse().expect("valid header"),
        );
        let err = http_response_to_error("ctx", StatusCode::TOO_MANY_REQUESTS, &headers, String::new());
        match err {
            HttpRemoteError::RateLimited { retry_after, .. } => {
                assert!(retry_after.unwrap() >= 44 && retry_after.unwrap() <= 46);
            }
            other => panic!("expected RateLimited, got {other:?}"),
        }
    }

    #[test]
    fn user_message_includes_retry_after() {
        let msg = HttpRemoteError::RateLimited {
            context: "rate limited".into(),
            retry_after: Some(30),
            body: "slow down".into(),
        }
        .user_message();
        assert!(msg.contains("(retry in 30s)"));
    }

    #[test]
    fn user_message_redacts_credentials() {
        let err = HttpRemoteError::Auth {
            context: "auth failed".into(),
            body: "GET https://token@host/api failed".into(),
        };
        let msg = err.user_message();
        assert!(!msg.contains("token@"));
        assert!(msg.contains("***@host"));
    }

    #[test]
    fn is_retryable_matrix() {
        assert!(HttpRemoteError::RateLimited {
            context: "x".into(),
            retry_after: None,
            body: String::new(),
        }
        .is_retryable());
        assert!(HttpRemoteError::Server {
            context: "x".into(),
            status: 503,
            body: String::new(),
        }
        .is_retryable());
        assert!(!HttpRemoteError::Auth {
            context: "x".into(),
            body: String::new(),
        }
        .is_retryable());
    }
}
