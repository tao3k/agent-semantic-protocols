// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Tree-sitter-only structural admission for Scheme source.

pub const SCHEME_GRAMMAR_ID: &str = "tree-sitter-scheme";
pub const SCHEME_GRAMMAR_VERSION: &str = "0.24.7";
pub const SCHEME_GRAMMAR_REPOSITORY: &str = "https://github.com/6cdh/tree-sitter-scheme";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SchemeSourceAdmission {
    pub grammar_id: &'static str,
    pub grammar_version: &'static str,
    pub grammar_repository: &'static str,
    pub byte_len: usize,
    pub root_kind: &'static str,
    pub top_level_form_count: usize,
    pub top_level_form_heads: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SchemeDatum {
    List(Vec<Self>),
    String(String),
    Symbol(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SchemeSourceAdmissionError {
    pub reason_kind: &'static str,
    pub byte_offset: usize,
    pub row: usize,
    pub column: usize,
}

impl std::fmt::Display for SchemeSourceAdmissionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "{} at byte {} ({}:{})",
            self.reason_kind,
            self.byte_offset,
            self.row + 1,
            self.column + 1
        )
    }
}

impl std::error::Error for SchemeSourceAdmissionError {}

pub fn admit_scheme_source(
    source: &str,
) -> Result<SchemeSourceAdmission, SchemeSourceAdmissionError> {
    let mut parser = tree_sitter::Parser::new();
    let language: tree_sitter::Language = tree_sitter_scheme::LANGUAGE.into();
    parser
        .set_language(&language)
        .map_err(|_| error("scheme-grammar-unavailable", 0, 0, 0))?;
    let tree = parser
        .parse(source, None)
        .ok_or_else(|| error("scheme-source-parse-unavailable", 0, 0, 0))?;
    let root = tree.root_node();
    if root.has_error() {
        let invalid = first_invalid_node(root).unwrap_or(root);
        let point = invalid.start_position();
        return Err(error(
            if invalid.is_missing() {
                "scheme-source-missing-syntax"
            } else {
                "scheme-source-invalid-syntax"
            },
            invalid.start_byte(),
            point.row,
            point.column,
        ));
    }
    let mut cursor = root.walk();
    let forms = root
        .named_children(&mut cursor)
        .filter(|node| !is_comment(*node))
        .collect::<Vec<_>>();
    if forms.is_empty() {
        return Err(error("scheme-source-empty", 0, 0, 0));
    }
    let top_level_form_heads = forms
        .iter()
        .filter_map(|node| list_head(*node, source.as_bytes()))
        .collect();
    Ok(SchemeSourceAdmission {
        grammar_id: SCHEME_GRAMMAR_ID,
        grammar_version: SCHEME_GRAMMAR_VERSION,
        grammar_repository: SCHEME_GRAMMAR_REPOSITORY,
        byte_len: source.len(),
        root_kind: root.kind(),
        top_level_form_count: forms.len(),
        top_level_form_heads,
    })
}

pub fn parse_scheme_datums(source: &str) -> Result<Vec<SchemeDatum>, SchemeSourceAdmissionError> {
    admit_scheme_source(source)?;
    let mut parser = tree_sitter::Parser::new();
    let language: tree_sitter::Language = tree_sitter_scheme::LANGUAGE.into();
    parser
        .set_language(&language)
        .map_err(|_| error("scheme-grammar-unavailable", 0, 0, 0))?;
    let tree = parser
        .parse(source, None)
        .ok_or_else(|| error("scheme-source-parse-unavailable", 0, 0, 0))?;
    let root = tree.root_node();
    let mut cursor = root.walk();
    root.named_children(&mut cursor)
        .filter(|node| !is_comment(*node))
        .map(|node| parse_datum(node, source.as_bytes()))
        .collect()
}

fn parse_datum(
    node: tree_sitter::Node<'_>,
    source: &[u8],
) -> Result<SchemeDatum, SchemeSourceAdmissionError> {
    match node.kind() {
        "list" => {
            let mut cursor = node.walk();
            Ok(SchemeDatum::List(
                node.named_children(&mut cursor)
                    .filter(|child| !is_comment(*child))
                    .map(|child| parse_datum(child, source))
                    .collect::<Result<Vec<_>, _>>()?,
            ))
        }
        "string" => node
            .utf8_text(source)
            .ok()
            .and_then(decode_string)
            .map(SchemeDatum::String)
            .ok_or_else(|| node_error("scheme-string-invalid", node)),
        "symbol" => node
            .utf8_text(source)
            .map(|value| SchemeDatum::Symbol(value.to_owned()))
            .map_err(|_| node_error("scheme-symbol-invalid", node)),
        _ => Err(node_error("scheme-datum-unsupported", node)),
    }
}

fn decode_string(source: &str) -> Option<String> {
    let body = source.strip_prefix('"')?.strip_suffix('"')?;
    let mut decoded = String::with_capacity(body.len());
    let mut chars = body.chars();
    while let Some(character) = chars.next() {
        if character != '\\' {
            decoded.push(character);
            continue;
        }
        decoded.push(match chars.next()? {
            '"' => '"',
            '\\' => '\\',
            'n' => '\n',
            'r' => '\r',
            't' => '\t',
            _ => return None,
        });
    }
    Some(decoded)
}

fn node_error(
    reason_kind: &'static str,
    node: tree_sitter::Node<'_>,
) -> SchemeSourceAdmissionError {
    let point = node.start_position();
    error(reason_kind, node.start_byte(), point.row, point.column)
}

fn list_head(node: tree_sitter::Node<'_>, source: &[u8]) -> Option<String> {
    if node.kind() != "list" {
        return None;
    }
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .find(|child| !is_comment(*child))
        .filter(|child| child.kind() == "symbol")
        .and_then(|child| child.utf8_text(source).ok())
        .map(str::to_owned)
}

fn is_comment(node: tree_sitter::Node<'_>) -> bool {
    matches!(node.kind(), "comment" | "block_comment" | "directive")
}

fn first_invalid_node(node: tree_sitter::Node<'_>) -> Option<tree_sitter::Node<'_>> {
    if node.is_error() || node.is_missing() {
        return Some(node);
    }
    let mut cursor = node.walk();
    node.children(&mut cursor).find_map(first_invalid_node)
}

fn error(
    reason_kind: &'static str,
    byte_offset: usize,
    row: usize,
    column: usize,
) -> SchemeSourceAdmissionError {
    SchemeSourceAdmissionError {
        reason_kind,
        byte_offset,
        row,
        column,
    }
}
