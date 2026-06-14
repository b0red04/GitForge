pub mod parser;
pub mod patch;
pub mod types;

pub use patch::extract_patch_from_selection;
pub use types::{DiffHunk, DiffLine, DiffLineType, FileDiff};
