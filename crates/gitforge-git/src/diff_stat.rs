use crate::status::DiffStat;
use std::collections::HashMap;
use std::path::Path;

/// Parses `git diff --numstat` lines (`added\tdeleted\tpath`).
pub fn parse_numstat(output: &str) -> HashMap<String, DiffStat> {
    let mut map = HashMap::new();
    for line in output.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let mut parts = line.splitn(3, '\t');
        let (Some(added_str), Some(deleted_str), Some(path)) =
            (parts.next(), parts.next(), parts.next())
        else {
            continue;
        };
        if added_str == "-" || deleted_str == "-" {
            continue;
        }
        let Ok(added) = added_str.parse::<u32>() else {
            continue;
        };
        let Ok(deleted) = deleted_str.parse::<u32>() else {
            continue;
        };
        map.insert(path.to_string(), DiffStat { added, deleted });
    }
    map
}

/// Line count for an untracked file (added lines only). Returns `None` for binary files.
pub fn untracked_line_count(repo_root: &Path, rel_path: &str) -> Option<DiffStat> {
    let full = repo_root.join(rel_path);
    let meta = std::fs::metadata(&full).ok()?;
    if !meta.is_file() {
        return None;
    }
    let data = std::fs::read(&full).ok()?;
    if data.contains(&0) {
        return None;
    }
    let added = if data.is_empty() {
        0
    } else {
        data.iter().filter(|&&b| b == b'\n').count() as u32 + u32::from(!data.ends_with(b"\n"))
    };
    Some(DiffStat { added, deleted: 0 })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_numstat_skips_binary() {
        let input = "-\t-\timage.png\n10\t5\tsrc/lib.rs\n";
        let map = parse_numstat(input);
        assert!(!map.contains_key("image.png"));
        assert_eq!(
            map.get("src/lib.rs"),
            Some(&DiffStat {
                added: 10,
                deleted: 5
            })
        );
    }
}
