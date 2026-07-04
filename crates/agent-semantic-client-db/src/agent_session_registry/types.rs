//! Types and status helpers for the agent session registry.

use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

/// Turso file name used for DB-owned agent session registry state.
pub const AGENT_SESSION_REGISTRY_DB_NAME: &str = "session-registry.turso";
/// Durable status used for sessions that failed routing validation.
pub const AGENT_SESSION_STATUS_INVALID: &str = "invalid";
/// Durable status used for sessions archived through the host client.
pub const AGENT_SESSION_STATUS_ARCHIVED: &str = "archived";
/// Durable status used when an archived session is restored without a running turn.
pub const AGENT_SESSION_STATUS_IDLE: &str = "idle";

/// Durable agent session registry row stored in the Turso DB Engine.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AgentSessionRecord {
    #[serde(rename = "projectId")]
    pub project_id: String,
    #[serde(rename = "rootSessionId")]
    pub root_session_id: String,
    #[serde(rename = "sessionId")]
    pub session_id: String,
    #[serde(rename = "parentSessionId", skip_serializing_if = "Option::is_none")]
    pub parent_session_id: Option<String>,
    pub name: String,
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    pub status: String,
    #[serde(rename = "createdAt")]
    pub created_at: i64,
    #[serde(rename = "updatedAt")]
    pub updated_at: i64,
    #[serde(rename = "lastSeenAt", skip_serializing_if = "Option::is_none")]
    pub last_seen_at: Option<i64>,
    #[serde(rename = "lastHeartbeatAt", skip_serializing_if = "Option::is_none")]
    pub last_heartbeat_at: Option<i64>,
    #[serde(rename = "expiresAt", skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<i64>,
    #[serde(rename = "archivedAt", skip_serializing_if = "Option::is_none")]
    pub archived_at: Option<i64>,
    #[serde(rename = "lastToolEvent", skip_serializing_if = "Option::is_none")]
    pub last_tool_event: Option<String>,
    #[serde(rename = "lastCommand", skip_serializing_if = "Option::is_none")]
    pub last_command: Option<String>,
    #[serde(rename = "lastEvidenceRef", skip_serializing_if = "Option::is_none")]
    pub last_evidence_ref: Option<String>,
    #[serde(rename = "metadataJson")]
    pub metadata_json: String,
}

/// Return whether a session status is eligible for routing.
#[must_use]
pub fn agent_session_status_is_routable(status: &str) -> bool {
    matches!(status, "active" | "idle")
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
    pub project_id: &'a str,
    pub root_session_id: &'a str,
    pub session_id: &'a str,
    pub parent_session_id: Option<&'a str>,
    pub name: &'a str,
    pub role: &'a str,
    pub model: Option<&'a str>,
    pub status: &'a str,
    pub expires_at: Option<i64>,
    pub metadata_json: &'a str,
    pub now: i64,
}

/// Request for looking up one registered agent session.
pub struct AgentSessionLookupRequest<'a> {
    pub project_id: &'a str,
    pub session_id: Option<&'a str>,
    pub root_session_id: Option<&'a str>,
    pub name: Option<&'a str>,
}

/// Request for recording the latest tool activity for one session.
pub struct AgentSessionToolEventRequest<'a> {
    pub session_id: &'a str,
    pub tool_event: &'a str,
    pub command: Option<&'a str>,
    pub evidence_ref: Option<&'a str>,
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
