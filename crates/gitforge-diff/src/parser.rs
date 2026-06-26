use crate::types::{DiffHunk, DiffLine, DiffLineType, FileDiff};
use std::sync::Arc;

fn normalize_diff_path(raw: &str) -> Option<String> {
    let s = raw.trim();
    if s == "/dev/null" {
        return None;
    }
    let s = s
        .strip_prefix("a/")
        .or_else(|| s.strip_prefix("b/"))
        .unwrap_or(s);
    Some(s.to_string())
}

fn flush_hunk(
    lines: &[DiffLine],
    hunks: &mut Vec<DiffHunk>,
    start: Option<usize>,
    end: usize,
    old_start: u32,
    new_start: u32,
) {
    if let Some(s) = start {
        let range = s..end;
        let old_count = range
            .clone()
            .filter_map(|i| {
                let t = lines[i].line_type;
                if t == DiffLineType::Removed || t == DiffLineType::Context {
                    Some(1)
                } else {
                    None
                }
            })
            .sum::<u32>();
        let new_count = range
            .clone()
            .filter_map(|i| {
                let t = lines[i].line_type;
                if t == DiffLineType::Added || t == DiffLineType::Context {
                    Some(1)
                } else {
                    None
                }
            })
            .sum::<u32>();
        hunks.push(DiffHunk {
            old_start,
            old_count,
            new_start,
            new_count,
            line_range: range,
        });
    }
}

struct FileBuilder {
    old_path: Option<String>,
    new_path: Option<String>,
    lines: Vec<DiffLine>,
    hunks: Vec<DiffHunk>,
    is_binary: bool,
}

impl FileBuilder {
    fn new() -> Self {
        Self {
            old_path: None,
            new_path: None,
            lines: Vec::new(),
            hunks: Vec::new(),
            is_binary: false,
        }
    }

    fn build(self) -> FileDiff {
        FileDiff {
            old_path: self.old_path,
            new_path: self.new_path,
            lines: Arc::from(self.lines),
            hunks: self.hunks,
            is_binary: self.is_binary,
        }
    }
}

pub fn parse_unified_diff(raw: &str) -> Vec<FileDiff> {
    let mut files = Vec::new();
    let mut current: Option<FileBuilder> = None;
    let mut old_line = 0u32;
    let mut new_line = 0u32;
    let mut hunk_start: Option<usize> = None;
    let mut hunk_old_start = 0u32;
    let mut hunk_new_start = 0u32;

    for line in raw.lines() {
        if line.starts_with("diff --git") {
            if let Some(mut b) = current.take() {
                let len = b.lines.len();
                flush_hunk(
                    &b.lines,
                    &mut b.hunks,
                    hunk_start,
                    len,
                    hunk_old_start,
                    hunk_new_start,
                );
                files.push(b.build());
            }
            current = Some(FileBuilder::new());
            old_line = 0;
            new_line = 0;
            hunk_start = None;
            continue;
        }

        let file = match &mut current {
            Some(f) => f,
            None => continue,
        };

        if line.starts_with("--- ") {
            file.old_path = normalize_diff_path(&line[4..]);
            continue;
        }

        if line.starts_with("+++ ") {
            file.new_path = normalize_diff_path(&line[4..]);
            continue;
        }

        if line.starts_with("@@") {
            let len = file.lines.len();
            flush_hunk(
                &file.lines,
                &mut file.hunks,
                hunk_start,
                len,
                hunk_old_start,
                hunk_new_start,
            );

            if let Some((ol, nl)) = parse_hunk_header(line) {
                old_line = ol;
                new_line = nl;
                hunk_old_start = ol;
                hunk_new_start = nl;
            }
            hunk_start = Some(file.lines.len());
            file.lines.push(DiffLine {
                line_type: DiffLineType::HunkHeader,
                old_line: None,
                new_line: None,
                content: line.to_string(),
            });
            continue;
        }

        if line.starts_with("Binary files") {
            file.is_binary = true;
            continue;
        }

        if line.starts_with('+') {
            if hunk_start.is_none() {
                continue;
            }
            let content = line[1..].to_string();
            file.lines.push(DiffLine {
                line_type: DiffLineType::Added,
                old_line: None,
                new_line: Some(new_line),
                content,
            });
            new_line += 1;
        } else if line.starts_with('-') {
            if hunk_start.is_none() {
                continue;
            }
            let content = line[1..].to_string();
            file.lines.push(DiffLine {
                line_type: DiffLineType::Removed,
                old_line: Some(old_line),
                new_line: None,
                content,
            });
            old_line += 1;
        } else if line.starts_with('\\') {
            if hunk_start.is_none() {
                continue;
            }
            file.lines.push(DiffLine {
                line_type: DiffLineType::NoNewlineAtEof,
                old_line: None,
                new_line: None,
                content: line[1..].to_string(),
            });
        } else {
            if hunk_start.is_none() {
                continue;
            }
            let content = if line.is_empty() {
                String::new()
            } else {
                line.to_string()
            };
            file.lines.push(DiffLine {
                line_type: DiffLineType::Context,
                old_line: Some(old_line),
                new_line: Some(new_line),
                content,
            });
            old_line += 1;
            new_line += 1;
        }
    }

    if let Some(mut b) = current.take() {
        let len = b.lines.len();
        flush_hunk(
            &b.lines,
            &mut b.hunks,
            hunk_start,
            len,
            hunk_old_start,
            hunk_new_start,
        );
        files.push(b.build());
    }

    files
}

