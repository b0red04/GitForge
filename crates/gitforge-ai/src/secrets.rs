use anyhow::Context;
use gitforge_remote::write_restricted_json;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use crate::error::{AiError, AiResult};

const KEYRING_SERVICE: &str = "gitforge-ai";

fn config_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("gitforge")
}

fn secrets_file() -> PathBuf {
    config_dir().join("ai-credentials.json")
}

fn load_file_secrets() -> HashMap<String, String> {
    let path = secrets_file();
    let Ok(content) = fs::read_to_string(&path) else {
        return HashMap::new();
    };
    serde_json::from_str(&content).unwrap_or_default()
}

fn save_file_secrets(secrets: &HashMap<String, String>) -> AiResult<()> {
    let dir = config_dir();
    fs::create_dir_all(&dir)
        .context("failed to create config directory")
        .map_err(AiError::config)?;
    let path = secrets_file();
    let json = serde_json::to_string(secrets).map_err(AiError::config)?;
    write_restricted_json(&path, &json)
        .context("failed to write API key file")
        .map_err(AiError::config)?;
    Ok(())
}

fn get_from_file(provider: &str) -> Option<String> {
    load_file_secrets().get(provider).cloned()
}

fn store_in_keyring(provider: &str, key: &str) -> AiResult<()> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, provider).map_err(AiError::config)?;
    entry.set_password(key).map_err(AiError::config)?;
    entry.get_password().map_err(AiError::config)?;
    Ok(())
}

fn get_from_keyring(provider: &str) -> AiResult<Option<String>> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, provider).map_err(AiError::config)?;
    match entry.get_password() {
        Ok(password) => Ok(Some(password)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(AiError::config(e)),
    }
}

fn delete_from_keyring(provider: &str) -> AiResult<()> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, provider).map_err(AiError::config)?;
    match entry.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(AiError::config(e)),
    }
}

pub fn store_api_key(provider: &str, key: &str) -> AiResult<()> {
    let mut secrets = load_file_secrets();
    secrets.insert(provider.to_string(), key.to_string());
    save_file_secrets(&secrets)?;

    if let Err(e) = store_in_keyring(provider, key) {
        tracing::debug!("Optional keyring mirror failed for provider {provider}: {e}");
    }

    get_api_key(provider).map(|_| ())
}

pub fn get_api_key(provider: &str) -> AiResult<String> {
    if let Some(key) = get_from_file(provider) {
        return Ok(key);
    }

    match get_from_keyring(provider)? {
        Some(key) => Ok(key),
        None => Err(AiError::api_key_not_configured(provider)),
    }
}

pub fn has_api_key(provider: &str) -> bool {
    get_api_key(provider).is_ok()
}

pub fn delete_api_key(provider: &str) -> AiResult<()> {
    delete_from_keyring(provider)?;
    let mut secrets = load_file_secrets();
    secrets.remove(provider);
    if secrets.is_empty() {
        let path = secrets_file();
        if path.exists() {
            fs::remove_file(path).map_err(AiError::config)?;
        }
    } else {
        save_file_secrets(&secrets)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_store_roundtrip() {
        let provider = "test-provider-file-fallback";
        let key = "sk-test-abc123";

        store_api_key(provider, key).expect("store");
        let read = get_api_key(provider).expect("get");
        assert_eq!(read, key);

        delete_api_key(provider).expect("delete");
        assert!(!has_api_key(provider));
    }
}
