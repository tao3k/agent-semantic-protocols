//! Terminal admission for the compact ASP Explorer search handoff.

use std::sync::LazyLock;

use serde_json::Value;

const CONTRACT_SCHEMA: &str = include_str!(
    "../../../schemas/semantic-asp-search-subagent-graph-route-receipt.v1.schema.json"
);
const EXPLORER_AGENT_TYPE: &str = "asp_explorer";

#[derive(Debug)]
struct NaturalLanguageContract {
    example: String,
    grammar: String,
    query_grammar: String,
}

static CONTRACT: LazyLock<NaturalLanguageContract> = LazyLock::new(|| {
    let schema: Value = serde_json::from_str(CONTRACT_SCHEMA)
        .expect("ASP search SubAgent output contract schema must be valid JSON");
    let contract = schema
        .get("x-asp-natural-language-contract")
        .expect("ASP search SubAgent schema must carry its natural-language contract");
    NaturalLanguageContract {
        example: contract
            .get("example")
            .and_then(Value::as_str)
            .expect("ASP search SubAgent contract requires Example")
            .to_owned(),
        grammar: contract
            .get("grammar")
            .and_then(Value::as_str)
            .expect("ASP search SubAgent contract requires Grammar")
            .to_owned(),
        query_grammar: schema
            .get("properties")
            .and_then(|properties| properties.get("queryGrammar"))
            .and_then(|property| property.get("const"))
            .and_then(Value::as_str)
            .expect("ASP search SubAgent contract requires exact QueryGrammar")
            .to_owned(),
    }
});

/// Evaluate one Host `SubagentStop` payload.
///
/// Non-Explorer Agents remain observational. ASP Explorer must expose a final
/// message and that message must satisfy the closed executable-evidence grammar.
pub(crate) fn evaluate_subagent_stop(payload: &Value) -> Value {
    let Some(agent_type) = payload.get("agent_type").and_then(Value::as_str) else {
        return blocked_terminal("missing `agent_type`");
    };
    if agent_type != EXPLORER_AGENT_TYPE {
        return serde_json::json!({});
    }
    let result = payload
        .get("last_assistant_message")
        .and_then(Value::as_str)
        .ok_or_else(|| "missing `last_assistant_message`".to_owned())
        .and_then(validate_search_handoff);
    match result {
        Ok(()) => serde_json::json!({}),
        Err(reason) => blocked_terminal(&reason),
    }
}

fn blocked_terminal(_reason: &str) -> Value {
    let contract = &*CONTRACT;
    let guidance = format!(
        "Example\n{}\n\nGrammar\n{}",
        contract.example, contract.grammar
    );
    serde_json::json!({
        "decision": "block",
        "reason": guidance,
    })
}

fn validate_search_handoff(message: &str) -> Result<(), String> {
    let lines = message.lines().collect::<Vec<_>>();
    if lines.first().copied() != Some("[asp-search-subagent]") {
        return Err("missing `[asp-search-subagent]` header".to_owned());
    }
    let state = lines
        .get(1)
        .and_then(|line| line.strip_prefix("state="))
        .filter(|state| is_terminal_state(state))
        .ok_or_else(|| "missing or invalid terminal state".to_owned())?;
    match state {
        "candidates" => {
            if !(4..=33).contains(&lines.len()) {
                return Err("candidates requires one to thirty Top-K evidence lines".to_owned());
            }
            if lines[2] != format!("QueryGrammar: {}", CONTRACT.query_grammar) {
                return Err("candidates requires the exact QueryGrammar".to_owned());
            }
            for (index, line) in lines[3..].iter().enumerate() {
                validate_evidence_line(line, index + 1)?;
            }
        }
        "empty" => {
            if lines.len() != 2 {
                return Err("empty forbids evidence and failure metadata".to_owned());
            }
        }
        "unavailable" => {
            if lines.len() != 4 {
                return Err("unavailable requires exactly stage and reasonKind".to_owned());
            }
            lines[2]
                .strip_prefix("stage=")
                .filter(|stage| {
                    matches!(*stage, "registration" | "runtime" | "search" | "projection")
                })
                .ok_or_else(|| "unavailable has invalid stage".to_owned())?;
            lines[3]
                .strip_prefix("reasonKind=")
                .filter(|reason| {
                    !reason.is_empty()
                        && reason.chars().all(|character| {
                            character.is_ascii_alphanumeric() || ".-_".contains(character)
                        })
                        && reason
                            .chars()
                            .next()
                            .is_some_and(|character| character.is_ascii_alphanumeric())
                })
                .ok_or_else(|| "unavailable has invalid reasonKind".to_owned())?;
        }
        _ => unreachable!("terminal state checked above"),
    }
    Ok(())
}

