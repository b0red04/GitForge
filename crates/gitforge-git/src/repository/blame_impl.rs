use crate::error::{GitError, GitResult};
use crate::repository::Repository;
use crate::blame::BlameLine;
use std::path::Path;
use std::collections::HashMap;

impl Repository {
    /// Spawns a `git` subprocess.
    pub fn blame_file(&self, file_path: &Path, revision: Option<&str>) -> GitResult<Vec<BlameLine>> {
        let mut args = vec!["blame", "--porcelain"];
        if let Some(rev) = revision {
            args.push(rev);
        }
        args.push("--");
        args.push(file_path.to_str().unwrap_or(""));

        let output = std::process::Command::new("git")
            .args(&args)
            .current_dir(&self.path)
            .output()
            .map_err(|e| GitError::OperationFailed(format!("Failed to run git blame: {}", e)))?;

        if !output.status.success() {
            return Err(GitError::OperationFailed(
                String::from_utf8_lossy(&output.stderr).to_string(),
            ));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        parse_blame_porcelain(&stdout)
    }
}

fn parse_blame_porcelain(output: &str) -> GitResult<Vec<BlameLine>> {
    let mut commits: HashMap<String, BlameCommitInfo> = HashMap::new();
    let mut lines: Vec<BlameLine> = Vec::new();

    let mut current_commit_id: Option<String> = None;
    let mut current_line_num: usize = 0;

    for raw_line in output.lines() {
        if raw_line.starts_with('\t') {
            let content = raw_line[1..].to_string();
            if let Some(commit_id) = &current_commit_id {
                let info = commits.get(commit_id).cloned().unwrap_or_else(|| BlameCommitInfo {
                    author: String::new(),
                    author_mail: String::new(),
                    author_time: String::new(),
                    summary: String::new(),
                    is_boundary: false,
                });
                lines.push(BlameLine {
                    line_number: current_line_num,
                    commit_id: commit_id.clone(),
                    short_id: commit_id[..7.min(commit_id.len())].to_string(),
                    author: info.author,
                    author_mail: info.author_mail,
                    author_time: info.author_time,
                    summary: info.summary,
                    content,
                    is_boundary: info.is_boundary,
                });
            }
            current_commit_id = None;
        } else if let Some(rest) = raw_line.strip_prefix("author-mail ") {
            if let Some(cid) = &current_commit_id {
                commits.entry(cid.clone()).or_default().author_mail = rest.to_string();
            }
        } else if let Some(rest) = raw_line.strip_prefix("author ") {
            if let Some(cid) = &current_commit_id {
                commits.entry(cid.clone()).or_default().author = rest.to_string();
            }
        } else if let Some(rest) = raw_line.strip_prefix("author-time ") {
            if let Some(cid) = &current_commit_id {
                let ts: i64 = rest.parse().unwrap_or(0);
                let dt = chrono::DateTime::from_timestamp(ts, 0)
                    .map(|d| d.format("%Y-%m-%d").to_string())
                    .unwrap_or_default();
                commits.entry(cid.clone()).or_default().author_time = dt;
            }
        } else if let Some(rest) = raw_line.strip_prefix("summary ") {
            if let Some(cid) = &current_commit_id {
                commits.entry(cid.clone()).or_default().summary = rest.to_string();
            }
        } else if raw_line.starts_with("boundary") {
            if let Some(cid) = &current_commit_id {
                commits.entry(cid.clone()).or_default().is_boundary = true;
            }
        } else if let Some(first_word) = raw_line.split_whitespace().next() {
            if first_word.len() >= 7 && first_word.chars().all(|c| c.is_ascii_hexdigit()) {
                let parts: Vec<&str> = raw_line.split_whitespace().collect();
                if parts.len() >= 3 {
                    current_commit_id = Some(first_word.to_string());
                    current_line_num = parts[2].parse().unwrap_or(0);
                }
            }
        }
    }

    Ok(lines)
}

#[derive(Clone)]
struct BlameCommitInfo {
    author: String,
    author_mail: String,
    author_time: String,
    summary: String,
    is_boundary: bool,
}

impl Default for BlameCommitInfo {
    fn default() -> Self {
        Self {
            author: String::new(),
            author_mail: String::new(),
            author_time: String::new(),
            summary: String::new(),
            is_boundary: false,
        }
    }
}
