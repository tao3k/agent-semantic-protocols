use super::{
    claude_fixture, install_codex_hooks, register_asp_explore_session,
    run_codex_hook_decision_with_env, show_agent_session_json,
};

#[test]
fn codex_session_start_resumes_completed_resident_after_restart_without_creating_another() {
    let root = claude_fixture();
    let codex_home = root.join(".codex-home");
    install_codex_hooks(&root, &codex_home);
    let root_session_id = "019f126d-0000-7000-8000-000000000070";
    let child_session_id = "019f126d-0000-7000-8000-000000000170";
    register_asp_explore_session(&root, root_session_id, child_session_id);
    super::rollout_fixture::append_codex_rollout_terminal_event(
        &root,
        child_session_id,
        "task_complete",
    );

    let decision = run_codex_hook_decision_with_env(
        &root,
        "session-start",
        serde_json::json!({"source": "restart-resume"}),
        &[("CODEX_THREAD_ID", root_session_id)],
    );

    assert_eq!(decision["decision"].as_str(), Some("deny"));
    assert_eq!(
        decision["fields"]["agentSessionAction"].as_str(),
        Some("resume-existing-resident-child")
    );
    assert_eq!(
        decision["fields"]["agentSessionReconciliation"].as_str(),
        Some("rollout-resumable")
    );
    assert_eq!(
        decision["fields"]["agentSessionRolloutLookup"].as_str(),
        Some("session-id-fast-path")
    );
    assert_eq!(
        decision["fields"]["childSessionId"].as_str(),
        Some(child_session_id)
    );
    let report = show_agent_session_json(&root, child_session_id);
    assert_eq!(report["sessions"].as_array().map(Vec::len), Some(1));
}

#[test]
fn codex_session_start_keeps_existing_resident_when_rollout_is_missing() {
    let root = claude_fixture();
    let codex_home = root.join(".codex-home");
    install_codex_hooks(&root, &codex_home);
    let root_session_id = "019f126d-0000-7000-8000-000000000071";
    let child_session_id = "019f126d-0000-7000-8000-000000000171";
    register_asp_explore_session(&root, root_session_id, child_session_id);
    std::fs::remove_dir_all(codex_home.join("sessions")).expect("remove rollout source");

    let decision = run_codex_hook_decision_with_env(
        &root,
        "session-start",
        serde_json::json!({"source": "restart-missing-rollout"}),
        &[("CODEX_THREAD_ID", root_session_id)],
    );

    assert_eq!(decision["decision"].as_str(), Some("deny"));
    assert_eq!(
        decision["fields"]["agentSessionAction"].as_str(),
        Some("resume-existing-resident-child")
    );
    assert_eq!(
        decision["fields"]["agentSessionReconciliation"].as_str(),
        Some("rollout-missing")
    );
    assert_eq!(
        decision["fields"]["childSessionId"].as_str(),
        Some(child_session_id)
    );
    let report = show_agent_session_json(&root, child_session_id);
    assert_eq!(report["sessions"].as_array().map(Vec::len), Some(1));
}

#[test]
fn codex_subagent_stop_preserves_resident_and_rejects_duplicate_start() {
    let root = claude_fixture();
    let codex_home = root.join(".codex-home");
    install_codex_hooks(&root, &codex_home);
    let root_session_id = "019f126d-0000-7000-8000-000000000072";
    let archived_child_id = "019f126d-0000-7000-8000-000000000172";
    let replacement_child_id = "019f126d-0000-7000-8000-000000000272";
    register_asp_explore_session(&root, root_session_id, archived_child_id);

    let stop = run_codex_hook_decision_with_env(
        &root,
        "subagent-stop",
        serde_json::json!({
            "hook_event_name": "SubagentStop",
            "session_id": root_session_id,
            "agent_id": archived_child_id,
            "agent_type": "asp_explorer",
            "model": "gpt-5.4-mini",
            "reasoning_effort": "low",
            "permission_mode": "default",
        }),
        &[("CODEX_THREAD_ID", root_session_id)],
    );
    assert_eq!(stop["decision"].as_str(), Some("allow"));
    assert_eq!(
        stop["fields"]["agentSessionAction"].as_str(),
        Some("subagent-stop-preserved-resident-idle")
    );

    let start = run_codex_hook_decision_with_env(
        &root,
        "subagent-start",
        serde_json::json!({
            "hook_event_name": "SubagentStart",
            "session_id": root_session_id,
            "agent_id": replacement_child_id,
            "agent_type": "asp_explorer",
            "model": "gpt-5.4-mini",
            "reasoning_effort": "low",
            "permission_mode": "default",
        }),
        &[("CODEX_THREAD_ID", root_session_id)],
    );
    assert_eq!(start["decision"].as_str(), Some("deny"));
    assert_eq!(
        start["fields"]["agentSessionAction"].as_str(),
        Some("reuse-resident-child")
    );
    let report = show_agent_session_json(&root, archived_child_id);
    assert_eq!(report["sessions"].as_array().map(Vec::len), Some(1));
    assert_eq!(
        report["sessions"][0]["sessionId"].as_str(),
        Some(archived_child_id)
    );
}
