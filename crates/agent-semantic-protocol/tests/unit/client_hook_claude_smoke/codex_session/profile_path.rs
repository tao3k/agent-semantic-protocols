use serde_json::json;

use crate::client_hook_claude_smoke::rollout_fixture::codex_rollout_test_stamp;

use super::{
    claude_fixture, install_codex_hooks, run_codex_hook_decision_with_env, show_agent_session_json,
};

fn write_subagent_rollout(
    codex_home: &std::path::Path,
    root_session_id: &str,
    child_session_id: &str,
    agent_path: &str,
) {
    let (rollout_dir_suffix, rollout_file_stamp) = codex_rollout_test_stamp(child_session_id);
    let rollout_dir = codex_home.join("sessions").join(rollout_dir_suffix);
    std::fs::create_dir_all(&rollout_dir).expect("create rollout directory");
    let rollout_path = rollout_dir.join(format!(
        "rollout-{rollout_file_stamp}-{child_session_id}.jsonl"
    ));
    let session_meta = json!({
        "timestamp": "2026-07-14T00:00:00.000Z",
        "type": "session_meta",
        "payload": {
            "id": child_session_id,
            "session_id": root_session_id,
            "parent_thread_id": root_session_id,
            "thread_source": "subagent",
            "agent_role": "default",
            "source": {
                "subagent": {
                    "thread_spawn": {
                        "parent_thread_id": root_session_id,
                        "depth": 1,
                        "agent_role": "default",
                        "agent_path": agent_path
                    }
                }
            }
        }
    });
    let turn_context = json!({
        "timestamp": "2026-07-14T00:00:01.000Z",
        "type": "turn_context",
        "payload": {
            "model": "gpt-5.4-mini",
            "reasoning_effort": "low",
            "sandbox_policy": "read-only",
            "approval_policy": "never",
            "collaboration_mode": "disabled"
        }
    });
    std::fs::write(rollout_path, format!("{session_meta}\n{turn_context}\n"))
        .expect("write rollout fixture");
}

#[test]
fn codex_subagent_start_maps_default_role_from_profile_path_to_resident_session_name() {
    let root = claude_fixture();
    let codex_home = root.join(".codex-home");
    install_codex_hooks(&root, &codex_home);
    let root_session_id = "019f126d-0000-7000-8000-000000000035";
    let child_session_id = "019f126d-0000-7000-8000-000000000135";
    write_subagent_rollout(
        &codex_home,
        root_session_id,
        child_session_id,
        "/Users/example/.codex/agents/asp-explorer.toml",
    );

    let decision = run_codex_hook_decision_with_env(
        &root,
        "subagent-start",
        json!({
            "hook_event_name": "SubagentStart",
            "session_id": root_session_id,
            "agent_id": child_session_id,
            "agent_type": "default",
            "model": "gpt-5.4-mini",
            "reasoning_effort": "low",
            "permission_mode": "default"
        }),
        &[("CODEX_THREAD_ID", "")],
    );
    assert_eq!(decision["decision"], "allow", "{decision:#}");

    let report = show_agent_session_json(&root, child_session_id);
    assert_eq!(report["sessions"][0]["rootSessionId"], root_session_id);
    assert_eq!(report["sessions"][0]["sessionId"], child_session_id);
    assert_eq!(report["sessions"][0]["name"], "asp-explore");
    assert_eq!(report["sessions"][0]["role"], "asp_explorer");
    assert_eq!(report["sessions"][0]["status"], "active");
    let message_target_id = report["sessions"][0].get("messageTargetId");
    assert_eq!(
        message_target_id.and_then(serde_json::Value::as_str),
        Some(child_session_id),
        "{report:#}"
    );
}

#[test]
fn codex_subagent_start_does_not_adopt_an_unrelated_default_profile() {
    let root = claude_fixture();
    let codex_home = root.join(".codex-home");
    install_codex_hooks(&root, &codex_home);
    let root_session_id = "019f126d-0000-7000-8000-000000000036";
    let child_session_id = "019f126d-0000-7000-8000-000000000136";
    write_subagent_rollout(
        &codex_home,
        root_session_id,
        child_session_id,
        "/Users/example/.codex/agents/asp-testing.toml",
    );

    let decision = run_codex_hook_decision_with_env(
        &root,
        "subagent-start",
        json!({
            "hook_event_name": "SubagentStart",
            "session_id": root_session_id,
            "agent_id": child_session_id,
            "agent_type": "default",
            "model": "gpt-5.4-mini",
            "reasoning_effort": "low",
            "permission_mode": "default"
        }),
        &[("CODEX_THREAD_ID", "")],
    );
    assert_eq!(
        decision["fields"]["agentSessionAction"], "ignore-unmanaged-native-subagent",
        "{decision:#}"
    );
    assert_eq!(
        decision["fields"]["agentSessionObservedAgentType"],
        "default"
    );
    assert_eq!(
        decision["fields"]["agentSessionExpectedAgentType"],
        "asp_explorer"
    );
}
