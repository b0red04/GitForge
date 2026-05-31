use crate::error::{GitError, GitResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

const SERVICE_NAME: &str = "gitforge";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredCredential {
    pub host: String,
    pub username: String,
    pub protocol: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CredentialEntry {
    pub host: String,
    pub username: String,
    pub password: String,
    pub protocol: Option<String>,
}

pub fn store_credential(
    host: &str,
    username: &str,
    password: &str,
    protocol: Option<&str>,
) -> GitResult<()> {
    let entry = keyring::Entry::new(SERVICE_NAME, &format!("{}@{}", username, host))
        .map_err(|e| GitError::OperationFailed(format!("Failed to create keyring entry: {}", e)))?;

    entry
        .set_password(password)
        .map_err(|e| GitError::OperationFailed(format!("Failed to store credential: {}", e)))?;

    let index = load_credential_index();
    let key = format!("{}@{}", username, host);
    let mut updated = index;
    updated.insert(
        key,
        StoredCredential {
            host: host.to_string(),
            username: username.to_string(),
            protocol: protocol.map(|p| p.to_string()),
            description: None,
        },
    );
    save_credential_index(&updated);

    Ok(())
}

pub fn get_credential(host: &str, username: &str) -> GitResult<Option<CredentialEntry>> {
    let entry = keyring::Entry::new(SERVICE_NAME, &format!("{}@{}", username, host))
        .map_err(|e| GitError::OperationFailed(format!("Failed to create keyring entry: {}", e)))?;

    match entry.get_password() {
        Ok(password) => Ok(Some(CredentialEntry {
            host: host.to_string(),
            username: username.to_string(),
            password,
            protocol: None,
        })),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(GitError::OperationFailed(format!(
            "Failed to retrieve credential: {}",
            e
        ))),
    }
}

pub fn delete_credential(host: &str, username: &str) -> GitResult<()> {
    let entry = keyring::Entry::new(SERVICE_NAME, &format!("{}@{}", username, host))
        .map_err(|e| GitError::OperationFailed(format!("Failed to create keyring entry: {}", e)))?;

    entry
        .delete_credential()
        .map_err(|e| GitError::OperationFailed(format!("Failed to delete credential: {}", e)))?;

    let mut index = load_credential_index();
    let key = format!("{}@{}", username, host);
    index.remove(&key);
    save_credential_index(&index);

    Ok(())
}

pub fn list_stored_credentials() -> Vec<StoredCredential> {
    let index = load_credential_index();
    index.into_values().collect()
}

pub fn git_credential_fill(
    protocol: &str,
    host: &str,
    path: Option<&str>,
) -> GitResult<Option<CredentialEntry>> {
    let mut input = format!("protocol={}\nhost={}", protocol, host);
    if let Some(p) = path {
        input.push_str(&format!("\npath={}", p));
    }
    input.push('\n');

    let mut child = std::process::Command::new("git")
        .arg("credential")
        .arg("fill")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| {
            GitError::OperationFailed(format!("Failed to run git credential fill: {}", e))
        })?;

    if let Some(mut stdin) = child.stdin.take() {
        use std::io::Write;
        let _ = stdin.write_all(input.as_bytes());
    }

    let output = child.wait_with_output().map_err(|e| {
        GitError::OperationFailed(format!("Failed to wait for git credential fill: {}", e))
    })?;

    if !output.status.success() {
        return Ok(None);
    }

    let response = String::from_utf8_lossy(&output.stdout);
    let mut username = None;
    let mut password = None;

    for line in response.lines() {
        if let Some((key, value)) = line.split_once('=') {
            match key {
                "username" => username = Some(value.to_string()),
                "password" => password = Some(value.to_string()),
                _ => {}
            }
        }
    }

    match (username, password) {
        (Some(u), Some(p)) => Ok(Some(CredentialEntry {
            host: host.to_string(),
            username: u,
            password: p,
            protocol: Some(protocol.to_string()),
        })),
        _ => Ok(None),
    }
}

pub fn git_credential_approve(
    protocol: &str,
    host: &str,
    username: &str,
    password: &str,
) -> GitResult<()> {
    let input = format!(
        "protocol={}\nhost={}\nusername={}\npassword={}\n",
        protocol, host, username, password
    );

    let mut child = std::process::Command::new("git")
        .arg("credential")
        .arg("approve")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| {
            GitError::OperationFailed(format!("Failed to run git credential approve: {}", e))
        })?;

    if let Some(mut stdin) = child.stdin.take() {
        use std::io::Write;
        let _ = stdin.write_all(input.as_bytes());
    }

    let output = child.wait_with_output().map_err(|e| {
        GitError::OperationFailed(format!("Failed to wait for git credential approve: {}", e))
    })?;

    if !output.status.success() {
        return Err(GitError::OperationFailed(
            String::from_utf8_lossy(&output.stderr).to_string(),
        ));
    }

    Ok(())
}

pub fn git_credential_reject(protocol: &str, host: &str, username: &str) -> GitResult<()> {
    let input = format!(
        "protocol={}\nhost={}\nusername={}\n",
        protocol, host, username
    );

    let mut child = std::process::Command::new("git")
        .arg("credential")
        .arg("reject")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| {
            GitError::OperationFailed(format!("Failed to run git credential reject: {}", e))
        })?;

    if let Some(mut stdin) = child.stdin.take() {
        use std::io::Write;
        let _ = stdin.write_all(input.as_bytes());
    }

    let output = child.wait_with_output().map_err(|e| {
        GitError::OperationFailed(format!("Failed to wait for git credential reject: {}", e))
    })?;

    if !output.status.success() {
        return Err(GitError::OperationFailed(
            String::from_utf8_lossy(&output.stderr).to_string(),
        ));
    }

    Ok(())
}

fn credential_index_path() -> Option<std::path::PathBuf> {
    dirs::config_dir().map(|p| p.join("gitforge").join("credentials.json"))
}

fn load_credential_index() -> HashMap<String, StoredCredential> {
    let Some(path) = credential_index_path() else {
        return HashMap::new();
    };

    if !path.exists() {
        return HashMap::new();
    }

    let data = std::fs::read_to_string(&path).unwrap_or_default();
    serde_json::from_str(&data).unwrap_or_default()
}

fn save_credential_index(index: &HashMap<String, StoredCredential>) {
    let Some(path) = credential_index_path() else {
        return;
    };

    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    if let Ok(data) = serde_json::to_string_pretty(index) {
        let _ = std::fs::write(&path, data);
    }
}
