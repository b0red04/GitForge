use anyhow::{Context, Result};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

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

fn save_file_secrets(secrets: &HashMap<String, String>) -> Result<()> {
    let dir = config_dir();
    fs::create_dir_all(&dir).context("failed to create config directory")?;
    let path = secrets_file();
    fs::write(&path, serde_json::to_string(secrets)?).context("failed to write API key file")?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600))
            .context("failed to set API key file permissions")?;
    }
    Ok(())
}

fn get_from_file(provider: &str) -> Option<String> {
    load_file_secrets().get(provider).cloned()
}

fn store_in_keyring(provider: &str, key: &str) -> Result<()> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, provider)?;
    entry.set_password(key)?;
    entry.get_password()?;
    Ok(())
}

fn get_from_keyring(provider: &str) -> Result<String> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, provider)?;
    Ok(entry.get_password()?)
}

fn delete_from_keyring(provider: &str) -> Result<()> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, provider)?;
    match entry.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(e.into()),
    }
}

pub fn store_api_key(provider: &str, key: &str) -> Result<()> {
    let mut secrets = load_file_secrets();
    secrets.insert(provider.to_string(), key.to_string());
    save_file_secrets(&secrets)?;

    if let Err(e) = store_in_keyring(provider, key) {
        tracing::debug!("Optional keyring mirror failed for provider {provider}: {e}");
    }

    get_api_key(provider).map(|_| ())
}

pub fn get_api_key(provider: &str) -> Result<String> {
    if let Some(key) = get_from_file(provider) {
        return Ok(key);
    }

    get_from_keyring(provider)
        .map_err(|_| anyhow::anyhow!("API key not configured for provider \"{provider}\""))
}

pub fn has_api_key(provider: &str) -> bool {
    get_api_key(provider).is_ok()
}

pub fn delete_api_key(provider: &str) -> Result<()> {
    let _ = delete_from_keyring(provider);
    let mut secrets = load_file_secrets();
    secrets.remove(provider);
    if secrets.is_empty() {
        let path = secrets_file();
        if path.exists() {
            fs::remove_file(path)?;
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
