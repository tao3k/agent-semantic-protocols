// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Codex rollout JSONL session index parser.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::Deserialize;
use serde::Serialize;

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
    pub status: CodexRolloutActivityStatus,
    pub(crate) rollout_path: PathBuf,
    pub last_event_at: Option<i64>,
    pub(crate) last_event_kind: Option<CodexRolloutActivityKind>,
    pub last_heartbeat_at: Option<i64>,
    pub(crate) last_heartbeat_kind: Option<CodexRolloutActivityKind>,
    pub(crate) recent_heartbeats: Vec<CodexRolloutActivityHeartbeat>,
    pub(crate) seconds_since_heartbeat: Option<i64>,
    pub(crate) current_turn_id: Option<CodexRolloutTurnId>,
    pub(crate) last_running_session_id: Option<CodexRolloutSessionId>,
    pub running_session_closed: bool,
    pub last_terminal_event: Option<CodexRolloutTerminalEvent>,
    pub(crate) agent_instruction: Option<CodexRolloutAgentInstruction>,
    pub(crate) scanned_line_count: usize,
}

impl CodexRolloutActivityReport {
    pub fn status(&self) -> &str {
        self.status.as_str()
    }

    pub fn current_turn_id(&self) -> Option<&str> {
        self.current_turn_id.as_ref().map(|value| value.as_str())
    }

    pub fn last_running_session_id(&self) -> Option<&str> {
        self.last_running_session_id
            .as_ref()
            .map(|value| value.as_str())
    }

    pub const fn running_session_closed(&self) -> bool {
        self.running_session_closed
    }

    pub fn last_terminal_event(&self) -> Option<&str> {
        self.last_terminal_event
            .as_ref()
            .map(|value| value.as_str())
    }

    pub const fn scanned_line_count(&self) -> usize {
        self.scanned_line_count
    }
}

/// Root-scoped index derived from Codex local rollout JSONL files.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexRolloutSessionIndex {
    pub(crate) root_session_id: CodexRolloutSessionId,
    pub(crate) sessions_dir: PathBuf,
    pub scanned_rollout_count: usize,
    pub skipped_rollout_count: usize,
    pub records: Vec<CodexRolloutSessionMetadata>,
    pub activity_by_session: BTreeMap<CodexRolloutSessionId, CodexRolloutActivityReport>,
    pub missing_rollout_by_session: BTreeMap<CodexRolloutSessionId, String>,
}

impl CodexRolloutSessionIndex {
    pub fn root_session_id(&self) -> &str {
        self.root_session_id.as_str()
    }

    pub const fn scanned_rollout_count(&self) -> usize {
        self.scanned_rollout_count
    }

    pub fn records(&self) -> &[CodexRolloutSessionMetadata] {
        &self.records
    }

    pub fn activity_count(&self) -> usize {
        self.activity_by_session.len()
    }

    pub fn activity_for_session(
        &self,
        session_id: &crate::RuntimeSessionId,
    ) -> Option<&CodexRolloutActivityReport> {
        self.activity_by_session
            .iter()
            .find(|(candidate, _)| candidate.as_str() == session_id.as_str())
            .map(|(_, report)| report)
    }
}
