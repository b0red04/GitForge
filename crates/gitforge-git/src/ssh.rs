use crate::error::{GitError, GitResult};
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct SshKey {
    pub path: PathBuf,
    pub name: String,
    pub key_type: SshKeyType,
    pub has_public_key: bool,
    pub is_agent_loaded: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SshKeyType {
    Rsa,
    Ed25519,
    Ecdsa,
    EcdsaSk,
    Ed25519Sk,
    Dsa,
    Unknown(String),
}

impl std::fmt::Display for SshKeyType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SshKeyType::Rsa => write!(f, "RSA"),
            SshKeyType::Ed25519 => write!(f, "Ed25519"),
            SshKeyType::Ecdsa => write!(f, "ECDSA"),
            SshKeyType::EcdsaSk => write!(f, "ECDSA-SK"),
            SshKeyType::Ed25519Sk => write!(f, "Ed25519-SK"),
            SshKeyType::Dsa => write!(f, "DSA"),
            SshKeyType::Unknown(s) => write!(f, "{}", s),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SshAgentStatus {
    pub available: bool,
    pub pid: Option<u32>,
    pub loaded_keys: Vec<String>,
}

fn ssh_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".ssh"))
}

fn detect_key_type(name: &str) -> SshKeyType {
    if name.contains("ed25519") {
        SshKeyType::Ed25519
    } else if name.contains("ecdsa") && name.contains("sk") {
        SshKeyType::EcdsaSk
    } else if name.contains("ecdsa") {
        SshKeyType::Ecdsa
    } else if name.contains("ed25519") && name.contains("sk") {
        SshKeyType::Ed25519Sk
    } else if name.contains("rsa") {
        SshKeyType::Rsa
    } else if name.contains("dsa") {
        SshKeyType::Dsa
    } else {
        SshKeyType::Unknown(name.to_string())
    }
}

pub fn list_ssh_keys() -> GitResult<Vec<SshKey>> {
    let ssh_dir = ssh_dir().ok_or_else(|| {
        GitError::OperationFailed("Cannot determine home directory".into())
    })?;

    if !ssh_dir.exists() {
        return Ok(Vec::new());
    }

    let agent_keys = list_agent_keys();
    let mut keys = Vec::new();

    let entries = std::fs::read_dir(&ssh_dir)
        .map_err(|e| GitError::OperationFailed(format!("Failed to read ~/.ssh: {}", e)))?;

    for entry in entries.flatten() {
        let path = entry.path();
        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        if name.starts_with('.')
            || name.ends_with(".pub")
            || name.ends_with(".pem")
            || name.ends_with(".known_hosts")
            || name.ends_with(".authorized_keys")
            || name.ends_with(".config")
            || name.ends_with("-cert.pub")
            || name == "config"
            || name == "known_hosts"
            || name == "authorized_keys"
            || name == "environment"
            || name == "rc"
        {
            continue;
        }

        let pub_path = path.with_extension("pub");
        let has_public_key = pub_path.exists();

        let is_loaded = agent_keys
            .iter()
            .any(|k| k.contains(&name));

        keys.push(SshKey {
            path: path.clone(),
            name,
            key_type: detect_key_type(path.to_string_lossy().as_ref()),
            has_public_key,
            is_agent_loaded: is_loaded,
        });
    }

    keys.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(keys)
}

pub fn get_public_key(key_name: &str) -> GitResult<String> {
    let ssh_dir = ssh_dir().ok_or_else(|| {
        GitError::OperationFailed("Cannot determine home directory".into())
    })?;

    let pub_path = ssh_dir.join(format!("{}.pub", key_name));
    if !pub_path.exists() {
        return Err(GitError::OperationFailed(format!(
            "Public key not found: {}",
            pub_path.display()
        )));
    }

    std::fs::read_to_string(&pub_path).map_err(|e| {
        GitError::OperationFailed(format!("Failed to read public key: {}", e))
    })
}

pub fn generate_ssh_key(
    key_type: &str,
    email: &str,
    passphrase: Option<&str>,
    output_name: Option<&str>,
) -> GitResult<PathBuf> {
    let ssh_dir = ssh_dir().ok_or_else(|| {
        GitError::OperationFailed("Cannot determine home directory".into())
    })?;

    if !ssh_dir.exists() {
        std::fs::create_dir_all(&ssh_dir).map_err(|e| {
            GitError::OperationFailed(format!("Failed to create ~/.ssh: {}", e))
        })?;
    }

    let filename = output_name.unwrap_or(match key_type {
        "ed25519" => "id_ed25519",
        "ecdsa" => "id_ecdsa",
        "rsa" => "id_rsa",
        _ => "id_ed25519",
    });

    let key_path = ssh_dir.join(filename);
    if key_path.exists() {
        return Err(GitError::OperationFailed(format!(
            "Key already exists: {}",
            key_path.display()
        )));
    }

    let mut args = vec!["-t", key_type, "-f"];
    let path_str = key_path.to_string_lossy().to_string();
    args.push(&path_str);
    args.extend(["-C", email]);

    if let Some(_pass) = passphrase {
        args.extend(["-N", _pass]);
    } else {
        args.extend(["-N", ""]);
    }

    let output = std::process::Command::new("ssh-keygen")
        .args(&args)
        .output()
        .map_err(|e| GitError::OperationFailed(format!("Failed to run ssh-keygen: {}", e)))?;

    if !output.status.success() {
        return Err(GitError::OperationFailed(
            String::from_utf8_lossy(&output.stderr).to_string(),
        ));
    }

    Ok(key_path)
}

