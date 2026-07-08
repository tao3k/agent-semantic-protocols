use std::fs;

use serde_json::json;

use crate::provider_command::support::{asp_command, temp_project_root};

#[test]
fn asp_agent_session_lifecycle_close_reconcile_and_gc_registry_rows() {
    let root = temp_project_root("agent-command-session-lifecycle-close-gc");

    let register_output = asp_command(&root)
        .args([
            "agent",
            "session",
            "register",
            "--name",
            "worker-a",
            "--child-session-id",
            "019f126d-0000-7000-8000-000000000130",
            "--root-session-id",
            "019f126d-0000-7000-8000-000000000030",
            "--roles",
            "subagent",
            "--model",
            "test-model",
            "--status",
            "active",
            "--json",
        ])
        .output()
        .expect("register lifecycle session");
    assert!(
        register_output.status.success(),
        "{}",
        String::from_utf8_lossy(&register_output.stderr)
    );

    let close_output = asp_command(&root)
        .args([
            "agent",
            "session",
            "close",
            "--name",
            "worker-a",
            "--root-session-id",
            "019f126d-0000-7000-8000-000000000030",
            "--json",
        ])
        .output()
        .expect("close lifecycle session");
    assert!(
        close_output.status.success(),
        "{}",
        String::from_utf8_lossy(&close_output.stderr)
    );
    let close_stdout = String::from_utf8(close_output.stdout).expect("close stdout");
    assert!(
        close_stdout.contains("\"command\": \"close\""),
        "{close_stdout}"
    );
    assert!(close_stdout.contains("\"affected\": 1"), "{close_stdout}");
    assert!(
        close_stdout.contains("\"status\": \"archived\""),
        "{close_stdout}"
    );

    let reconcile_output = asp_command(&root)
        .args([
            "agent",
            "session",
            "reconcile",
            "--root-session-id",
            "019f126d-0000-7000-8000-000000000030",
            "--json",
        ])
        .output()
        .expect("reconcile lifecycle sessions");
    assert!(
        reconcile_output.status.success(),
        "{}",
        String::from_utf8_lossy(&reconcile_output.stderr)
    );
    let reconcile_stdout = String::from_utf8(reconcile_output.stdout).expect("reconcile stdout");
    assert!(
        reconcile_stdout.contains("\"command\": \"reconcile\""),
        "{reconcile_stdout}"
    );
    assert!(
        reconcile_stdout.contains("\"affected\": 1"),
        "{reconcile_stdout}"
    );

    let gc_output = asp_command(&root)
        .args([
            "agent",
            "session",
            "gc",
            "--name",
            "worker-a",
            "--root-session-id",
            "019f126d-0000-7000-8000-000000000030",
            "--json",
        ])
        .output()
        .expect("gc lifecycle session");
    assert!(
        gc_output.status.success(),
        "{}",
        String::from_utf8_lossy(&gc_output.stderr)
    );
    let gc_stdout = String::from_utf8(gc_output.stdout).expect("gc stdout");
    assert!(gc_stdout.contains("\"command\": \"gc\""), "{gc_stdout}");
    assert!(gc_stdout.contains("\"affected\": 1"), "{gc_stdout}");
    assert!(
        gc_stdout.contains("019f126d-0000-7000-8000-000000000130"),
        "{gc_stdout}"
    );

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn asp_agent_session_lifecycle_audit_stays_root_scoped_and_fast() {
    let root = temp_project_root("agent-command-session-lifecycle-audit-hot-path");
    let sessions_dir = root.join("home").join(".codex").join("sessions");

    let register_output = asp_command(&root)
        .args([
            "agent",
            "session",
            "register",
            "--name",
            "worker-a",
            "--child-session-id",
            "019f126d-0000-7000-8000-000000000130",
            "--root-session-id",
            "019f126d-0000-7000-8000-000000000030",
            "--roles",
            "subagent",
            "--model",
            "test-model",
            "--status",
            "active",
            "--json",
        ])
        .output()
        .expect("register lifecycle session");
    assert!(
        register_output.status.success(),
        "{}",
        String::from_utf8_lossy(&register_output.stderr)
    );

    let relevant_root = sessions_dir.join("2026").join("07").join("08");
    fs::create_dir_all(&relevant_root).expect("create relevant rollout directory");
    fs::write(
        relevant_root
            .join("rollout-2026-07-08T08-09-21-019f126d-0000-7000-8000-000000000030.jsonl"),
        format!(
            "{}\n{}\n{}\n",
            json!({
                "type": "session_meta",
                "payload": {
                    "id": "019f126d-0000-7000-8000-000000000030",
                    "session_id": "019f126d-0000-7000-8000-000000000030",
                    "parent_thread_id": "019f126d-0000-7000-8000-000000000030",
                    "thread_source": "user",
                    "agent_role": "user"
                }
            }),
            json!({
                "timestamp": "2026-07-08T08:09:21Z",
                "type": "event_msg",
                "payload": {
                    "type": "agent_message",
                    "turn_id": "turn-root"
                }
            }),
            json!({
                "timestamp": "2026-07-08T08:09:22Z",
                "type": "response_item",
                "payload": {
                    "type": "thread_spawn",
                    "id": "019f126d-0000-7000-8000-000000000130",
                    "parent_thread_id": "019f126d-0000-7000-8000-000000000030",
                    "agent_role": "subagent"
                }
            })
        ),
    )
    .expect("write relevant rollout file");
    fs::write(
        relevant_root
            .join("rollout-2026-07-08T08-09-21-019f126d-0000-7000-8000-000000000130.jsonl"),
        format!(
            "{}\n{}\n{}\n",
            json!({
                "type": "session_meta",
                "payload": {
                    "id": "019f126d-0000-7000-8000-000000000130",
                    "session_id": "019f126d-0000-7000-8000-000000000030",
                    "parent_thread_id": "019f126d-0000-7000-8000-000000000030",
                    "thread_source": "subagent",
                    "agent_role": "subagent",
                    "source": {
                        "subagent": {
                            "thread_spawn": {
                                "parent_thread_id": "019f126d-0000-7000-8000-000000000030",
                                "depth": 1,
                                "agent_role": "subagent",
                                "agent_nickname": "worker-a"
                            }
                        }
                    }
                }
            }),
            json!({
                "timestamp": "2026-07-08T08:09:22Z",
                "type": "event_msg",
                "payload": {
                    "type": "agent_message",
                    "turn_id": "turn-child"
                }
            }),
            json!({
                "timestamp": "2026-07-08T08:09:23Z",
                "type": "event_msg",
                "payload": {
                    "type": "task_complete",
                    "turn_id": "turn-child"
                }
            })
        ),
    )
    .expect("write child rollout file");

    let close_output = asp_command(&root)
        .args([
            "agent",
            "session",
            "close",
            "--child-session-id",
            "019f126d-0000-7000-8000-000000000130",
            "--json",
        ])
        .output()
        .expect("close registered lifecycle session");
    assert!(
        close_output.status.success(),
        "{}",
        String::from_utf8_lossy(&close_output.stderr)
    );

    let status_output = asp_command(&root)
        .args([
            "agent",
            "session",
            "status",
            "--name",
            "worker-a",
            "--root-session-id",
            "019f126d-0000-7000-8000-000000000030",
            "--json",
        ])
        .output()
        .expect("status archived registered lifecycle session");
    assert!(
        status_output.status.success(),
        "{}",
        String::from_utf8_lossy(&status_output.stderr)
    );
    let status_stdout = String::from_utf8(status_output.stdout).expect("status stdout");
    let status_json: serde_json::Value =
        serde_json::from_str(&status_stdout).expect("parse status json");
    assert_eq!(
        status_json["registryStatus"].as_str(),
        Some("archived"),
        "{status_stdout}"
    );
    assert_eq!(
        status_json["routable"].as_bool(),
        Some(false),
        "{status_stdout}"
    );
    assert_eq!(
        status_json["sessionLifetime"].as_str(),
        Some("temporary"),
        "{status_stdout}"
    );
    assert_eq!(
        status_json["resident"].as_bool(),
        Some(false),
        "{status_stdout}"
    );
    assert_eq!(
        status_json["sessionLifetimeSource"].as_str(),
        Some("default"),
        "{status_stdout}"
    );

    let reuse_output = asp_command(&root)
        .args([
            "agent",
            "session",
            "resume",
            "--name",
            "worker-a",
            "--root-session-id",
            "019f126d-0000-7000-8000-000000000030",
            "--json",
        ])
        .output()
        .expect("reuse archived registered lifecycle session");
    let reuse_stdout = String::from_utf8(reuse_output.stdout).expect("reuse stdout");
    assert!(
        !reuse_output.status.success() || !reuse_stdout.contains("\"reused\": true"),
        "{reuse_stdout}"
    );
    assert!(
        !reuse_stdout.contains("019f126d-0000-7000-8000-000000000130"),
        "{reuse_stdout}"
    );

    let irrelevant_root = sessions_dir.join("2000").join("01").join("01");
    fs::create_dir_all(&irrelevant_root).expect("create irrelevant rollout directory");
    for index in 0..64 {
        let rollout_path = irrelevant_root.join(format!("rollout-irrelevant-{index}.jsonl"));
        fs::write(
            rollout_path,
            format!(
                "{}\n{}\n",
                json!({
                    "type": "session_meta",
                    "payload": {
                        "id": format!("irrelevant-session-{index}"),
                        "session_id": format!("irrelevant-root-{index}"),
                        "parent_thread_id": format!("irrelevant-parent-{index}"),
                        "agent_role": "subagent"
                    }
                }),
                json!({
                    "timestamp": "2000-01-01T00:00:00Z",
                    "type": "event_msg",
                    "payload": {
                        "type": "agent_message",
                        "turn_id": "turn-irrelevant"
                    }
                })
            ),
        )
        .expect("write irrelevant rollout file");
    }

    let audit_output = asp_command(&root)
        .env("CODEX_HOME", root.join("home").join(".codex"))
        .env("CODEX_THREAD_ID", "019f126d-0000-7000-8000-000000000030")
        .args([
            "agent",
            "session",
            "lifecycle",
            "audit",
            "--root-session-id",
            "019f126d-0000-7000-8000-000000000030",
            "--json",
        ])
        .output()
        .expect("run lifecycle audit");
    assert!(
        audit_output.status.success(),
        "{}",
        String::from_utf8_lossy(&audit_output.stderr)
    );

    let audit_stdout = String::from_utf8(audit_output.stdout).expect("audit stdout");
    let audit_json: serde_json::Value =
        serde_json::from_str(&audit_stdout).expect("parse lifecycle audit json");
    assert_eq!(
        audit_json["summary"]["rolloutSessionCount"].as_u64(),
        Some(0),
        "{audit_stdout}"
    );
    assert_eq!(
        audit_json["summary"]["missingRegisteredRolloutCount"].as_u64(),
        Some(1),
        "{audit_stdout}"
    );
    assert_eq!(
        audit_json["summary"]["rolloutOnlySessionCount"].as_u64(),
        Some(0),
        "{audit_stdout}"
    );
    assert_eq!(
        audit_json["summary"]["rolloutOnlyActiveCount"].as_u64(),
        Some(0),
        "{audit_stdout}"
    );
    assert_eq!(
        audit_json["summary"]["activeSubagentRollouts"].as_u64(),
        Some(0),
        "{audit_stdout}"
    );
    assert_eq!(
        audit_json["summary"]["scannedRolloutCount"].as_u64(),
        Some(0),
        "{audit_stdout}"
    );
    assert_eq!(
        audit_json["summary"]["skippedRolloutCount"].as_u64(),
        Some(0),
        "{audit_stdout}"
    );
    assert_eq!(
        audit_json["missingRegisteredRolloutSessions"][0]["status"].as_str(),
        Some("archived"),
        "{audit_stdout}"
    );

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn asp_agent_session_lifecycle_audit_downgrades_stale_rollout_only_active_child() {
    let root = temp_project_root("agent-command-session-lifecycle-audit-rollout-only-stale");
    let sessions_dir = root.join("home").join(".codex").join("sessions");
    let relevant_root = sessions_dir.join("2026").join("07").join("08");
    fs::create_dir_all(&relevant_root).expect("create relevant rollout directory");
    fs::write(
        relevant_root
            .join("rollout-2026-07-08T08-09-21-019f126d-0000-7000-8000-000000000031.jsonl"),
        format!(
            "{}\n{}\n{}\n",
            json!({
                "type": "session_meta",
                "payload": {
                    "id": "019f126d-0000-7000-8000-000000000031",
                    "session_id": "019f126d-0000-7000-8000-000000000031",
                    "parent_thread_id": "019f126d-0000-7000-8000-000000000031",
                    "thread_source": "user",
                    "agent_role": "user"
                }
            }),
            json!({
                "timestamp": "2026-07-08T08:09:21Z",
                "type": "event_msg",
                "payload": {
                    "type": "agent_message",
                    "turn_id": "turn-root"
                }
            }),
            json!({
                "timestamp": "2026-07-08T08:09:22Z",
                "type": "response_item",
                "payload": {
                    "type": "thread_spawn",
                    "id": "019f126d-0000-7000-8000-000000000131",
                    "parent_thread_id": "019f126d-0000-7000-8000-000000000031",
                    "agent_role": "asp_explorer"
                }
            })
        ),
    )
    .expect("write root rollout file");
    fs::write(
        relevant_root
            .join("rollout-2026-07-08T08-09-22-019f126d-0000-7000-8000-000000000131.jsonl"),
        format!(
            "{}\n{}\n",
            json!({
                "type": "session_meta",
                "payload": {
                    "id": "019f126d-0000-7000-8000-000000000131",
                    "session_id": "019f126d-0000-7000-8000-000000000031",
                    "parent_thread_id": "019f126d-0000-7000-8000-000000000031",
                    "thread_source": "subagent",
                    "agent_role": "asp_explorer",
                    "source": {
                        "subagent": {
                            "thread_spawn": {
                                "parent_thread_id": "019f126d-0000-7000-8000-000000000031",
                                "depth": 1,
                                "agent_role": "asp_explorer",
                                "agent_nickname": "ASP search"
                            }
                        }
                    }
                }
            }),
            json!({
                "timestamp": "2026-07-08T08:09:22Z",
                "type": "event_msg",
                "payload": {
                    "type": "agent_message",
                    "turn_id": "turn-child"
                }
            })
        ),
    )
    .expect("write stale child rollout file");

    let audit_output = asp_command(&root)
        .env("CODEX_HOME", root.join("home").join(".codex"))
        .env("CODEX_THREAD_ID", "019f126d-0000-7000-8000-000000000031")
        .args([
            "agent",
            "session",
            "lifecycle",
            "audit",
            "--root-session-id",
            "019f126d-0000-7000-8000-000000000031",
            "--json",
        ])
        .output()
        .expect("run lifecycle audit");
    assert!(
        audit_output.status.success(),
        "{}",
        String::from_utf8_lossy(&audit_output.stderr)
    );

    let audit_stdout = String::from_utf8(audit_output.stdout).expect("audit stdout");
    let audit_json: serde_json::Value =
        serde_json::from_str(&audit_stdout).expect("parse lifecycle audit json");
    assert_eq!(
        audit_json["summary"]["rolloutOnlySessionCount"].as_u64(),
        Some(0),
        "{audit_stdout}"
    );
    assert_eq!(
        audit_json["summary"]["rolloutOnlyActiveCount"].as_u64(),
        Some(0),
        "{audit_stdout}"
    );
    assert_eq!(
        audit_json["summary"]["rolloutOnlyOrphanRiskCount"].as_u64(),
        Some(0),
        "{audit_stdout}"
    );
    assert_eq!(
        audit_json["summary"]["activeSubagentRollouts"].as_u64(),
        Some(0),
        "{audit_stdout}"
    );
    assert!(
        audit_json["rolloutOnlySessions"]
            .as_array()
            .is_some_and(|sessions| sessions.is_empty()),
        "{audit_stdout}"
    );

    let _ = std::fs::remove_dir_all(root);
}
