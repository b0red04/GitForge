use crate::types::{DiffLine, DiffLineType};

pub fn extract_patch_from_selection(file_lines: &[DiffLine], selected_indices: &[usize]) -> String {
    if selected_indices.is_empty() {
        return String::new();
    }

    let mut hunks: Vec<HunkBuilder> = Vec::new();
    let mut current_hunk: Option<HunkBuilder> = None;

    for (i, line) in file_lines.iter().enumerate() {
        if !selected_indices.contains(&i) {
            if current_hunk.is_some() {
                hunks.push(current_hunk.take().unwrap());
            }
            continue;
        }

        match line.line_type {
            DiffLineType::HunkHeader => {
                if let Some(h) = current_hunk.take() {
                    hunks.push(h);
                }
            }
            DiffLineType::Context | DiffLineType::Added | DiffLineType::Removed => {
                let hunk = current_hunk.get_or_insert_with(|| HunkBuilder {
                    old_start: line.old_line.unwrap_or(1),
                    new_start: line.new_line.unwrap_or(1),
                    lines: Vec::new(),
                });
                hunk.lines.push(HunkLine {
                    line_type: line.line_type,
                    content: line.content.clone(),
                });
            }
            DiffLineType::NoNewlineAtEof => {
                if let Some(hunk) = &mut current_hunk {
                    hunk.lines.push(HunkLine {
                        line_type: DiffLineType::NoNewlineAtEof,
                        content: line.content.clone(),
                    });
                }
            }
        }
    }

    if let Some(h) = current_hunk.take() {
        hunks.push(h);
    }

    let mut result = String::new();

    for hunk in &hunks {
        let old_count: u32 = hunk
            .lines
            .iter()
            .filter(|l| {
                l.line_type == DiffLineType::Removed || l.line_type == DiffLineType::Context
            })
            .count() as u32;
        let new_count: u32 = hunk
            .lines
            .iter()
            .filter(|l| l.line_type == DiffLineType::Added || l.line_type == DiffLineType::Context)
            .count() as u32;

        result.push_str(&format!(
            "@@ -{},{} +{},{} @@\n",
            hunk.old_start, old_count, hunk.new_start, new_count,
        ));

        for hl in &hunk.lines {
            match hl.line_type {
                DiffLineType::Added => result.push_str(&format!("+{}\n", hl.content)),
                DiffLineType::Removed => result.push_str(&format!("-{}\n", hl.content)),
                DiffLineType::Context => result.push_str(&format!(" {}\n", hl.content)),
                DiffLineType::NoNewlineAtEof => result.push_str(&format!("\\{}\n", hl.content)),
                DiffLineType::HunkHeader => {}
            }
        }
    }

    result
}

struct HunkBuilder {
    old_start: u32,
    new_start: u32,
    lines: Vec<HunkLine>,
}

struct HunkLine {
    line_type: DiffLineType,
    content: String,
}
