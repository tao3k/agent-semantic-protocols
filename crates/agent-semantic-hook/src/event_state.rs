//! Append-only hook event state written by `asp hook`.

use crate::command::semantic_shell_tokens;
use fs2::FileExt;
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use agent_semantic_runtime::{ensure_project_hook_cache_dir, ensure_project_hook_state_dir};
use serde_json::{Value, json};

use crate::protocol::{DecisionKind, HOOK_PROTOCOL_ID, HookDecision};

pub(crate) const HOOK_EVENT_STATE_FILE: &str = "events.jsonl";
const HOOK_EVENT_SCHEMA_ID: &str = "agent.semantic-protocols.hook.event";
const DENY_REPLAY_WINDOW_MS: u128 = 5 * 60 * 1000;
const SEARCH_PIPE_FEEDBACK_WINDOW_MS: u128 = 10 * 60 * 1000;
const HOOK_EVENT_STATE_TAIL_BYTES: u64 = 1024 * 1024;
const HOOK_EVENT_STATE_TAIL_LINE_CAP: usize = 4096;
const HOOK_EVENT_STATE_MAX_BYTES: u64 = HOOK_EVENT_STATE_TAIL_BYTES * 4;

/// Recent search state for a prompt/session that needs `search pipe`.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct SearchPipeFeedback {
    pub(crate) language_id: String,
    pub(crate) saw_pipe: bool,
}

/// ASP command stage that matters for prompt search-flow feedback.
#[derive(Debug, Eq, PartialEq)]
pub(crate) enum AspSearchCommandStage {
    Prime(String),
    Pipe(String),
}

/// Convert a repeated deny in the same source-access lane into a compact replay.
pub fn apply_repeated_deny_replay(
    project_root: &Path,
    decision: &mut HookDecision,
) -> Result<bool, String> {
    let Some(replay_key) = deny_replay_key(decision) else {
        return Ok(false);
    };
    decision.fields.insert(
        "denyReplayKey".to_string(),
        Value::String(replay_key.clone()),
    );

    if !has_recent_matching_deny(project_root, &replay_key)? {
        decision.fields.insert(
            "denyReplay".to_string(),
            Value::String("record".to_string()),
        );
        return Ok(false);
    }

    decision.fields.insert(
        "denyReplay".to_string(),
        Value::String("repeated".to_string()),
    );
    decision.message = repeated_deny_message(decision);
    Ok(true)
}

/// Append one compact hook decision record to `events.jsonl`.
pub fn append_hook_event_state(
    project_root: &Path,
    decision: &HookDecision,
) -> Result<PathBuf, String> {
    let state_dir = ensure_project_hook_state_dir(project_root)?;
    let state_path = state_dir.join(HOOK_EVENT_STATE_FILE);
    let event = json!({
        "schemaId": HOOK_EVENT_SCHEMA_ID,
        "schemaVersion": "1",
        "protocolId": decision.protocol_id,
        "protocolVersion": decision.protocol_version,
        "recordedAtUnixMs": unix_time_ms(),
        "platform": decision.platform,
        "event": decision.event,
        "decision": decision.decision,
        "reasonKind": decision.reason_kind,
        "languageIds": decision.language_ids,
        "subject": decision.subject,
        "routeKinds": decision.routes.iter().map(|route| route.kind).collect::<Vec<_>>(),
        "fields": decision.fields,
        "denyReplayKey": decision.fields.get("denyReplayKey"),
    });
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .read(true)
        .open(&state_path)
        .map_err(|error| {
            format!(
                "failed to open hook state {}: {error}",
                state_path.display()
            )
        })?;
    file.lock_exclusive().map_err(|error| {
        format!(
            "failed to lock hook state {}: {error}",
            state_path.display()
        )
    })?;
    if file
        .metadata()
        .map_err(|error| {
            format!(
                "failed to stat hook state {}: {error}",
                state_path.display()
            )
        })?
        .len()
        > HOOK_EVENT_STATE_MAX_BYTES
    {
        file.set_len(0).map_err(|error| {
            format!(
                "failed to truncate hook state {}: {error}",
                state_path.display()
            )
        })?;
        file.seek(SeekFrom::Start(0)).map_err(|error| {
            format!(
                "failed to seek hook state {}: {error}",
                state_path.display()
            )
        })?;
    }
    let mut line = event.to_string();
    line.push('\n');
    file.write_all(line.as_bytes()).map_err(|error| {
        format!(
            "failed to write hook state {}: {error}",
            state_path.display()
        )
    })?;
    file.flush().map_err(|error| {
        format!(
            "failed to flush hook state {}: {error}",
            state_path.display()
        )
    })?;
    file.unlock().map_err(|error| {
        format!(
            "failed to unlock hook state {}: {error}",
            state_path.display()
        )
    })?;
    Ok(state_path)
}

