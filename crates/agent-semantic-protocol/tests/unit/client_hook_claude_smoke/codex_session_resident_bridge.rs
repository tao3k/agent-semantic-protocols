use serde_json::json;

use super::{
    claude_fixture, codex_asp_query_payload, install_codex_hooks, register_asp_explore_session,
    run_codex_hook_decision_with_activation, run_codex_hook_decision_with_env,
    run_codex_pre_tool_decision_with_env, show_agent_session_json,
};

#[test]
fn codex_session_start_reuses_registered_asp_explore() {
    let root = claude_fixture();
    let codex_home = root.join(".codex-home");
    install_codex_hooks(&root, &codex_home);
    register_asp_explore_session(
        &root,
        "019f126d-0000-7000-8000-000000000030",
        "019f126d-0000-7000-8000-000000000130",
    );

    let decision = run_codex_hook_decision_with_env(
        &root,
        "session-start",
        json!({"source": "session-start-smoke"}),
        &[("CODEX_THREAD_ID", "019f126d-0000-7000-8000-000000000030")],
    );

    assert_eq!(decision["decision"].as_str(), Some("deny"));
    assert_eq!(
        decision["fields"]["agentSessionAction"].as_str(),
        Some("reuse-resident-child")
    );
    assert_eq!(
        decision["fields"]["agentSessionSpawnPolicy"].as_str(),
        Some("registered-profile-valid-child-only")
    );
    assert_eq!(
        decision["fields"]["agentSessionValidationPolicy"].as_str(),
        Some("register-hard-validates-profile")
    );
    assert_eq!(
        decision["fields"]["agentSessionInvalidChildAction"].as_str(),
        Some("close-native-subagent-or-archive-temporary-thread-and-create-configured-child")
    );
    assert_eq!(
        decision["fields"]["agentSessionDuplicatePolicy"].as_str(),
        Some("one-active-resident-child-per-root-session-and-name")
    );
    assert_eq!(
        decision["fields"]["childSessionId"].as_str(),
        Some("019f126d-0000-7000-8000-000000000130")
    );
    assert_eq!(
        decision["fields"]["agentSessionExistingChildId"].as_str(),
        Some("019f126d-0000-7000-8000-000000000130")
    );
    assert_eq!(
        decision["fields"]["nextAction"].as_str(),
        Some("enter-bootstrap-pane-for-existing-child")
    );
}

#[test]
fn codex_subagent_start_reuses_registered_asp_explore() {
    let root = claude_fixture();
    let codex_home = root.join(".codex-home");
    install_codex_hooks(&root, &codex_home);
    let root_session_id = "019f126d-0000-7000-8000-000000000032";
    let child_session_id = "019f126d-0000-7000-8000-000000000132";
    register_asp_explore_session(&root, root_session_id, child_session_id);
    let decision = run_codex_hook_decision_with_env(
        &root,
        "subagent-start",
        json!({
            "agentRole": "asp_explorer",
            "childSessionId": "019f126d-0000-7000-8000-000000000232",
            "parentThreadId": root_session_id,
        }),
        &[("CODEX_THREAD_ID", root_session_id)],
    );
    assert_eq!(decision["decision"].as_str(), Some("deny"));
    assert_eq!(
        decision["fields"]["agentSessionAction"].as_str(),
        Some("reuse-resident-child")
    );
    assert_eq!(
        decision["fields"]["agentSessionExistingChildId"].as_str(),
        Some(child_session_id)
    );
    assert_eq!(
        decision["fields"]["childSessionId"].as_str(),
        Some(child_session_id)
    );
}

