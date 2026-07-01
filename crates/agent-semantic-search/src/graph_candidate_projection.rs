use serde_json::{Value, json};

use crate::stable_graph_node_id;

const HOT_CONTEXT_BEFORE_LINES: usize = 8;
const HOT_CONTEXT_AFTER_LINES: usize = 12;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GraphProjectionCandidate {
    pub path: String,
    pub line: usize,
    pub end_line: usize,
    pub symbol: String,
    pub text: String,
    pub source: String,
    pub confidence: String,
}

impl GraphProjectionCandidate {
    pub fn new(
        path: impl Into<String>,
        line: usize,
        end_line: usize,
        symbol: impl Into<String>,
        text: impl Into<String>,
        source: impl Into<String>,
        confidence: impl Into<String>,
    ) -> Self {
        Self {
            path: path.into(),
            line,
            end_line,
            symbol: symbol.into(),
            text: text.into(),
            source: source.into(),
            confidence: confidence.into(),
        }
    }

    pub fn end_line(&self) -> usize {
        self.end_line.max(self.line)
    }
}

pub fn graph_candidate_item_nodes(
    language_id: &str,
    candidates: &[GraphProjectionCandidate],
    limit: usize,
) -> Vec<Value> {
    candidates
        .iter()
        .take(limit)
        .map(|candidate| {
            let source_locator_hint = graph_candidate_selector(language_id, candidate);
            let structural_selector =
                graph_candidate_structural_selector(language_id, candidate, "item", "symbol");
            let display_line_range = display_line_range(candidate.line, candidate.end_line());
            json!({
                "id": graph_candidate_item_node_id(candidate),
                "kind": "item",
                "role": "symbol",
                "value": candidate.symbol,
                "action": "syntax",
                "path": candidate.path,
                "ownerPath": candidate.path,
                "symbol": candidate.symbol,
                "startLine": candidate.line,
                "endLine": candidate.end_line(),
                "locator": structural_selector,
                "structuralSelector": structural_selector,
                "displayLineRange": display_line_range,
                "sourceLocatorHint": source_locator_hint,
                "matchText": candidate.text,
                "syntaxQuery": graph_candidate_tree_sitter_pattern(language_id, &candidate.symbol),
                "projection": "outline",
                "codePolicy": "code-after-exact-selector",
                "fields": {
                    "structuralSelector": structural_selector,
                    "displayLineRange": display_line_range,
                    "sourceLocatorHint": source_locator_hint,
                    "projection": "outline",
                    "codePolicy": "code-after-exact-selector",
                },
                "source": candidate.source,
                "confidence": candidate.confidence,
            })
        })
        .collect()
}

pub fn graph_candidate_hot_nodes(
    language_id: &str,
    candidates: &[GraphProjectionCandidate],
    limit: usize,
) -> Vec<Value> {
    candidates
        .iter()
        .take(limit)
        .map(|candidate| {
            let document = graph_projection_document_language(language_id);
            let (start_line, end_line) = if document {
                (candidate.line, candidate.end_line())
            } else {
                hot_context_range(candidate.line)
            };
            let source_locator_hint = if document {
                graph_candidate_selector(language_id, candidate)
            } else {
                format!("{}:{}:{end_line}", candidate.path, start_line)
            };
            let structural_selector =
                graph_candidate_structural_selector(language_id, candidate, "range", "hot");
            let display_line_range = display_line_range(start_line, end_line);
            let action = graph_projection_action(language_id);
            let (projection, code_policy) = if action == "code" {
                ("code", "requires-exact-code")
            } else {
                ("outline", "code-after-exact-selector")
            };
            json!({
                "id": graph_candidate_hot_node_id(candidate),
                "kind": "hot",
                "role": "range",
                "value": candidate.symbol,
                "action": action,
                "path": candidate.path,
                "ownerPath": candidate.path,
                "symbol": candidate.symbol,
                "startLine": start_line,
                "endLine": end_line,
                "locator": structural_selector,
                "structuralSelector": structural_selector,
                "displayLineRange": display_line_range,
                "sourceLocatorHint": source_locator_hint,
                "matchText": candidate.text,
                "projection": projection,
                "codePolicy": code_policy,
                "fields": {
                    "structuralSelector": structural_selector,
                    "displayLineRange": display_line_range,
                    "sourceLocatorHint": source_locator_hint,
                    "projection": projection,
                    "codePolicy": code_policy,
                },
                "source": candidate.source,
                "confidence": candidate.confidence,
            })
        })
        .collect()
}

pub fn graph_candidate_item_node_id(candidate: &GraphProjectionCandidate) -> String {
    stable_graph_node_id(
        "item",
        &format!(
            "{}:{}:{}-{}",
            candidate.path,
            candidate.symbol,
            candidate.line,
            candidate.end_line()
        ),
    )
}

pub fn graph_candidate_hot_node_id(candidate: &GraphProjectionCandidate) -> String {
    stable_graph_node_id(
        "hot",
        &format!(
            "{}:{}:{}-{}",
            candidate.path,
            candidate.symbol,
            candidate.line,
            candidate.end_line()
        ),
    )
}

pub fn graph_candidate_selector(language_id: &str, candidate: &GraphProjectionCandidate) -> String {
    let end_line = candidate.end_line();
    if graph_projection_document_language(language_id) {
        format!("{}:{}-{end_line}", candidate.path, candidate.line)
    } else {
        format!("{}:{}:{end_line}", candidate.path, candidate.line)
    }
}

pub fn graph_projection_action(language_id: &str) -> &'static str {
    if graph_projection_document_language(language_id) {
        "content"
    } else {
        "code"
    }
}

fn graph_projection_document_language(language_id: &str) -> bool {
    matches!(language_id, "markdown" | "md" | "org" | "text")
}

fn graph_candidate_structural_selector(
    language_id: &str,
    candidate: &GraphProjectionCandidate,
    kind: &str,
    role: &str,
) -> String {
    let language = if language_id.is_empty() {
        "code"
    } else {
        language_id
    };
    format!(
        "{}://{}#{}/{}/{}",
        selector_token(language),
        selector_token(&candidate.path),
        selector_token(kind),
        selector_token(role),
        selector_token(&candidate.symbol)
    )
}

fn hot_context_range(line: usize) -> (usize, usize) {
    (
        line.saturating_sub(HOT_CONTEXT_BEFORE_LINES).max(1),
        line + HOT_CONTEXT_AFTER_LINES,
    )
}

fn display_line_range(start_line: usize, end_line: usize) -> String {
    format!("{start_line}:{end_line}")
}

fn selector_token(value: &str) -> String {
    let token = value
        .trim()
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '_' | '-' | '.' | '/' | ':')
            {
                character
            } else {
                '-'
            }
        })
        .collect::<String>();
    if token.is_empty() {
        "selector".to_string()
    } else {
        token
    }
}

fn graph_candidate_tree_sitter_pattern(language_id: &str, symbol: &str) -> Option<String> {
    let escaped_symbol = symbol.replace('\\', "\\\\").replace('"', "\\\"");
    match language_id {
        "rust" => Some(format!(
            "((function_item name: (_) @function.name) (#eq? @function.name \"{escaped_symbol}\"))"
        )),
        "python" => Some(format!(
            "((function_definition name: (identifier) @function.name) (#eq? @function.name \"{escaped_symbol}\"))"
        )),
        _ => None,
    }
}
