use serde_json::json;

use super::{
    claude_fixture, codex_asp_query_payload, install_codex_hooks, register_asp_explore_session,
    register_expired_asp_explore_session, run_codex_hook_decision_with_env,
    run_codex_pre_tool_decision_with_env, show_agent_session_json, write_codex_asp_explore_rollout,
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

    assert_eq!(decision["decision"].as_str(), Some("allow"));
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
        Some("close-delete-and-create-configured-child")
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
        decision["fields"]["agentSessionResumeId"].as_str(),
        Some("019f126d-0000-7000-8000-000000000130")
    );
    let message = decision["message"].as_str().unwrap_or_default();
    assert!(message.contains("ASP session-start reuse"));
    assert!(message.contains("already has active resident asp-explore child session"));
    assert!(
        message.contains("do not spawn another asp-explore session"),
        "{message}"
    );
    assert!(message.contains("019f126d-0000-7000-8000-000000000130"));
}

#[test]
fn codex_session_start_resumes_existing_non_routable_asp_explore() {
    let root = claude_fixture();
    let codex_home = root.join(".codex-home");
    install_codex_hooks(&root, &codex_home);
    register_expired_asp_explore_session(
        &root,
        "019f126d-0000-7000-8000-000000000032",
        "019f126d-0000-7000-8000-000000000132",
    );

    let decision = run_codex_hook_decision_with_env(
        &root,
        "session-start",
        json!({"source": "session-start-smoke"}),
        &[("CODEX_THREAD_ID", "019f126d-0000-7000-8000-000000000032")],
    );

    assert_eq!(decision["decision"].as_str(), Some("deny"));
    assert_eq!(
        decision["fields"]["agentSessionAction"].as_str(),
        Some("resume-existing-resident-child")
    );
    assert_eq!(
        decision["fields"]["nextAction"].as_str(),
        Some("resume-existing-resident-child")
    );
    assert_eq!(
        decision["fields"]["childSessionId"].as_str(),
        Some("019f126d-0000-7000-8000-000000000132")
    );
    assert_eq!(
        decision["fields"]["agentSessionResumeId"].as_str(),
        Some("019f126d-0000-7000-8000-000000000132")
    );
    assert!(decision["fields"].get("agentSessionBootstrap").is_none());
    let message = decision["message"].as_str().unwrap_or_default();
    assert!(message.contains("Resume that child session instead of creating a replacement"));
    assert!(message.contains("archive or delete"));
}

#[test]
fn codex_global_activation_uses_cwd_repo_registry_for_session_reuse() {
    let root = claude_fixture();
    let codex_home = root.join(".codex-home");
    install_codex_hooks(&root, &codex_home);
    force_activation_project_root_to_hook_state(&root);
    register_asp_explore_session(
        &root,
        "019f126d-0000-7000-8000-000000000031",
        "019f126d-0000-7000-8000-000000000131",
    );

    let decision = run_codex_hook_decision_with_env(
        &root,
        "session-start",
        json!({"source": "session-start-smoke"}),
        &[("CODEX_THREAD_ID", "019f126d-0000-7000-8000-000000000031")],
    );

    assert_eq!(decision["decision"].as_str(), Some("allow"));
    assert_eq!(
        decision["fields"]["agentSessionAction"].as_str(),
        Some("reuse-resident-child")
    );
    assert_eq!(
        decision["fields"]["childSessionId"].as_str(),
        Some("019f126d-0000-7000-8000-000000000131")
    );
    let message = decision["message"].as_str().unwrap_or_default();
    assert!(message.contains("ASP session-start reuse"));
    assert!(message.contains("do not spawn another asp-explore session"));
}

