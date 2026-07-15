use std::path::{Path, PathBuf};

use agent_semantic_hook::{latest_subagent_runtime_drift, latest_subagent_runtime_rebind_verified};
use agent_semantic_runtime::ensure_project_hook_state_dir;
use serde_json::Value;

const HOOK_EVENT_STATE_FILE: &str = "events.jsonl";

fn fixture_root(name: &str) -> PathBuf {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock after unix epoch")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "agent-semantic-hook-{name}-{}-{nonce}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).expect("create fixture root");
    root
}

fn write_events(root: &Path, events: &[Value]) {
    let state_path = ensure_project_hook_state_dir(root)
        .expect("create hook state")
        .join(HOOK_EVENT_STATE_FILE);
    let rendered = events
        .iter()
        .map(Value::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(state_path, format!("{rendered}\n")).expect("write hook events");
}

fn drift_start(root_session_id: &str, child_session_id: &str) -> Value {
    serde_json::json!({
        "event": "subagent-start",
        "fields": {
            "rootSessionId": root_session_id,
            "agentSessionObservedChildId": child_session_id,
            "agentSessionAction": "repair-native-subagent-runtime",
            "agentSessionObservedAgentType": "default",
            "agentSessionExpectedAgentType": "asp_explorer",
            "agentSessionObservedModel": "gpt-5.6-sol",
            "agentSessionExpectedModel": "gpt-5.4-mini",
            "agentSessionObservedReasoningEffort": "xhigh",
            "agentSessionExpectedReasoningEffort": "low"
        }
    })
}

fn completed_turn(
    root_session_id: &str,
    child_session_id: &str,
    model: &str,
    reasoning_effort: &str,
) -> Value {
    serde_json::json!({
        "event": "subagent-stop",
        "fields": {
            "rootSessionId": root_session_id,
            "hookObservedChildId": child_session_id,
            "agentSessionAction": "ignore-unmanaged-native-subagent",
            "hookObservedModel": model,
            "hookObservedReasoningEffort": reasoning_effort
        }
    })
}

#[test]
fn completed_v2_turn_preserves_runtime_drift_for_same_child_resume() {
    let root = fixture_root("runtime-drift-completed-turn");
    let root_session_id = "root-thread";
    let child_session_id = "resident-child";
    write_events(
        &root,
        &[
            drift_start(root_session_id, child_session_id),
            serde_json::json!({
                "event": "subagent-stop",
                "fields": {
                    "rootSessionId": root_session_id,
                    "agentSessionObservedChildId": child_session_id,
                    "agentSessionAction": "ignore-unmanaged-native-subagent"
                }
            }),
        ],
    );

    let observation = latest_subagent_runtime_drift(&root, root_session_id)
        .expect("read runtime drift")
        .expect("completed turn must preserve drift");
    assert_eq!(observation.child_session_id, child_session_id);
    assert_eq!(observation.observed_model.as_deref(), Some("gpt-5.6-sol"));
    assert_eq!(observation.consecutive_observation_count, 1);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn repeated_same_child_drift_counts_failed_runtime_rebind() {
    let root = fixture_root("runtime-drift-repeated-rebind");
    let root_session_id = "root-thread";
    let child_session_id = "resident-child";
    write_events(
        &root,
        &[
            drift_start(root_session_id, child_session_id),
            serde_json::json!({
                "event": "subagent-stop",
                "fields": {
                    "rootSessionId": root_session_id,
                    "agentSessionObservedChildId": child_session_id,
                    "agentSessionAction": "ignore-unmanaged-native-subagent"
                }
            }),
            drift_start(root_session_id, child_session_id),
        ],
    );

    let observation = latest_subagent_runtime_drift(&root, root_session_id)
        .expect("read runtime drift")
        .expect("repeated drift remains active");
    assert_eq!(observation.child_session_id, child_session_id);
    assert_eq!(observation.consecutive_observation_count, 2);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn resumed_turn_stop_counts_as_fresh_runtime_observation_without_new_start() {
    let root = fixture_root("runtime-drift-resume-stop");
    let root_session_id = "root-thread";
    let child_session_id = "resident-child";
    write_events(
        &root,
        &[
            drift_start(root_session_id, child_session_id),
            completed_turn(root_session_id, child_session_id, "gpt-5.6-sol", "xhigh"),
            completed_turn(root_session_id, child_session_id, "gpt-5.6-sol", "xhigh"),
        ],
    );

    let observation = latest_subagent_runtime_drift(&root, root_session_id)
        .expect("read runtime drift")
        .expect("resumed turn still drifts");
    assert_eq!(observation.child_session_id, child_session_id);
    assert_eq!(observation.consecutive_observation_count, 2);
    assert_eq!(observation.observed_model.as_deref(), Some("gpt-5.6-sol"));
    assert_eq!(
        observation.observed_reasoning_effort.as_deref(),
        Some("xhigh")
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn matching_runtime_on_untyped_child_does_not_clear_drift() {
    let root = fixture_root("runtime-drift-resume-repaired");
    let root_session_id = "root-thread";
    let child_session_id = "resident-child";
    write_events(
        &root,
        &[
            drift_start(root_session_id, child_session_id),
            completed_turn(root_session_id, child_session_id, "gpt-5.6-sol", "xhigh"),
            completed_turn(root_session_id, child_session_id, "gpt-5.4-mini", "low"),
        ],
    );

    let drift = latest_subagent_runtime_drift(&root, root_session_id)
        .expect("read runtime drift")
        .expect("model values cannot attest an untyped child");
    assert_eq!(drift.child_session_id, child_session_id);
    assert_eq!(drift.observed_agent_type, "default");
    assert_eq!(
        latest_subagent_runtime_rebind_verified(&root, root_session_id)
            .expect("read verified runtime rebind"),
        None
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn fresh_typed_replacement_start_clears_drift() {
    let root = fixture_root("runtime-drift-typed-replacement");
    let root_session_id = "root-thread";
    write_events(
        &root,
        &[
            drift_start(root_session_id, "drifted-child"),
            serde_json::json!({
                "event": "subagent-start",
                "fields": {
                    "rootSessionId": root_session_id,
                    "hookObservedChildId": "typed-replacement",
                    "hookObservedAgentType": "asp_explorer",
                    "hookObservedModel": "gpt-5.4-mini",
                    "hookObservedReasoningEffort": "low"
                }
            }),
        ],
    );

    assert_eq!(
        latest_subagent_runtime_drift(&root, root_session_id).expect("read runtime drift"),
        None
    );
    let verified = latest_subagent_runtime_rebind_verified(&root, root_session_id)
        .expect("read verified replacement")
        .expect("typed replacement must close drift");
    assert_eq!(verified.child_session_id, "typed-replacement");
    assert_eq!(verified.observed_agent_type, "asp_explorer");
    assert_eq!(verified.observation_source, "subagent-start");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn explicit_resident_archive_supersedes_runtime_drift() {
    let root = fixture_root("runtime-drift-explicit-archive");
    let root_session_id = "root-thread";
    let child_session_id = "resident-child";
    write_events(
        &root,
        &[
            drift_start(root_session_id, child_session_id),
            serde_json::json!({
                "event": "subagent-stop",
                "fields": {
                    "rootSessionId": root_session_id,
                    "agentSessionObservedChildId": child_session_id,
                    "agentSessionAction": "subagent-stop-archived-managed-child"
                }
            }),
        ],
    );

    assert_eq!(
        latest_subagent_runtime_drift(&root, root_session_id).expect("read runtime drift"),
        None
    );
    let _ = std::fs::remove_dir_all(root);
}
