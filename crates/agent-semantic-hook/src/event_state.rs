// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Append-only hook event state persisted by `asp hook`.

use std::collections::BTreeSet;
use std::fs::File;
use std::fs::OpenOptions;
use std::fs::{self};
use std::io::BufRead;
use std::io::BufReader;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;
use std::time::Instant;

use agent_semantic_runtime::ensure_project_hook_state_dir;
use fs2::FileExt;
use serde_json::Value;
use serde_json::json;

use crate::ReaderProbeAccess;
use crate::ReaderProbeObservation;
use crate::event_replay::compact_source_access_deny_message;
use crate::event_replay::deny_replay_key;
use crate::event_replay::is_source_access_replay_key;
use crate::event_replay::recovery_ref_for_replay_key;
use crate::event_replay::repeated_deny_message;
use crate::event_replay::should_compact_source_access_deny_message;
use crate::protocol::HOOK_PROTOCOL_ID;
use crate::protocol::HookDecision;

#[path = "event_state_parts/replay_window.rs"]
mod replay_window;
pub(crate) use replay_window::read_hook_event_state_tail;
use replay_window::{
    event_matches_prompt_scope, has_recent_matching_deny, is_current_hook_event_state_line,
    is_prompt_scope_boundary, is_recent_for_window, unix_time_ms,
};

pub(crate) const HOOK_EVENT_STATE_FILE: &str = "events.jsonl";
const PROMPT_SCOPE_WINDOW_MS: u128 = 10 * 60 * 1000;
const HOOK_EVENT_SCHEMA_ID: &str = "agent.semantic-protocols.hook.event";
const DENY_REPLAY_WINDOW_MS: u128 = 3 * 60 * 1000;
const HOOK_EVENT_STATE_TAIL_BYTES: u64 = 1024 * 1024;
const HOOK_EVENT_STATE_TAIL_LINE_CAP: usize = 4096;
const HOOK_EVENT_STATE_LOCK_TIMEOUT: Duration = Duration::from_millis(100);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HookEventSessionId(String);

impl HookEventSessionId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for HookEventSessionId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for HookEventSessionId {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HookEventTranscriptPath(String);

impl HookEventTranscriptPath {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for HookEventTranscriptPath {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for HookEventTranscriptPath {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HookEventStateError(String);

impl std::fmt::Display for HookEventStateError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for HookEventStateError {}

impl From<String> for HookEventStateError {
    fn from(value: String) -> Self {
        Self(value)
    }
}

/// Policy selection recorded by a denied Hook event.
///
/// Resident identity is intentionally absent. `asp session` resolves the
/// selected rule's stable Agent route key through the current managed config
/// and agent registry, so a Hook receipt cannot become a second registry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HookSessionAgentRoute {
    pub command_digest: Option<String>,
    pub config_rule_id: String,
    pub deny_evidence_ref: Option<String>,
    pub reason_kind: String,
    pub root_session_id: String,
    pub subject_command: Option<String>,
}

fn should_preserve_parser_route_message(decision: &HookDecision) -> bool {
    !decision.routes.is_empty()
        || decision.has_registered_agent_dispatch()
        || decision.fields.contains_key("agentSessionAction")
            && decision.fields.contains_key("agentSessionRoute")
}
const HOOK_EVENT_STATE_MAX_BYTES: u64 = HOOK_EVENT_STATE_TAIL_BYTES * 4;

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
    let recovery_ref = recovery_ref_for_replay_key(&replay_key);
    decision.fields.insert(
        "recoveryRef".to_string(),
        Value::String(recovery_ref.clone()),
    );
    let source_access_replay = is_source_access_replay_key(&replay_key);
    let preserve_parser_route_message = should_preserve_parser_route_message(decision);
    if source_access_replay && !preserve_parser_route_message {
        insert_collaboration_recovery_action_fields(decision);
    }
    let compact_first_source_access_replay =
        source_access_replay && should_compact_source_access_deny_message(decision);

