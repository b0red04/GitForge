use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlameLine {
    pub line_number: usize,
    pub commit_id: String,
    pub short_id: String,
    pub author: String,
    pub author_mail: String,
    pub author_time: String,
    pub summary: String,
    pub content: String,
    pub is_boundary: bool,
}
