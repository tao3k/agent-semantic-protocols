//! Rendering helpers for the agent session registry CLI.

use super::agent_session_registry_rollout_activity::RolloutActivityReport;
use serde::Serialize;
use std::path::Path;

use agent_semantic_client_db::AgentSessionRecord;
use agent_semantic_runtime::{AgentSessionValidationReport, CodexRolloutSessionIndex};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SessionStatusReport {
    pub(super) owner: &'static str,
    #[serde(rename = "dbPath")]
    pub(super) db_path: String,
    #[serde(rename = "rootSessionId", skip_serializing_if = "Option::is_none")]
    pub(super) root_session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) name: Option<String>,
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_status_session_record"
    )]
    pub(super) session: Option<AgentSessionRecord>,
    #[serde(rename = "registryStatus")]
    pub(super) registry_status: String,
    pub(super) routable: bool,
    #[serde(rename = "sessionLifetime")]
    pub(super) session_lifetime: String,
    pub(super) resident: bool,
    #[serde(rename = "sessionLifetimeSource")]
    pub(super) session_lifetime_source: String,
    #[serde(rename = "validationStatus")]
    pub(super) validation_status: String,
    #[serde(rename = "validationReason")]
    pub(super) validation_reason: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) validation: Option<AgentSessionValidationReport>,
    #[serde(
        rename = "rolloutSessionIndex",
        skip_serializing_if = "Option::is_none"
    )]
    pub(super) rollout_session_index: Option<CodexRolloutSessionIndex>,
    #[serde(rename = "rolloutActivity", skip_serializing_if = "Option::is_none")]
    pub(super) rollout_activity: Option<RolloutActivityReport>,
    #[serde(
        rename = "sessionLifecycleIndex",
        skip_serializing_if = "Option::is_none"
    )]
    pub(super) session_lifecycle_index: Option<SessionLifecycleIndex>,
    #[serde(
        rename = "activitySnapshotShort",
        skip_serializing_if = "Option::is_none"
    )]
    pub(super) activity_snapshot_short: Option<ActivitySnapshotShort>,
    #[serde(rename = "hostClient", skip_serializing_if = "Option::is_none")]
    pub(super) host_client: Option<String>,
    #[serde(rename = "hostThreadId", skip_serializing_if = "Option::is_none")]
    pub(super) host_thread_id: Option<String>,
    #[serde(rename = "hostStatusSource")]
    pub(super) host_status_source: String,
    #[serde(rename = "hostStatus")]
    pub(super) host_status: String,
    #[serde(rename = "hostStatusReason")]
    pub(super) host_status_reason: String,
    #[serde(rename = "hostThreadExistence")]
    pub(super) host_thread_existence: String,
    #[serde(rename = "hostThreadExistenceReason")]
    pub(super) host_thread_existence_reason: String,
    #[serde(rename = "multiAgentChildState")]
    pub(super) multi_agent_child_state: String,
    #[serde(rename = "hostRawStatus", skip_serializing_if = "Option::is_none")]
    pub(super) host_raw_status: Option<String>,
    #[serde(rename = "healthStatus")]
    pub(super) health_status: String,
    #[serde(rename = "timeoutSemantics")]
    pub(super) timeout_semantics: &'static str,
    #[serde(rename = "duplicateWorkerAllowed")]
    pub(super) duplicate_worker_allowed: bool,
    #[serde(rename = "artifactsDir")]
    pub(super) artifacts_dir: String,
    #[serde(rename = "artifactStatus")]
    pub(super) artifact_status: String,
    #[serde(rename = "artifactStaleAfterSeconds")]
    pub(super) artifact_stale_after_seconds: i64,
    #[serde(
        rename = "lastArtifactUpdatedAt",
        skip_serializing_if = "Option::is_none"
    )]
    pub(super) last_artifact_updated_at: Option<i64>,
    #[serde(rename = "artifactAgeSeconds", skip_serializing_if = "Option::is_none")]
    pub(super) artifact_age_seconds: Option<i64>,
    #[serde(rename = "lastArtifactPath", skip_serializing_if = "Option::is_none")]
    pub(super) last_artifact_path: Option<String>,
    #[serde(rename = "nextAction")]
    pub(super) next_action: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ActivitySnapshotShort {
    pub(super) source: &'static str,
    #[serde(rename = "rootSessionId", skip_serializing_if = "Option::is_none")]
    pub(super) root_session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) selected_session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) selected_role: Option<String>,
    pub(super) registry_status: String,
    pub(super) host_status: String,
    pub(super) health_status: String,
    pub(super) next_action: String,
    #[serde(
        rename = "rolloutActivityStatus",
        skip_serializing_if = "Option::is_none"
    )]
    pub(super) rollout_activity_status: Option<String>,
    #[serde(
        rename = "rolloutLastHeartbeatKind",
        skip_serializing_if = "Option::is_none"
    )]
    pub(super) rollout_last_heartbeat_kind: Option<String>,
    #[serde(
        rename = "rolloutLastTerminalEvent",
        skip_serializing_if = "Option::is_none"
    )]
    pub(super) rollout_last_terminal_event: Option<String>,
    #[serde(
        rename = "rolloutRunningSessionClosed",
        skip_serializing_if = "Option::is_none"
    )]
    pub(super) rollout_running_session_closed: Option<bool>,
    #[serde(
        rename = "secondsSinceHeartbeat",
        skip_serializing_if = "Option::is_none"
    )]
    pub(super) seconds_since_heartbeat: Option<i64>,
    #[serde(rename = "rolloutIndexError", skip_serializing_if = "Option::is_none")]
    pub(super) rollout_index_error: Option<String>,
    pub(super) missing_rollout_by_session: std::collections::BTreeMap<String, String>,
    pub(super) rollout_status_by_session: std::collections::BTreeMap<String, String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SessionLifecycleIndex {
    #[serde(rename = "rootSessionId", skip_serializing_if = "Option::is_none")]
    pub(super) root_session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) selected_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) selected_session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) selected_role: Option<String>,
    pub(super) registry_status: String,
    pub(super) routable: bool,
    pub(super) rollout_session_count: usize,
    pub(super) rollout_activity_count: usize,
    pub(super) missing_rollout_count: usize,
    #[serde(rename = "rolloutIndexError", skip_serializing_if = "Option::is_none")]
    pub(super) rollout_index_error: Option<String>,
    pub(super) missing_rollout_by_session: std::collections::BTreeMap<String, String>,
    pub(super) rollout_status_by_session: std::collections::BTreeMap<String, String>,
}

