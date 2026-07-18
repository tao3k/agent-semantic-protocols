use serde_json::json;

use super::{
    claude_fixture, codex_asp_query_payload, install_codex_hooks, register_asp_explore_session,
    run_codex_hook_decision_with_env, run_codex_pre_tool_decision_with_env,
    show_agent_session_json, write_hook_config,
};

#[test]
fn codex_main_session_allows_agent_session_register_bootstrap() {
    let root = claude_fixture();
    let codex_home = root.join(".codex-home");
    install_codex_hooks(&root, &codex_home);
    let decision = run_codex_pre_tool_decision_with_env(
        &root,
        codex_asp_query_payload(
            "asp agent session register --name asp-explore --child-session-id child --role asp-explore",
        ),
        &[("CODEX_THREAD_ID", "019f126d-0000-7000-8000-000000000005")],
    );
    assert_eq!(decision["decision"].as_str(), Some("allow"));
}

#[test]
fn codex_main_session_allows_agent_session_reuse_lookup() {
    let root = claude_fixture();
    let codex_home = root.join(".codex-home");
    install_codex_hooks(&root, &codex_home);
    let decision = run_codex_pre_tool_decision_with_env(
        &root,
        codex_asp_query_payload("asp agent session reuse --name asp-explore --json"),
        &[("CODEX_THREAD_ID", "019f126d-0000-7000-8000-000000000007")],
    );
    assert_eq!(decision["decision"].as_str(), Some("allow"));
}

#[test]
fn codex_asp_explore_session_can_run_asp_query() {
    let root = claude_fixture();
    let codex_home = root.join(".codex-home");
    install_codex_hooks(&root, &codex_home);
    register_asp_explore_session(
        &root,
        "019f126d-0000-7000-8000-000000000003",
        "019f126d-0000-7000-8000-000000000103",
    );
    let decision = run_codex_pre_tool_decision_with_env(
        &root,
        codex_asp_query_payload("asp rust query src/lib.rs --workspace . --code"),
        &[
            ("CODEX_THREAD_ID", "019f126d-0000-7000-8000-000000000103"),
            (
                "ASP_ROOT_SESSION_ID",
                "019f126d-0000-7000-8000-000000000003",
            ),
        ],
    );
    assert_eq!(decision["decision"].as_str(), Some("allow"));
    assert_eq!(
        decision["fields"]["executionTransport"],
        "resident-child-terminal"
    );
    assert_eq!(decision["fields"]["routingTerminal"], true);
    assert_eq!(decision["fields"]["redispatchAllowed"], false);
    assert_eq!(
        decision["fields"]["executionReceiptKind"],
        "resident-command-dispatch-receipt"
    );
}

#[test]
fn codex_asp_explore_session_denies_write_tools() {
    let root = claude_fixture();
    let codex_home = root.join(".codex-home");
    install_codex_hooks(&root, &codex_home);
    register_asp_explore_session(
        &root,
        "019f126d-0000-7000-8000-000000000040",
        "019f126d-0000-7000-8000-000000000140",
    );
    let decision = run_codex_pre_tool_decision_with_env(
        &root,
        json!({"session_id":"019f126d-0000-7000-8000-000000000140","tool_name":"Write","tool_input":{"path":"src/lib.rs","content":"not allowed from asp-explore"}}),
        &[
            ("CODEX_THREAD_ID", "019f126d-0000-7000-8000-000000000140"),
            (
                "ASP_ROOT_SESSION_ID",
                "019f126d-0000-7000-8000-000000000040",
            ),
        ],
    );
    assert_eq!(decision["decision"].as_str(), Some("deny"));
    assert_eq!(
        decision["reasonKind"].as_str(),
        Some("read-only-subagent-write")
    );
    assert_eq!(
        decision["fields"]["residentChildName"].as_str(),
        Some("asp-explore")
    );
    assert_eq!(
        decision["fields"]["readOnlySessionId"].as_str(),
        Some("019f126d-0000-7000-8000-000000000140")
    );
    assert!(
        decision["message"]
            .as_str()
            .unwrap_or_default()
            .contains("Use ASP query/search routes"),
        "{decision}"
    );
}

