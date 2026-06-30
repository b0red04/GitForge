//! JSON response mappers driven by per-family key tables.

use crate::models::{PullRequest, RemoteRepo};

use super::config::{
    ForkDetection, PrHeadBranchKey, PrJsonKeys, PrivateDetection, RepoJsonKeys,
};

pub(crate) fn extract_envelope(json: &serde_json::Value, envelope: &str) -> Vec<serde_json::Value> {
    if envelope.is_empty() {
        json.as_array().cloned().unwrap_or_default()
    } else {
        json[envelope].as_array().cloned().unwrap_or_default()
    }
}

pub(crate) fn json_to_pull_request(pr: &serde_json::Value, keys: &PrJsonKeys) -> PullRequest {
    let head_branch = match keys.head_branch {
        PrHeadBranchKey::Nested(parent, child) => pr[parent][child]
            .as_str()
            .map(|s| s.to_string()),
        PrHeadBranchKey::Flat(key) => pr[key].as_str().map(|s| s.to_string()),
    };

    PullRequest {
        number: pr[keys.number].as_u64().unwrap_or(0),
        title: pr["title"].as_str().unwrap_or("").to_string(),
        html_url: pr[keys.html_url].as_str().unwrap_or("").to_string(),
        state: pr["state"]
            .as_str()
            .unwrap_or(keys.state_default)
            .to_string(),
        head_branch,
        draft: pr["draft"].as_bool().unwrap_or(false),
    }
}

pub(crate) fn json_to_remote_repo(repo: &serde_json::Value, keys: &RepoJsonKeys) -> RemoteRepo {
    use chrono::{DateTime, Utc};

    let updated_at = repo[keys.updated_at]
        .as_str()
        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.with_timezone(&Utc));

    let is_fork = match keys.fork {
        ForkDetection::Bool(key) => repo[key].as_bool().unwrap_or(false),
        ForkDetection::ObjectExists(key) => repo[key].is_object(),
    };

    let is_private = match keys.private {
        PrivateDetection::Bool(key) => repo[key].as_bool().unwrap_or(false),
        PrivateDetection::VisibilityPrivate(key) => repo[key].as_str() == Some("private"),
    };

    RemoteRepo {
        name: repo["name"].as_str().unwrap_or("").to_string(),
        full_name: repo[keys.full_name].as_str().unwrap_or("").to_string(),
        description: repo["description"].as_str().map(|s| s.to_string()),
        clone_url: repo[keys.clone_url].as_str().unwrap_or("").to_string(),
        ssh_url: repo[keys.ssh_url].as_str().map(|s| s.to_string()),
        html_url: repo[keys.html_url].as_str().unwrap_or("").to_string(),
        is_fork,
        is_private,
        default_branch: repo["default_branch"].as_str().map(|s| s.to_string()),
        stars: repo[keys.stars].as_u64().unwrap_or(0),
        updated_at,
    }
}
