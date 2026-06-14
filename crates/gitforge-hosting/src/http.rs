//! Shared HTTP primitives for hosting-provider implementations.
//!
//! Every provider repeats the same three shapes: build an authenticated client,
//! check a response status, and paginate a JSON array.  This module lifts those
//! shapes into free functions so each provider struct only supplies the parts
//! that genuinely differ (auth-header scheme, URL templates, JSON key names).

use anyhow::Result;

/// Build a `reqwest::Client` with the `gitforge` user-agent and the given
/// default headers (per-provider auth + content-negotiation headers).
pub fn make_client(headers: reqwest::header::HeaderMap) -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent("gitforge")
        .default_headers(headers)
        .build()
        .expect("reqwest client build only fails for invalid builder config; user-agent 'gitforge' and pre-parsed headers are always valid")
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

/// Returns `Ok(response)` on 2xx, otherwise reads the body and bails with a
/// `"\<context\>: \<status\> - \<body\>"` message.
pub async fn ensure_success(
    response: reqwest::Response,
    context: &str,
) -> Result<reqwest::Response> {
    if response.status().is_success() {
        Ok(response)
    } else {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        anyhow::bail!("{context}: {status} - {body}")
    }
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
) -> Result<Vec<T>> {
    let mut all = Vec::new();
    let mut page = 1;
    loop {
        let response = client.get(url_for_page(page)).send().await?;
        let response = ensure_success(response, context).await?;
        let json: serde_json::Value = response.json().await?;
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