/// Return whether the latest matching subagent lifecycle event marks this hook
/// payload as running inside a subagent.
pub fn has_recorded_subagent_context(
    project_root: &Path,
    session_id: Option<&str>,
    transcript_path: Option<&str>,
) -> Result<bool, String> {
    if session_id.is_none() && transcript_path.is_none() {
        return Ok(false);
    }
    let state_path = ensure_project_hook_state_dir(project_root)?.join(HOOK_EVENT_STATE_FILE);
    if !state_path.is_file() {
        return Ok(false);
    }
    let lines = read_hook_event_state_tail(&state_path)?;
    for line in lines.iter().rev() {
        let Ok(event) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if !matches_subagent_identity(&event, session_id, transcript_path) {
            continue;
        }
        return Ok(event.get("event").and_then(Value::as_str) == Some("subagent-start"));
    }
    Ok(false)
}

/// Return feedback when a prompt/session has run `search prime` but no pipe.
pub(crate) fn missing_search_pipe_after_prime(
    project_root: &Path,
    session_id: Option<&str>,
    transcript_path: Option<&str>,
) -> Result<Option<SearchPipeFeedback>, String> {
    Ok(
        prompt_search_flow_after_prime(project_root, session_id, transcript_path)?
            .filter(|feedback| !feedback.saw_pipe),
    )
}

/// Return recent prompt/session search-flow state after prime or pipe has run.
pub(crate) fn prompt_search_flow_after_prime(
    project_root: &Path,
    session_id: Option<&str>,
    transcript_path: Option<&str>,
) -> Result<Option<SearchPipeFeedback>, String> {
    if session_id.is_none() && transcript_path.is_none() {
        return Ok(None);
    }
    let state_path = ensure_project_hook_state_dir(project_root)?.join(HOOK_EVENT_STATE_FILE);
    if !state_path.is_file() {
        return Ok(None);
    }
    let now = unix_time_ms();
    let lines = read_hook_event_state_tail(&state_path)?;
    let mut prime_language_id = None;
    let mut saw_pipe = false;
    for line in lines.iter().rev() {
        let Ok(event) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if !is_recent_for_window(&event, now, SEARCH_PIPE_FEEDBACK_WINDOW_MS) {
            break;
        }
        if !event_matches_prompt_scope(&event, session_id, transcript_path) {
            continue;
        }
        if is_prompt_scope_boundary(&event) {
            break;
        }
        let Some(command) = event.pointer("/subject/command").and_then(Value::as_str) else {
            continue;
        };
        match asp_search_stage(command) {
            Some(AspSearchCommandStage::Pipe(language_id)) => {
                saw_pipe = true;
                prime_language_id.get_or_insert(language_id);
            }
            Some(AspSearchCommandStage::Prime(language_id)) => {
                prime_language_id.get_or_insert(language_id);
            }
            None => {}
        }
    }
    Ok(prime_language_id.map(|language_id| SearchPipeFeedback {
        language_id,
        saw_pipe,
    }))
}

/// Count ASP commands that completed in the current prompt/session.
pub(crate) fn prompt_asp_command_count(
    project_root: &Path,
    session_id: Option<&str>,
    transcript_path: Option<&str>,
) -> Result<usize, String> {
    if session_id.is_none() && transcript_path.is_none() {
        return Ok(0);
    }
    let state_path = ensure_project_hook_state_dir(project_root)?.join(HOOK_EVENT_STATE_FILE);
    if !state_path.is_file() {
        return Ok(0);
    }
    let now = unix_time_ms();
    let mut count = 0;
    let lines = read_hook_event_state_tail(&state_path)?;
    for event in lines
        .iter()
        .rev()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
    {
        if !is_recent_for_window(&event, now, SEARCH_PIPE_FEEDBACK_WINDOW_MS) {
            break;
        }
        if !event_matches_prompt_scope(&event, session_id, transcript_path) {
            continue;
        }
        if is_prompt_scope_boundary(&event) {
            break;
        }
        if event.get("event").and_then(Value::as_str) != Some("post-tool") {
            continue;
        }
        let Some(command) = event.pointer("/subject/command").and_then(Value::as_str) else {
            continue;
        };
        if asp_command(command) {
            count += 1;
        }
    }
    Ok(count)
}

