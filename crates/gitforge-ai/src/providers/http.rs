use crate::error::{http_response_to_error, AiResult};

/// Returns `Ok(response)` on 2xx, otherwise reads the body and returns a
/// structured [`AiError`] mapped from the HTTP status.
pub async fn ensure_success(
    response: reqwest::Response,
    context: &str,
) -> AiResult<reqwest::Response> {
    if response.status().is_success() {
        Ok(response)
    } else {
        let status = response.status();
        let headers = response.headers().clone();
        let body = response.text().await.unwrap_or_default();
        Err(http_response_to_error(context, status, &headers, body))
    }
}