#[test]
fn codex_main_session_denies_asp_query_when_asp_explore_registered() {
    let root = claude_fixture();
    let codex_home = root.join(".codex-home");
    install_codex_hooks(&root, &codex_home);
    register_asp_explore_session(
        &root,
        "019f126d-0000-7000-8000-000000000001",
        "019f126d-0000-7000-8000-000000000101",
    );

    let decision = run_codex_pre_tool_decision_with_env(
        &root,
        codex_asp_query_payload("asp rust query src/lib.rs --workspace . --code"),
        &[("CODEX_THREAD_ID", "019f126d-0000-7000-8000-000000000001")],
    );

    assert_eq!(decision["decision"].as_str(), Some("deny"));
    assert_eq!(decision["reasonKind"].as_str(), Some("raw-broad-search"));
    assert_eq!(
        decision["fields"]["agentSessionRoute"].as_str(),
        Some("asp-explore")
    );
    assert_eq!(
        decision["fields"]["residentCodexAgentName"].as_str(),
        Some("asp_explorer")
    );
    assert_eq!(
        decision["fields"]["agentSessionLookupCommand"].as_str(),
        Some("asp agent session reuse --name asp-explore --json")
    );
    assert_eq!(
        decision["fields"]["agentSessionRegisterCommandTemplate"].as_str(),
        Some(
            "asp agent session register --name asp-explore --child-session-id <child-session-id> --role asp-explore",
        )
    );
    assert_eq!(
        decision["fields"]["agentSessionLifecycle"].as_str(),
        Some("resident")
    );
    assert_eq!(
        decision["fields"]["agentSessionStatusCheck"].as_str(),
        Some("asp-session-status-command")
    );
    assert_eq!(
        decision["fields"]["agentSessionStatusCommand"].as_str(),
        Some("asp agent session status --name asp-explore --json")
    );
    assert_eq!(
        decision["fields"]["agentSessionTimeoutPolicy"].as_str(),
        Some("timeout-is-not-duplicate-worker-trigger")
    );
    assert_eq!(
        decision["fields"]["agentSessionAction"].as_str(),
        Some("reuse-resident-child")
    );
    assert_eq!(
        decision["fields"]["requiredAction"].as_str(),
        Some("send-to-asp-explore")
    );
    assert_eq!(
        decision["fields"]["nextAction"].as_str(),
        Some("run-asp-command-in-registered-asp-explore-child")
    );
    assert_eq!(
        decision["fields"]["targetAgentName"].as_str(),
        Some("asp-explore")
    );
    assert_eq!(
        decision["fields"]["targetAgentRole"].as_str(),
        Some("asp-explore")
    );
    assert_eq!(
        decision["fields"]["forbiddenUntilResolved"].as_str(),
        Some("raw-source-fallback")
    );
    assert_eq!(
        decision["fields"]["completionReceipt"].as_str(),
        Some("asp-explore-child-command")
    );
    assert_eq!(
        decision["fields"]["childSessionId"].as_str(),
        Some("019f126d-0000-7000-8000-000000000101")
    );
    assert_eq!(
        decision["fields"]["agentSessionResumeId"].as_str(),
        Some("019f126d-0000-7000-8000-000000000101")
    );
    let message = decision["message"].as_str().unwrap_or_default();
    assert!(message.contains("ASP denied main-session ASP exploration"));
    assert!(message.contains("asp-explore"));
    assert!(message.contains("Reuse or resume"));
    assert!(message.contains("019f126d-0000-7000-8000-000000000101"));

    let repeated = run_codex_pre_tool_decision_with_env(
        &root,
        codex_asp_query_payload("asp rust query src/lib.rs --workspace . --code"),
        &[("CODEX_THREAD_ID", "019f126d-0000-7000-8000-000000000001")],
    );
    assert_eq!(repeated["fields"]["denyReplay"].as_str(), Some("repeated"));
    assert_eq!(
        repeated["fields"]["denyReplayMessagePolicy"].as_str(),
        Some("preserve-agent-session-route")
    );
    let repeated_message = repeated["message"].as_str().unwrap_or_default();
    assert!(repeated_message.contains("ASP denied main-session ASP exploration"));
    assert!(repeated_message.contains("asp-explore"));
}

#[test]
fn codex_main_session_denies_env_prefixed_asp_query_when_asp_explore_registered() {
    let root = claude_fixture();
    let codex_home = root.join(".codex-home");
    install_codex_hooks(&root, &codex_home);
    register_asp_explore_session(
        &root,
        "019f126d-0000-7000-8000-000000000002",
        "019f126d-0000-7000-8000-000000000102",
    );

    let decision = run_codex_pre_tool_decision_with_env(
        &root,
        codex_asp_query_payload(
            "env CODEX_THREAD_ID=019f126d-0000-7000-8000-000000000102 \
             ASP_ROOT_SESSION_ID=019f126d-0000-7000-8000-000000000002 \
             ./target/debug/asp rust query src/lib.rs --workspace . --code",
        ),
        &[("CODEX_THREAD_ID", "019f126d-0000-7000-8000-000000000002")],
    );

    assert_eq!(decision["decision"].as_str(), Some("deny"));
    assert_eq!(decision["reasonKind"].as_str(), Some("raw-broad-search"));
    assert_eq!(
        decision["fields"]["agentSessionRoute"].as_str(),
        Some("asp-explore")
    );
    assert_eq!(
        decision["fields"]["agentSessionAction"].as_str(),
        Some("reuse-resident-child")
    );
    assert_eq!(
        decision["fields"]["requiredAction"].as_str(),
        Some("send-to-asp-explore")
    );
    assert_eq!(
        decision["fields"]["childSessionId"].as_str(),
        Some("019f126d-0000-7000-8000-000000000102")
    );
    let message = decision["message"].as_str().unwrap_or_default();
    assert!(message.contains("ASP denied main-session ASP exploration"));
    assert!(message.contains("019f126d-0000-7000-8000-000000000102"));
}

