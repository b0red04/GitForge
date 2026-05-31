pub mod parser;
pub mod types;
pub mod patch;

pub use types::{DiffLine, DiffLineType, DiffHunk, FileDiff};
pub use patch::{extract_hunk_patch, extract_patch_from_selection};
