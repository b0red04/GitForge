use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitInfo {
    pub id: String,
    pub short_id: String,
    pub message: String,
    pub summary: String,
    pub author_name: String,
    pub author_email: String,
    pub author_date: chrono::DateTime<chrono::Utc>,
    pub committer_name: String,
    pub committer_email: String,
    pub committer_date: chrono::DateTime<chrono::Utc>,
    pub parent_ids: Vec<String>,
}

impl CommitInfo {
    pub fn is_merge(&self) -> bool {
        self.parent_ids.len() > 1
    }
}
