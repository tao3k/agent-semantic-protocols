//! Types and status helpers for the agent session registry.

use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

/// Turso file name used for DB-owned agent session registry state.
pub const AGENT_SESSION_REGISTRY_DB_NAME: &str = "session-registry.turso";
pub const AGENT_SESSION_STATUS_ACTIVE: &str = "active";
pub const AGENT_SESSION_STATUS_IDLE: &str = "idle";
pub const AGENT_SESSION_STATUS_ARCHIVED: &str = "archived";
pub const AGENT_SESSION_STATUS_INVALID: &str = "invalid";

/// Durable agent session registry row stored in the Turso DB Engine.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AgentSessionRecord {
    /// Stable State Core project scope that owns this session row.
    #[serde(rename = "projectId")]
    pub project_id: String,
    /// Root Codex session id used as the routing tree root.
    #[serde(rename = "rootSessionId")]
    pub root_session_id: String,
    /// Concrete Codex session id registered for this agent.
    #[serde(rename = "sessionId")]
    pub session_id: String,
    /// Native host message-agent target id, when it differs from durable session identity.
    #[serde(rename = "messageTargetId", skip_serializing_if = "Option::is_none")]
    pub message_target_id: Option<String>,
    /// Optional parent session id when this row represents a delegated agent.
    #[serde(rename = "parentSessionId", skip_serializing_if = "Option::is_none")]
    pub parent_session_id: Option<String>,
    /// Stable registry lane name, such as `asp-explore`.
    pub name: String,
    /// Agent role advertised by the registry lane.
    pub role: String,
    /// Optional model id observed or requested for this agent session.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Durable routing status stored in the registry.
    pub status: String,
    /// Unix timestamp when this row was first created.
    #[serde(rename = "createdAt")]
    pub created_at: i64,
    /// Unix timestamp when this row last changed.
    #[serde(rename = "updatedAt")]
    pub updated_at: i64,
    /// Unix timestamp for the latest registry observation.
    #[serde(rename = "lastSeenAt", skip_serializing_if = "Option::is_none")]
    pub last_seen_at: Option<i64>,
    /// Unix timestamp for the latest child heartbeat.
    #[serde(rename = "lastHeartbeatAt", skip_serializing_if = "Option::is_none")]
    pub last_heartbeat_at: Option<i64>,
    /// Unix timestamp after which this row stops being routable.
    #[serde(rename = "expiresAt", skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<i64>,
    /// Unix timestamp when this row was archived.
    #[serde(rename = "archivedAt", skip_serializing_if = "Option::is_none")]
    pub archived_at: Option<i64>,
    /// Latest tool event kind observed for this session.
    #[serde(rename = "lastToolEvent", skip_serializing_if = "Option::is_none")]
    pub last_tool_event: Option<String>,
    /// Latest command line recorded for this session.
    #[serde(rename = "lastCommand", skip_serializing_if = "Option::is_none")]
    pub last_command: Option<String>,
    /// Latest compact evidence reference produced by this session.
    #[serde(rename = "lastEvidenceRef", skip_serializing_if = "Option::is_none")]
    pub last_evidence_ref: Option<String>,
    /// Caller metadata plus validation receipt JSON.
    #[serde(rename = "metadataJson")]
    pub metadata_json: String,
}

impl AgentSessionRecord {
    /// Stable State Core project scope that owns this session row.
    #[must_use]
    pub fn project_id(&self) -> &str {
        &self.project_id
    }

    /// Root Codex session id used as the routing tree root.
    #[must_use]
    pub fn root_session_id(&self) -> &str {
        &self.root_session_id
    }

    /// Concrete Codex session id registered for this agent.
    #[must_use]
    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    /// Native host message-agent target id, when available.
    #[must_use]
    pub fn message_target_id(&self) -> Option<&str> {
        self.message_target_id.as_deref()
    }

    /// Optional parent session id when this row represents a delegated agent.
    #[must_use]
    pub fn parent_session_id(&self) -> Option<&str> {
        self.parent_session_id.as_deref()
    }

    /// Stable registry lane name, such as `asp-explore`.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Agent role advertised by the registry lane.
    #[must_use]
    pub fn role(&self) -> &str {
        &self.role
    }

    /// Optional model id observed or requested for this agent session.
    #[must_use]
    pub fn model(&self) -> Option<&str> {
        self.model.as_deref()
    }

    /// Durable routing status stored in the registry.
    #[must_use]
    pub fn status(&self) -> &str {
        &self.status
    }

    /// Unix timestamp when this row was first created.
    #[must_use]
    pub fn created_at(&self) -> i64 {
        self.created_at
    }

    /// Unix timestamp when this row last changed.
    #[must_use]
    pub fn updated_at(&self) -> i64 {
        self.updated_at
    }