#[derive(Serialize)]
struct SessionReport<'a> {
    owner: &'static str,
    #[serde(rename = "dbPath")]
    db_path: &'a str,
    #[serde(rename = "rootSessionId", skip_serializing_if = "Option::is_none")]
    root_session_id: Option<&'a str>,
    #[serde(serialize_with = "serialize_session_records")]
    sessions: Vec<AgentSessionRecord>,
}

pub(super) fn print_reuse_session(
    db_path: &Path,
    root_session_id: Option<&str>,
    session: AgentSessionRecord,
    json: bool,
) -> Result<(), String> {
    if json {
        return print_json_report(db_path, root_session_id, vec![session]);
    }
    println!(
        "[agent-session-reuse] owner=rust status=\"found\" rootSession={} name=\"{}\" childSessionId=\"{}\" role=\"{}\" sessionStatus=\"{}\" db=\"{}\"",
        root_session_id
            .map(|value| format!("\"{}\"", escape_field(value)))
            .unwrap_or_else(|| "\"*\"".to_string()),
        escape_field(&session.name),
        escape_field(&session.session_id),
        escape_field(&session.role),
        escape_field(&session.status),
        db_path.display()
    );
    Ok(())
}

pub(super) fn print_reuse_miss(
    db_path: &Path,
    root_session_id: Option<&str>,
    name: &str,
    reason: &str,
    json: bool,
) -> Result<(), String> {
    if json {
        return print_json_report(db_path, root_session_id, Vec::new());
    }
    println!(
        "[agent-session-reuse] owner=rust status=\"miss\" rootSession={} name=\"{}\" reason=\"{}\" db=\"{}\"",
        root_session_id
            .map(|value| format!("\"{}\"", escape_field(value)))
            .unwrap_or_else(|| "\"*\"".to_string()),
        escape_field(name),
        escape_field(reason),
        db_path.display()
    );
    Ok(())
}

pub(super) fn print_json_report(
    db_path: &Path,
    root_session_id: Option<&str>,
    sessions: Vec<AgentSessionRecord>,
) -> Result<(), String> {
    let report = SessionReport {
        owner: "rust",
        db_path: &db_path.display().to_string(),
        root_session_id,
        sessions,
    };
    println!(
        "{}",
        serde_json::to_string_pretty(&report)
            .map_err(|error| format!("failed to render session json: {error}"))?
    );
    Ok(())
}

