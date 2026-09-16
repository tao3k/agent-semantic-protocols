// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Terminal admission for the ASP Explorer Search handoff.

use orgize::Org;
use orgize::ast::{BlockKind, ElementData};
use serde_json::Value;

const EXPLORER_AGENT_TYPE: &str = "asp_explorer";
const REPAIR_GUIDANCE: &str = "Return exactly one Org source block with language `gql`, headers `:profile search-evidence.v1 :eval never`, and the Search graph as its body. Return no prose, Markdown fence, command grammar, flat evidence line, source body, or prescribed next action.";

/// Evaluate one Host `SubagentStop` payload.
///
/// Non-Explorer Agents remain observational. ASP Explorer must expose a final
/// message whose entire outer syntax is one Org-owned GQL source block.
pub(crate) fn evaluate_subagent_stop(payload: &Value) -> Value {
    let Some(agent_type) = payload.get("agent_type").and_then(Value::as_str) else {
        return blocked_terminal();
    };
    if agent_type != EXPLORER_AGENT_TYPE {
        return serde_json::json!({});
    }
    let admitted = payload
        .get("last_assistant_message")
        .and_then(Value::as_str)
        .ok_or(())
        .and_then(validate_search_handoff);
    if admitted.is_ok() {
        serde_json::json!({})
    } else {
        blocked_terminal()
    }
}

fn blocked_terminal() -> Value {
    serde_json::json!({
        "decision": "block",
        "reason": REPAIR_GUIDANCE,
    })
}

fn validate_search_handoff(message: &str) -> Result<(), ()> {
    if message.trim().is_empty()
        || message.contains("[asp-search-subagent]")
        || message.contains("QueryGrammar")
        || message.contains("[search-result]")
    {
        return Err(());
    }
    let parsed = Org::parse(message);
    let document = parsed.document();
    if !document.diagnostics.is_empty() || document.children.len() != 1 {
        return Err(());
    }
    if !matches!(
        &document.children[0].data,
        ElementData::Block(block) if block.kind == BlockKind::Source
    ) {
        return Err(());
    }
    let records = document.source_block_records();
    let [record] = records.as_slice() else {
        return Err(());
    };
    if record.language.as_deref() != Some("gql") || record.value.trim().is_empty() {
        return Err(());
    }
    let headers = record
        .header_args
        .iter()
        .map(|header| (header.key.as_str(), header.value.as_deref()))
        .collect::<Vec<_>>();
    if headers
        != [
            ("profile", Some("search-evidence.v1")),
            ("eval", Some("never")),
        ]
    {
        return Err(());
    }
    Ok(())
}

#[cfg(test)]
#[path = "../tests/unit/search_subagent_output_contract.rs"]
mod tests;
