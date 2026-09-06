// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

//! Presentation of typed exact-query responses owned by the ASP Client.

use agent_semantic_client_protocol::ClientFrame;
use agent_semantic_search::WORKSPACE_SEARCH_QUERY_GRAMMAR;
use serde_json::Value;

/// Output representation requested by an ASP Client command entry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProjectionPresentation {
    /// Render the semantic projection as human-readable UTF-8 source text.
    Text,
    /// Preserve the complete typed wire response as JSON.
    MachineJson,
}

/// Render one exact-query response without provider-specific behavior.
pub fn render_exact_projection_response(
    frame: &ClientFrame,
    presentation: ProjectionPresentation,
) -> Result<String, String> {
    if presentation == ProjectionPresentation::MachineJson {
        return serde_json::to_string(frame)
            .map_err(|error| format!("encode exact projection response: {error}"));
    }
    let ClientFrame::Response { result, error, .. } = frame else {
        return Err("exact projection did not return a successful typed response".to_owned());
    };
    if let Some(error) = error {
        // JSON is a deliberate protocol/debug representation.  A normal
        // query must never turn a source request into a JSON document merely
        // because the Runtime returned a typed terminal.
        let message = error
            .get("message")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| "exact projection failed without a human diagnostic".to_owned())?;
        return Err(message);
    }
    let Some(result) = result else {
        return Err("exact projection response has neither payload nor typed failure".to_owned());
    };
    render_projection_result(result)
}

/// Render contract recovery as the two sections an Agent needs to construct a
/// native Search Playbook request. Runtime identities remain inside admission
/// and are deliberately not copied into this projection.
pub fn render_search_playbook_contract_response(frame: &ClientFrame) -> Result<String, String> {
    let ClientFrame::Response { result, error, .. } = frame else {
        return Err("Search Playbook contract did not return a typed response".to_owned());
    };
    if let Some(error) = error {
        return Err(error
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("Search Playbook contract failed")
            .to_owned());
    }
    let result = result
        .as_ref()
        .ok_or_else(|| "Search Playbook contract response is empty".to_owned())?;
    if result.get("result").and_then(Value::as_str) != Some("ready") {
        return Err(result
            .get("reason")
            .and_then(Value::as_str)
            .unwrap_or("Search Playbook contract is unavailable")
            .to_owned());
    }
    let projection = result
        .get("projection")
        .ok_or_else(|| "Search Playbook contract has no projection".to_owned())?;
    let example = projection
        .get("example")
        .and_then(Value::as_str)
        .ok_or_else(|| "Search Playbook contract has no Example".to_owned())?;
    let grammar = projection
        .get("grammar")
        .and_then(Value::as_str)
        .ok_or_else(|| "Search Playbook contract has no Grammar".to_owned())?;
    Ok(format!("Example\n{example}\n\nGrammar\n{grammar}"))
}

/// Render an executed Search Playbook as one explicit result and bounded
/// selector evidence. Runtime audit identities remain on the server side.
pub fn render_workspace_search_playbook_result(frame: &ClientFrame) -> Result<String, String> {
    let ClientFrame::Response { result, error, .. } = frame else {
        return Err("Workspace Search Playbook did not return a typed response".to_owned());
    };
    if let Some(error) = error {
        return Err(error
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("Workspace Search Playbook failed")
            .to_owned());
    }
    let result = result
        .as_ref()
        .ok_or_else(|| "Workspace Search Playbook response is empty".to_owned())?;
    if result.get("schemaId").and_then(Value::as_str)
        != Some("agent.semantic-protocols.workspace-search-playbook-result")
    {
        return Err("Workspace Search Playbook did not execute to a public result".to_owned());
    }
    let result_kind = result
        .get("result")
        .and_then(Value::as_str)
        .ok_or_else(|| "Workspace Search Playbook result kind is missing".to_owned())?;
    let evidence = result
        .get("evidence")
        .and_then(Value::as_array)
        .ok_or_else(|| "Workspace Search Playbook evidence is missing".to_owned())?;
    if evidence.len() > 30 {
        return Err("Workspace Search Playbook exceeded its Top-30 projection".to_owned());
    }
    let query_grammar = result.get("queryGrammar").and_then(Value::as_str);
    if evidence.is_empty() {
        if query_grammar.is_some() {
            return Err(
                "Workspace Search Playbook empty result must not expose QueryGrammar".to_owned(),
            );
        }
    } else if query_grammar != Some(WORKSPACE_SEARCH_QUERY_GRAMMAR) {
        return Err(
            "Workspace Search Playbook evidence requires the exact QueryGrammar".to_owned(),
        );
    }
    let lines = evidence
        .iter()
        .enumerate()
        .map(|(index, item)| {
            let owner = item
                .get("owner")
                .and_then(Value::as_str)
                .ok_or_else(|| "Workspace Search Playbook evidence has no owner".to_owned())?;
            let exact_item = item
                .get("item")
                .and_then(Value::as_str)
                .ok_or_else(|| "Workspace Search Playbook evidence has no exact item".to_owned())?;
            let selector = item
                .get("selector")
                .and_then(Value::as_str)
                .ok_or_else(|| "Workspace Search Playbook evidence has no selector".to_owned())?;
            let selector_item = selector
                .split_once("#item/")
                .map(|(_, value)| value)
                .filter(|value| !value.is_empty());
            if selector_item != Some(exact_item) {
                return Err("Workspace Search Playbook exact item does not match its selector".to_owned());
            }
            let matched_by = item
                .get("matchedBy")
                .and_then(Value::as_array)
                .filter(|values| !values.is_empty())
                .ok_or_else(|| "Workspace Search Playbook evidence has no supporting clauses".to_owned())?
                .iter()
                .map(|value| {
                    value
                        .as_str()
                        .filter(|value| is_search_clause_reference(value))
                        .ok_or_else(|| {
                            "Workspace Search Playbook evidence has an invalid supporting clause"
                                .to_owned()
                        })
                })
                .collect::<Result<Vec<_>, String>>()?;
            if !search_clause_references_are_progressive(&matched_by) {
                return Err(
                    "Workspace Search Playbook supporting clauses violate progressive order"
                        .to_owned(),
                );
            }
            let relation = item
                .get("relation")
                .and_then(Value::as_str)
                .ok_or_else(|| "Workspace Search Playbook evidence has no relation".to_owned())?;
            Ok(format!(
                "E{} | owner={owner} | item={exact_item} | selector={selector} | matchedBy={} | relation={relation}",
                index + 1,
                matched_by.join("|")
            ))
        })
        .collect::<Result<Vec<_>, String>>()?;
    let header = format!(
        "[search-result] result={result_kind} evidence={}",
        lines.len()
    );
    if lines.is_empty() {
        Ok(header)
    } else {
        Ok(format!(
            "{header}\nQueryGrammar: {WORKSPACE_SEARCH_QUERY_GRAMMAR}\n{}",
            lines.join("\n")
        ))
    }
}