#[test]
fn codex_subagent_start_claims_native_child_without_thread_env() {
    let root = claude_fixture();
    let codex_home = root.join(".codex-home");
    install_codex_hooks(&root, &codex_home);
    let root_session_id = "019f126d-0000-7000-8000-000000000033";
    let child_session_id = "019f126d-0000-7000-8000-000000000133";
    let decision = run_codex_hook_decision_with_env(
        &root,
        "subagent-start",
        json!({
            "hook_event_name": "SubagentStart", "session_id": root_session_id,
            "agent_id": child_session_id, "agent_type": "asp_explorer",
            "model": "gpt-5.4-mini", "permission_mode": "default",
        }),
        &[("CODEX_THREAD_ID", "")],
    );
    assert_eq!(decision["decision"].as_str(), Some("allow"));
    let report = show_agent_session_json(&root, child_session_id);
    let session = &report["sessions"][0];
    assert_eq!(session["rootSessionId"].as_str(), Some(root_session_id));
    assert_eq!(session["sessionId"].as_str(), Some(child_session_id));
    assert_eq!(session["name"].as_str(), Some("asp-explore"));
    assert_eq!(session["status"].as_str(), Some("active"));
    assert_eq!(session["messageTargetId"].as_str(), Some(child_session_id));
    let stop = run_codex_hook_decision_with_env(
        &root,
        "subagent-stop",
        json!({
            "hook_event_name": "SubagentStop", "session_id": root_session_id,
            "agent_id": child_session_id, "agent_type": "asp_explorer",
            "model": "gpt-5.4-mini", "reasoning_effort": "low",
            "permission_mode": "default",
        }),
        &[("CODEX_THREAD_ID", "")],
    );
    assert_eq!(stop["decision"].as_str(), Some("allow"));
    assert_eq!(
        stop["fields"]["agentSessionAction"].as_str(),
        Some("subagent-stop-preserved-resident-idle")
    );
    assert_eq!(
        show_agent_session_json(&root, child_session_id)["sessions"][0]["status"].as_str(),
        Some("idle")
    );
}

#[test]
fn codex_subagent_start_rejects_duplicate_native_child_without_replacing_owner() {
    let root = claude_fixture();
    let codex_home = root.join(".codex-home");
    install_codex_hooks(&root, &codex_home);
    let root_session_id = "019f126d-0000-7000-8000-000000000034";
    let existing_child_id = "019f126d-0000-7000-8000-000000000134";
    register_asp_explore_session(&root, root_session_id, existing_child_id);
    let decision = run_codex_hook_decision_with_env(
        &root,
        "subagent-start",
        json!({
            "hook_event_name": "SubagentStart", "session_id": root_session_id,
            "agent_id": "019f126d-0000-7000-8000-000000000234",
            "agent_type": "asp_explorer", "model": "gpt-5.4-mini",
            "reasoning_effort": "low", "permission_mode": "default",
        }),
        &[("CODEX_THREAD_ID", "")],
    );
    assert_eq!(decision["decision"].as_str(), Some("deny"));
    assert_eq!(
        decision["fields"]["agentSessionExistingChildId"].as_str(),
        Some(existing_child_id)
    );
    assert_eq!(
        decision["fields"]["agentSessionDuplicateChildId"].as_str(),
        Some("019f126d-0000-7000-8000-000000000234")
    );
    assert_eq!(
        decision["fields"]["agentSessionDuplicateChildAction"].as_str(),
        Some("close-native-subagent")
    );
    assert_eq!(
        show_agent_session_json(&root, existing_child_id)["sessions"][0]["sessionId"].as_str(),
        Some(existing_child_id)
    );
}

#[test]
fn codex_installed_hook_full_resident_child_lifecycle_scenario() {
    let root = claude_fixture();
    let codex_home = root.join(".codex-home");
    install_codex_hooks(&root, &codex_home);
    let root_session_id = "019f126d-0000-7000-8000-000000000051";
    let child_session_id = "019f126d-0000-7000-8000-000000000151";
    register_asp_explore_session(&root, root_session_id, child_session_id);
    let child_start = run_codex_hook_decision_with_env(
        &root,
        "subagent-start",
        json!({
            "hook_event_name": "SubagentStart", "session_id": root_session_id,
            "agent_id": child_session_id, "agent_type": "asp_explorer", "model": "gpt-5.4-mini",
            "reasoning_effort": "low", "permission_mode": "default",
        }),
        &[("CODEX_THREAD_ID", root_session_id)],
    );
    assert_eq!(child_start["decision"].as_str(), Some("allow"));
    let session_start = run_codex_hook_decision_with_env(
        &root,
        "session-start",
        json!({"source": "full-lifecycle-scenario"}),
        &[("CODEX_THREAD_ID", root_session_id)],
    );
    assert_eq!(session_start["decision"].as_str(), Some("deny"));
    assert_eq!(
        session_start["fields"]["agentSessionAction"].as_str(),
        Some("reuse-resident-child")
    );
    assert_eq!(
        session_start["fields"]["childSessionId"].as_str(),
        Some(child_session_id)
    );
    let main_query = run_codex_pre_tool_decision_with_env(
        &root,
        codex_asp_query_payload("asp rust search lexical resident owner --view seeds"),
        &[("CODEX_THREAD_ID", root_session_id)],
    );
    assert_eq!(main_query["decision"].as_str(), Some("deny"));
    assert_eq!(
        main_query["fields"]["agentSessionRoute"].as_str(),
        Some("asp-explore")
    );
    assert_eq!(
        main_query["fields"]["childSessionId"].as_str(),
        Some(child_session_id),
        "{main_query}"
    );
    let child_query = run_codex_pre_tool_decision_with_env(
        &root,
        codex_asp_query_payload("asp rust search lexical resident owner --view seeds"),
        &[("CODEX_THREAD_ID", child_session_id)],
    );
    assert_eq!(child_query["decision"].as_str(), Some("allow"));
    assert_eq!(
        child_query["fields"]["agentSessionAction"].as_str(),
        Some("active-resident-child")
    );
    let post_tool = run_codex_hook_decision_with_env(
        &root,
        "post-tool",
        json!({
            "tool_name": "Bash",
            "tool_input": { "command": "asp rust search lexical resident owner --view seeds" },
            "tool_result": { "evidenceRef": "asp-evidence:full-lifecycle" }
        }),
        &[("CODEX_THREAD_ID", child_session_id)],
    );
    assert_eq!(post_tool["decision"].as_str(), Some("allow"));
    let report = show_agent_session_json(&root, child_session_id);
    let session = &report["sessions"][0];
    assert_eq!(
        session["lastEvidenceRef"].as_str(),
        Some("asp-evidence:full-lifecycle")
    );
    assert_eq!(
        session["lastCommand"].as_str(),
        Some("asp rust search lexical resident owner --view seeds")
    );
}