fn parse_hunk_header(line: &str) -> Option<(u32, u32)> {
    let re = hunk_regex();
    let caps = re.captures(line)?;
    let old_start: u32 = caps.get(1)?.as_str().parse().ok()?;
    let new_start: u32 = caps.get(2)?.as_str().parse().ok()?;
    Some((old_start, new_start))
}

use std::sync::OnceLock;
static HUNK_REGEX: OnceLock<regex_lite::Regex> = OnceLock::new();
fn hunk_regex() -> &'static regex_lite::Regex {
    HUNK_REGEX
        .get_or_init(|| regex_lite::Regex::new(r"@@ -(\d+)(?:,\d+)? \+(\d+)(?:,\d+)? @@").unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::DiffLineType::*;

    #[test]
    fn empty_input() {
        let result = parse_unified_diff("");
        assert!(result.is_empty());
    }

    #[test]
    fn single_file_single_hunk() {
        let raw = "\
diff --git a/foo.rs b/foo.rs
--- a/foo.rs
+++ b/foo.rs
@@ -1,3 +1,4 @@
 line1
-line2
+line2a
+line2b
 line3
";
        let files = parse_unified_diff(raw);
        assert_eq!(files.len(), 1);
        let f = &files[0];
        assert_eq!(f.old_path.as_deref(), Some("foo.rs"));
        assert_eq!(f.new_path.as_deref(), Some("foo.rs"));
        assert!(!f.is_binary);

        let types: Vec<_> = f.lines.iter().map(|l| l.line_type).collect();
        assert_eq!(
            types,
            vec![HunkHeader, Context, Removed, Added, Added, Context]
        );
    }

    #[test]
    fn line_number_tracking() {
        let raw = "\
diff --git a/a.txt b/a.txt
--- a/a.txt
+++ b/a.txt
@@ -10,3 +10,3 @@
 first
-second
+replaced
 third
";
        let files = parse_unified_diff(raw);
        let lines = &files[0].lines;

        assert_eq!(lines[0].old_line, None);
        assert_eq!(lines[0].new_line, None);

        assert_eq!(lines[1].old_line, Some(10));
        assert_eq!(lines[1].new_line, Some(10));
        assert_eq!(lines[1].line_type, Context);

        assert_eq!(lines[2].old_line, Some(11));
        assert_eq!(lines[2].new_line, None);
        assert_eq!(lines[2].line_type, Removed);

        assert_eq!(lines[3].old_line, None);
        assert_eq!(lines[3].new_line, Some(11));
        assert_eq!(lines[3].line_type, Added);

        assert_eq!(lines[4].old_line, Some(12));
        assert_eq!(lines[4].new_line, Some(12));
        assert_eq!(lines[4].line_type, Context);
    }

    #[test]
    fn multi_hunk_diff() {
        let raw = "\
diff --git a/big.rs b/big.rs
--- a/big.rs
+++ b/big.rs
@@ -1,3 +1,3 @@
 ctx1
-old1
+new1
 ctx2
@@ -50,3 +50,3 @@
 ctx50
-old50
+new50
 ctx51
";
        let files = parse_unified_diff(raw);
        assert_eq!(files.len(), 1);
        let hunks: Vec<_> = files[0]
            .lines
            .iter()
            .filter(|l| l.line_type == HunkHeader)
            .collect();
        assert_eq!(hunks.len(), 2);

        assert_eq!(files[0].lines[0].old_line, None);
        assert_eq!(files[0].lines[6].old_line, Some(50));
    }

    #[test]
    fn multi_file_diff() {
        let raw = "\
diff --git a/a.rs b/a.rs
--- a/a.rs
+++ b/a.rs
@@ -1,2 +1,2 @@
 x
-y
+z
diff --git a/b.rs b/b.rs
--- a/b.rs
+++ b/b.rs
@@ -5,2 +5,2 @@
 p
-q
+r
";
        let files = parse_unified_diff(raw);
        assert_eq!(files.len(), 2);
        assert_eq!(files[0].old_path.as_deref(), Some("a.rs"));
        assert_eq!(files[1].old_path.as_deref(), Some("b.rs"));
    }

    #[test]
    fn binary_file() {
        let raw = "\
diff --git a/image.png b/image.png
Binary files a/image.png and b/image.png differ
";
        let files = parse_unified_diff(raw);
        assert_eq!(files.len(), 1);
        assert!(files[0].is_binary);
    }

    #[test]
    fn no_newline_at_eof() {
        let raw = "\
diff --git a/f.txt b/f.txt
--- a/f.txt
+++ b/f.txt
@@ -1,1 +1,1 @@
-old
\\ No newline at end of file
+new
\\ No newline at end of file
";
        let files = parse_unified_diff(raw);
        let types: Vec<_> = files[0].lines.iter().map(|l| l.line_type).collect();
        assert_eq!(
            types,
            vec![HunkHeader, Removed, NoNewlineAtEof, Added, NoNewlineAtEof]
        );
    }

    #[test]
    fn pure_addition() {
        let raw = "\
diff --git a/new.txt b/new.txt
--- /dev/null
+++ b/new.txt
@@ -0,0 +1,3 @@
+line1
+line2
+line3
";
        let files = parse_unified_diff(raw);
        let lines = &files[0].lines;
        assert_eq!(lines[0].line_type, HunkHeader);
        assert!(lines[1..].iter().all(|l| l.line_type == Added));
    }

    #[test]
    fn pure_deletion() {
        let raw = "\
diff --git a/gone.txt b/gone.txt
--- a/gone.txt
+++ /dev/null
@@ -1,2 +0,0 @@
-line1
-line2
";
        let files = parse_unified_diff(raw);
        let lines = &files[0].lines;
        assert_eq!(lines[0].line_type, HunkHeader);
        assert!(lines[1..].iter().all(|l| l.line_type == Removed));
    }

    #[test]
    fn diff_without_paths() {
        let raw = "\
diff --git a/foo b/foo
@@ -1,1 +1,1 @@
-old
+new
";
        let files = parse_unified_diff(raw);
        assert_eq!(files.len(), 1);
        assert!(files[0].old_path.is_none());
        assert!(files[0].new_path.is_none());
        assert_eq!(files[0].lines.len(), 3);
    }

    #[test]
    fn hunk_header_parsing() {
        assert_eq!(parse_hunk_header("@@ -1,3 +1,4 @@"), Some((1, 1)));
        assert_eq!(parse_hunk_header("@@ -50 +50 @@"), Some((50, 50)));
        assert_eq!(parse_hunk_header("@@ -0,0 +1,3 @@"), Some((0, 1)));
        assert_eq!(parse_hunk_header("not a hunk"), None);
    }

    #[test]
    fn skips_non_hunk_metadata_lines() {
        let raw = "\
diff --git a/readme.md b/readme.md
index 1234567..89abcde 100644
--- a/readme.md
+++ b/readme.md
@@ -2,2 +2,2 @@
-old
+new
 context
";
        let files = parse_unified_diff(raw);
        assert_eq!(files.len(), 1);
        let lines = &files[0].lines;
        assert_eq!(lines[0].line_type, HunkHeader);
        assert!(lines.iter().all(|l| !l.content.starts_with("index ")));
        assert_eq!(lines[1].old_line, Some(2));
        assert_eq!(lines[1].new_line, None);
    }

    #[test]
    fn hunk_populated_single_hunk() {
        let raw = "\
diff --git a/foo.rs b/foo.rs
--- a/foo.rs
+++ b/foo.rs
@@ -1,3 +1,4 @@
 line1
-line2
+line2a
+line2b
 line3
";
        let files = parse_unified_diff(raw);
        let f = &files[0];
        assert_eq!(f.hunks.len(), 1);
        let h = &f.hunks[0];
        assert_eq!(h.old_start, 1);
        assert_eq!(h.new_start, 1);
        assert_eq!(h.old_count, 3);
        assert_eq!(h.new_count, 4);
        assert_eq!(h.line_range, 0..6);
        assert_eq!(&f.lines[h.line_range.clone()], &f.lines[..]);
    }

    #[test]
    fn hunk_populated_multi_hunk() {
        let raw = "\
diff --git a/big.rs b/big.rs
--- a/big.rs
+++ b/big.rs
@@ -1,3 +1,3 @@
 ctx1
-old1
+new1
 ctx2
@@ -50,3 +50,3 @@
 ctx50
-old50
+new50
 ctx51
";
        let files = parse_unified_diff(raw);
        let f = &files[0];
        assert_eq!(f.hunks.len(), 2);

        assert_eq!(f.hunks[0].old_start, 1);
        assert_eq!(f.hunks[0].new_start, 1);
        assert_eq!(f.hunks[0].old_count, 3);
        assert_eq!(f.hunks[0].new_count, 3);

        assert_eq!(f.hunks[1].old_start, 50);
        assert_eq!(f.hunks[1].new_start, 50);
        assert_eq!(f.hunks[1].old_count, 3);
        assert_eq!(f.hunks[1].new_count, 3);

        assert!(f.hunks[0].line_range.end == f.hunks[1].line_range.start);
    }

    #[test]
    fn no_hunks_for_binary() {
        let raw = "\
diff --git a/img.png b/img.png
Binary files a/img.png and b/img.png differ
";
        let files = parse_unified_diff(raw);
        assert!(files[0].hunks.is_empty());
    }

    #[test]
    fn hunks_span_correct_lines() {
        let raw = "\
diff --git a/a.rs b/a.rs
--- a/a.rs
+++ b/a.rs
@@ -1,2 +1,2 @@
 x
-y
+z
@@ -10,2 +10,2 @@
 p
-q
+r
";
        let files = parse_unified_diff(raw);
        let f = &files[0];

        let h0_lines: Vec<_> = f.lines[f.hunks[0].line_range.clone()]
            .iter()
            .map(|l| l.content.clone())
            .collect();
        assert!(h0_lines[0].starts_with("@@"));
        assert!(h0_lines.iter().any(|c| c.contains('x')));
        assert!(h0_lines.iter().any(|c| c == "z"));

        let h1_lines: Vec<_> = f.lines[f.hunks[1].line_range.clone()]
            .iter()
            .map(|l| l.content.clone())
            .collect();
        assert!(h1_lines.iter().any(|c| c.contains('p')));
        assert!(h1_lines.iter().any(|c| c == "r"));
    }
}
