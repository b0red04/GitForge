use serde::{Deserialize, Serialize};
use std::ops::Range;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiffLineType {
    Context,
    Added,
    Removed,
    HunkHeader,
    NoNewlineAtEof,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DiffLine {
    pub line_type: DiffLineType,
    pub old_line: Option<u32>,
    pub new_line: Option<u32>,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffHunk {
    pub old_start: u32,
    pub old_count: u32,
    pub new_start: u32,
    pub new_count: u32,
    pub line_range: Range<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileDiff {
    pub old_path: Option<String>,
    pub new_path: Option<String>,
    #[serde(
        serialize_with = "serialize_lines",
        deserialize_with = "deserialize_lines"
    )]
    pub lines: Arc<[DiffLine]>,
    pub hunks: Vec<DiffHunk>,
    pub is_binary: bool,
}

fn serialize_lines<S: serde::Serializer>(lines: &Arc<[DiffLine]>, s: S) -> Result<S::Ok, S::Error> {
    serde::Serialize::serialize(&**lines, s)
}

fn deserialize_lines<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Arc<[DiffLine]>, D::Error> {
    let vec: Vec<DiffLine> = serde::Deserialize::deserialize(d)?;
    Ok(Arc::from(vec))
}