#[test]
fn codex_hook_repairs_missing_activation_before_classification() {
    let root = claude_fixture();
    let codex_home = root.join(".codex-home");
    install_codex_hooks(&root, &codex_home);
    let sync = std::process::Command::new(env!("CARGO_BIN_EXE_asp"))
        .arg("sync")
        .current_dir(&root)
        .env("HOME", &root)
        .env("CODEX_HOME", &codex_home)
        .env("ASP_STATE_HOME", root.join(".agent-semantic-protocols"))
        .env_remove("PRJ_CACHE_HOME")
        .output()
        .expect("sync activation fixture");
    assert!(
        sync.status.success(),
        "sync stdout: {}\nsync stderr: {}",
        String::from_utf8_lossy(&sync.stdout),
        String::from_utf8_lossy(&sync.stderr)
    );
    let paths = std::process::Command::new(env!("CARGO_BIN_EXE_asp"))
        .args(["hook", "paths"])
        .arg(&root)
        .env("HOME", &root)
        .env_remove("PRJ_CACHE_HOME")
        .output()
        .expect("read activation fixture path");
    assert!(
        paths.status.success(),
        "{}",
        String::from_utf8_lossy(&paths.stderr)
    );
    let paths_stdout = String::from_utf8(paths.stdout).expect("paths stdout is utf8");
    let activation_path = paths_stdout
        .lines()
        .find_map(|line| line.strip_prefix("activation="))
        .map(std::path::PathBuf::from)
        .expect("activation path receipt");
    assert!(activation_path.is_file(), "{}", activation_path.display());
    std::fs::remove_file(&activation_path).expect("remove activation fixture");

    let unrelated = run_codex_hook_decision_with_activation(
        &root,
        "pre-tool",
        json!({
            "tool_name": "Bash",
            "tool_input": { "command": "cargo test -p agent-semantic-protocol" }
        }),
        &[("HOME", root.to_str().expect("utf8 home path"))],
        &activation_path,
    );
    assert_eq!(unrelated["decision"].as_str(), Some("allow"), "{unrelated}");
    assert_eq!(
        unrelated["fields"]["activationRecoveryStatus"].as_str(),
        Some("reloaded-and-classified"),
        "{unrelated}"
    );
    assert!(activation_path.is_file(), "activation was not recreated");

    std::fs::remove_file(&activation_path).expect("remove repaired activation fixture");
    let source_read = run_codex_hook_decision_with_activation(
        &root,
        "pre-tool",
        json!({
            "tool_name": "Bash",
            "tool_input": { "command": "cat crates/agent-semantic-protocol/src/lib.rs" }
        }),
        &[("HOME", root.to_str().expect("utf8 home path"))],
        &activation_path,
    );
    assert_eq!(
        source_read["decision"].as_str(),
        Some("deny"),
        "{source_read}"
    );
    assert_eq!(
        source_read["fields"]["activationRecoveryStatus"].as_str(),
        Some("reloaded-and-classified"),
        "{source_read}"
    );
    assert!(activation_path.is_file(), "activation was not recreated");
}