fn serialize_status_session_record<S>(
    session: &Option<AgentSessionRecord>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    let Some(session) = session else {
        return serializer.serialize_none();
    };
    let value = session_record_json_without_metadata(session).map_err(serde::ser::Error::custom)?;
    serde::Serialize::serialize(&value, serializer)
}

fn serialize_session_records<S>(
    sessions: &[AgentSessionRecord],
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    let values = sessions
        .iter()
        .map(session_record_json_with_validation_projection)
        .collect::<Result<Vec<_>, _>>()
        .map_err(serde::ser::Error::custom)?;
    serde::Serialize::serialize(&values, serializer)
}

fn session_record_json_without_metadata(
    session: &AgentSessionRecord,
) -> Result<serde_json::Value, serde_json::Error> {
    let mut value = serde_json::to_value(session)?;
    if let Some(object) = value.as_object_mut() {
        object.remove("metadataJson");
    }
    Ok(value)
}

fn session_record_json_with_validation_projection(
    session: &AgentSessionRecord,
) -> Result<serde_json::Value, serde_json::Error> {
    let mut value = session_record_json_without_metadata(session)?;
    if let Some(object) = value.as_object_mut() {
        if let Ok(metadata) = serde_json::from_str::<serde_json::Value>(&session.metadata_json) {
            if let Some(validation) = metadata.get("validation") {
                object.insert("validation".to_string(), validation.clone());
            }
        }
    }
    Ok(value)
}

pub(super) fn print_status_report(report: SessionStatusReport, json: bool) -> Result<(), String> {
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&report)
                .map_err(|error| format!("failed to render session status json: {error}"))?
        );
        return Ok(());
    }
    let rollout_activity_status = report
        .rollout_activity
        .as_ref()
        .map(|activity| activity.status.as_str())
        .unwrap_or("not-reported");
    let rollout_last_heartbeat_at = report
        .rollout_activity
        .as_ref()
        .and_then(|activity| activity.last_heartbeat_at.as_deref())
        .unwrap_or("none");
    let rollout_last_heartbeat_kind = report
        .rollout_activity
        .as_ref()
        .and_then(|activity| activity.last_heartbeat_kind.as_deref())
        .unwrap_or("none");
    let rollout_last_terminal_event = report
        .rollout_activity
        .as_ref()
        .and_then(|activity| activity.last_terminal_event.as_deref())
        .unwrap_or("none");
    let rollout_running_session_closed = report
        .rollout_activity
        .as_ref()
        .map(|activity| activity.running_session_closed)
        .unwrap_or(false);
    println!(
        "[agent-session-status] owner=rust rootSession={} name={} registryStatus=\"{}\" routable={} validationStatus=\"{}\" validationReason=\"{}\" hostClient={} hostStatus=\"{}\" hostThreadExistence=\"{}\" multiAgentChildState=\"{}\" rolloutActivityStatus=\"{}\" rolloutLastHeartbeatAt=\"{}\" rolloutLastHeartbeatKind=\"{}\" rolloutLastTerminalEvent=\"{}\" rolloutRunningSessionClosed={} healthStatus=\"{}\" artifactStatus=\"{}\" artifactAgeSeconds={} nextAction=\"{}\" duplicateWorkerAllowed={} db=\"{}\" artifactsDir=\"{}\"",
        report
            .root_session_id
            .as_deref()
            .map(|value| format!("\"{}\"", escape_field(value)))
            .unwrap_or_else(|| "\"\"".to_string()),
        report
            .name
            .as_deref()
            .map(|value| format!("\"{}\"", escape_field(value)))
            .unwrap_or_else(|| "\"\"".to_string()),
        escape_field(&report.registry_status),
        report.routable,
        escape_field(&report.validation_status),
        escape_field(&report.validation_reason),
        report
            .host_client
            .as_deref()
            .map(|value| format!("\"{}\"", escape_field(value)))
            .unwrap_or_else(|| "\"\"".to_string()),
        escape_field(&report.host_status),
        escape_field(&report.host_thread_existence),
        escape_field(&report.multi_agent_child_state),
        escape_field(rollout_activity_status),
        escape_field(rollout_last_heartbeat_at),
        escape_field(rollout_last_heartbeat_kind),
        escape_field(rollout_last_terminal_event),
        rollout_running_session_closed,
        escape_field(&report.health_status),
        escape_field(&report.artifact_status),
        optional_i64_field(report.artifact_age_seconds),
        escape_field(&report.next_action),
        report.duplicate_worker_allowed,
        report.db_path,
        report.artifacts_dir
    );
    if let Some(session) = report.session.as_ref() {
        print_session_row(session);
    }
    Ok(())
}

