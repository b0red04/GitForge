use std::path::Path;

pub fn split_path_display(path: &str) -> (String, Option<String>) {
    let path = Path::new(path);
    let file_name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string_lossy().into_owned());
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map(|p| p.to_string_lossy().replace('\\', "/"));
    (file_name, parent)
}

pub fn format_parent_path(parent: &str) -> String {
    const MAX: usize = 36;
    const ELLIPSIS: &str = "...";
    const TAIL: usize = MAX - ELLIPSIS.len();

    let char_count = parent.chars().count();
    if char_count <= MAX {
        return format!("{ELLIPSIS}{parent}");
    }

    let start = parent
        .char_indices()
        .nth(char_count - TAIL)
        .map_or(0, |(byte_idx, _)| byte_idx);
    format!("{ELLIPSIS}{}", &parent[start..])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_path_kept_intact() {
        assert_eq!(format_parent_path("src/views"), "...src/views");
    }

    #[test]
    fn exactly_max_chars_not_truncated() {
        let parent = "a".repeat(36);
        assert_eq!(format_parent_path(&parent), format!("...{parent}"));
    }

    #[test]
    fn long_ascii_path_truncated_to_tail() {
        let parent = "a".repeat(50);
        assert_eq!(
            format_parent_path(&parent),
            format!("...{}", "a".repeat(33))
        );
    }

    #[test]
    fn multibyte_path_does_not_panic() {
        let parent = "é".repeat(50);
        let result = format_parent_path(&parent);
        assert!(result.starts_with("..."));
        assert_eq!(result.chars().count(), 3 + 33);
    }

    #[test]
    fn multibyte_path_truncates_on_char_boundary() {
        let parent: String = "字".repeat(40) + "END";
        let result = format_parent_path(&parent);
        assert!(result.starts_with("..."));
        assert!(result.ends_with("END"));
    }
}
