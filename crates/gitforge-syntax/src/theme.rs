use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyntaxTheme {
    pub name: String,
    pub token_colors: Vec<TokenColor>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenColor {
    pub scope: Vec<String>,
    pub foreground: Option<String>,
    pub background: Option<String>,
    pub font_style: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HighlightScope {
    Keyword,
    Function,
    String,
    Number,
    Comment,
    Type,
    Variable,
    Operator,
    Punctuation,
    Property,
    Tag,
    Attribute,
    Constant,
    Module,
    Default,
}

