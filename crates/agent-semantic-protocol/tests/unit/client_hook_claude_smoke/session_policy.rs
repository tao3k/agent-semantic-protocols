use serde_json::json;

use super::{
    claude_fixture, codex_asp_query_payload, install_codex_hooks, register_asp_explore_session,
    run_codex_hook_decision_with_env, run_codex_pre_tool_decision_with_env,
    show_agent_session_json,
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
    let target_dir = root.join("target/session-validation-check");
    let cargo = root.join("toolchain/bin/cargo");
    let command = format!(
        "CARGO_TARGET_DIR={} {} test -p agent-semantic-content-identity packet_builder_tests --lib --offline",
        target_dir.display(),
        cargo.display()
    );
    let decision = run_codex_pre_tool_decision_with_env(
        &root,
        codex_asp_query_payload(&command),
        &[("CODEX_THREAD_ID", "019f126d-0000-7000-8000-000000000010")],
    );
    assert_eq!(decision["decision"].as_str(), Some("deny"));
    assert_eq!(
        decision["fields"]["residentChildName"].as_str(),
        Some("asp-testing"),
        "{decision}"
    );
    assert_eq!(
        decision["fields"]["targetAgentName"].as_str(),
        Some("asp_testing"),
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
            .is_some_and(|subject| subject == command)
    );
}

#[test]
fn codex_hook_rejects_invalid_matcher_config_without_builtin_fallback() {
    let root = claude_fixture();
    let codex_home = root.join(".codex-home");
    install_codex_hooks(&root, &codex_home);
    let config_path = root.join(".agent-semantic-protocols/hooks/config.toml");
    std::fs::create_dir_all(config_path.parent().expect("config parent"))
        .expect("create hook config parent");
    std::fs::write(&config_path, "[[rules]\ninvalid = true\n")
        .expect("write invalid hook matcher config");

    let cargo = root.join("toolchain/bin/cargo");
    let command = format!("{} test --offline", cargo.display());
    let output = super::support::run_codex_pre_tool_output_with_env(
        &root,
        codex_asp_query_payload(&command),
        &[("CODEX_THREAD_ID", "019f126d-0000-7000-8000-000000000010")],
    );

    assert!(!output.status.success(), "hook unexpectedly failed open");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("hook matcher config freshness gate failed"),
        "{stderr}"
    );
    assert!(
        !stderr.contains("continuing with built-in policy"),
        "{stderr}"
    );
}

#[test]
fn codex_hook_rejects_stale_matcher_contract_without_builtin_fallback() {
    let root = claude_fixture();
    let codex_home = root.join(".codex-home");
    install_codex_hooks(&root, &codex_home);
    let config_path = root.join(".agent-semantic-protocols/hooks/config.toml");
    std::fs::create_dir_all(config_path.parent().expect("config parent"))
        .expect("create hook config parent");
    let current_fingerprint = agent_semantic_config::hook_client_contract_fingerprint();
    let stale_config = agent_semantic_config::default_hook_client_config_template()
        .replace(&current_fingerprint, "hook-client-v1-0000000000000000");
    std::fs::write(&config_path, stale_config).expect("write stale hook matcher config");

    let cargo = root.join("toolchain/bin/cargo");
    let command = format!("{} test --offline", cargo.display());
    let output = super::support::run_codex_pre_tool_output_with_env(
        &root,
        codex_asp_query_payload(&command),
        &[("CODEX_THREAD_ID", "019f126d-0000-7000-8000-000000000010")],
    );

    assert!(!output.status.success(), "hook unexpectedly failed open");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("configured fingerprint hook-client-v1-0000000000000000"),
        "{stderr}"
    );
    assert!(
        stderr.contains("does not match binary fingerprint hook-client-v1-"),
        "{stderr}"
    );
    assert!(
        !stderr.contains("continuing with built-in policy"),
        "{stderr}"
    );
}

#[test]
fn codex_hook_auto_syncs_stale_managed_matcher_contract() {
    let root = claude_fixture();
    let codex_home = root.join(".codex-home");
    install_codex_hooks(&root, &codex_home);
    let config_path = root.join(".agent-semantic-protocols/hooks/config.toml");
    let current_fingerprint = agent_semantic_config::hook_client_contract_fingerprint();
    let stale_config = agent_semantic_config::default_hook_client_config_template()
        .replace(&current_fingerprint, "hook-client-v1-0000000000000000");
    std::fs::write(&config_path, &stale_config).expect("write stale managed hook config");
    std::fs::write(
        managed_config_sidecar(&config_path),
        test_sha256(stale_config.as_bytes()),
    )
    .expect("prove stale hook config ownership");

    let cargo = root.join("toolchain/bin/cargo");
    let command = format!("{} test --offline", cargo.display());
    let decision = run_codex_pre_tool_decision_with_env(
        &root,
        codex_asp_query_payload(&command),
        &[("CODEX_THREAD_ID", "019f126d-0000-7000-8000-000000000011")],
    );

    assert_eq!(decision["decision"].as_str(), Some("deny"));
    assert_eq!(
        decision["fields"]["targetAgentName"].as_str(),
        Some("asp_testing"),
        "{decision}"
    );
    assert_eq!(
        decision["fields"]["hookConfigStatus"].as_str(),
        Some("repaired-by-asp-sync"),
        "{decision}"
    );
    assert_eq!(
        decision["fields"]["hookConfigFailurePolicy"].as_str(),
        Some("fail-closed"),
        "{decision}"
    );
    assert!(
        decision["fields"]
            .get("hookConfigRecoveryCommand")
            .is_none(),
        "automatic repair must not instruct the user to run asp sync: {decision}"
    );
    assert!(
        decision["message"].as_str().is_some_and(|message| {
            message.contains("automatically ran `asp sync`") && !message.contains("Run `asp sync`")
        }),
        "{decision}"
    );

    let current_config = agent_semantic_config::default_hook_client_config_template();
    assert_eq!(
        std::fs::read_to_string(&config_path).expect("read repaired hook config"),
        current_config
    );
    assert_eq!(
        std::fs::read_to_string(managed_config_sidecar(&config_path))
            .expect("read repaired hook config sidecar"),
        test_sha256(current_config.as_bytes())
    );
}

fn managed_config_sidecar(config: &std::path::Path) -> std::path::PathBuf {
    config.with_file_name(format!(
        "{}.managed.sha256",
        config
            .file_name()
            .and_then(|name| name.to_str())
            .expect("config name")
    ))
}

fn test_sha256(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(bytes))
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