    if !has_recent_matching_deny(project_root, &replay_key)? {
        decision.fields.insert(
            "denyReplay".to_string(),
            Value::String("record".to_string()),
        );
        if preserve_parser_route_message {
            decision.fields.insert(
                "denyReplayMessagePolicy".to_string(),
                Value::String("preserve-parser-route".to_string()),
            );
        } else if compact_first_source_access_replay {
            decision.message = compact_source_access_deny_message(decision, &recovery_ref);
        }
        return Ok(false);
    }

    decision.fields.insert(
        "denyReplay".to_string(),
        Value::String("repeated".to_string()),
    );
    if preserve_parser_route_message {
        decision.fields.insert(
            "denyReplayMessagePolicy".to_string(),
            Value::String("preserve-parser-route".to_string()),
        );
        return Ok(true);
    }
    decision.message = if source_access_replay {
        compact_source_access_deny_message(decision, &recovery_ref)
    } else {
        repeated_deny_message(decision)
    };
    Ok(true)
}

fn insert_collaboration_recovery_action_fields(decision: &mut HookDecision) {
    decision
        .fields
        .entry("requiredAction".to_string())
        .or_insert_with(|| Value::String("collaboration.spawn_agent".to_string()));
    decision
        .fields
        .entry("nextAction".to_string())
        .or_insert_with(|| Value::String("spawn-configured-agent".to_string()));
    decision
        .fields
        .entry("forbiddenUntilResolved".to_string())
        .or_insert_with(|| Value::String("raw-source-fallback".to_string()));
    decision
        .fields
        .entry("completionReceipt".to_string())
        .or_insert_with(|| Value::String("host-agent-action-receipt".to_string()));
    decision
        .fields
        .entry("collaborationNamespace".to_string())
        .or_insert_with(|| Value::String("collaboration".to_string()));
    decision
        .fields
        .entry("collaborationTool".to_string())
        .or_insert_with(|| Value::String("spawn_agent".to_string()));
}

/// Return the newest denied Hook policy selection for this root session.
pub fn latest_hook_session_agent_route(
    project_root: &Path,
) -> Result<Option<HookSessionAgentRoute>, String> {
    latest_hook_session_agent_route_for_root(project_root, None)
}

pub fn latest_hook_session_agent_route_for_root(
    project_root: &Path,
    root_session_id: Option<&str>,
) -> Result<Option<HookSessionAgentRoute>, String> {
    let state_path = ensure_project_hook_state_dir(project_root)?.join(HOOK_EVENT_STATE_FILE);
    if !state_path.is_file() {
        return Ok(None);
    }
    let lines = read_hook_event_state_tail(&state_path)?;
    Ok(latest_hook_session_agent_route_from_lines(
        &lines,
        root_session_id,
    ))
}

/// Read the newest route that still belongs to the current immutable Hook
/// configuration. Events from older generations remain audit evidence, but
/// cannot select a rule which the active configuration no longer declares.
pub fn latest_hook_session_agent_route_for_root_matching_rules(
    project_root: &Path,
    root_session_id: Option<&str>,
    current_rule_ids: &BTreeSet<String>,
) -> Result<Option<HookSessionAgentRoute>, String> {
    let state_path = ensure_project_hook_state_dir(project_root)?.join(HOOK_EVENT_STATE_FILE);
    if !state_path.is_file() {
        return Ok(None);
    }
    let lines = read_hook_event_state_tail(&state_path)?;
    Ok(latest_hook_session_agent_route_from_lines_matching(
        &lines,
        root_session_id,
        |route| current_rule_ids.contains(&route.config_rule_id),
    ))
}

fn latest_hook_session_agent_route_from_lines(
    lines: &[String],
    required_root_session_id: Option<&str>,
) -> Option<HookSessionAgentRoute> {
    latest_hook_session_agent_route_from_lines_matching(lines, required_root_session_id, |_| true)
}

fn latest_hook_session_agent_route_from_lines_matching(
    lines: &[String],
    required_root_session_id: Option<&str>,
    accepts: impl Fn(&HookSessionAgentRoute) -> bool,
) -> Option<HookSessionAgentRoute> {
    lines.iter().rev().find_map(|line| {
        let event = serde_json::from_str::<Value>(line).ok()?;
        if !matches!(
            event.get("decision").and_then(Value::as_str),
            Some("deny" | "block")
        ) || event
            .pointer("/fields/collaborationTool")
            .and_then(Value::as_str)
            != Some("spawn_agent")
            || event
                .pointer("/fields/collaborationNamespace")
                .and_then(Value::as_str)
                != Some("collaboration")
        {
            return None;
        }
        let root_session_id = event
            .pointer("/fields/hostRootSessionId")
            .or_else(|| event.pointer("/fields/sessionId"))
            .and_then(Value::as_str)?;
        if required_root_session_id.is_some_and(|required| required != root_session_id) {
            return None;
        }
        let route = HookSessionAgentRoute {
            command_digest: event
                .pointer("/fields/commandDigest")
                .and_then(Value::as_str)
                .map(str::to_owned),
            config_rule_id: required_event_string(&event, "/fields/configRuleId")?,
            deny_evidence_ref: event
                .pointer("/fields/recoveryRef")
                .or_else(|| event.pointer("/fields/denyEvidenceRef"))
                .and_then(Value::as_str)
                .map(str::to_owned),
            reason_kind: required_event_string(&event, "/reasonKind")?,
            root_session_id: root_session_id.to_owned(),
            subject_command: event
                .pointer("/subject/command")
                .and_then(Value::as_str)
                .map(str::to_owned),
        };
        accepts(&route).then_some(route)
    })
}

fn required_event_string(event: &Value, pointer: &str) -> Option<String> {
    event
        .pointer(pointer)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

/// Append one compact hook decision record to `events.jsonl`.
pub fn append_hook_event_state(
    project_root: &Path,
    decision: &HookDecision,
) -> Result<PathBuf, String> {
    append_hook_event_state_with_lock_timeout(project_root, decision, HOOK_EVENT_STATE_LOCK_TIMEOUT)
}

/// Try to project a decision without putting the authoritative Hook response
/// behind the diagnostic writer's normal contention budget.
pub fn try_append_hook_event_state(
    project_root: &Path,
    decision: &HookDecision,
) -> Result<PathBuf, String> {
    append_hook_event_state_with_lock_timeout(project_root, decision, Duration::ZERO)
}

/// Persist a dynamic Reader observation beside Hook decisions without placing
/// internal receipt fields on the Codex Host wire envelope.
///
/// A probe record deliberately uses the existing V1 Hook event identity: it
/// has the same retention, locking, and project authority as a policy
/// decision, while `fields.recordKind` keeps it out of route/replay selection.
pub fn append_reader_probe_event_state(
    project_root: &Path,
    state_home: Option<&Path>,
    host_matcher: &str,
    payload: &Value,
    observation: &ReaderProbeObservation,
    decision: Option<&crate::aot_evaluator::AotHookDecision<'_>>,
) -> Result<PathBuf, String> {
    let state_dir = match state_home {
        Some(state_home) => {
            let paths = agent_semantic_runtime::project_state_paths_with_state_home(
                project_root,
                state_home,
            )?;
            fs::create_dir_all(&paths.hook_state_dir).map_err(|error| {
                format!(
                    "create Reader probe Hook state {}: {error}",
                    paths.hook_state_dir.display()
                )
            })?;
            paths.hook_state_dir
        }
        None => ensure_project_hook_state_dir(project_root)?,
    };
    let state_path = state_dir.join(HOOK_EVENT_STATE_FILE);
    let writer_lock = acquire_event_state_writer(&state_dir, HOOK_EVENT_STATE_LOCK_TIMEOUT)?;
    let reader_probe = json!({
        "schemaId": "agent.semantic-protocols.reader-probe-observation",
        "schemaVersion": 1,
        "subject": observation.subject,
        "access": match observation.access {
            ReaderProbeAccess::Read => "read",
            ReaderProbeAccess::Unknown => "unknown",
        },
        "accessMode": match observation.access {
            ReaderProbeAccess::Read => "read-permission",
            ReaderProbeAccess::Unknown => "unknown",
        },
        "backend": observation.backend,
        "terminal": observation.terminal,
        "elapsedMicros": observation.elapsed_micros,
        "probeProcessLaunched": observation.probe_process_launched,
        "cleanupVerified": observation.cleanup_verified,
        "cacheHit": observation.cache_hit,
        "behaviorKey": observation.behavior_key,
    });
    let policy = decision
        .map(serde_json::to_value)
        .transpose()
        .map_err(|error| format!("encode Reader probe policy decision: {error}"))?
        .unwrap_or_else(|| json!({ "decision": "allow", "state": "no-matching-rule" }));
    let event = json!({
        "schemaId": HOOK_EVENT_SCHEMA_ID,
        "schemaVersion": "1",
        "protocolId": HOOK_PROTOCOL_ID,
        "protocolVersion": crate::protocol::HOOK_PROTOCOL_VERSION,
        "recordedAtUnixMs": unix_time_ms(),
        "platform": "codex",
        "event": "pre-tool",
        "decision": decision.map_or("allow", |value| value.decision),
        "reasonKind": decision.map_or("none", |value| value.reason_kind),
        "languageIds": decision
            .and_then(|value| value.language)
            .map(|language| vec![language])
            .unwrap_or_default(),
        "subject": { "path": observation.subject },
        "routeKinds": [],
        "fields": {
            "recordKind": "reader-probe-observation",
            "hostMatcher": host_matcher,
            "sessionId": payload.get("session_id").and_then(Value::as_str),
            "toolUseId": payload.get("tool_use_id").and_then(Value::as_str),
            "readerProbe": reader_probe,
            "policyDecision": policy,
        },
    });
    append_hook_event_value(&state_path, &event)?;
    FileExt::unlock(&writer_lock)
        .map_err(|error| format!("unlock Hook event writer {}: {error}", state_dir.display()))?;
    Ok(state_path)
}

fn append_hook_event_state_with_lock_timeout(
    project_root: &Path,
    decision: &HookDecision,
    lock_timeout: Duration,
) -> Result<PathBuf, String> {
    let state_dir = ensure_project_hook_state_dir(project_root)?;
    let state_path = state_dir.join(HOOK_EVENT_STATE_FILE);
    let writer_lock = acquire_event_state_writer(&state_dir, lock_timeout)?;
    let mut fields = decision.fields.clone();
    if decision.decision == crate::DecisionKind::Deny {
        fields
            .entry("denyEvidenceRef".to_owned())
            .or_insert_with(|| Value::String(state_path.display().to_string()));
    }
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
        "fields": fields,
        "denyReplayKey": decision.fields.get("denyReplayKey"),
    });
    append_hook_event_value(&state_path, &event)?;
    FileExt::unlock(&writer_lock)
        .map_err(|error| format!("unlock Hook event writer {}: {error}", state_dir.display()))?;
    Ok(state_path)
}

fn append_hook_event_value(state_path: &Path, event: &Value) -> Result<(), String> {
    let mut line = event.to_string();
    line.push('\n');
    let state_len = fs::metadata(state_path)
        .map(|metadata| metadata.len())
        .or_else(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                Ok(0)
            } else {
                Err(error)
            }
        })
        .map_err(|error| {
            format!(
                "failed to stat hook state {}: {error}",
                state_path.display()
            )
        })?;
    if state_len.saturating_add(line.len() as u64) > HOOK_EVENT_STATE_MAX_BYTES {
        let mut compacted = read_hook_event_state_tail(state_path)?
            .into_iter()
            .filter(|retained| is_current_hook_event_state_line(retained))
            .collect::<Vec<_>>()
            .join("\n");
        if !compacted.is_empty() {
            compacted.push('\n');
        }
        compacted.push_str(&line);
        replace_hook_event_state(state_path, compacted.as_bytes())?;
    } else {
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(state_path)
            .map_err(|error| {
                format!(
                    "failed to open hook state {}: {error}",
                    state_path.display()
                )
            })?;
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
    }
    Ok(())
}