fn is_terminal_state(state: &str) -> bool {
    matches!(state, "candidates" | "empty" | "unavailable")
}

fn validate_evidence_line(line: &str, expected_rank: usize) -> Result<(), String> {
    let fields = line.split(" | ").collect::<Vec<_>>();
    if fields.len() != 6 || fields[0] != format!("E{expected_rank}") {
        return Err(format!(
            "evidence E{expected_rank} does not match the closed pipe grammar"
        ));
    }
    for (field, prefix) in
        fields[1..]
            .iter()
            .zip(["owner=", "item=", "selector=", "matchedBy=", "relation="])
    {
        field
            .strip_prefix(prefix)
            .filter(|value| !value.is_empty() && !value.chars().any(char::is_whitespace))
            .ok_or_else(|| format!("evidence E{expected_rank} has invalid `{prefix}` value"))?;
    }
    let selector = fields[3]
        .strip_prefix("selector=")
        .expect("selector prefix checked above");
    let owner = fields[1]
        .strip_prefix("owner=")
        .expect("owner prefix checked above");
    let item = fields[2]
        .strip_prefix("item=")
        .expect("item prefix checked above");
    let selector_owner = selector
        .split_once("://")
        .and_then(|(_, value)| value.split_once("#item/"))
        .filter(|(_, item)| !item.is_empty())
        .map(|(selector_owner, selector_item)| (selector_owner, selector_item));
    if selector_owner != Some((owner, item)) {
        return Err(format!(
            "evidence E{expected_rank} selector does not map to item `{item}` owned by `{owner}`"
        ));
    }
    let matched_by = fields[4]
        .strip_prefix("matchedBy=")
        .expect("matchedBy prefix checked above")
        .split('|')
        .collect::<Vec<_>>();
    if matched_by.is_empty()
        || matched_by.iter().any(|value| !is_clause_reference(value))
        || matched_by
            .iter()
            .enumerate()
            .any(|(index, value)| matched_by[..index].contains(value))
        || !clause_references_are_progressive(&matched_by)
    {
        return Err(format!(
            "evidence E{expected_rank} has invalid or duplicate supporting clauses"
        ));
    }
    if has_display_line_suffix(selector) {
        return Err(format!(
            "evidence E{expected_rank} uses a display line range as identity"
        ));
    }
    Ok(())
}

fn is_clause_reference(value: &str) -> bool {
    let Some((axis, index)) = value.split_once(':') else {
        return false;
    };
    matches!(axis, "fd" | "rg" | "tantivy" | "syntax" | "graph")
        && !index.is_empty()
        && index.bytes().all(|byte| byte.is_ascii_digit())
}

fn clause_references_are_progressive(values: &[&str]) -> bool {
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

fn has_display_line_suffix(selector: &str) -> bool {
    let Some((_, suffix)) = selector.rsplit_once(':') else {
        return false;
    };
    suffix.bytes().all(|byte| byte.is_ascii_digit())
        || suffix.split_once('-').is_some_and(|(start, end)| {
            !start.is_empty()
                && !end.is_empty()
                && start.bytes().all(|byte| byte.is_ascii_digit())
                && end.bytes().all(|byte| byte.is_ascii_digit())
        })
}

#[cfg(test)]
#[path = "../tests/unit/search_subagent_output_contract.rs"]
mod tests;
