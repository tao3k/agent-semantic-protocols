//! Search packet replay safety and hint projection.

use bytes::Bytes;
use serde_json::Value;

/// Return whether rendered search stdout is safe to replay from cache.
pub fn search_output_artifact_replay_safe(stdout: &[u8]) -> bool {
    let Ok(stdout) = std::str::from_utf8(stdout) else {
        return false;
    };
    let Some(header) = stdout.lines().next() else {
        return false;
    };
    if header.starts_with("[search-deps]") {
        return deps_search_output_replay_safe(stdout);
    }
    let has_frontier_header = header.contains("[graph-frontier]")
        || header.contains("alg=seed-frontier")
        || (header.starts_with("[search-prime]")
            && stdout.contains("|decision purpose=decision-primer"));
    if header.starts_with("[search-prime]") && !stdout.contains("|decision purpose=decision-primer")
    {
        return false;
    }
    let has_alias_graph = stdout.contains("aliases=") || stdout.contains("aliases:");
    has_frontier_header
        && has_alias_graph
        && stdout.contains("legend: ID=kind:role(value)!next;")
        && stdout.contains("frontier ID.next")
        && !stdout.contains('\0')
}

/// Insert or canonicalize safe delegation hint lines from a search packet.
pub fn output_with_delegation_hint_lines(output: Bytes, packet_bytes: &[u8]) -> Bytes {
    let lines = delegation_hint_lines(packet_bytes);
    if lines.is_empty() {
        return output;
    }
    if bytes_contains(&output, b"subagentHint=") {
        let Ok(existing) = std::str::from_utf8(&output) else {
            return output;
        };
        let mut rendered_lines = Vec::new();
        let mut replaced = false;
        for line in existing.lines() {
            if line.starts_with("subagentHint=") {
                if !replaced {
                    rendered_lines.extend(lines.iter().cloned());
                    replaced = true;
                }
            } else {
                rendered_lines.push(line.to_string());
            }
        }
        if !replaced {
            rendered_lines.extend(lines);
        }
        let mut rendered = rendered_lines.join("\n").into_bytes();
        if existing.ends_with('\n') {
            rendered.push(b'\n');
        }
        return Bytes::from(rendered);
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

fn deps_search_output_replay_safe(stdout: &str) -> bool {
    !stdout.contains('\0')
        && stdout.lines().skip(1).all(|line| {
            line.is_empty()
                || line.starts_with("|dep ")
                || line.starts_with("|dependency ")
                || line.starts_with("|hit ")
                || line.starts_with("|owner ")
                || line.starts_with("|note ")
                || line.starts_with("|next ")
        })
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
    if string_field(receipt, "kind")? != "asp-search-subagent" {
        return None;
    }
    let profile = safe_token(string_field(hint, "profile")?)?;
    let _mode = literal_or_default(hint, "mode", "resident", &["resident"])?;
    let _instances = literal_or_default(hint, "instances", "single", &["single", "targetActions"])?;
    let _reuse = literal_or_default(hint, "reuse", "send_input", &["send_input"])?;
    let _spawn = literal_or_default(hint, "spawn", "if-missing", &["if-missing"])?;
    if bool_or_default(hint, "forkContext", false)? {
        return None;
    }
    let branch_prompt =
        literal_or_default(hint, "branchPrompt", "reasoning-tree", &["reasoning-tree"])?;
    let state_owner = literal_or_default(hint, "stateOwner", "parent", &["parent"])?;
    let fanin = literal_or_default(hint, "fanin", "receipt", &["receipt"])?;
    let iterative = bool_or_default(hint, "iterative", true)?;
    let model_class = model_class_or_default(hint)?;
    let target_actions = safe_string_array(hint.get("targetActions")?, safe_action_token)?;
    let required_fields = safe_string_array(receipt.get("requiredFields")?, safe_token)?;
    let max_commands = positive_u64_or_default(hint, "maxCommands", 8)?;
    let max_turns = positive_u64_or_default(hint, "maxTurns", 1)?;
    let reason = safe_token(string_field(hint, "reason")?)?;

    Some(format!(
        "subagentHint=profile={profile} mode=resident instances=single reuse=send_input spawn=if-missing forkContext=false branchPrompt={branch_prompt} stateOwner={state_owner} fanin={fanin} iterative={iterative} decision=advisory runtimeOwner=agent-client modelClass={model_class} readOnly=true noCode=true targetActions={} maxCommands={max_commands} maxTurns={max_turns} receipt=asp-search-subagent({}) reason={reason}",
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

fn literal_or_default<'a>(
    value: &'a Value,
    field: &str,
    default: &'static str,
    allowed: &[&'static str],
) -> Option<&'a str> {
    match value.get(field) {
        None => Some(default),
        Some(Value::String(value)) if allowed.contains(&value.as_str()) => Some(value.as_str()),
        Some(_) => None,
    }
}

fn bool_or_default(value: &Value, field: &str, default: bool) -> Option<bool> {
    match value.get(field) {
        None => Some(default),
        Some(value) => value.as_bool(),
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