#[test]
fn codex_asp_explore_post_tool_records_session_evidence() {
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
        "post-tool",
        json!({"tool_name":"Bash","tool_input":{"command":"asp rust query src/lib.rs --workspace . --code"},"tool_result":{"evidenceRef":"asp-evidence:test-post-tool"}}),
        &[
            ("CODEX_THREAD_ID", "019f126d-0000-7000-8000-000000000130"),
            (
                "ASP_ROOT_SESSION_ID",
                "019f126d-0000-7000-8000-000000000030",
            ),
        ],
    );
    assert_eq!(decision["decision"].as_str(), Some("allow"));
    let report = show_agent_session_json(&root, "019f126d-0000-7000-8000-000000000130");
    let session = &report["sessions"][0];
    assert_eq!(session["lastToolEvent"].as_str(), Some("post-tool"));
    assert_eq!(
        session["lastCommand"].as_str(),
        Some("asp rust query src/lib.rs --workspace . --code")
    );
    assert_eq!(
        session["lastEvidenceRef"].as_str(),
        Some("asp-evidence:test-post-tool")
    );
    assert!(
        session["lastHeartbeatAt"].as_i64().is_some(),
        "post-tool should refresh heartbeat: {report}"
    );
}

#[test]
fn codex_main_session_denies_non_recovery_asp_command_when_asp_explore_registered() {
    let root = claude_fixture();
    let codex_home = root.join(".codex-home");
    install_codex_hooks(&root, &codex_home);
    register_asp_explore_session(
        &root,
        "019f126d-0000-7000-8000-000000000006",
        "019f126d-0000-7000-8000-000000000106",
    );
    let decision = run_codex_pre_tool_decision_with_env(
        &root,
        codex_asp_query_payload("asp install plugin --codex ."),
        &[("CODEX_THREAD_ID", "019f126d-0000-7000-8000-000000000006")],
    );
    assert_eq!(decision["decision"].as_str(), Some("deny"));
    assert_eq!(
        decision["fields"]["mainSessionAspPolicy"].as_str(),
        Some("session-checkpoint-recovery-only")
    );
    assert_eq!(
        decision["fields"]["blockedAspFacade"].as_str(),
        Some("install")
    );
    assert_eq!(
        decision["fields"]["childSessionId"].as_str(),
        Some("019f126d-0000-7000-8000-000000000106")
    );
    let message = decision["message"].as_str().unwrap_or_default();
    assert!(message.contains("Main-session ASP usage is limited"));
    assert!(message.contains("asp agent session ..."));
    assert!(message.contains("asp org recall ..."));
    assert!(message.contains("asp org capture ..."));
}

#[test]
fn codex_permission_request_enforces_main_session_search_denial() {
    let root = claude_fixture();
    let codex_home = root.join(".codex-home");
    install_codex_hooks(&root, &codex_home);
    let decision = run_codex_hook_decision_with_env(
        &root,
        "permission-request",
        codex_asp_query_payload("asp gerbil-scheme search prime --workspace . --view seeds"),
        &[("CODEX_THREAD_ID", "019f126d-0000-7000-8000-000000000016")],
    );
    assert_eq!(decision["decision"].as_str(), Some("deny"));
    assert_eq!(decision["event"].as_str(), Some("permission-request"));
    assert_eq!(
        decision["fields"]["agentSessionLoopCommand"].as_str(),
        Some("asp agent session bootstrap --name asp-explore")
    );
}