/// Remove cached hook event state when it belongs to an older hook protocol.
pub fn remove_incompatible_hook_event_state(
    project_root: &Path,
) -> Result<Option<PathBuf>, String> {
    let state_path = ensure_project_hook_state_dir(project_root)?.join(HOOK_EVENT_STATE_FILE);
    let mut removed_path = remove_incompatible_hook_event_state_path(&state_path)?;
    let previous_cache_state_path =
        ensure_project_hook_cache_dir(project_root)?.join(HOOK_EVENT_STATE_FILE);
    if previous_cache_state_path != state_path && previous_cache_state_path.is_file() {
        fs::remove_file(&previous_cache_state_path).map_err(|error| {
            format!(
                "failed to remove previous hook state {}: {error}",
                previous_cache_state_path.display()
            )
        })?;
        removed_path.get_or_insert(previous_cache_state_path);
    }
    Ok(removed_path)
}

fn remove_incompatible_hook_event_state_path(state_path: &Path) -> Result<Option<PathBuf>, String> {
    if !state_path.is_file() {
        return Ok(None);
    }
    let file = fs::File::open(state_path).map_err(|error| {
        format!(
            "failed to read hook state {}: {error}",
            state_path.display()
        )
    })?;
    let mut reader = BufReader::new(file);
    let mut line = String::new();
    loop {
        line.clear();
        let bytes = reader.read_line(&mut line).map_err(|error| {
            format!(
                "failed to read hook state {}: {error}",
                state_path.display()
            )
        })?;
        if bytes == 0 {
            return Ok(None);
        }
        if line.trim().is_empty() {
            continue;
        }
        if is_current_hook_event_state_line(&line) {
            return Ok(None);
        }
        break;
    }
    fs::remove_file(state_path).map_err(|error| {
        format!(
            "failed to remove hook state {}: {error}",
            state_path.display()
        )
    })?;
    Ok(Some(state_path.to_path_buf()))
}

fn is_current_hook_event_state_line(line: &str) -> bool {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
        return false;
    };
    value.get("schemaId").and_then(serde_json::Value::as_str) == Some(HOOK_EVENT_SCHEMA_ID)
        && value.get("protocolId").and_then(serde_json::Value::as_str) == Some(HOOK_PROTOCOL_ID)
}

fn matches_subagent_identity(
    event: &Value,
    session_id: Option<&str>,
    transcript_path: Option<&str>,
) -> bool {
    let event_name = event.get("event").and_then(Value::as_str);
    if !matches!(event_name, Some("subagent-start" | "subagent-stop")) {
        return false;
    }
    let fields = event.get("fields").unwrap_or(event);
    session_id.is_some_and(|session_id| {
        field_string(fields, &["sessionId", "session_id"]).is_some_and(|value| value == session_id)
    }) || transcript_path.is_some_and(|transcript_path| {
        field_string(fields, &["transcriptPath", "transcript_path"])
            .is_some_and(|value| value == transcript_path)
    })
}

fn field_string<'a>(value: &'a Value, fields: &[&str]) -> Option<&'a str> {
    fields
        .iter()
        .find_map(|field| value.get(*field).and_then(Value::as_str))
}

fn unix_time_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}

fn has_recent_matching_deny(project_root: &Path, replay_key: &str) -> Result<bool, String> {
    let state_path = ensure_project_hook_state_dir(project_root)?.join(HOOK_EVENT_STATE_FILE);
    if !state_path.is_file() {
        return Ok(false);
    }
    let now = unix_time_ms();
    let replay_key_json = serde_json::to_string(replay_key)
        .map_err(|error| format!("failed to encode hook replay key: {error}"))?;
    let lines = read_hook_event_state_tail(&state_path)?;
    for line in lines.iter().rev() {
        if !line.contains(&replay_key_json) {
            continue;
        }
        let Ok(event) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if !is_recent_event(&event, now) {
            break;
        }
        if event.get("decision").and_then(Value::as_str) == Some("deny")
            && event.get("denyReplayKey").and_then(Value::as_str) == Some(replay_key)
        {
            return Ok(true);
        }
    }
    Ok(false)
}

