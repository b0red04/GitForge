use reqwest::header::HeaderMap;
use reqwest::StatusCode;

pub type HostingResult<T> = Result<T, HostingError>;

#[derive(Debug, thiserror::Error)]
pub enum HostingError {
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

    #[error("{0}")]
    TokenNotFound(String),

    #[error("{0}")]
    Config(#[from] anyhow::Error),

    #[error("Missing numeric id for GitLab project {path}")]
    MissingProjectId { path: String },
}

impl HostingError {
    pub fn network(context: impl Into<String>, detail: reqwest::Error) -> Self {
        Self::Network {
            context: context.into(),
            detail,
        }
    }

    pub fn token_not_found(token_key: &str) -> Self {
        Self::TokenNotFound(format!(
            "Hosting token not found for \"{token_key}\". Re-add the account in Settings → Accounts."
        ))
    }

    pub fn config(err: impl Into<anyhow::Error>) -> Self {
        Self::Config(err.into())
    }

    /// Single-line, credential-redacted message for toasts and banners.
    pub fn user_message(&self) -> String {
        match self {
            Self::Auth { context, body } => redact_for_display(&format!("{context}: {body}")),
            Self::NotFound { context, body } => redact_for_display(&format!("{context}: {body}")),
            Self::RateLimited { context, body, .. } => {
                redact_for_display(&format!("{context}: {body}"))
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
            Self::TokenNotFound(msg) => redact_for_display(msg),
            Self::Config(err) => redact_for_display(&err.to_string()),
            Self::MissingProjectId { path } => format!("Missing numeric id for GitLab project {path}"),
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
) -> HostingError {
    let context = context.into();
    let code = status.as_u16();
    match code {
        401 | 403 => HostingError::Auth { context, body },
        404 => HostingError::NotFound { context, body },
        429 => HostingError::RateLimited {
            context,
            retry_after: parse_retry_after(headers),
            body,
        },
        400..=499 => HostingError::Api {
            context,
            status: code,
            body,
        },
        500..=599 => HostingError::Server {
            context,
            status: code,
            body,
        },
        _ => HostingError::Api {
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

pub trait HostingNet<T> {
    fn hosting_context(self, context: impl Into<String>) -> HostingResult<T>;
}

impl<T> HostingNet<T> for std::result::Result<T, reqwest::Error> {
    fn hosting_context(self, context: impl Into<String>) -> HostingResult<T> {
        self.map_err(|detail| HostingError::network(context, detail))
    }
}

fn first_line(s: &str) -> String {
    s.lines().next().unwrap_or(s).trim().to_string()
}

fn redact_credentials(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut remaining = s;

    while let Some(scheme_end) = remaining.find("://") {
        result.push_str(&remaining[..scheme_end + 3]);
        let after_scheme = &remaining[scheme_end + 3..];

        let host_end = after_scheme
            .find(['/', ' ', '\'', '"'])
            .unwrap_or(after_scheme.len());
        let authority = &after_scheme[..host_end];

        if let Some(at_pos) = authority.rfind('@') {
            result.push_str("***@");
            result.push_str(&authority[at_pos + 1..]);
        } else {
            result.push_str(authority);
        }
        remaining = &after_scheme[host_end..];
    }
    result.push_str(remaining);
    result
}

fn redact_for_display(s: &str) -> String {
    redact_credentials(&first_line(s))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_status_codes_to_variants() {
        let headers = HeaderMap::new();
        assert!(matches!(
            http_response_to_error("ctx", StatusCode::UNAUTHORIZED, &headers, String::new()),
            HostingError::Auth { .. }
        ));
        assert!(matches!(
            http_response_to_error("ctx", StatusCode::FORBIDDEN, &headers, String::new()),
            HostingError::Auth { .. }
        ));
        assert!(matches!(
            http_response_to_error("ctx", StatusCode::NOT_FOUND, &headers, String::new()),
            HostingError::NotFound { .. }
        ));
        assert!(matches!(
            http_response_to_error("ctx", StatusCode::UNPROCESSABLE_ENTITY, &headers, String::new()),
            HostingError::Api { status: 422, .. }
        ));
        assert!(matches!(
            http_response_to_error("ctx", StatusCode::INTERNAL_SERVER_ERROR, &headers, String::new()),
            HostingError::Server { status: 500, .. }
        ));
    }

    #[test]
    fn maps_rate_limit_with_retry_after() {
        let mut headers = HeaderMap::new();
        headers.insert(reqwest::header::RETRY_AFTER, "42".parse().unwrap());
        let err = http_response_to_error("ctx", StatusCode::TOO_MANY_REQUESTS, &headers, String::new());
        match err {
            HostingError::RateLimited { retry_after, .. } => assert_eq!(retry_after, Some(42)),
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
            HostingError::RateLimited { retry_after, .. } => {
                assert!(retry_after.unwrap() >= 44 && retry_after.unwrap() <= 46);
            }
            other => panic!("expected RateLimited, got {other:?}"),
        }
    }

    #[test]
    fn user_message_redacts_credentials() {
        let err = HostingError::Auth {
            context: "auth failed".into(),
            body: "GET https://token@host/api failed".into(),
        };
        let msg = err.user_message();
        assert!(!msg.contains("token@"));
        assert!(msg.contains("***@host"));
    }

    #[test]
    fn is_retryable_matrix() {
        assert!(HostingError::RateLimited {
            context: "x".into(),
            retry_after: None,
            body: String::new(),
        }
        .is_retryable());
        assert!(HostingError::Server {
            context: "x".into(),
            status: 503,
            body: String::new(),
        }
        .is_retryable());
        assert!(!HostingError::Auth {
            context: "x".into(),
            body: String::new(),
        }
        .is_retryable());
    }
}
