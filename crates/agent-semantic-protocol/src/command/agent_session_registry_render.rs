//! Rendering helpers for the agent session registry CLI.

use serde::Serialize;
use std::path::Path;

use super::SessionRecord;

#[derive(Serialize)]
struct SessionReport<'a> {
    owner: &'static str,
    #[serde(rename = "dbPath")]
    db_path: &'a str,
    #[serde(rename = "rootSessionId", skip_serializing_if = "Option::is_none")]
    root_session_id: Option<&'a str>,
    sessions: Vec<SessionRecord>,
}

pub(super) fn print_reuse_session(
    db_path: &Path,
    root_session_id: Option<&str>,
    session: SessionRecord,
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
    sessions: Vec<SessionRecord>,
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

pub(super) fn print_session_row(session: &SessionRecord) {
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
