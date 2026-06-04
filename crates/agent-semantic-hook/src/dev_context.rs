//! Development-mode active hook context markers.

use crate::protocol::HookDecision;
use serde_json::{Value, json};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const PROJECT_ANCHORS: &[&str] = &[
    "Cargo.toml",
    "package.json",
    "pnpm-lock.yaml",
    "pyproject.toml",
    "Project.toml",
    ".git",
];

/// Named input for writing a development active-context marker.
pub struct ActiveContextRecord<'a> {
    /// Project activation path used to infer the workspace root.
    pub activation_path: &'a Path,
    /// Hook client identifier such as `codex`.
    pub platform: &'a str,
    /// Canonical hook event name such as `pre-tool`.
    pub event: &'a str,
    /// Raw platform hook payload.
    pub payload: &'a Value,
    /// Decision emitted by the hook classifier.
    pub decision: &'a HookDecision,
}

/// Record a short-lived active hook context marker in development mode.
pub fn record_active_context(record: ActiveContextRecord<'_>) {
    if !env_truthy("SEMANTIC_PROTOCOL_DEV_MODE") {
        return;
    }
    let Some(marker) = build_active_context_marker(record) else {
        return;
    };
    write_active_context_marker(&marker);
}

struct ActiveContextMarker {
    log_root: PathBuf,
    project_root_hash: String,
    content: Value,
}

fn build_active_context_marker(record: ActiveContextRecord<'_>) -> Option<ActiveContextMarker> {
    let project_root = infer_project_root(record.activation_path)?;
    let project_root_hash = stable_hash_hex(&project_root.display().to_string());
    let log_root = resolve_log_root(&project_root, &project_root_hash)?;
    let hook_run_id = env_first(&[
        "SEMANTIC_PROTOCOL_HOOK_RUN_ID",
        "CODEX_HOOK_RUN_ID",
        "AGENT_HOOK_RUN_ID",
    ])
    .or_else(|| json_string(record.payload, &["hookRunId"]))
    .or_else(|| json_string(record.payload, &["hook_run_id"]))
    .or_else(|| json_string(record.payload, &["run_id"]));
    let parent_event_id = env_first(&["SEMANTIC_PROTOCOL_PARENT_EVENT_ID"])
        .or_else(|| hook_run_id.clone())
        .unwrap_or_else(|| make_hook_event_id(record.event));
    let session_id = env_first(&[
        "SEMANTIC_PROTOCOL_SESSION_ID",
        "CODEX_SESSION_ID",
        "CLAUDE_SESSION_ID",
        "TERM_SESSION_ID",
    ])
    .or_else(|| json_string(record.payload, &["sessionId"]))
    .or_else(|| json_string(record.payload, &["session_id"]))
    .or_else(|| json_string(record.payload, &["conversation_id"]))
    .or_else(|| {
        hook_run_id
            .as_ref()
            .map(|id| format!("hook-{}", stable_hash_hex(id)))
    })
    .unwrap_or_else(|| format!("project-{project_root_hash}"));
    let decision_kind = serde_json::to_value(record.decision)
        .ok()
        .and_then(|value| json_string(&value, &["decision"]))
        .unwrap_or_else(|| "unknown".to_string());
    let now = SystemTime::now();
    Some(ActiveContextMarker {
        log_root,
        project_root_hash: project_root_hash.clone(),
        content: json!({
            "schemaId": "agent.semantic-protocols.dev-active-context",
            "schemaVersion": "1",
            "writtenAtUtc": format_utc_timestamp(now),
            "ttlSeconds": 1800,
            "projectRoot": project_root.display().to_string(),
            "projectRootHash": project_root_hash,
            "platform": record.platform,
            "event": record.event,
            "decision": decision_kind,
            "sessionId": session_id,
            "parentEventId": parent_event_id,
            "hookRunId": hook_run_id,
        }),
    })
}

