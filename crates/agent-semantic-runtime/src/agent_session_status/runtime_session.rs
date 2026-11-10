// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Runtime session identity discovered from host environment variables.

use serde::Deserialize;
use serde::Serialize;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RuntimeSessionId(String);

impl RuntimeSessionId {
    pub fn try_new(value: impl Into<String>) -> Result<Self, String> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err("RuntimeSessionId must be non-empty".to_string());
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for RuntimeSessionId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for RuntimeSessionId {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl std::fmt::Display for RuntimeSessionId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(formatter)
    }
}

/// Runtime-visible agent session discovered from host environment variables.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentRuntimeSession {
    /// Host client that supplied the session id.
    pub client: String,
    /// Host session id, such as `CODEX_THREAD_ID` or Claude Code session id.
    pub id: String,
}

impl AgentRuntimeSession {
    /// The host-provided id used as recall identity before registry parent lookup.
    pub fn recall_session_id(&self) -> &str {
        &self.id
    }
}

/// Discover the current host agent session from well-known environment ids.
#[must_use]
pub fn current_agent_runtime_session() -> Option<AgentRuntimeSession> {
    agent_runtime_session_from_platform_ids(
        env_value("CODEX_SESSION_ID"),
        env_value("CODEX_THREAD_ID"),
        env_value("CLAUDE_CODE_SESSION_ID"),
        env_value("CLAUDE_CODE_REMOTE_SESSION_ID"),
    )
}

fn agent_runtime_session_from_platform_ids(
    codex_session_id: Option<String>,
    codex_thread_id: Option<String>,
    claude_session_id: Option<String>,
    claude_remote_session_id: Option<String>,
) -> Option<AgentRuntimeSession> {
    let sessions = [
        codex_session_id
            .or(codex_thread_id)
            .map(|id| AgentRuntimeSession {
                client: "codex".to_string(),
                id,
            }),
        claude_session_id
            .or(claude_remote_session_id)
            .map(|id| AgentRuntimeSession {
                client: "claude-code".to_string(),
                id,
            }),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();
    (sessions.len() == 1).then(|| sessions.into_iter().next().expect("one session"))
}

#[cfg(test)]
#[path = "../../tests/unit/runtime_session_identity.rs"]
mod tests;
fn env_value(name: &str) -> Option<String> {
    std::env::var(name).ok().and_then(non_empty_value)
}

fn non_empty_value(value: String) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}
