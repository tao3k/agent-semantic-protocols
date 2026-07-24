//! Codex rollout JSONL session index parser.

use std::{collections::BTreeMap, path::PathBuf};

use serde::{Deserialize, Serialize};

use crate::CodexRolloutSessionMetadata;

macro_rules! rollout_session_text {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            #[allow(dead_code)]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl From<String> for $name {
            fn from(value: String) -> Self {
                Self(value)
            }
        }
    };
}

rollout_session_text!(CodexRolloutActivityKind);
rollout_session_text!(CodexRolloutTurnId);
rollout_session_text!(CodexRolloutActivityStatus);
rollout_session_text!(CodexRolloutSessionId);
rollout_session_text!(CodexRolloutTerminalEvent);
rollout_session_text!(CodexRolloutAgentInstruction);

/// Compact heartbeat/event entry parsed from one rollout JSONL stream.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexRolloutActivityHeartbeat {
    pub(crate) at: Option<i64>,
    pub(crate) kind: CodexRolloutActivityKind,
    pub(crate) turn_id: Option<CodexRolloutTurnId>,
}

/// Liveness summary derived from a single exact Codex rollout JSONL file.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexRolloutActivityReport {
    pub(crate) status: CodexRolloutActivityStatus,
    pub(crate) rollout_path: PathBuf,
    pub(crate) last_event_at: Option<i64>,
    pub(crate) last_event_kind: Option<CodexRolloutActivityKind>,
    pub(crate) last_heartbeat_at: Option<i64>,
    pub(crate) last_heartbeat_kind: Option<CodexRolloutActivityKind>,
    pub(crate) recent_heartbeats: Vec<CodexRolloutActivityHeartbeat>,
    pub(crate) seconds_since_heartbeat: Option<i64>,
    pub(crate) current_turn_id: Option<CodexRolloutTurnId>,
    pub(crate) last_running_session_id: Option<CodexRolloutSessionId>,
    pub(crate) running_session_closed: bool,
    pub(crate) last_terminal_event: Option<CodexRolloutTerminalEvent>,
    pub(crate) agent_instruction: Option<CodexRolloutAgentInstruction>,
    pub(crate) scanned_line_count: usize,
}

/// Root-scoped index derived from Codex local rollout JSONL files.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexRolloutSessionIndex {
    pub(crate) root_session_id: CodexRolloutSessionId,
    pub(crate) sessions_dir: PathBuf,
    pub(crate) scanned_rollout_count: usize,
    pub(crate) skipped_rollout_count: usize,
    pub(crate) records: Vec<CodexRolloutSessionMetadata>,
    pub(crate) activity_by_session: BTreeMap<CodexRolloutSessionId, CodexRolloutActivityReport>,
    pub(crate) missing_rollout_by_session: BTreeMap<CodexRolloutSessionId, String>,
}