#[test]
fn codex_installed_hook_full_resident_child_lifecycle_scenario() {
    let root = claude_fixture();
    let codex_home = root.join(".codex-home");
    install_codex_hooks(&root, &codex_home);
    let root_session_id = "019f126d-0000-7000-8000-000000000051";
    let child_session_id = "019f126d-0000-7000-8000-000000000151";
    register_asp_explore_session(&root, root_session_id, child_session_id);

    let session_start = run_codex_hook_decision_with_env(
        &root,
        "session-start",
        json!({"source": "full-lifecycle-scenario"}),
        &[("CODEX_THREAD_ID", root_session_id)],
    );
    assert_eq!(session_start["decision"].as_str(), Some("allow"));
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
        codex_asp_query_payload("asp rust query src/lib.rs --workspace . --code"),
        &[("CODEX_THREAD_ID", root_session_id)],
    );
    assert_eq!(main_query["decision"].as_str(), Some("deny"));
    assert_eq!(
        main_query["fields"]["agentSessionRoute"].as_str(),
        Some("asp-explore")
    );
    assert_eq!(
        main_query["fields"]["childSessionId"].as_str(),
        Some(child_session_id)
    );

    let child_query = run_codex_pre_tool_decision_with_env(
        &root,
        codex_asp_query_payload("asp rust query src/lib.rs --workspace . --code"),
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
            "tool_input": {
                "command": "asp rust query src/lib.rs --workspace . --code"
            },
            "tool_result": {
                "evidenceRef": "asp-evidence:full-lifecycle"
            }
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
        Some("asp rust query src/lib.rs --workspace . --code")
    );
}

#[test]
fn codex_main_session_reuses_model_drifted_asp_explore_registration() {
    let root = claude_fixture();
    let codex_home = root.join(".codex-home");
    install_codex_hooks(&root, &codex_home);
    register_asp_explore_session(
        &root,
        "019f126d-0000-7000-8000-000000000011",
        "019f126d-0000-7000-8000-000000000111",
    );
    write_codex_asp_explore_rollout(
        &root,
        "019f126d-0000-7000-8000-000000000011",
        "019f126d-0000-7000-8000-000000000111",
        "gpt-5.5",
    );

    let decision = run_codex_pre_tool_decision_with_env(
        &root,
        codex_asp_query_payload("asp rust query src/lib.rs --workspace . --code"),
        &[("CODEX_THREAD_ID", "019f126d-0000-7000-8000-000000000011")],
    );

    assert_eq!(decision["decision"].as_str(), Some("deny"));
    assert_eq!(
        decision["fields"]["agentSessionAction"].as_str(),
        Some("reuse-resident-child")
    );
    assert_eq!(
        decision["fields"]["childSessionId"].as_str(),
        Some("019f126d-0000-7000-8000-000000000111")
    );
    let message = decision["message"].as_str().unwrap_or_default();
    assert!(message.contains("do not spawn another asp-explore session"));
}

#[test]
fn asp_binary_denies_main_session_query_when_asp_explore_registered() {
    let root = claude_fixture();
    register_asp_explore_session(
        &root,
        "019f126d-0000-7000-8000-000000000040",
        "019f126d-0000-7000-8000-000000000140",
    );

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_asp"))
        .args(["rust", "query", "src/lib.rs", "--workspace", ".", "--code"])
        .current_dir(&root)
        .env("CODEX_HOME", root.join(".codex-home"))
        .env("CODEX_THREAD_ID", "019f126d-0000-7000-8000-000000000040")
        .env("ASP_STATE_HOME", root.join(".agent-semantic-protocols"))
        .env_remove("PRJ_CACHE_HOME")
        .output()
        .expect("run asp rust query in main session");

    assert!(!output.status.success());
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(combined.contains("ASP query/search command denied in main agent session"));
    assert!(combined.contains("childSessionId=019f126d-0000-7000-8000-000000000140"));
    assert!(combined.contains("do not spawn another asp-explore session"));
}