pub(super) fn print_status_activity_report(
    rollout_activity: Option<&RolloutActivityReport>,
    next_action: &str,
    json: bool,
) -> Result<(), String> {
    if json {
        let rollout_activity = rollout_activity.map(|activity| {
            serde_json::json!({
                "status": &activity.status,
                "sessionMeta": &activity.session_meta,
                "sessionActivity": &activity.session_activity,
                "lastHeartbeatAt": &activity.last_heartbeat_at,
                "lastHeartbeatKind": &activity.last_heartbeat_kind,
                "recentHeartbeats": &activity.recent_heartbeats,
                "runningSessionClosed": activity.running_session_closed,
                "agentInstruction": &activity.agent_instruction,
            })
        });
        let report = serde_json::json!({
            "rolloutActivity": rollout_activity,
            "nextAction": next_action,
        });
        println!(
            "{}",
            serde_json::to_string_pretty(&report)
                .map_err(|error| format!("failed to render session activity json: {error}"))?
        );
        return Ok(());
    }

    let status = rollout_activity
        .map(|activity| activity.status.as_str())
        .unwrap_or("unknown");
    let last_heartbeat_at = rollout_activity
        .and_then(|activity| activity.last_heartbeat_at.as_deref())
        .unwrap_or("none");
    let last_heartbeat_kind = rollout_activity
        .and_then(|activity| activity.last_heartbeat_kind.as_deref())
        .unwrap_or("none");
    let running_session_closed = rollout_activity
        .map(|activity| activity.running_session_closed)
        .unwrap_or(false);
    let agent_instruction = rollout_activity
        .map(|activity| activity.agent_instruction.as_str())
        .unwrap_or("rollout activity unavailable");
    let session_meta = rollout_activity.and_then(|activity| activity.session_meta.as_ref());
    let child_session_id = session_meta
        .and_then(|meta| meta.child_session_id.as_deref())
        .unwrap_or("none");
    let source_session_id = session_meta
        .and_then(|meta| meta.source_session_id.as_deref())
        .unwrap_or("none");
    let parent_thread_id = session_meta
        .and_then(|meta| meta.parent_thread_id.as_deref())
        .unwrap_or("none");
    let agent_role = session_meta
        .and_then(|meta| meta.agent_role.as_deref())
        .unwrap_or("none");

    println!(
        "[agent-session-activity] status=\"{}\" childSessionId={} sourceSessionId={} parentThreadId={} agentRole={} lastHeartbeatAt={} lastHeartbeatKind={} runningSessionClosed={} nextAction=\"{}\" agentInstruction=\"{}\"",
        status,
        child_session_id,
        source_session_id,
        parent_thread_id,
        agent_role,
        last_heartbeat_at,
        last_heartbeat_kind,
        running_session_closed,
        next_action,
        agent_instruction,
    );
    Ok(())
}

pub(super) fn print_session_row(session: &AgentSessionRecord) {
    println!(
        "|session name=\"{}\" session=\"{}\" rootSession=\"{}\" parentSession={} role=\"{}\" model={} status=\"{}\" updatedAt={} lastSeenAt={} lastHeartbeatAt={} expiresAt={}",
        escape_field(&session.name),
        escape_field(&session.session_id),
        escape_field(&session.root_session_id),
        optional_field(session.parent_session_id.as_deref()),
        escape_field(&session.role),
        optional_field(session.model.as_deref()),
        escape_field(&session.status),
        session.updated_at,
        optional_i64_field(session.last_seen_at),
        optional_i64_field(session.last_heartbeat_at),
        optional_i64_field(session.expires_at)
    );
}

pub(super) fn escape_field(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn optional_i64_field(value: Option<i64>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "\"\"".to_string())
}

fn optional_field(value: Option<&str>) -> String {
    value
        .map(|value| format!("\"{}\"", escape_field(value)))
        .unwrap_or_else(|| "\"\"".to_string())
}
