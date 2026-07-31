use super::{
    claude_fixture, codex_asp_query_payload, install_codex_hooks, run_codex_hook_decision_with_env,
    run_codex_pre_tool_decision_with_env,
};
use serde_json::json;

#[test]
fn codex_ready_bootstrap_projects_exact_derived_dispatch_claim() {
    let root = claude_fixture();
    let codex_home = root.join(".codex-home");
    install_codex_hooks(&root, &codex_home);
    let root_session_id = "019f126d-0000-7000-8000-000000000060";
    let child_session_id = "019f126d-0000-7000-8000-000000000160";
    let start = run_codex_hook_decision_with_env(
        &root,
        "subagent-start",
        json!({
            "hook_event_name": "SubagentStart",
            "session_id": root_session_id,
            "agent_id": child_session_id,
            "agent_type": "asp_explorer",
            "model": "gpt-5.4-mini",
            "permission_mode": "default",
        }),
        &[("CODEX_THREAD_ID", root_session_id)],
    );
    assert_eq!(start["decision"].as_str(), Some("allow"));

    let command_json =
        serde_json::to_string(&vec!["/usr/bin/true"]).expect("canonical command JSON");
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_asp"))
        .args([
            "agent",
            "session",
            "bootstrap",
            "--name",
            "asp-explore",
            "--root-session-id",
            root_session_id,
            "--receipt-kind",
            "dispatch-execution-receipt.v1",
            "--command-json",
            command_json.as_str(),
            "--json",
        ])
        .current_dir(&root)
        .env("CODEX_HOME", &codex_home)
        .env("CODEX_THREAD_ID", root_session_id)
        .env("ASP_STATE_HOME", root.join(".agent-semantic-protocols"))
        .env_remove("PRJ_CACHE_HOME")
        .output()
        .expect("bootstrap Ready dispatch projection");
    assert!(
        output.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    let bootstrap: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("bootstrap JSON");
    assert_eq!(bootstrap["state"].as_str(), Some("Ready"));
    assert_eq!(
        bootstrap["choices"][0]["platformAction"].as_str(),
        Some(
            "asp agent session dispatch-claim --name 'asp-explore' \
--root-session-id '019f126d-0000-7000-8000-000000000060' \
--receipt-kind 'dispatch-execution-receipt.v1' \
--command-json '[\"/usr/bin/true\"]' --resident-bridge --json"
        )
    );
    assert_eq!(
        bootstrap["choices"][0]["requiredInputs"],
        serde_json::json!([])
    );
}

#[test]
fn codex_native_subagent_start_stop_bridge_stays_inside_scenario_gate() {
    let root = claude_fixture();
    let codex_home = root.join(".codex-home");
    install_codex_hooks(&root, &codex_home);
    let root_session_id = "019f126d-0000-7000-8000-000000000063";
    let child_session_id = "019f126d-0000-7000-8000-000000000163";
    let payload = json!({
        "hook_event_name": "SubagentStart",
        "session_id": root_session_id,
        "agent_id": child_session_id,
        "agent_type": "asp_explorer",
        "model": "gpt-5.4-mini",
        "permission_mode": "default",
    });

    let start_at = std::time::Instant::now();
    let start = run_codex_hook_decision_with_env(
        &root,
        "subagent-start",
        payload,
        &[("CODEX_THREAD_ID", root_session_id)],
    );
    let start_elapsed = start_at.elapsed();
    assert_eq!(start["decision"].as_str(), Some("allow"));
    assert!(
        start_elapsed < std::time::Duration::from_millis(500),
        "SubagentStart bridge exceeded 500ms: {start_elapsed:?}"
    );

    let stop_at = std::time::Instant::now();
    let stop = run_codex_hook_decision_with_env(
        &root,
        "subagent-stop",
        json!({
            "hook_event_name": "SubagentStop",
            "session_id": root_session_id,
            "agent_id": child_session_id,
            "agent_type": "asp_explorer",
            "model": "gpt-5.4-mini",
            "permission_mode": "default",
        }),
        &[("CODEX_THREAD_ID", root_session_id)],
    );
    let stop_elapsed = stop_at.elapsed();
    assert_eq!(stop["decision"].as_str(), Some("allow"));
    assert!(
        stop_elapsed < std::time::Duration::from_millis(500),
        "SubagentStop bridge exceeded 500ms: {stop_elapsed:?}"
    );
}

#[test]
fn codex_main_session_does_not_require_asp_explore_before_non_asp_tool() {
    let root = claude_fixture();
    let codex_home = root.join(".codex-home");
    install_codex_hooks(&root, &codex_home);

    let decision = run_codex_pre_tool_decision_with_env(
        &root,
        codex_asp_query_payload("printf ok"),
        &[("CODEX_THREAD_ID", "019f126d-0000-7000-8000-000000000004")],
    );

    assert_eq!(decision["decision"].as_str(), Some("allow"));
}
