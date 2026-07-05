#[path = "../../../src/command/agent_session_registry_rollout_activity.rs"]
mod agent_session_registry_rollout_activity;

use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_secs() as i64
}

fn write_rollout_fixture(name: &str, lines: &[&str]) -> PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!(
        "asp-rollout-activity-{name}-{}-{}.jsonl",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos()
    ));
    fs::write(&path, format!("{}\n", lines.join("\n"))).expect("write rollout fixture");
    path
}

fn remove_fixture(path: PathBuf) {
    let _ = fs::remove_file(path);
}

#[test]
fn rollout_activity_active_running_when_recent_heartbeat_exists() {
    let path = write_rollout_fixture(
        "active",
        &[
            r#"{"timestamp":"2026-07-02T22:00:00.000Z","type":"turn_context","payload":{"turn_id":"turn-active"}}"#,
            r#"{"timestamp":"2026-07-02T22:00:00.500Z","type":"event_msg","payload":{"type":"agent_message","turn_id":"turn-active"}}"#,
            r#"{"timestamp":"2026-07-02T22:00:01.000Z","type":"response_item","payload":{"type":"function_call_output","output":"Chunk ID: abc\nProcess running with session ID 12345\nOutput:"}}"#,
            r#"{"timestamp":"2026-07-02T22:00:02.000Z","type":"event_msg","payload":{"type":"token_count","turn_id":"turn-active"}}"#,
            r#"{"timestamp":"2026-07-02T22:00:03.000Z","type":"event_msg","payload":{"type":"agent_message","turn_id":"turn-active"}}"#,
        ],
    );

    let report =
        agent_session_registry_rollout_activity::rollout_activity_report(&path, now_unix());

    assert_eq!(report.status, "agent-active");
    assert_eq!(report.agent_instruction, "child-activity-running-wait");
    assert_eq!(report.current_turn_id.as_deref(), Some("turn-active"));
    assert_eq!(report.last_running_session_id.as_deref(), Some("12345"));
    assert_eq!(report.recent_heartbeats.len(), 3);
    assert_eq!(
        report
            .recent_heartbeats
            .first()
            .map(|heartbeat| heartbeat.kind.as_str()),
        Some("function_call_output")
    );
    assert_eq!(
        report
            .recent_heartbeats
            .last()
            .map(|heartbeat| heartbeat.kind.as_str()),
        Some("agent_message")
    );
    assert!(!report.running_session_closed);
    remove_fixture(path);
}

#[test]
fn rollout_activity_active_running_without_running_session_id_stays_open() {
    let path = write_rollout_fixture(
        "active-open",
        &[
            r#"{"timestamp":"2026-07-02T22:00:00.000Z","type":"turn_context","payload":{"turn_id":"turn-active-open"}}"#,
            r#"{"timestamp":"2026-07-02T22:00:00.500Z","type":"event_msg","payload":{"type":"agent_message","turn_id":"turn-active-open"}}"#,
            r#"{"timestamp":"2026-07-02T22:00:01.000Z","type":"event_msg","payload":{"type":"token_count","turn_id":"turn-active-open"}}"#,
        ],
    );

    let report =
        agent_session_registry_rollout_activity::rollout_activity_report(&path, now_unix());

    assert_eq!(report.status, "agent-active");
    assert_eq!(report.agent_instruction, "child-activity-running-wait");
    assert!(report.last_running_session_id.is_none());
    assert!(!report.running_session_closed);
    remove_fixture(path);
}

#[test]
fn rollout_activity_completed_when_last_event_is_terminal() {
    let path = write_rollout_fixture(
        "completed",
        &[
            r#"{"timestamp":"2026-07-02T22:00:00.000Z","type":"event_msg","payload":{"type":"agent_message","turn_id":"turn-complete"}}"#,
            r#"{"timestamp":"2026-07-02T22:00:01.000Z","type":"event_msg","payload":{"type":"task_complete","turn_id":"turn-complete"}}"#,
        ],
    );

    let report =
        agent_session_registry_rollout_activity::rollout_activity_report(&path, now_unix() + 300);

    assert_eq!(report.status, "idle-resumable");
    assert_eq!(
        report.agent_instruction,
        "child-idle-resumable-reuse-existing-child"
    );
    assert_eq!(report.last_terminal_event.as_deref(), Some("task_complete"));
    assert!(!report.running_session_closed);
    remove_fixture(path);
}

#[test]
fn rollout_activity_orphan_risk_when_running_session_is_stale_without_close() {
    let path = write_rollout_fixture(
        "orphan",
        &[
            r#"{"timestamp":"2026-07-02T22:00:00.000Z","type":"response_item","payload":{"type":"function_call_output","output":"Chunk ID: abc\nProcess running with session ID 67890\nOutput:"}}"#,
        ],
    );

    let report =
        agent_session_registry_rollout_activity::rollout_activity_report(&path, now_unix() + 300);

    assert_eq!(report.status, "agent-active");
    assert_eq!(report.agent_instruction, "child-activity-running-wait");
    assert_eq!(report.last_running_session_id.as_deref(), Some("67890"));
    assert!(!report.running_session_closed);
    remove_fixture(path);
}

#[test]
fn rollout_activity_silent_without_heartbeat_requires_bounded_interrupt_receipt() {
    let path = write_rollout_fixture(
        "silent",
        &[
            r#"{"timestamp":"2026-07-02T22:00:00.000Z","type":"turn_context","payload":{"turn_id":"turn-silent"}}"#,
        ],
    );

    let report =
        agent_session_registry_rollout_activity::rollout_activity_report(&path, now_unix());

    assert_eq!(report.status, "silent");
    assert_eq!(
        report.agent_instruction,
        "child-activity-state-authoritative"
    );
    assert_eq!(report.current_turn_id.as_deref(), Some("turn-silent"));
    assert!(!report.running_session_closed);
    remove_fixture(path);
}