#[test]
fn codex_main_session_routes_test_command_to_asp_testing() {
    let root = claude_fixture();
    let codex_home = root.join(".codex-home");
    install_codex_hooks(&root, &codex_home);
    let decision = run_codex_pre_tool_decision_with_env(
        &root,
        codex_asp_query_payload("cargo test -p agent-semantic-protocol"),
        &[("CODEX_THREAD_ID", "019f126d-0000-7000-8000-000000000009")],
    );
    assert_eq!(decision["decision"].as_str(), Some("deny"));
    assert_eq!(
        decision["fields"]["residentChildName"].as_str(),
        Some("asp-testing"),
        "{decision}"
    );
    assert_eq!(
        decision["fields"]["executionLane"].as_str(),
        Some("testing")
    );
    assert_eq!(
        decision["fields"]["blockedCommandClass"].as_str(),
        Some("testing")
    );
    let message = decision["message"].as_str().unwrap_or_default();
    assert!(message.contains("Route it exactly through hook-selected lane `testing`"));
    assert!(
        message.contains("asp-testing"),
        "expected routing guidance to mention asp-testing: {message}"
    );
    assert!(
        message.contains("configured resident `asp-testing`")
            && message.contains("/root/asp_testing"),
        "expected actionable routing hint: {message}"
    );
}

#[test]
fn codex_main_session_routes_wrapped_test_command_to_asp_testing() {
    let root = claude_fixture();
    let codex_home = root.join(".codex-home");
    install_codex_hooks(&root, &codex_home);
    let decision = run_codex_pre_tool_decision_with_env(
        &root,
        codex_asp_query_payload(
            "direnv exec . env CARGO_TARGET_DIR=target/session-validation-check cargo test -p agent-semantic-protocol asp_agent_session_rejects_mismatched_codex_agent_config_path --test unit_test -- --nocapture",
        ),
        &[("CODEX_THREAD_ID", "019f126d-0000-7000-8000-000000000010")],
    );
    assert_eq!(decision["decision"].as_str(), Some("deny"));
    assert_eq!(
        decision["fields"]["residentChildName"].as_str(),
        Some("asp-testing"),
        "{decision}"
    );
    assert_eq!(
        decision["fields"]["blockedCommandClass"].as_str(),
        Some("testing")
    );
    let message = decision["message"].as_str().unwrap_or_default();
    assert!(message.contains("Route it exactly through hook-selected lane `testing`"));
    assert!(
        decision["subject"]["command"]
            .as_str()
            .is_some_and(|command| command.contains("direnv exec . env CARGO_TARGET_DIR"))
    );
}

#[test]
fn codex_main_session_allows_configured_main_asp_command_prefix() {
    let root = claude_fixture();
    let codex_home = root.join(".codex-home");
    install_codex_hooks(&root, &codex_home);
    write_hook_config(
        &root,
        r#"
[agents]
residentAgents = [
  { name = "asp-explore", role = "asp_explorer", lifecycle = "resident", mainAllowedAspCommandPrefixes = ["help", "agent session", "org recall", "org capture", "install plugin"] },
  { name = "asp-testing", role = "asp_testing", lifecycle = "ephemeral" },
]
"#,
    );
    let decision = run_codex_pre_tool_decision_with_env(
        &root,
        codex_asp_query_payload("asp install plugin --codex ."),
        &[("CODEX_THREAD_ID", "019f126d-0000-7000-8000-000000000008")],
    );
    assert_eq!(decision["decision"].as_str(), Some("allow"));
}

#[test]
fn codex_main_session_allows_recovery_checkpoint_and_session_commands() {
    let root = claude_fixture();
    let codex_home = root.join(".codex-home");
    install_codex_hooks(&root, &codex_home);
    register_asp_explore_session(
        &root,
        "019f126d-0000-7000-8000-000000000007",
        "019f126d-0000-7000-8000-000000000107",
    );
    for command in [
        "asp agent session list",
        "asp org recall plans",
        "asp org capture --contract agent.plan.v1 --title plan --target-file plan.org --no-confirm",
    ] {
        let decision = run_codex_pre_tool_decision_with_env(
            &root,
            codex_asp_query_payload(command),
            &[("CODEX_THREAD_ID", "019f126d-0000-7000-8000-000000000007")],
        );
        assert_eq!(
            decision["decision"].as_str(),
            Some("allow"),
            "command should be allowed: {command}\ndecision: {decision}"
        );
    }
}