fn acquire_event_state_writer(state_dir: &Path, lock_timeout: Duration) -> Result<File, String> {
    let lock_path = state_dir.join("events.jsonl.lock");
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(&lock_path)
        .map_err(|error| {
            format!(
                "open Hook event writer lock {}: {error}",
                lock_path.display()
            )
        })?;
    let started = Instant::now();
    loop {
        match lock.try_lock_exclusive() {
            Ok(()) => return Ok(lock),
            Err(error)
                if error.kind() == std::io::ErrorKind::WouldBlock
                    && started.elapsed() < lock_timeout =>
            {
                std::thread::sleep(Duration::from_millis(1));
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                return Err(format!(
                    "Hook event writer lock exceeded {}ms at {}",
                    lock_timeout.as_millis(),
                    lock_path.display()
                ));
            }
            Err(error) => {
                return Err(format!(
                    "lock Hook event writer {}: {error}",
                    lock_path.display()
                ));
            }
        }
    }
}

fn replace_hook_event_state(state_path: &Path, content: &[u8]) -> Result<(), String> {
    let file_name = state_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(HOOK_EVENT_STATE_FILE);
    let temporary_path = state_path.with_file_name(format!(".{file_name}.tmp"));
    let replace_result = (|| {
        let mut temporary = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&temporary_path)
            .map_err(|error| {
                format!(
                    "failed to open temporary hook state {}: {error}",
                    temporary_path.display()
                )
            })?;
        temporary.write_all(content).map_err(|error| {
            format!(
                "failed to write temporary hook state {}: {error}",
                temporary_path.display()
            )
        })?;
        temporary.sync_all().map_err(|error| {
            format!(
                "failed to sync temporary hook state {}: {error}",
                temporary_path.display()
            )
        })?;
        fs::rename(&temporary_path, state_path).map_err(|error| {
            format!(
                "failed to replace hook state {} from {}: {error}",
                state_path.display(),
                temporary_path.display()
            )
        })
    })();
    if replace_result.is_err() {
        let _ = fs::remove_file(&temporary_path);
    }
    replace_result
}

