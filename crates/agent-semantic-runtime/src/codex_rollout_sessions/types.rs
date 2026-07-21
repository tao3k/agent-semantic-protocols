//! Codex rollout JSONL session index parser.

use std::{collections::BTreeMap, path::PathBuf};

use serde::{Deserialize, Serialize};

use crate::CodexRolloutSessionMetadata;

/// Compact heartbeat/event entry parsed from one rollout JSONL stream.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexRolloutActivityHeartbeat {
    pub at: Option<i64>,
    pub kind: String,
    pub turn_id: Option<String>,
}

/// Liveness summary derived from a single exact Codex rollout JSONL file.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexRolloutActivityReport {
    pub status: String,
    pub rollout_path: PathBuf,
    pub last_event_at: Option<i64>,
    pub last_event_kind: Option<String>,
    pub last_heartbeat_at: Option<i64>,
    pub last_heartbeat_kind: Option<String>,
    pub recent_heartbeats: Vec<CodexRolloutActivityHeartbeat>,
    pub seconds_since_heartbeat: Option<i64>,
    pub current_turn_id: Option<String>,
    pub last_running_session_id: Option<String>,
    pub running_session_closed: bool,
    pub last_terminal_event: Option<String>,
    pub agent_instruction: Option<String>,
    pub scanned_line_count: usize,
}

/// Root-scoped index derived from Codex local rollout JSONL files.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexRolloutSessionIndex {
    pub root_session_id: String,
    pub sessions_dir: PathBuf,
    pub scanned_rollout_count: usize,
    pub skipped_rollout_count: usize,
    pub records: Vec<CodexRolloutSessionMetadata>,
    pub activity_by_session: BTreeMap<String, CodexRolloutActivityReport>,
    pub missing_rollout_by_session: BTreeMap<String, String>,
}
