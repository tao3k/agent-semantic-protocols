//! Search packet artifact replay through the local graph renderer.

use std::fs;
use std::path::{Path, PathBuf};

use bytes::Bytes;
use serde_json::Value;

use super::graph_render::{
    GraphRenderReceiptRequest, run_graph_render_packet, run_graph_render_packet_bytes,
    run_graph_render_packet_bytes_with_receipt,
};
use super::limits::MAX_CACHE_REPLAY_ARTIFACT_BYTES;

pub(crate) struct SearchFrontierReceiptRequest {
    pub(crate) out_path: PathBuf,
    pub(crate) receipt_id: String,
    pub(crate) task_fingerprint: String,
    pub(crate) command_fingerprint: String,
}

pub(crate) fn search_output_artifact_replay_safe(stdout: &[u8]) -> bool {
    let Ok(stdout) = std::str::from_utf8(stdout) else {
        return false;
    };
    let Some(header) = stdout.lines().next() else {
        return false;
    };
    if header.starts_with("[search-deps]") {
        return deps_search_output_replay_safe(stdout);
    }
    let has_frontier_header = header.contains("[graph-frontier]") || header.starts_with("[search-");
    if header.starts_with("[search-prime]") && !stdout.contains("|decision purpose=decision-primer")
    {
        return false;
    }
    let has_alias_graph = stdout.contains("aliases=");
    has_frontier_header
        && has_alias_graph
        && stdout.contains("legend: ID=kind:role(value)!next;")
        && stdout.contains("frontier ID.next")
        && !stdout.contains('\0')
}

fn deps_search_output_replay_safe(stdout: &str) -> bool {
    !stdout.contains('\0')
        && stdout.lines().skip(1).all(|line| {
            line.is_empty()
                || line.starts_with("|dep ")
                || line.starts_with("|owner ")
                || line.starts_with("|note ")
                || line.starts_with("|next ")
        })
}

pub(crate) fn render_search_packet_bytes(packet_bytes: Bytes) -> Option<Bytes> {
    if packet_bytes.is_empty() || packet_bytes.len() as u64 > MAX_CACHE_REPLAY_ARTIFACT_BYTES {
        return None;
    }
    let output =
        run_graph_render_packet_bytes(packet_bytes.clone(), MAX_CACHE_REPLAY_ARTIFACT_BYTES)?;
    let output = output_with_delegation_hint_lines(output, &packet_bytes);
    if !search_output_artifact_replay_safe(&output) {
        return None;
    }
    Some(output)
}

pub(crate) fn render_search_packet_bytes_with_receipt(
    packet_bytes: Bytes,
    receipt: &SearchFrontierReceiptRequest,
) -> Result<Option<Bytes>, String> {
    if packet_bytes.is_empty() || packet_bytes.len() as u64 > MAX_CACHE_REPLAY_ARTIFACT_BYTES {
        return Ok(None);
    }
    let output = run_graph_render_packet_bytes_with_receipt(
        packet_bytes.clone(),
        MAX_CACHE_REPLAY_ARTIFACT_BYTES,
        &GraphRenderReceiptRequest {
            out_path: receipt.out_path.clone(),
            receipt_id: receipt.receipt_id.clone(),
            task_fingerprint: receipt.task_fingerprint.clone(),
            command_fingerprint: receipt.command_fingerprint.clone(),
        },
    )?;
    let Some(output) = output else {
        return Ok(None);
    };
    if !search_output_artifact_replay_safe(&output) {
        return Ok(None);
    }
    Ok(Some(output_with_delegation_hint_lines(
        output,
        &packet_bytes,
    )))
}

pub(crate) fn render_search_packet_artifact_stdout(artifact_path: &Path) -> Option<Bytes> {
    let metadata = fs::metadata(artifact_path).ok()?;
    if !metadata.is_file() || metadata.len() > MAX_CACHE_REPLAY_ARTIFACT_BYTES {
        return None;
    }
    run_graph_render_packet(artifact_path, MAX_CACHE_REPLAY_ARTIFACT_BYTES)
}

