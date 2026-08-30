use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const COLLABORATION_LIVE_AGENT_SNAPSHOT_SCHEMA_ID: &str =
    "agent.semantic-protocols.codex-collaboration-live-agent-snapshot";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CollaborationLiveAgents {
    pub agents: Vec<CollaborationLiveAgent>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CollaborationLiveAgent {
    pub agent_name: String,
    pub agent_status: Value,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CollaborationLiveAgentSnapshot {
    pub schema_id: String,
    pub schema_version: String,
    pub workspace_root: String,
    pub root_session_id: String,
    pub observed_at_unix_ms: u64,
    pub agents: CollaborationLiveAgents,
}

impl CollaborationLiveAgentSnapshot {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_id != COLLABORATION_LIVE_AGENT_SNAPSHOT_SCHEMA_ID
            || self.schema_version != "1"
        {
            return Err("collaboration live-agent snapshot schema identity is invalid".to_owned());
        }
        if self.workspace_root.is_empty() || !self.workspace_root.starts_with('/') {
            return Err(
                "collaboration live-agent snapshot workspaceRoot must be absolute".to_owned(),
            );
        }
        if self.root_session_id.is_empty() || self.observed_at_unix_ms == 0 {
            return Err("collaboration live-agent snapshot identity is incomplete".to_owned());
        }
        self.agents.validate()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CollaborationDispatchState {
    Absent,
    Running,
    Reusable,
}

impl CollaborationLiveAgents {
    pub fn validate(&self) -> Result<(), String> {
        let mut paths = BTreeSet::new();
        for agent in &self.agents {
            if !is_canonical_agent_path(&agent.agent_name) {
                return Err(format!(
                    "collaboration.list_agents returned a non-canonical path: {}",
                    agent.agent_name
                ));
            }
            if !paths.insert(agent.agent_name.as_str()) {
                return Err(format!(
                    "collaboration.list_agents returned a duplicate path: {}",
                    agent.agent_name
                ));
            }
            validate_status(&agent.agent_status)?;
        }
        if !paths.contains("/root") {
            return Err(
                "collaboration.list_agents must include the current /root Agent".to_owned(),
            );
        }
        Ok(())
    }

    pub fn dispatch_state(
        &self,
        canonical_path: &str,
    ) -> Result<CollaborationDispatchState, String> {
        self.validate()?;
        let Some(agent) = self
            .agents
            .iter()
            .find(|agent| agent.agent_name == canonical_path)
        else {
            return Ok(CollaborationDispatchState::Absent);
        };
        Ok(if agent.agent_status.as_str() == Some("running") {
            CollaborationDispatchState::Running
        } else {
            CollaborationDispatchState::Reusable
        })
    }
}

fn is_canonical_agent_path(path: &str) -> bool {
    if path == "/root" {
        return true;
    }
    let Some(tail) = path.strip_prefix("/root/") else {
        return false;
    };
    !tail.is_empty()
        && tail.split('/').all(|part| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
        })
}

fn validate_status(status: &Value) -> Result<(), String> {
    match status {
        Value::String(value) if !value.is_empty() => Ok(()),
        Value::Object(fields)
            if fields.len() == 1
                && fields
                    .values()
                    .all(|value| value.as_str().is_some_and(|value| !value.is_empty())) =>
        {
            Ok(())
        }
        _ => Err("collaboration.list_agents returned an invalid agent_status".to_owned()),
    }
}

#[cfg(test)]
#[path = "../tests/unit/codex_collaboration.rs"]
mod tests;