fn write_active_context_marker(marker: &ActiveContextMarker) {
    let path = marker
        .log_root
        .join("dev-context")
        .join(format!("{}.json", marker.project_root_hash));
    let Some(parent) = path.parent() else {
        return;
    };
    if fs::create_dir_all(parent).is_err() {
        return;
    }
    if let Ok(content) = serde_json::to_string(&marker.content) {
        let _ = fs::write(path, content);
    }
}

fn env_truthy(name: &str) -> bool {
    env::var(name)
        .map(|value| {
            matches!(
                value.as_str(),
                "1" | "true" | "TRUE" | "yes" | "YES" | "on" | "ON"
            )
        })
        .unwrap_or(false)
}

fn env_first(names: &[&str]) -> Option<String> {
    names.iter().find_map(|name| env_non_empty(name))
}

fn env_non_empty(name: &str) -> Option<String> {
    env::var(name).ok().and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn json_string(value: &Value, path: &[&str]) -> Option<String> {
    let mut cursor = value;
    for key in path {
        cursor = cursor.get(*key)?;
    }
    cursor.as_str().map(ToOwned::to_owned)
}

fn infer_project_root(activation_path: &Path) -> Option<PathBuf> {
    let cwd = env::current_dir().ok();
    let activation_path = if activation_path.is_absolute() {
        activation_path.to_path_buf()
    } else if let Some(cwd) = &cwd {
        cwd.join(activation_path)
    } else {
        activation_path.to_path_buf()
    };
    if let Some(root) = project_root_from_path(&activation_path) {
        return Some(root);
    }
    cwd.as_deref().and_then(project_root_from_path)
}

fn project_root_from_path(path: &Path) -> Option<PathBuf> {
    let mut cursor = if path.is_dir() {
        path.to_path_buf()
    } else {
        path.parent()?.to_path_buf()
    };
    loop {
        if PROJECT_ANCHORS
            .iter()
            .any(|anchor| cursor.join(anchor).exists())
        {
            return Some(fs::canonicalize(&cursor).unwrap_or(cursor));
        }
        if !cursor.pop() {
            return None;
        }
    }
}

fn resolve_log_root(project_root: &Path, project_root_hash: &str) -> Option<PathBuf> {
    if let Some(value) = env_non_empty("SEMANTIC_PROTOCOL_TRACE_DIR") {
        return Some(path_from_env(&value, project_root));
    }
    if let Some(value) = env_non_empty("PRJ_CACHE_HOME") {
        return Some(path_from_env(&value, project_root).join("semantic_protocol"));
    }
    if let Some(value) = env_non_empty("XDG_CACHE_HOME") {
        return Some(
            PathBuf::from(value)
                .join("agent-semantic-protocols")
                .join(project_root_hash)
                .join("semantic_protocol"),
        );
    }
    env_non_empty("HOME").map(|home| {
        PathBuf::from(home)
            .join(".cache")
            .join("agent-semantic-protocols")
            .join(project_root_hash)
            .join("semantic_protocol")
    })
}

fn path_from_env(value: &str, project_root: &Path) -> PathBuf {
    let path = PathBuf::from(value);
    if path.is_absolute() {
        path
    } else {
        project_root.join(path)
    }
}

fn make_hook_event_id(event: &str) -> String {
    format!(
        "hook-{}-{}-{}",
        event,
        millis_since_epoch(SystemTime::now()),
        process::id()
    )
}

fn stable_hash_hex(value: &str) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in value.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

fn millis_since_epoch(time: SystemTime) -> u128 {
    time.duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_millis()
}

fn format_utc_timestamp(time: SystemTime) -> String {
    let total_seconds = time
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_secs() as i64;
    let days = total_seconds.div_euclid(86_400);
    let seconds_of_day = total_seconds.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    let hour = seconds_of_day / 3_600;
    let minute = (seconds_of_day % 3_600) / 60;
    let second = seconds_of_day % 60;
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

fn civil_from_days(days: i64) -> (i32, i64, i64) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    let year = y + if month <= 2 { 1 } else { 0 };
    (year as i32, month, day)
}