pub fn delete_ssh_key(key_name: &str) -> GitResult<()> {
    let ssh_dir = ssh_dir().ok_or_else(|| {
        GitError::OperationFailed("Cannot determine home directory".into())
    })?;

    let key_path = ssh_dir.join(key_name);
    let pub_path = ssh_dir.join(format!("{}.pub", key_name));

    if key_path.exists() {
        std::fs::remove_file(&key_path).map_err(|e| {
            GitError::OperationFailed(format!("Failed to delete private key: {}", e))
        })?;
    }
    if pub_path.exists() {
        std::fs::remove_file(&pub_path).map_err(|e| {
            GitError::OperationFailed(format!("Failed to delete public key: {}", e))
        })?;
    }

    Ok(())
}

fn list_agent_keys() -> Vec<String> {
    let output = std::process::Command::new("ssh-add")
        .arg("-l")
        .output();

    match output {
        Ok(out) if out.status.success() => {
            String::from_utf8_lossy(&out.stdout)
                .lines()
                .filter_map(|line| {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    parts.last().map(|s| s.to_string())
                })
                .collect()
        }
        _ => Vec::new(),
    }
}

pub fn add_key_to_agent(key_name: &str) -> GitResult<()> {
    let ssh_dir = ssh_dir().ok_or_else(|| {
        GitError::OperationFailed("Cannot determine home directory".into())
    })?;

    let key_path = ssh_dir.join(key_name);
    if !key_path.exists() {
        return Err(GitError::OperationFailed(format!(
            "Key not found: {}",
            key_path.display()
        )));
    }

    let output = std::process::Command::new("ssh-add")
        .arg(&key_path)
        .output()
        .map_err(|e| GitError::OperationFailed(format!("Failed to run ssh-add: {}", e)))?;

    if !output.status.success() {
        return Err(GitError::OperationFailed(
            String::from_utf8_lossy(&output.stderr).to_string(),
        ));
    }

    Ok(())
}

pub fn remove_key_from_agent(key_name: &str) -> GitResult<()> {
    let ssh_dir = ssh_dir().ok_or_else(|| {
        GitError::OperationFailed("Cannot determine home directory".into())
    })?;

    let key_path = ssh_dir.join(key_name);

    let output = std::process::Command::new("ssh-add")
        .arg("-d")
        .arg(&key_path)
        .output()
        .map_err(|e| GitError::OperationFailed(format!("Failed to run ssh-add -d: {}", e)))?;

    if !output.status.success() {
        return Err(GitError::OperationFailed(
            String::from_utf8_lossy(&output.stderr).to_string(),
        ));
    }

    Ok(())
}

pub fn check_ssh_agent() -> SshAgentStatus {
    let output = std::process::Command::new("ssh-add")
        .arg("-l")
        .output();

    match output {
        Ok(out) => {
            let available = out.status.success()
                || !String::from_utf8_lossy(&out.stderr).contains("Could not open a connection");
            let loaded_keys = if out.status.success() {
                String::from_utf8_lossy(&out.stdout)
                    .lines()
                    .filter_map(|line| {
                        let parts: Vec<&str> = line.split_whitespace().collect();
                        parts.last().map(|s| s.to_string())
                    })
                    .collect()
            } else {
                Vec::new()
            };

            let pid = std::env::var("SSH_AGENT_PID")
                .ok()
                .and_then(|p| p.parse::<u32>().ok());

            SshAgentStatus {
                available,
                pid,
                loaded_keys,
            }
        }
        Err(_) => SshAgentStatus {
            available: false,
            pid: None,
            loaded_keys: Vec::new(),
        },
    }
}

pub fn test_ssh_connection(host: &str) -> GitResult<String> {
    let output = std::process::Command::new("ssh")
        .args(["-T", &format!("git@{}", host)])
        .output()
        .map_err(|e| GitError::OperationFailed(format!("Failed to run ssh: {}", e)))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    Ok(format!("{}{}", stdout, stderr).trim().to_string())
}
