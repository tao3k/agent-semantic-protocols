use serde_json::{Value, json};

use crate::stable_graph_node_id;

const HOT_CONTEXT_BEFORE_LINES: usize = 8;
const HOT_CONTEXT_AFTER_LINES: usize = 12;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GraphCandidateLanguageId(String);

impl GraphCandidateLanguageId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for GraphCandidateLanguageId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for GraphCandidateLanguageId {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GraphProjectionCandidate {
    pub(crate) path: String,
    pub(crate) line: usize,
    pub(crate) end_line: usize,
    pub(crate) symbol: String,
    pub(crate) text: String,
    pub(crate) source: String,
    pub(crate) confidence: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CandidateReadiness {
    SelectorReady,
    InventoryOnly,
    StaleIndex,
    InvalidSelector,
    EmptyPayload,
    Quarantined,
}

impl CandidateReadiness {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SelectorReady => "selector-ready",
            Self::InventoryOnly => "inventory-only",
            Self::StaleIndex => "stale-index",
            Self::InvalidSelector => "invalid-selector",
            Self::EmptyPayload => "empty-payload",
            Self::Quarantined => "quarantined",
        }
    }

    pub fn rank_eligible(self) -> bool {
        matches!(self, Self::SelectorReady)
    }
}

pub fn graph_projection_candidate_readiness(
    candidate: &GraphProjectionCandidate,
) -> CandidateReadiness {
    if candidate.path.trim().is_empty()
        || candidate.symbol.trim().is_empty()
        || candidate.confidence == "invalid-selector"
    {
        return CandidateReadiness::InvalidSelector;
    }
    if candidate.source.contains("stale-index") || candidate.confidence == "stale-index" {
        return CandidateReadiness::StaleIndex;
    }
    if candidate.source.contains("empty-payload") || candidate.confidence == "empty-payload" {
        return CandidateReadiness::EmptyPayload;
    }
    if candidate.source.contains("quarantined") || candidate.confidence == "quarantined" {
        return CandidateReadiness::Quarantined;
    }
    if candidate.source.contains("finder")
        || candidate.confidence == "path"
        || candidate.confidence == "inventory-only"
    {
        return CandidateReadiness::InventoryOnly;
    }
    CandidateReadiness::SelectorReady
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

pub struct GraphCandidateItemNodesRequest<'a> {
    language_id: &'a GraphCandidateLanguageId,
    candidates: &'a [GraphProjectionCandidate],
    limit: usize,
}

impl<'a> GraphCandidateItemNodesRequest<'a> {
    pub fn new(
        language_id: &'a GraphCandidateLanguageId,
        candidates: &'a [GraphProjectionCandidate],
        limit: usize,
    ) -> Self {
        Self {
            language_id,
            candidates,
            limit,
        }
    }
}

pub struct GraphCandidateHotNodesRequest<'a> {
    language_id: &'a GraphCandidateLanguageId,
    candidates: &'a [GraphProjectionCandidate],
    limit: usize,
}

impl<'a> GraphCandidateHotNodesRequest<'a> {
    pub fn new(
        language_id: &'a GraphCandidateLanguageId,
        candidates: &'a [GraphProjectionCandidate],
        limit: usize,
    ) -> Self {
        Self {
            language_id,
            candidates,
            limit,
        }
    }
}

pub(crate) struct GraphCandidateSelectorRequest<'a> {
    language_id: &'a str,
    candidate: &'a GraphProjectionCandidate,
}

pub(crate) struct GraphProjectionActionRequest<'a> {
    language_id: &'a str,
}

impl<'a> From<(&'a str, &'a [GraphProjectionCandidate], usize)>
    for GraphCandidateHotNodesRequest<'a>
{
    fn from(
        (language_id, candidates, limit): (&'a str, &'a [GraphProjectionCandidate], usize),
    ) -> Self {
        Self {
            language_id,
            candidates,
            limit,
        }
    }
}

impl<'a> From<(&'a str, &'a [GraphProjectionCandidate], usize)>
    for GraphCandidateItemNodesRequest<'a>
{
    fn from(
        (language_id, candidates, limit): (&'a str, &'a [GraphProjectionCandidate], usize),
    ) -> Self {
        Self {
            language_id,
            candidates,
            limit,
        }
    }
}

pub fn graph_candidate_item_nodes(request: GraphCandidateItemNodesRequest<'_>) -> Vec<Value> {
    let language_id = request.language_id;
    request
        .candidates
        .iter()
        .take(request.limit)
        .map(|candidate| {
            let candidate_readiness = graph_projection_candidate_readiness(candidate);
            let candidate_state = candidate_readiness.as_str();
            let rank_eligible = candidate_readiness.rank_eligible();
            let code_policy = if rank_eligible {
                "code-after-exact-selector"
            } else {
                "inventory-only-refine-before-code"
            };
            let source_locator_hint = graph_candidate_selector(GraphCandidateSelectorRequest {
                language_id: request.language_id,
                candidate,
            });
            let structural_selector = graph_candidate_structural_selector(
                request.language_id,
                candidate,
                "item",
                "symbol",
            );
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
                "candidateState": candidate_state,
                "rankEligible": rank_eligible,
                "codePolicy": code_policy,
                "fields": {
                    "structuralSelector": structural_selector,
                    "displayLineRange": display_line_range,
                    "sourceLocatorHint": source_locator_hint,
                    "projection": "outline",
                    "candidateState": candidate_state,
                    "rankEligible": rank_eligible,
                    "codePolicy": code_policy,
                },
                "source": candidate.source,
                "confidence": candidate.confidence,
            })
        })
        .collect()
}

pub fn graph_candidate_hot_nodes(request: GraphCandidateHotNodesRequest<'_>) -> Vec<Value> {
    request
        .candidates
        .iter()
        .take(request.limit)
        .map(|candidate| {
            let document = graph_projection_document_language(request.language_id);
            let (start_line, end_line) = if document {
                (candidate.line, candidate.end_line())
            } else {
                hot_context_range(candidate.line)
            };
            let source_locator_hint = if document {
                graph_candidate_selector(GraphCandidateSelectorRequest {
                    language_id: request.language_id,
                    candidate,
                })
            } else {
                format!("{}:{}:{end_line}", candidate.path, start_line)
            };
            let structural_selector =
                graph_candidate_structural_selector(request.language_id, candidate, "range", "hot");
            let display_line_range = display_line_range(start_line, end_line);
            let action = graph_projection_action(GraphProjectionActionRequest {
                language_id: request.language_id,
            });
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

pub(crate) fn graph_candidate_selector(request: GraphCandidateSelectorRequest<'_>) -> String {
    let end_line = request.candidate.end_line();
    if graph_projection_document_language(request.language_id) {
        format!(
            "{}:{}-{end_line}",
            request.candidate.path, request.candidate.line
        )
    } else {
        format!(
            "{}:{}:{end_line}",
            request.candidate.path, request.candidate.line
        )
    }
}

pub(crate) fn graph_projection_action(request: GraphProjectionActionRequest<'_>) -> &'static str {
    if graph_projection_document_language(request.language_id) {
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