/// Return whether the current prompt/session already recorded subagent context.
pub fn has_recorded_subagent_context(
    project_root: &Path,
    session_id: Option<HookEventSessionId>,
    transcript_path: Option<HookEventTranscriptPath>,
) -> Result<bool, HookEventStateError> {
    if session_id.is_none() && transcript_path.is_none() {
        return Ok(false);
    }
    let state_path = ensure_project_hook_state_dir(project_root)?.join(HOOK_EVENT_STATE_FILE);
    if !state_path.is_file() {
        return Ok(false);
    }
    let now = unix_time_ms();
    let lines = read_hook_event_state_tail(&state_path)?;
    for line in lines.iter().rev() {
        let Ok(event) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if !is_recent_for_window(&event, now, PROMPT_SCOPE_WINDOW_MS) {
            break;
        }
        if !event_matches_prompt_scope(
            &event,
            session_id.as_ref().map(HookEventSessionId::as_str),
            transcript_path
                .as_ref()
                .map(HookEventTranscriptPath::as_str),
        ) {
            continue;
        }
        if is_prompt_scope_boundary(&event) {
            break;
        }
        match event.get("event").and_then(Value::as_str) {
            Some("subagent-start") => return Ok(true),
            Some("subagent-stop") => return Ok(false),
            _ => {}
        }
        if event
            .pointer("/fields/subagentContext")
            .and_then(Value::as_bool)
            == Some(true)
        {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Remove cached hook event state when it belongs to an older hook protocol.
pub fn remove_incompatible_hook_event_state(
    project_root: &Path,
) -> Result<Option<PathBuf>, String> {
    let state_path = ensure_project_hook_state_dir(project_root)?.join(HOOK_EVENT_STATE_FILE);
    remove_incompatible_hook_event_state_path(&state_path)
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
