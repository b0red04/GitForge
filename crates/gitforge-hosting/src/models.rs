use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostingAccount {
    pub provider: String,
    pub username: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
    pub token_key: String,
    pub created_at: Option<DateTime<Utc>>,
}

impl HostingAccount {
    pub fn token(&self) -> Result<String, keyring::Error> {
        let entry = keyring::Entry::new("gitforge-hosting", &self.token_key)?;
        entry.get_password()
    }

    pub fn store_token(token_key: &str, token: &str) -> Result<(), keyring::Error> {
        let entry = keyring::Entry::new("gitforge-hosting", token_key)?;
        entry.set_password(token)
    }

    pub fn delete_token(token_key: &str) -> Result<(), keyring::Error> {
        let entry = keyring::Entry::new("gitforge-hosting", token_key)?;
        entry.delete_credential()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatePullRequestRequest {
    pub owner: String,
    pub repo: String,
    pub title: String,
    pub body: String,
    pub head_owner: String,
    pub head_branch: String,
    pub base_branch: String,
    pub draft: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PullRequest {
    pub number: u64,
    pub title: String,
    pub html_url: String,
    pub state: String,
    #[serde(default)]
    pub head_branch: Option<String>,
    #[serde(default)]
    pub draft: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteRepo {
    pub name: String,
    pub full_name: String,
    pub description: Option<String>,
    pub clone_url: String,
    pub ssh_url: Option<String>,
    pub html_url: String,
    pub is_fork: bool,
    pub is_private: bool,
    pub default_branch: Option<String>,
    pub stars: u64,
    pub updated_at: Option<DateTime<Utc>>,
}
