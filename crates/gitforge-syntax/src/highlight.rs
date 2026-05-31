use crate::theme::HighlightScope;
use std::cell::RefCell;
use std::path::Path;

thread_local! {
    static PARSER_POOL: RefCell<Option<tree_sitter::Parser>> = RefCell::new(None);
}

#[derive(Debug, Clone)]
pub struct HighlightedLine {
    pub segments: Vec<HighlightedSegment>,
}

#[derive(Debug, Clone)]
pub struct HighlightedSegment {
    pub text: String,
    pub scope: HighlightScope,
}

pub struct SyntaxHighlighter {
    parsers: Vec<(&'static str, tree_sitter::Language)>,
}

impl SyntaxHighlighter {
    pub fn new() -> Self {
        let parsers: Vec<(&str, tree_sitter::Language)> = vec![
            ("rs", tree_sitter_rust::LANGUAGE.into()),
            ("js", tree_sitter_javascript::LANGUAGE.into()),
            ("ts", tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()),
            ("tsx", tree_sitter_typescript::LANGUAGE_TSX.into()),
            ("py", tree_sitter_python::LANGUAGE.into()),
            ("go", tree_sitter_go::LANGUAGE.into()),
            ("c", tree_sitter_c::LANGUAGE.into()),
            ("h", tree_sitter_c::LANGUAGE.into()),
            ("cpp", tree_sitter_cpp::LANGUAGE.into()),
            ("cc", tree_sitter_cpp::LANGUAGE.into()),
            ("cxx", tree_sitter_cpp::LANGUAGE.into()),
            ("hpp", tree_sitter_cpp::LANGUAGE.into()),
            ("java", tree_sitter_java::LANGUAGE.into()),
            ("json", tree_sitter_json::LANGUAGE.into()),
            ("html", tree_sitter_html::LANGUAGE.into()),
            ("htm", tree_sitter_html::LANGUAGE.into()),
            ("css", tree_sitter_css::LANGUAGE.into()),
            ("sh", tree_sitter_bash::LANGUAGE.into()),
            ("bash", tree_sitter_bash::LANGUAGE.into()),
        ];

        Self { parsers }
    }

    pub fn language_for_path(&self, path: &str) -> Option<tree_sitter::Language> {
        let ext = Path::new(path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");

        self.parsers
            .iter()
            .find(|(e, _)| *e == ext)
            .map(|(_, lang)| lang.clone())
    }

    pub fn highlight_line(
        &self,
        line: &str,
        _byte_offset: usize,
        language: &tree_sitter::Language,
    ) -> HighlightedLine {
        let full_line = format!("{}\n", line);
        let tree = PARSER_POOL.with(|cell| {
            let mut pool = cell.borrow_mut();
            let parser = pool.get_or_insert_with(tree_sitter::Parser::new);
            parser.set_language(language).ok();
            parser.parse(&full_line, None)
        });

        let mut highlights: Vec<(usize, usize, HighlightScope)> = Vec::new();

        if let Some(tree) = tree {
            Self::collect_highlights(&tree.root_node(), &mut highlights, 0);
        }

        highlights.sort_by_key(|(start, _, _)| *start);

        let mut segments = Vec::new();
        let mut cursor = 0usize;

        for (start, end, scope) in &highlights {
            let s = *start.min(&line.len());
            let e = *end.min(&line.len());

            if s > cursor {
                segments.push(HighlightedSegment {
                    text: line[cursor..s].to_string(),
                    scope: HighlightScope::Default,
                });
            }

            if s < e {
                segments.push(HighlightedSegment {
                    text: line[s..e].to_string(),
                    scope: *scope,
                });
                cursor = e;
            }
        }

        if cursor < line.len() {
            segments.push(HighlightedSegment {
                text: line[cursor..].to_string(),
                scope: HighlightScope::Default,
            });
        }

        if segments.is_empty() {
            segments.push(HighlightedSegment {
                text: line.to_string(),
                scope: HighlightScope::Default,
            });
        }

        HighlightedLine { segments }
    }

    fn collect_highlights(
        node: &tree_sitter::Node,
        highlights: &mut Vec<(usize, usize, HighlightScope)>,
        depth: usize,
    ) {
        if depth > 50 {
            return;
        }

        let kind = node.kind();
        let scope = Self::scope_for_kind(kind);

        if scope != HighlightScope::Default && node.child_count() == 0 {
            highlights.push((node.start_byte(), node.end_byte(), scope));
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            Self::collect_highlights(&child, highlights, depth + 1);
        }
    }

    fn scope_for_kind(kind: &str) -> HighlightScope {
        match kind {
            "function" | "function_item" | "function_declaration" | "method_declaration"
            | "arrow_function" | "generator_function_declaration" => HighlightScope::Function,

            "string" | "string_literal" | "string_content" | "template_string"
            | "raw_string_literal" | "interpreted_string_literal" => HighlightScope::String,

            "integer_literal" | "float_literal" | "number_literal" | "number"
            | "decimal_integer_literal" | "decimal_literal" => HighlightScope::Number,

            "line_comment" | "block_comment" | "comment" => HighlightScope::Comment,

            "type_identifier" | "primitive_type" | "struct_item" | "enum_item"
            | "trait_item" | "impl_item" | "type_item" | "class_declaration"
            | "interface_declaration" | "enum_declaration" | "struct_declaration"
            | "type_declaration" | "generic_type" | "array_type" | "optional_type"
            | "union_type" | "intersection_type" | "parenthesized_type" => HighlightScope::Type,

            "identifier" | "value_identifier" | "property_identifier"
            | "shorthand_property_identifier" | "field_identifier" => HighlightScope::Variable,

            "keyword" | "if" | "else" | "for" | "while" | "loop" | "match" | "return"
            | "let" | "const" | "var" | "fn" | "func" | "def" | "class" | "struct"
            | "enum" | "impl" | "trait" | "pub" | "mut" | "use" | "import" | "export"
            | "from" | "async" | "await" | "try" | "catch" | "throw" | "new" | "delete"
            | "break" | "continue" | "goto" | "switch" | "case" | "default"
            | "true" | "false" | "None" | "null" | "nil" | "self" | "Self"
            | "where" | "mod" | "crate" | "super" | "as" | "in" | "ref"
            | "static" | "type" | "unsafe" | "extern" | "yield" | "with"
            | "raise" | "pass" | "lambda" | "global" | "nonlocal" | "assert"
            | "package" | "interface" | "extends" | "implements" | "abstract"
            | "final" | "private" | "protected" | "public" | "synchronized"
            | "volatile" | "transient" | "native" | "throws"
            | "select" | "chan" | "defer" | "go" | "range" | "map"
            | "make" | "append" | "cap" | "copy" | "len" | "close" | "panic"
            | "recover" => HighlightScope::Keyword,

            "property" | "field_expression" | "member_expression"
            | "subscript_expression" | "attribute_item" | "meta_item"
            | "outer_attribute_item" | "inner_attribute_item" => HighlightScope::Property,

            _ => HighlightScope::Default,
        }
    }
}

impl Default for SyntaxHighlighter {
    fn default() -> Self {
        Self::new()
    }
}
