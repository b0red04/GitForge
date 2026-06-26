use anyhow::{Context, Result};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;

const KEYRING_SERVICE: &str = "gitforge-hosting";

/// Process-global override for [`config_dir`]. Tests install this once per
/// process to redirect token storage at a temp dir. Unlike mutating the
/// process environment (`std::env::set_var`, which is `unsafe` on edition
/// 2024 and races with concurrent env readers spawned by the test harness),
/// a `OnceLock` read is lock-free and data-race-free. Setting is idempotent:
/// the first call wins, later calls are ignored.
static CONFIG_DIR_OVERRIDE: OnceLock<PathBuf> = OnceLock::new();

/// Install a process-global config-dir override. Intended for tests; safe to
/// call in production (e.g. to relocate token storage), but the first call
/// wins and subsequent calls are silently ignored. Idempotent.
pub fn set_config_dir_override(path: PathBuf) {
    let _ = CONFIG_DIR_OVERRIDE.set(path);
}

fn config_dir() -> PathBuf {
    // In-process override takes precedence (used by tests).
    if let Some(dir) = CONFIG_DIR_OVERRIDE.get() {
        return dir.clone();
    }
    // Fallback for external callers that still steer via the env var.
    if let Ok(dir) = std::env::var("GITFORGE_CONFIG_DIR") {
        return PathBuf::from(dir);
    }
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("gitforge")
}

fn tokens_file() -> PathBuf {
    config_dir().join("hosting_tokens.json")
}

fn load_file_tokens() -> HashMap<String, String> {
    let path = tokens_file();
    let Ok(content) = fs::read_to_string(&path) else {
        return HashMap::new();
    };
    serde_json::from_str(&content).unwrap_or_default()
}

fn save_file_tokens(tokens: &HashMap<String, String>) -> Result<()> {
    let dir = config_dir();
    fs::create_dir_all(&dir).context("failed to create config directory")?;
    let path = tokens_file();
    fs::write(&path, serde_json::to_string(tokens)?).context("failed to write hosting token file")?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600))
            .context("failed to set hosting token file permissions")?;
    }
    Ok(())
}

fn get_from_file(token_key: &str) -> Option<String> {
    load_file_tokens().get(token_key).cloned()
}

fn store_in_keyring(token_key: &str, token: &str) -> Result<()> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, token_key)?;
    entry.set_password(token)?;
    entry.get_password()?;
    Ok(())
}

fn get_from_keyring(token_key: &str) -> Result<String> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, token_key)?;
    Ok(entry.get_password()?)
}

fn delete_from_keyring(token_key: &str) -> Result<()> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, token_key)?;
    match entry.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(e.into()),
    }
}

pub fn store_token(token_key: &str, token: &str) -> Result<()> {
    let mut tokens = load_file_tokens();
    tokens.insert(token_key.to_string(), token.to_string());
    save_file_tokens(&tokens)?;

    if let Err(e) = store_in_keyring(token_key, token) {
        tracing::debug!("Optional keyring mirror failed for {token_key}: {e}");
    }

    get_token(token_key).map(|_| ())
}

pub fn get_token(token_key: &str) -> Result<String> {
    if let Some(token) = get_from_file(token_key) {
        return Ok(token);
    }

    match get_from_keyring(token_key) {
        Ok(token) => {
            let mut tokens = load_file_tokens();
            tokens.insert(token_key.to_string(), token.clone());
            if let Err(e) = save_file_tokens(&tokens) {
                tracing::debug!("Failed to backfill hosting token file for {token_key}: {e}");
            }
            Ok(token)
        }
        Err(_) => anyhow::bail!(
            "Hosting token not found for \"{token_key}\". Re-add the account in Settings → Accounts."
        ),
    }
}

pub fn delete_token(token_key: &str) -> Result<()> {
    let mut tokens = load_file_tokens();
    tokens.remove(token_key);
    if tokens.is_empty() {
        let path = tokens_file();
        if path.exists() {
            fs::remove_file(path)?;
        }
    } else {
        save_file_tokens(&tokens)?;
    }

    if let Err(e) = delete_from_keyring(token_key) {
        tracing::warn!("Failed to delete {token_key} from keyring: {e}");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_store_roundtrip() {
        let token_key = "test-provider:file-fallback";
        let token = "ghp_test_abc123";

        store_token(token_key, token).expect("store");
        let read = get_token(token_key).expect("get");
        assert_eq!(read, token);

        delete_token(token_key).expect("delete");
        assert!(get_token(token_key).is_err());
    }
}