fn read_hook_event_state_tail(state_path: &Path) -> Result<Vec<String>, String> {
    let mut file = fs::File::open(state_path).map_err(|error| {
        format!(
            "failed to read hook state {}: {error}",
            state_path.display()
        )
    })?;
    let file_len = file
        .metadata()
        .map_err(|error| {
            format!(
                "failed to stat hook state {}: {error}",
                state_path.display()
            )
        })?
        .len();
    let start = file_len.saturating_sub(HOOK_EVENT_STATE_TAIL_BYTES);
    file.seek(SeekFrom::Start(start)).map_err(|error| {
        format!(
            "failed to seek hook state {}: {error}",
            state_path.display()
        )
    })?;

    let mut content = String::new();
    file.read_to_string(&mut content).map_err(|error| {
        format!(
            "failed to read hook state {}: {error}",
            state_path.display()
        )
    })?;

    let mut lines = content.lines().collect::<Vec<_>>();
    if start > 0 && !lines.is_empty() {
        lines.remove(0);
    }
    let first_line = lines.len().saturating_sub(HOOK_EVENT_STATE_TAIL_LINE_CAP);
    Ok(lines[first_line..]
        .iter()
        .map(|line| (*line).to_string())
        .collect())
}

fn is_recent_event(event: &Value, now: u128) -> bool {
    is_recent_for_window(event, now, DENY_REPLAY_WINDOW_MS)
}

fn is_recent_for_window(event: &Value, now: u128, window_ms: u128) -> bool {
    let Some(recorded_at) = event.get("recordedAtUnixMs").and_then(Value::as_u64) else {
        return false;
    };
    now.saturating_sub(u128::from(recorded_at)) <= window_ms
}

fn event_matches_prompt_scope(
    event: &Value,
    session_id: Option<&str>,
    transcript_path: Option<&str>,
) -> bool {
    let fields = event.get("fields").unwrap_or(event);
    let session_matches = session_id
        .is_some_and(|expected| fields.get("sessionId").and_then(Value::as_str) == Some(expected));
    let transcript_matches = transcript_path.is_some_and(|expected| {
        fields.get("transcriptPath").and_then(Value::as_str) == Some(expected)
    });
    session_matches || transcript_matches
}

fn is_prompt_scope_boundary(event: &Value) -> bool {
    event.get("event").and_then(Value::as_str) == Some("user-prompt")
}

/// Classify an ASP search command into prime/pipe stages.
pub(crate) fn asp_search_stage(command: &str) -> Option<AspSearchCommandStage> {
    let tokens = semantic_shell_tokens(command);
    asp_search_stage_tokens(&tokens)
}

pub(crate) fn asp_search_stage_tokens(tokens: &[String]) -> Option<AspSearchCommandStage> {
    let asp_index = asp_token_index(tokens)?;
    let after_asp = &tokens[asp_index + 1..];
    if after_asp.first().map(String::as_str) == Some("search") {
        let language_id = language_from_flags(after_asp)?;
        return search_stage_from_tokens(after_asp, language_id);
    }
    let language_id = after_asp.first()?.to_string();
    if after_asp.get(1).map(String::as_str) != Some("search") {
        return None;
    }
    search_stage_from_tokens(&after_asp[1..], language_id)
}

pub(crate) fn asp_query_code_or_direct_read_tokens(tokens: &[String]) -> bool {
    let Some(asp_index) = asp_token_index(tokens) else {
        return false;
    };
    let after_asp = &tokens[asp_index + 1..];
    let query_tokens = if after_asp.first().map(String::as_str) == Some("query") {
        after_asp
    } else if after_asp.get(1).map(String::as_str) == Some("query") {
        &after_asp[1..]
    } else {
        return false;
    };
    query_tokens.iter().any(|token| token == "--code")
        || query_tokens
            .windows(2)
            .any(|pair| pair[0] == "--from-hook" && pair[1] == "direct-source-read")
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AspDirectSourceReadShape {
    Bounded { line_span: usize },
    Unbounded,
}

pub(crate) fn asp_query_direct_source_read_shape_tokens(
    tokens: &[String],
) -> Option<AspDirectSourceReadShape> {
    let asp_index = asp_token_index(tokens)?;
    let after_asp = &tokens[asp_index + 1..];
    let query_tokens = if after_asp.first().map(String::as_str) == Some("query") {
        after_asp
    } else if after_asp.get(1).map(String::as_str) == Some("query") {
        &after_asp[1..]
    } else {
        return None;
    };
    if !query_tokens
        .windows(2)
        .any(|pair| pair[0] == "--from-hook" && pair[1] == "direct-source-read")
    {
        return None;
    }
    let Some(selector) = option_value(query_tokens, "--selector") else {
        return Some(AspDirectSourceReadShape::Unbounded);
    };
    selector_line_span(selector)
        .map(|line_span| AspDirectSourceReadShape::Bounded { line_span })
        .or(Some(AspDirectSourceReadShape::Unbounded))
}

fn selector_line_span(selector: &str) -> Option<usize> {
    parse_colon_line_span(selector).or_else(|| parse_dash_line_span(selector))
}

fn parse_colon_line_span(selector: &str) -> Option<usize> {
    let (path_or_start, end_text) = selector.rsplit_once(':')?;
    let end = end_text.parse::<usize>().ok()?;
    let Some((_, start_text)) = path_or_start.rsplit_once(':') else {
        return (end > 0).then_some(1);
    };
    let start = start_text.parse::<usize>().ok()?;
    valid_line_span(start, end)
}

fn parse_dash_line_span(selector: &str) -> Option<usize> {
    let (_, range_text) = selector.rsplit_once(':')?;
    let (start_text, end_text) = range_text.split_once('-')?;
    let start = start_text.parse::<usize>().ok()?;
    let end = end_text.parse::<usize>().ok()?;
    valid_line_span(start, end)
}

fn valid_line_span(start: usize, end: usize) -> Option<usize> {
    (start > 0 && end >= start).then_some(end - start + 1)
}

fn option_value<'a>(args: &'a [String], option: &str) -> Option<&'a str> {
    args.windows(2).find_map(|window| {
        if window[0] == option {
            Some(window[1].as_str())
        } else {
            None
        }
    })
}