#[test]
fn asp_binary_denies_main_session_query_without_asp_explore_registered() {
    let root = claude_fixture();

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_asp"))
        .args(["org", "query", "agent-plan-id"])
        .current_dir(&root)
        .env("CODEX_HOME", root.join(".codex-home"))
        .env("CODEX_THREAD_ID", "019f126d-0000-7000-8000-000000000041")
        .env("ASP_STATE_HOME", root.join(".agent-semantic-protocols"))
        .env_remove("PRJ_CACHE_HOME")
        .output()
        .expect("run asp org query in main session");

    assert!(!output.status.success());
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(combined.contains("no active asp-explore child session is registered"));
    assert!(combined.contains("asp agent session register --guide"));
}

#[test]
fn asp_binary_session_gate_does_not_apply_outside_agent_session() {
    let root = claude_fixture();

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_asp"))
        .args(["rust", "query", "src/lib.rs", "--workspace", ".", "--code"])
        .current_dir(&root)
        .env("PATH", prepend_path(&root.join(".bin")))
        .env("CODEX_HOME", root.join(".codex-home"))
        .env("ASP_STATE_HOME", root.join(".agent-semantic-protocols"))
        .env_remove("CODEX_THREAD_ID")
        .env_remove("CLAUDE_CODE_SESSION_ID")
        .env_remove("AGENT_SESSION_ID")
        .env_remove("SESSION_ID")
        .env_remove("PRJ_CACHE_HOME")
        .output()
        .expect("run asp rust query outside agent session");

    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!combined.contains("ASP query/search command denied"));
}

#[test]
fn codex_main_session_denies_asp_query_when_asp_explore_is_expired() {
    let root = claude_fixture();
    let codex_home = root.join(".codex-home");
    install_codex_hooks(&root, &codex_home);
    register_expired_asp_explore_session(
        &root,
        "019f126d-0000-7000-8000-000000000006",
        "019f126d-0000-7000-8000-000000000106",
    );

    let decision = run_codex_pre_tool_decision_with_env(
        &root,
        codex_asp_query_payload("asp rust query src/lib.rs --workspace . --code"),
        &[("CODEX_THREAD_ID", "019f126d-0000-7000-8000-000000000006")],
    );

    assert_eq!(decision["decision"].as_str(), Some("deny"));
    assert_eq!(
        decision["fields"]["agentSessionRoute"].as_str(),
        Some("asp-explore")
    );
    assert_eq!(
        decision["fields"]["agentSessionAction"].as_str(),
        Some("start-resident-child")
    );
    assert_eq!(
        decision["fields"]["residentCodexAgentName"].as_str(),
        Some("asp_explorer")
    );
    assert_eq!(
        decision["fields"]["agentSessionLookupCommand"].as_str(),
        Some("asp agent session reuse --name asp-explore --json")
    );
    assert_eq!(
        decision["fields"]["agentSessionRegisterCommandTemplate"].as_str(),
        Some(
            "asp agent session register --name asp-explore --child-session-id <child-session-id> --role asp-explore",
        )
    );
    assert!(decision["fields"].get("childSessionId").is_none());
    let message = decision["message"].as_str().unwrap_or_default();
    assert!(message.contains("ASP session-start bootstrap required"));
    assert!(message.contains("asp agent session status --name asp-explore --json"));
}