    /// Unix timestamp for the latest registry observation.
    #[must_use]
    pub fn last_seen_at(&self) -> Option<i64> {
        self.last_seen_at
    }

    /// Unix timestamp for the latest child heartbeat.
    #[must_use]
    pub fn last_heartbeat_at(&self) -> Option<i64> {
        self.last_heartbeat_at
    }

    /// Unix timestamp after which this row stops being routable.
    #[must_use]
    pub fn expires_at(&self) -> Option<i64> {
        self.expires_at
    }

    /// Unix timestamp when this row was archived.
    #[must_use]
    pub fn archived_at(&self) -> Option<i64> {
        self.archived_at
    }

    /// Latest tool event kind observed for this session.
    #[must_use]
    pub fn last_tool_event(&self) -> Option<&str> {
        self.last_tool_event.as_deref()
    }

    /// Latest command line recorded for this session.
    #[must_use]
    pub fn last_command(&self) -> Option<&str> {
        self.last_command.as_deref()
    }

    /// Latest compact evidence reference produced by this session.
    #[must_use]
    pub fn last_evidence_ref(&self) -> Option<&str> {
        self.last_evidence_ref.as_deref()
    }

    /// Caller metadata plus validation receipt JSON.
    #[must_use]
    pub fn metadata_json(&self) -> &str {
        &self.metadata_json
    }
}

/// Return whether a session status is eligible for routing.
#[must_use]
pub fn agent_session_status_is_routable(status: &str) -> bool {
    if status == AGENT_SESSION_STATUS_IDLE {
        return true;
    }
    matches!(
        status,
        AGENT_SESSION_STATUS_ACTIVE | AGENT_SESSION_STATUS_IDLE
    )
}

/// Return the current Unix timestamp used by session registry mutations.
pub fn agent_session_unix_timestamp() -> Result<i64, String> {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("system clock is before unix epoch: {error}"))?;
    i64::try_from(duration.as_secs()).map_err(|error| format!("timestamp overflow: {error}"))
}

/// Request for registering or updating one named agent session.
pub struct AgentSessionRegisterRequest<'a> {
    /// Stable State Core project scope that owns the registration.
    pub project_id: &'a str,
    /// Root Codex session id for this agent topology.
    pub root_session_id: &'a str,
    /// Concrete child session id to register or refresh.
    pub session_id: &'a str,
    /// Native host message-agent target id for sending follow-up messages.
    pub message_target_id: Option<&'a str>,
    /// Optional parent session id for delegated agents.
    pub parent_session_id: Option<&'a str>,
    /// Stable registry lane name, such as `asp-explore`.
    pub name: &'a str,
    /// Agent role advertised to routing and validation.
    pub role: &'a str,
    /// Optional expected or observed model id.
    pub model: Option<&'a str>,
    /// Durable routing status to store.
    pub status: &'a str,
    /// Optional expiration timestamp for routability.
    pub expires_at: Option<i64>,
    /// Caller metadata plus validation receipt JSON.
    pub metadata_json: &'a str,
    /// Mutation timestamp supplied by the caller.
    pub now: i64,
}

/// Request for locating a single session row by a stable registry key.
pub struct AgentSessionLookupRequest<'a> {
    /// Stable State Core project scope that owns the lookup.
    pub project_id: &'a str,
    /// Optional concrete child session id.
    pub session_id: Option<&'a str>,
    /// Optional root session id for the lookup tree.
    pub root_session_id: Option<&'a str>,
    /// Optional stable registry lane name.
    pub name: Option<&'a str>,
}

/// Request for recording the latest tool event observed for one agent session.
pub struct AgentSessionToolEventRequest<'a> {
    /// Concrete Codex session id whose routing receipt should be updated.
    pub session_id: &'a str,
    /// Tool event kind observed for this session.
    pub tool_event: &'a str,
    /// Optional command line associated with the event.
    pub command: Option<&'a str>,
    /// Optional compact evidence reference associated with the event.
    pub evidence_ref: Option<&'a str>,
    /// Mutation timestamp supplied by the caller.
    pub now: i64,
}

/// Merge optional caller metadata with registry validation metadata.
pub fn agent_session_normalized_metadata_json(
    value: Option<&str>,
    validation: &impl Serialize,
) -> Result<String, String> {
    let mut parsed: serde_json::Value = match value {
        Some(value) => serde_json::from_str(value)
            .map_err(|error| format!("--metadata-json must be valid JSON: {error}"))?,
        None => serde_json::json!({}),
    };
    let Some(object) = parsed.as_object_mut() else {
        return Err("--metadata-json must be a JSON object".to_string());
    };
    object.insert(
        "validation".to_string(),
        serde_json::to_value(validation)
            .map_err(|error| format!("failed to render validation metadata: {error}"))?,
    );
    Ok(parsed.to_string())
}
