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
    if parent.len() <= MAX {
        return format!("...{parent}");
    }
    format!("...{}", &parent[parent.len() - (MAX - 3)..])
}