#[test]
fn codex_main_session_denies_asp_query_without_asp_explore_registered() {
    let root = claude_fixture();
    let codex_home = root.join(".codex-home");
    install_codex_hooks(&root, &codex_home);

    let decision = run_codex_pre_tool_decision_with_env(
        &root,
        codex_asp_query_payload("asp rust query src/lib.rs --workspace . --code"),
        &[("CODEX_THREAD_ID", "019f126d-0000-7000-8000-000000000002")],
    );

    assert_eq!(decision["decision"].as_str(), Some("deny"));
    assert_eq!(
        decision["fields"]["agentSessionRoute"].as_str(),
        Some("asp-explore")
    );
    assert_eq!(
        decision["fields"]["agentSessionLifecycle"].as_str(),
        Some("resident")
    );
    assert_eq!(
        decision["fields"]["agentSessionStatusCheck"].as_str(),
        Some("asp-session-status-command")
    );
    assert_eq!(
        decision["fields"]["agentSessionStatusCommand"].as_str(),
        Some("asp agent session status --name asp-explore --json")
    );
    assert_eq!(
        decision["fields"]["agentSessionTimeoutPolicy"].as_str(),
        Some("timeout-is-not-duplicate-worker-trigger")
    );
    assert_eq!(
        decision["fields"]["agentSessionAction"].as_str(),
        Some("start-resident-child")
    );
    assert_eq!(
        decision["fields"]["requiredAction"].as_str(),
        Some("start-asp-explore-child")
    );
    assert_eq!(
        decision["fields"]["nextAction"].as_str(),
        Some("run-asp-agent-session-register-guide")
    );
    assert_eq!(
        decision["fields"]["targetAgentName"].as_str(),
        Some("asp-explore")
    );
    assert_eq!(
        decision["fields"]["targetAgentRole"].as_str(),
        Some("asp-explore")
    );
    assert_eq!(
        decision["fields"]["forbiddenUntilResolved"].as_str(),
        Some("raw-source-fallback")
    );
    assert_eq!(
        decision["fields"]["completionReceipt"].as_str(),
        Some("asp-explore-child-registration")
    );
    assert_eq!(
        decision["fields"]["agentSessionBootstrap"].as_str(),
        Some("session-start-reminder")
    );
    assert_eq!(
        decision["fields"]["agentSessionBootstrapGuideCommand"].as_str(),
        Some("asp agent session register --guide")
    );
    assert_eq!(
        decision["fields"]["residentCodexAgentName"].as_str(),
        Some("asp_explorer")
    );
    assert_eq!(
        decision["fields"]["agentSessionLookupCommand"].as_str(),
        Some("asp agent session reuse --name asp-explore --json")
    );
    assert_eq!(
        decision["fields"]["agentSessionRegisterCommandTemplate"].as_str(),
        Some(
            "asp agent session register --name asp-explore --child-session-id <child-session-id> --role asp-explore",
        )
    );
    assert_eq!(
        decision["fields"]["rootSessionId"].as_str(),
        Some("019f126d-0000-7000-8000-000000000002")
    );
    assert!(decision["fields"].get("childSessionId").is_none());
    let message = decision["message"].as_str().unwrap_or_default();
    assert!(message.contains("ASP session-start bootstrap required"));
    assert!(message.contains("Action step flow"));
    assert!(message.contains("Codex action: start the configured subagent `asp_explorer`"));
    assert!(message.contains("Shell action: register the returned child session id"));
    assert!(message.contains("asp agent session status --name asp-explore --json"));
    assert!(message.contains("--child-session-id <child-session-id>"));
    assert!(message.contains("Do not use `asp agent session fork` as bootstrap"));
    assert!(message.contains("Retry the original tool command"));
}

#[test]
fn codex_session_start_bootstraps_missing_asp_explore() {
    let root = claude_fixture();
    let codex_home = root.join(".codex-home");
    install_codex_hooks(&root, &codex_home);

    let decision = run_codex_hook_decision_with_env(
        &root,
        "session-start",
        json!({"source": "session-start-smoke"}),
        &[("CODEX_THREAD_ID", "019f126d-0000-7000-8000-000000000020")],
    );

    assert_eq!(decision["decision"].as_str(), Some("allow"));
    assert_eq!(
        decision["fields"]["agentSessionBootstrap"].as_str(),
        Some("session-start-reminder")
    );
    assert_eq!(
        decision["fields"]["agentSessionAction"].as_str(),
        Some("start-resident-child")
    );
    assert_eq!(
        decision["fields"]["residentCodexAgentName"].as_str(),
        Some("asp_explorer")
    );
    assert_eq!(
        decision["fields"]["agentSessionLookupCommand"].as_str(),
        Some("asp agent session reuse --name asp-explore --json")
    );
    assert_eq!(
        decision["fields"]["agentSessionRegisterCommandTemplate"].as_str(),
        Some(
            "asp agent session register --name asp-explore --child-session-id <child-session-id> --role asp-explore",
        )
    );
    assert_eq!(
        decision["fields"]["rootSessionId"].as_str(),
        Some("019f126d-0000-7000-8000-000000000020")
    );
    let message = decision["message"].as_str().unwrap_or_default();
    assert!(message.contains("ASP session-start bootstrap"));
    assert!(message.contains("agent `asp_explorer`"));
    assert!(message.contains("--child-session-id <child-session-id>"));
    assert!(message.contains("asp-explore"));
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
use super::{force_activation_project_root_to_hook_state, prepend_path};
