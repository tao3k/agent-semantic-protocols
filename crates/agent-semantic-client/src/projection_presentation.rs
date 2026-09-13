// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Presentation of typed exact-query responses owned by the ASP Client.

use agent_semantic_client_protocol::ClientFrame;
use orgize::ast::{
    OrgSourceBlock, OrgSourceBlockDocument, OrgSourceBlockHeader, OrgSourceBlockHeaderValue,
};
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

/// Render an admitted Search settlement through its sole canonical Org/GQL renderer.
pub fn render_workspace_search_playbook_gql(frame: &ClientFrame) -> Result<String, String> {
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
    agent_semantic_search_projection::SearchTopologySettlement::admit(result.clone())
        .and_then(|settlement| settlement.render_org_gql())
        .map_err(|error| format!("render admitted Search topology settlement: {error}"))
}

/// Render one all-or-nothing Query Playbook receipt as native Org source blocks.
pub fn render_workspace_query_playbook_response(
    frame: &ClientFrame,
    presentation: ProjectionPresentation,
) -> Result<String, String> {
    if presentation == ProjectionPresentation::MachineJson {
        return serde_json::to_string(frame)
            .map_err(|error| format!("encode Query Playbook response: {error}"));
    }
    let ClientFrame::Response { result, error, .. } = frame else {
        return Err("Query Playbook did not return a typed response".to_owned());
    };
    if let Some(error) = error {
        return Err(error
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("Query Playbook failed")
            .to_owned());
    }
    let receipt = result
        .as_ref()
        .ok_or_else(|| "Query Playbook response is empty".to_owned())?;
    if receipt.get("schemaId").and_then(Value::as_str)
        != Some("agent.semantic-protocols.query-playbook-materialization-receipt")
        || receipt.get("schemaVersion").and_then(Value::as_str) != Some("1")
        || receipt
            .pointer("/terminal/terminalCount")
            .and_then(Value::as_u64)
            != Some(1)
    {
        return Err("Query Playbook did not return one Ready terminal".to_owned());
    }
    if receipt.pointer("/terminal/state").and_then(Value::as_str) != Some("ready") {
        let reason_kind = receipt
            .pointer("/terminal/reasonKind")
            .and_then(Value::as_str)
            .unwrap_or("query-playbook-failed");
        return Err(format!("Query Playbook failed: reasonKind={reason_kind}"));
    }
    let requested_selectors = receipt
        .get("requestedSelectors")
        .and_then(Value::as_array)
        .ok_or_else(|| "Query Playbook receipt has no requestedSelectors".to_owned())?;
    let materializations = receipt
        .get("materializations")
        .and_then(Value::as_array)
        .ok_or_else(|| "Query Playbook receipt has no materializations".to_owned())?;
    if materializations.len() != requested_selectors.len()
        || materializations
            .iter()
            .zip(requested_selectors)
            .any(|(materialization, selector)| materialization.get("selector") != Some(selector))
    {
        return Err(
            "Query Playbook materializations do not preserve the complete request order".to_owned(),
        );
    }
    let blocks = materializations
        .iter()
        .map(render_query_materialization)
        .collect::<Result<Vec<_>, _>>()?;
    OrgSourceBlockDocument::new(blocks)
        .and_then(|document| document.render())
        .map_err(|error| format!("render Query Org source-block document: {error}"))
}

fn render_query_materialization(materialization: &Value) -> Result<OrgSourceBlock, String> {
    let selector = materialization
        .get("selector")
        .and_then(Value::as_str)
        .ok_or_else(|| "Query materialization has no selector".to_owned())?;
    let language = materialization
        .get("languageId")
        .and_then(Value::as_str)
        .ok_or_else(|| "Query materialization has no languageId".to_owned())?;
    let owner_path = materialization
        .get("ownerPath")
        .and_then(Value::as_str)
        .ok_or_else(|| "Query materialization has no ownerPath".to_owned())?;
    let bytes = materialization
        .get("bytes")
        .and_then(Value::as_array)
        .ok_or_else(|| "Query materialization has no source bytes".to_owned())?
        .iter()
        .map(|value| {
            value
                .as_u64()
                .and_then(|value| u8::try_from(value).ok())
                .ok_or_else(|| "Query materialization contains a non-octet byte".to_owned())
        })
        .collect::<Result<Vec<_>, _>>()?;
    let source = String::from_utf8(bytes)
        .map_err(|_| "Query materialization source is not UTF-8".to_owned())?;
    OrgSourceBlock::new(
        language,
        vec![
            org_text_header("query", selector)?,
            org_text_header("filename", owner_path)?,
        ],
        Vec::new(),
        source,
    )
    .map_err(|error| format!("construct Query Org source block: {error}"))
}

fn org_text_header(key: &str, value: &str) -> Result<OrgSourceBlockHeader, String> {
    OrgSourceBlockHeaderValue::text(value)
        .and_then(|value| OrgSourceBlockHeader::new(key, value))
        .map_err(|error| format!("construct Org source-block header: {error}"))
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