fn is_search_clause_reference(value: &str) -> bool {
    let Some((axis, index)) = value.split_once(':') else {
        return false;
    };
    matches!(axis, "fd" | "rg" | "tantivy" | "syntax" | "graph")
        && !index.is_empty()
        && index.bytes().all(|byte| byte.is_ascii_digit())
}

fn search_clause_references_are_progressive(values: &[&str]) -> bool {
    let mut graph_seen = false;
    for value in values {
        if value.starts_with("graph:") {
            graph_seen = true;
        } else if graph_seen {
            return false;
        }
    }
    true
}

pub fn render_workspace_syntax_query_response(
    frame: &ClientFrame,
    presentation: ProjectionPresentation,
) -> Result<String, String> {
    if presentation == ProjectionPresentation::MachineJson {
        return serde_json::to_string(frame)
            .map_err(|error| format!("encode workspace syntax Query response: {error}"));
    }
    let ClientFrame::Response { result, error, .. } = frame else {
        return Err("workspace syntax Query did not return a typed response".to_owned());
    };
    if let Some(error) = error {
        return Err(error
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("workspace syntax Query failed")
            .to_owned());
    }
    let result = result
        .as_ref()
        .ok_or_else(|| "workspace syntax Query response is empty".to_owned())?;
    let evidence = result
        .get("evidence")
        .and_then(Value::as_array)
        .ok_or_else(|| "workspace syntax Query response has no evidence".to_owned())?;
    if evidence.len() > 3 {
        return Err("workspace syntax Query exceeded its Top-3 projection".to_owned());
    }
    let lines = evidence
        .iter()
        .enumerate()
        .map(|(index, item)| {
            let owner = item
                .get("owner")
                .and_then(Value::as_str)
                .ok_or_else(|| "workspace syntax Query evidence has no owner".to_owned())?;
            let selector = item
                .get("selector")
                .and_then(Value::as_str)
                .ok_or_else(|| "workspace syntax Query evidence has no selector".to_owned())?;
            let relation = item
                .get("relation")
                .and_then(Value::as_str)
                .ok_or_else(|| "workspace syntax Query evidence has no relation".to_owned())?;
            Ok(format!(
                "E{} | owner={owner} | selector={selector} | relation={relation}",
                index + 1
            ))
        })
        .collect::<Result<Vec<_>, String>>()?;
    let header = format!("[query-result] state=ready evidence={}", lines.len());
    if lines.is_empty() {
        Ok(header)
    } else {
        Ok(format!("{header}\n{}", lines.join("\n")))
    }
}

fn render_projection_result(result: &Value) -> Result<String, String> {
    let payload = result.get("result").unwrap_or(result);
    if let Some(text) = payload.as_str() {
        return Ok(text.to_owned());
    }
    let bytes = payload
        .get("bytes")
        .and_then(Value::as_array)
        .ok_or_else(|| "exact projection response has no text or byte payload".to_owned())?;
    let bytes = bytes
        .iter()
        .map(|byte| {
            byte.as_u64()
                .filter(|byte| *byte <= u8::MAX as u64)
                .map(|byte| byte as u8)
                .ok_or_else(|| "exact projection response contains a non-byte value".to_owned())
        })
        .collect::<Result<Vec<_>, _>>()?;
    String::from_utf8(bytes)
        .map_err(|error| format!("exact projection response is not UTF-8 text: {error}"))
}
