use serde_json::json;

use super::{
    claude_fixture, install_codex_hooks, run_codex_hook_decision_with_env, show_agent_session_json,
    write_codex_asp_explore_rollout,
};

#[test]
fn native_asp_subagent_runtime_drift_requires_typed_replacement() {
    let root = claude_fixture();
    let codex_home = root.join(".codex-home");
    install_codex_hooks(&root, &codex_home);
    let root_session_id = "019f126d-0000-7000-8000-000000000062";
    let child_session_id = "019f126d-0000-7000-8000-000000000162";

    let decision = run_codex_hook_decision_with_env(
        &root,
        "subagent-start",
        json!({
            "hook_event_name": "SubagentStart",
            "session_id": root_session_id,
            "agent_id": child_session_id,
            "agent_type": "asp_explorer",
            "model": "gpt-5.6-sol",
            "reasoning_effort": "xhigh",
            "permission_mode": "default",
        }),
        &[("CODEX_THREAD_ID", root_session_id)],
    );

    assert_eq!(decision["decision"].as_str(), Some("allow"));
    assert_eq!(
        decision["fields"]["childSessionId"].as_str(),
        Some(child_session_id)
    );
    assert_eq!(
        decision["fields"]["observedModel"].as_str(),
        Some("gpt-5.6-sol")
    );
    assert_eq!(
        decision["fields"]["expectedModel"].as_str(),
        Some("gpt-5.4-mini")
    );
    let report = show_agent_session_json(&root, child_session_id);
    let session = &report["sessions"][0];
    assert_eq!(session["rootSessionId"].as_str(), Some(root_session_id));
    assert_eq!(session["sessionId"].as_str(), Some(child_session_id));
    assert_eq!(session["name"].as_str(), Some("asp-explore"));
    assert_eq!(session["status"].as_str(), Some("replacement-required"));
    assert_eq!(session["messageTargetId"].as_str(), Some(child_session_id));
    assert_eq!(session["model"].as_str(), Some("gpt-5.6-sol"));
    assert_eq!(
        decision["fields"]["nextAction"].as_str(),
        Some("retire-drifted-child-and-create-configured-replacement")
    );
}

#[test]
fn bootstrap_adopts_drifted_rollout_identity_as_unbound_before_runtime_repair() {
    let root = claude_fixture();
    let codex_home = root.join(".codex-home");
    install_codex_hooks(&root, &codex_home);
    let root_session_id = "019f126d-0000-7000-8000-000000000064";
    let child_session_id = "019f126d-0000-7000-8000-000000000164";
    write_codex_asp_explore_rollout(&root, root_session_id, child_session_id, "gpt-5.6-sol");

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_asp"))
        .args([
            "agent",
            "session",
            "bootstrap",
            "--name",
            "asp-explore",
            "--json",
        ])
        .current_dir(&root)
        .env("CODEX_HOME", &codex_home)
        .env("CODEX_THREAD_ID", root_session_id)
        .env("ASP_STATE_HOME", root.join(".agent-semantic-protocols"))
        .env_remove("PRJ_CACHE_HOME")
        .output()
        .expect("bootstrap drifted rollout resident");
    assert!(
        output.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    let bootstrap: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("bootstrap JSON");

    assert_eq!(
        bootstrap["state"].as_str(),
        Some("RebindExistingChildTarget")
    );
    assert_eq!(
        bootstrap["rolloutHistoryStatus"].as_str(),
        Some("adopted-reusable-rollout")
    );
    assert_eq!(
        bootstrap["rolloutHistoryAction"].as_str(),
        Some("resume-adopted-existing-child-then-validate-message-route")
    );
    assert_eq!(
        bootstrap["session"]["childSessionId"].as_str(),
        Some(child_session_id)
    );
    assert_eq!(
        bootstrap["session"]["messageTargetStatus"].as_str(),
        Some("unbound")
    );
    assert_eq!(bootstrap["session"]["model"].as_str(), Some("gpt-5.6-sol"));
    assert!(bootstrap.get("hostControlDirective").is_none());
    assert_eq!(
        bootstrap["choices"][0]["id"].as_str(),
        Some("resume-existing-child-for-live-target-rebind")
    );
}