pub(crate) fn output_with_delegation_hint_lines(output: Bytes, packet_bytes: &[u8]) -> Bytes {
    if bytes_contains(&output, b"subagentHint=") {
        return output;
    }
    let lines = delegation_hint_lines(packet_bytes);
    if lines.is_empty() {
        return output;
    }
    let mut rendered = Vec::with_capacity(
        output.len() + lines.iter().map(|line| line.len() + 1).sum::<usize>() + 1,
    );
    rendered.extend_from_slice(&output);
    if !rendered.ends_with(b"\n") {
        rendered.push(b'\n');
    }
    for line in lines {
        rendered.extend_from_slice(line.as_bytes());
        rendered.push(b'\n');
    }
    Bytes::from(rendered)
}

fn delegation_hint_lines(packet_bytes: &[u8]) -> Vec<String> {
    let Ok(packet) = serde_json::from_slice::<Value>(packet_bytes) else {
        return Vec::new();
    };
    packet
        .get("delegationHints")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(delegation_hint_line)
        .collect()
}

fn delegation_hint_line(hint: &Value) -> Option<String> {
    if string_field(hint, "decision")? != "advisory"
        || string_field(hint, "runtimeOwner")? != "agent-client"
        || !bool_field(hint, "readOnly")?
        || !bool_field(hint, "noCode")?
    {
        return None;
    }
    let receipt = hint.get("receipt")?;
    if string_field(receipt, "kind")? != "search-subagent" {
        return None;
    }
    let profile = safe_token(string_field(hint, "profile")?)?;
    let model_class = model_class_or_default(hint)?;
    let target_actions = safe_string_array(hint.get("targetActions")?, safe_action_token)?;
    let required_fields = safe_string_array(receipt.get("requiredFields")?, safe_token)?;
    let max_commands = positive_u64_or_default(hint, "maxCommands", 8)?;
    let max_turns = positive_u64_or_default(hint, "maxTurns", 1)?;
    let reason = safe_token(string_field(hint, "reason")?)?;

    Some(format!(
        "subagentHint=profile={profile} decision=advisory runtimeOwner=agent-client modelClass={model_class} readOnly=true noCode=true targetActions={} maxCommands={max_commands} maxTurns={max_turns} receipt=search-subagent({}) reason={reason}",
        target_actions.join(","),
        required_fields.join(",")
    ))
}

fn bytes_contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

fn string_field<'a>(value: &'a Value, field: &str) -> Option<&'a str> {
    value.get(field).and_then(Value::as_str)
}

fn bool_field(value: &Value, field: &str) -> Option<bool> {
    value.get(field).and_then(Value::as_bool)
}

fn model_class_or_default(value: &Value) -> Option<&str> {
    match value.get("modelClass") {
        None => Some("cheap"),
        Some(Value::String(model_class))
            if matches!(
                model_class.as_str(),
                "cheap" | "standard" | "strong" | "inherit" | "custom"
            ) =>
        {
            Some(model_class)
        }
        Some(_) => None,
    }
}

fn positive_u64_or_default(value: &Value, field: &str, default: u64) -> Option<u64> {
    match value.get(field) {
        None => Some(default),
        Some(value) => value.as_u64().filter(|value| *value > 0),
    }
}

fn safe_string_array(value: &Value, token_filter: fn(&str) -> Option<&str>) -> Option<Vec<String>> {
    let tokens = value
        .as_array()?
        .iter()
        .map(Value::as_str)
        .map(|token| token.and_then(token_filter).map(str::to_string))
        .collect::<Option<Vec<_>>>()?;
    (!tokens.is_empty()).then_some(tokens)
}

fn safe_token(value: &str) -> Option<&str> {
    value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-'))
        .then_some(value)
}

fn safe_action_token(value: &str) -> Option<&str> {
    value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.'))
        .then_some(value)
}