/// Return true when a shell command invokes ASP.
pub(crate) fn asp_command(command: &str) -> bool {
    let tokens = semantic_shell_tokens(command);
    asp_command_tokens(&tokens)
}

pub(crate) fn asp_command_tokens(tokens: &[String]) -> bool {
    asp_token_index(tokens).is_some()
}

fn asp_token_index(tokens: &[String]) -> Option<usize> {
    tokens
        .iter()
        .position(|token| token == "asp" || token.ends_with("/asp") || token.ends_with(".bin/asp"))
}

fn search_stage_from_tokens(
    tokens: &[String],
    language_id: String,
) -> Option<AspSearchCommandStage> {
    if !tokens.iter().any(|token| token == "search") {
        return None;
    }
    let stage = tokens
        .iter()
        .find_map(|token| matches!(token.as_str(), "prime" | "pipe").then_some(token.as_str()))?;
    if stage == "prime" {
        return Some(AspSearchCommandStage::Prime(language_id));
    }
    if stage == "pipe" {
        return Some(AspSearchCommandStage::Pipe(language_id));
    }
    None
}

fn language_from_flags(tokens: &[String]) -> Option<String> {
    tokens.windows(2).find_map(|pair| {
        (pair[0] == "--language")
            .then(|| pair[1].clone())
            .filter(|value| !value.starts_with('-'))
    })
}

fn deny_replay_key(decision: &HookDecision) -> Option<String> {
    if decision.decision != DecisionKind::Deny {
        return None;
    }
    let reason = serde_json::to_value(decision.reason_kind).ok()?;
    let mut language_ids = decision.language_ids.clone();
    language_ids.sort();
    language_ids.dedup();
    let routes = decision
        .routes
        .iter()
        .map(|route| {
            json!({
                "languageId": route.language_id,
                "providerId": route.provider_id,
                "kind": route.kind,
                "argv": route.argv,
            })
        })
        .collect::<Vec<_>>();
    let subject = if routes.is_empty() {
        serde_json::to_value(&decision.subject).unwrap_or(Value::Null)
    } else {
        Value::Null
    };
    let key = json!({
        "platform": decision.platform,
        "reasonKind": reason,
        "languageIds": language_ids,
        "operationIntent": decision.fields.get("operationIntent").cloned().unwrap_or(Value::Null),
        "toolSurface": decision.fields.get("toolSurface").cloned().unwrap_or(Value::Null),
        "sessionId": decision.fields.get("sessionId").cloned().unwrap_or(Value::Null),
        "transcriptPath": decision.fields.get("transcriptPath").cloned().unwrap_or(Value::Null),
        "routes": routes,
        "subject": subject,
    });
    serde_json::to_string(&key).ok()
}

fn repeated_deny_message(decision: &HookDecision) -> String {
    let reason = serde_json::to_value(decision.reason_kind)
        .ok()
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_else(|| "source-access".to_string());
    [
        format!("ASP hook already denied `{reason}` on this source-access lane."),
        "See @.agents/skills/agent-semantic-protocols/SKILL.md for the active ASP agent workflow."
            .to_string(),
        String::new(),
        "## ASP Hook Recovery".to_string(),
        "Follow the previous recovery route instead of retrying raw source tools.".to_string(),
        String::new(),
        "## Stop".to_string(),
        "Do not retry `Read`, `cat`, `sed`, `rg`, or source-dump commands on the matched source. The hook has already denied this lane."
            .to_string(),
    ]
    .join("\n")
}
