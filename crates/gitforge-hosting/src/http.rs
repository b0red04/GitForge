//! Shared HTTP primitives for hosting-provider implementations.
//!
//! Every provider repeats the same three shapes: build an authenticated client,
//! check a response status, and paginate a JSON array.  This module lifts those
//! shapes into free functions so each provider struct only supplies the parts
//! that genuinely differ (auth-header scheme, URL templates, JSON key names).

use crate::error::{HostingError, HostingNet, HostingResult};
use gitforge_remote::ensure_success as remote_ensure_success;
use serde::de::DeserializeOwned;
use serde::Serialize;

/// Build a `reqwest::Client` with the `gitforge` user-agent and the given
/// default headers and cookies.
pub fn make_client(headers: reqwest::header::HeaderMap) -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent("gitforge")
        .default_headers(headers)
        .build()
        .expect("reqwest client build only fails for invalid builder config; user-agent 'gitforge' and pre-parsed headers are always valid")
}

/// Percent-encode a namespace path for use as a single URL path segment
/// (e.g. GitLab `owner/repo` → `owner%2Frepo`).
pub fn url_encode_path(s: &str) -> String {
    s.replace('%', "%25")
        .replace('/', "%2F")
        .replace(' ', "%20")
}

/// Percent-encode a free-form string for use as a single URL query-parameter
/// value. Encodes the characters that would otherwise alter query structure
/// (`&`, `=`, `#`, `?`), the `+`/space ambiguity, and `%` itself (encoded
/// first to avoid double-escaping).
pub fn url_encode_query(s: &str) -> String {
    s.replace('%', "%25")
        .replace('&', "%26")
        .replace('=', "%3D")
        .replace('#', "%23")
        .replace('?', "%3F")
        .replace('+', "%2B")
        .replace(' ', "%20")
}

/// Returns `Ok(response)` on 2xx, otherwise reads the body and returns a
/// structured [`HostingError`] mapped from the HTTP status.
pub async fn ensure_success(
    response: reqwest::Response,
    context: &str,
) -> HostingResult<reqwest::Response> {
    remote_ensure_success(response, context)
        .await
        .map_err(HostingError::from)
}

/// GET `url`, check status, and deserialize the JSON body.
pub async fn get_json<T: DeserializeOwned>(
    client: &reqwest::Client,
    url: impl AsRef<str>,
    context: &str,
) -> HostingResult<T> {
    let response = client
        .get(url.as_ref())
        .send()
        .await
        .hosting_context(context)?;
    let response = ensure_success(response, context).await?;
    response.json().await.hosting_context(context)
}

/// POST `url` with optional JSON body, check status, and deserialize the response.
pub async fn post_json<T: DeserializeOwned, B: Serialize>(
    client: &reqwest::Client,
    url: impl AsRef<str>,
    body: Option<&B>,
    context: &str,
) -> HostingResult<T> {
    let mut request = client.post(url.as_ref());
    if let Some(body) = body {
        request = request.json(body);
    }
    let response = request.send().await.hosting_context(context)?;
    let response = ensure_success(response, context).await?;
    response.json().await.hosting_context(context)
}

/// Paginate a JSON array endpoint.
///
/// Loops pages starting at 1, building the URL via `url_for_page`,
/// checking the response via [`ensure_success`], extracting the item array
/// via `extract_items`, and mapping each item through `map_item` (which may
/// return `None` to skip).  Stops when a page is empty or returns fewer than
/// `page_size` items.
pub async fn paginate<T>(
    client: &reqwest::Client,
    url_for_page: impl Fn(usize) -> String,
    page_size: usize,
    context: &str,
    extract_items: impl Fn(&serde_json::Value) -> Vec<serde_json::Value>,
    map_item: impl Fn(&serde_json::Value) -> Option<T>,
) -> HostingResult<Vec<T>> {
    let mut all = Vec::new();
    let mut page = 1;
    loop {
        let json: serde_json::Value = get_json(client, url_for_page(page), context).await?;
        let items = extract_items(&json);
        let raw_count = items.len();
        if items.is_empty() {
            break;
        }
        for item in &items {
            if let Some(mapped) = map_item(item) {
                all.push(mapped);
            }
        }
        page += 1;
        if raw_count < page_size {
            break;
        }
    }
    Ok(all)
}
