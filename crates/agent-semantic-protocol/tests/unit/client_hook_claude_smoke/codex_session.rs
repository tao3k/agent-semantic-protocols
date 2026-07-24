use serde_json::json;

use super::{
    claude_fixture, codex_asp_query_payload, install_codex_hooks, register_asp_explore_session,
    register_expired_asp_explore_session, run_codex_hook_decision_with_env,
    run_codex_pre_tool_decision_with_env, show_agent_session_json, write_codex_asp_explore_rollout,
};

fn assert_configured_asp_explore_dispatch(decision: &serde_json::Value) {
    assert_eq!(decision["decision"].as_str(), Some("deny"), "{decision}");
    assert_eq!(
        decision["reasonKind"].as_str(),
        Some("subagent-receipt-required"),
        "{decision}"
    );
    assert_eq!(
        decision["fields"]["agentSessionAction"].as_str(),
        Some("dispatch-configured-resident"),
        "{decision}"
    );
    assert_eq!(
        decision["fields"]["residentName"].as_str(),
        Some("asp-explore")
    );
    assert_eq!(
        decision["fields"]["canonicalTarget"].as_str(),
        Some("/root/asp_explorer")
    );
    assert_eq!(
        decision["fields"]["receiptKind"].as_str(),
        Some("asp-explore-search-v1")
    );
    assert_eq!(
        decision["fields"]["requiredAction"].as_str(),
        Some("route-exact-command-to-hook-selected-resident")
    );
    let root_session_id = decision["fields"]
        .get("rootSessionId")
        .or_else(|| decision["fields"].get("sessionId"))
        .and_then(serde_json::Value::as_str);
    let command_json = serde_json::to_string(&[
        "/bin/sh",
        "-c",
        decision["subject"]["command"]
            .as_str()
            .expect("configured resident dispatch exact command"),
    ])
    .expect("configured resident dispatch command JSON");
    let mut expected_argv = vec![
        "asp".to_string(),
        "agent".to_string(),
        "session".to_string(),
        "bootstrap".to_string(),
        "--name".to_string(),
        "asp-explore".to_string(),
    ];
    if let Some(root_session_id) = root_session_id {
        expected_argv.extend(["--root-session-id".to_string(), root_session_id.to_string()]);
    }
    expected_argv.extend([
        "--receipt-kind".to_string(),
        "asp-explore-search-v1".to_string(),
        "--command-json".to_string(),
        command_json,
    ]);
    assert_eq!(decision["interactiveCommand"]["argv"], json!(expected_argv));
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

    assert_eq!(
        decision["decision"].as_str(),
        Some("deny"),
        "decision={decision}"
    );
    assert_eq!(
        decision["fields"]["agentSessionAction"].as_str(),
        Some("resume-existing-resident-child")
    );
    assert_eq!(
        decision["fields"]["nextAction"].as_str(),
        Some("enter-bootstrap-pane-for-existing-child")
    );
    assert_eq!(
        decision["fields"]["childSessionId"].as_str(),
        Some("019f126d-0000-7000-8000-000000000132")
    );
    assert!(decision["fields"].get("agentSessionBootstrap").is_none());
    let message = decision["message"].as_str().unwrap_or_default();
    assert!(!message.contains("Resume that child session instead of creating a replacement"));
    assert!(!message.contains("archive or delete"));
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

    assert_eq!(
        decision["decision"].as_str(),
        Some("allow"),
        "decision: {}",
        serde_json::to_string_pretty(&decision).expect("serialize decision")
    );
    assert_eq!(
        decision["fields"]["agentSessionAction"].as_str(),
        Some("reuse-resident-child"),
        "decision: {}",
        serde_json::to_string_pretty(&decision).expect("serialize decision")
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
fn codex_main_session_denies_reasoning_flow_commands_for_every_language_facade() {
    let root = claude_fixture();
    let codex_home = root.join(".codex-home");
    install_codex_hooks(&root, &codex_home);
    register_asp_explore_session(
        &root,
        "019f126d-0000-7000-8000-000000000003",
        "019f126d-0000-7000-8000-000000000103",
    );

    let commands = [
        "asp rust search owner src/lib.rs items --workspace . --view seeds",
        "asp rust search lexical --query run --workspace . --view seeds",
        "asp rust search pipe --query run --workspace . --view seeds",
        "asp rust search prime --query run --workspace . --view seeds",
        "asp rust search deps --workspace . --view seeds",
        "asp rust search failure --query run --workspace . --view seeds",
        "asp rust search reasoning --query run --workspace . --view seeds",
        "asp rust search guide --query run --workspace . --view seeds",
        "asp typescript search owner src/app.ts items --workspace . --view seeds",
        "asp python search owner src/app.py items --workspace . --view seeds",
        "direnv exec . asp julia search owner src/app.jl items --workspace . --view seeds",
        "asp gerbil-scheme search owner src/main.ss items --workspace . --view seeds",
        "asp org search owner docs/spec.org items --workspace . --view seeds",
        "asp md search owner docs/spec.md items --workspace . --view seeds",
        "asp rust query --term run --workspace .",
    ];

    for command in commands {
        let decision = run_codex_pre_tool_decision_with_env(
            &root,
            codex_asp_query_payload(command),
            &[("CODEX_THREAD_ID", "019f126d-0000-7000-8000-000000000003")],
        );

        assert_configured_asp_explore_dispatch(&decision);
    }
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
        codex_asp_query_payload("asp rust query --term demo --workspace . --code"),
        &[("CODEX_THREAD_ID", "019f126d-0000-7000-8000-000000000001")],
    );

    assert_configured_asp_explore_dispatch(&decision);
    assert!(decision["fields"].get("childSessionId").is_none());

    let repeated = run_codex_pre_tool_decision_with_env(
        &root,
        codex_asp_query_payload("asp rust query --term demo --workspace . --code"),
        &[("CODEX_THREAD_ID", "019f126d-0000-7000-8000-000000000001")],
    );
    assert_eq!(repeated["fields"]["denyReplay"].as_str(), Some("repeated"));
    assert_eq!(
        repeated["fields"]["denyReplayMessagePolicy"].as_str(),
        Some("preserve-agent-session-route")
    );
    assert_configured_asp_explore_dispatch(&repeated);
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
         ./target/debug/asp rust query --term demo --workspace . --code",
        ),
        &[("CODEX_THREAD_ID", "019f126d-0000-7000-8000-000000000002")],
    );

    assert_configured_asp_explore_dispatch(&decision);
    assert!(decision["fields"].get("childSessionId").is_none());
}

#[test]
fn codex_main_session_denies_registered_language_reasoning_query_and_search() {
    let root = claude_fixture();
    let codex_home = root.join(".codex-home");
    install_codex_hooks(&root, &codex_home);
    register_asp_explore_session(
        &root,
        "019f126d-0000-7000-8000-000000000014",
        "019f126d-0000-7000-8000-000000000114",
    );

    let commands = [
        (
            "asp typescript query --term useEffect --workspace . --code",
            "typescript",
        ),
        (
            "asp python search pipe 'import django' --workspace . --view seeds",
            "python",
        ),
        (
            "direnv exec . asp python search pipe 'import django' --workspace . --view seeds",
            "python",
        ),
        ("asp julia query --term graph --workspace . --code", "julia"),
        (
            "asp gerbil-scheme search pipe 'session case' --workspace . --view seeds",
            "gerbil-scheme",
        ),
        (
            "asp org query --term lifecycle --workspace . --content",
            "org",
        ),
        (
            "asp md search pipe 'pane grammar' --workspace . --view seeds",
            "md",
        ),
    ];

    for (command, language_id) in commands {
        let decision = run_codex_pre_tool_decision_with_env(
            &root,
            codex_asp_query_payload(command),
            &[("CODEX_THREAD_ID", "019f126d-0000-7000-8000-000000000014")],
        );

        assert_eq!(
            decision["decision"].as_str(),
            Some("deny"),
            "command={command} decision={decision}"
        );
        assert_eq!(
            decision["reasonKind"].as_str(),
            Some("subagent-receipt-required"),
            "command={command} decision={decision}"
        );
        assert!(
            decision["languageIds"]
                .as_array()
                .expect("languageIds array")
                .iter()
                .any(|value| value.as_str() == Some(language_id)),
            "command={command} languageId={language_id} decision={decision}"
        );
    }
}

#[test]
fn codex_main_session_allows_registered_language_exact_item_queries() {
    let root = claude_fixture();
    let codex_home = root.join(".codex-home");
    install_codex_hooks(&root, &codex_home);
    register_asp_explore_session(
        &root,
        "019f126d-0000-7000-8000-000000000003",
        "019f126d-0000-7000-8000-000000000103",
    );

    let commands = [
        "asp rust query --selector rust://src/lib.rs#item/function/run --workspace . --code",
        "asp typescript query --selector typescript://src/app.ts#item/function/run --workspace . --code",
        "asp python query --selector python://src/app.py#item/function/run --workspace . --code",
        "asp julia query --selector julia://src/app.jl#item/function/run --workspace . --code",
        "asp gerbil-scheme query --selector gerbil-scheme://src/main.ss#item/function/run --workspace . --code",
        "asp rust query --selector rust://src/lib.rs#item/function/run --workspace . --names-only",
    ];

    for command in commands {
        let decision = run_codex_pre_tool_decision_with_env(
            &root,
            codex_asp_query_payload(command),
            &[("CODEX_THREAD_ID", "019f126d-0000-7000-8000-000000000003")],
        );

        assert_eq!(
            decision["decision"].as_str(),
            Some("allow"),
            "command={command} decision={decision}"
        );
    }
}

#[test]
fn codex_main_session_denies_reasoning_search_pipe_when_asp_explore_registered() {
    let root = claude_fixture();
    let codex_home = root.join(".codex-home");
    install_codex_hooks(&root, &codex_home);
    register_asp_explore_session(
        &root,
        "019f126d-0000-7000-8000-000000000004",
        "019f126d-0000-7000-8000-000000000104",
    );

    let decision = run_codex_pre_tool_decision_with_env(
        &root,
        codex_asp_query_payload("asp rust search pipe 'run transport' --workspace . --view seeds"),
        &[("CODEX_THREAD_ID", "019f126d-0000-7000-8000-000000000004")],
    );

    assert_configured_asp_explore_dispatch(&decision);
}

#[test]
fn codex_main_session_routes_model_drifted_asp_explore_through_canonical_dispatch() {
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
        codex_asp_query_payload("asp rust query --term demo --workspace . --code"),
        &[("CODEX_THREAD_ID", "019f126d-0000-7000-8000-000000000011")],
    );

    assert_configured_asp_explore_dispatch(&decision);
    assert!(decision["fields"].get("childSessionId").is_none());
}

#[test]
fn asp_binary_does_not_deny_main_session_query_when_asp_explore_registered() {
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

    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !combined.contains("ASP query/search command denied in main agent session"),
        "{combined}"
    );
    assert!(
        !combined.contains("childSessionId=019f126d-0000-7000-8000-000000000140"),
        "{combined}"
    );
    assert!(
        !combined.contains("do not spawn another asp-explore session"),
        "{combined}"
    );
}

#[test]
fn asp_binary_does_not_deny_main_session_query_without_asp_explore_registered() {
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

    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !combined.contains("no active asp-explore child session is registered"),
        "{combined}"
    );
    assert!(
        !combined.contains("asp agent session register --guide"),
        "{combined}"
    );
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
        codex_asp_query_payload("asp rust query --term demo --workspace . --code"),
        &[("CODEX_THREAD_ID", "019f126d-0000-7000-8000-000000000006")],
    );

    assert_configured_asp_explore_dispatch(&decision);
    assert!(decision["fields"].get("childSessionId").is_none());
}

#[test]
fn codex_main_session_denies_asp_query_without_asp_explore_registered() {
    let root = claude_fixture();
    let codex_home = root.join(".codex-home");
    install_codex_hooks(&root, &codex_home);

    let decision = run_codex_pre_tool_decision_with_env(
        &root,
        codex_asp_query_payload("asp rust query --term src/lib.rs --workspace . --code"),
        &[("CODEX_THREAD_ID", "019f126d-0000-7000-8000-000000000002")],
    );

    assert_configured_asp_explore_dispatch(&decision);
    assert!(decision["fields"].get("childSessionId").is_none());
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

    assert_eq!(
        decision["decision"].as_str(),
        Some("deny"),
        "decision={decision}"
    );
    assert_eq!(
        decision["fields"]["agentSessionBootstrap"].as_str(),
        Some("session-start-reminder")
    );
    assert_eq!(
        decision["fields"]["agentSessionAction"].as_str(),
        Some("enter-resident-child-bootstrap-pane")
    );
    assert_eq!(
        decision["fields"]["residentCodexAgentName"].as_str(),
        Some("asp_explorer")
    );
    assert_eq!(
        decision["fields"]["agentSessionLoopCommand"].as_str(),
        Some("asp agent session bootstrap --name asp-explore")
    );
    assert!(decision["fields"]["agentSessionLookupCommand"].is_null());
    assert!(decision["fields"]["agentSessionRegisterCommandTemplate"].is_null());
    assert_eq!(
        decision["fields"]["rootSessionId"].as_str(),
        Some("019f126d-0000-7000-8000-000000000020")
    );
    let message = decision["message"].as_str().unwrap_or_default();
    assert!(!message.contains("asp agent session bootstrap --name asp-explore --json"));
}

#[path = "codex_session/payload_identity.rs"]
mod payload_identity;

#[path = "codex_session/profile_path.rs"]
mod profile_path;

#[path = "codex_session/runtime_drift.rs"]
mod runtime_drift;

#[path = "codex_session/inline_parser_fallback.rs"]
mod inline_parser_fallback;
#[path = "codex_session/resident_child_deny.rs"]
mod resident_child_deny;

#[test]
fn codex_subagent_stop_preserves_registered_asp_explore_as_idle() {
    let root = claude_fixture();
    let codex_home = root.join(".codex-home");
    install_codex_hooks(&root, &codex_home);
    let root_session_id = "019f126d-0000-7000-8000-000000000060";
    let child_session_id = "019f126d-0000-7000-8000-000000000160";
    register_asp_explore_session(&root, root_session_id, child_session_id);

    let decision = run_codex_hook_decision_with_env(
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

    assert_eq!(decision["decision"].as_str(), Some("allow"));
    assert_eq!(
        decision["fields"]["agentSessionAction"].as_str(),
        Some("subagent-stop-preserved-resident-idle")
    );
    assert_eq!(
        decision["fields"]["childSessionId"].as_str(),
        Some(child_session_id)
    );

    let report = show_agent_session_json(&root, child_session_id);
    assert_eq!(report["sessions"][0]["status"].as_str(), Some("idle"));
    assert_eq!(report["sessions"][0]["archivedAt"].as_i64(), None);
}

#[test]
fn codex_native_custom_subagent_is_outside_asp_lifecycle_management() {
    let root = claude_fixture();
    let codex_home = root.join(".codex-home");
    install_codex_hooks(&root, &codex_home);
    let root_session_id = "019f126d-0000-7000-8000-000000000061";
    let resident_child_id = "019f126d-0000-7000-8000-000000000161";
    register_asp_explore_session(&root, root_session_id, resident_child_id);

    let start = run_codex_hook_decision_with_env(
        &root,
        "subagent-start",
        json!({
            "hook_event_name": "SubagentStart",
            "session_id": root_session_id,
            "agent_id": "user-custom-child",
            "agent_type": "explorer",
            "model": "gpt-5.5",
            "permission_mode": "default",
        }),
        &[("CODEX_THREAD_ID", root_session_id)],
    );
    assert_eq!(start["decision"].as_str(), Some("allow"));
    assert_eq!(
        start["fields"]["agentSessionAction"].as_str(),
        Some("ignore-unmanaged-native-subagent"),
        "{start}"
    );
    assert_eq!(
        start["fields"]["agentSessionObservedAgentType"].as_str(),
        Some("explorer")
    );
    assert_eq!(
        start["fields"]["agentSessionExpectedAgentType"].as_str(),
        Some("asp_explorer")
    );
    assert!(start["fields"].get("bootstrapBlocked").is_none());

    let stop = run_codex_hook_decision_with_env(
        &root,
        "subagent-stop",
        json!({
            "hook_event_name": "SubagentStop",
            "session_id": root_session_id,
            "agent_id": "user-custom-child",
            "agent_type": "explorer",
            "model": "gpt-5.5",
            "permission_mode": "default",
        }),
        &[("CODEX_THREAD_ID", root_session_id)],
    );
    assert_eq!(stop["decision"].as_str(), Some("allow"));

    let report = show_agent_session_json(&root, resident_child_id);
    assert_eq!(report["sessions"][0]["status"].as_str(), Some("active"));
}

#[test]
fn codex_bootstrap_attests_unobservable_reasoning_from_typed_profile() {
    let root = claude_fixture();
    let codex_home = root.join(".codex-home");
    install_codex_hooks(&root, &codex_home);
    let root_session_id = "019f126d-0000-7000-8000-000000000062";
    let child_session_id = "019f126d-0000-7000-8000-000000000162";
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

    let bootstrap_output = std::process::Command::new(env!("CARGO_BIN_EXE_asp"))
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
        .expect("bootstrap typed resident with unobservable reasoning");
    assert!(
        bootstrap_output.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&bootstrap_output.stdout),
        String::from_utf8_lossy(&bootstrap_output.stderr),
    );
    let bootstrap: serde_json::Value =
        serde_json::from_slice(&bootstrap_output.stdout).expect("bootstrap JSON");
    assert_eq!(bootstrap["state"].as_str(), Some("Ready"));
    assert_eq!(
        bootstrap
            .pointer("/hostLifecycleObservation/reasoningVerificationStatus")
            .and_then(serde_json::Value::as_str),
        Some("host-profile-attested-unobservable"),
        "bootstrap={bootstrap}"
    );
}

#[test]
fn codex_dispatch_terminal_receipt_recovers_across_cli_processes() {
    let root = claude_fixture();
    let codex_home = root.join(".codex-home");
    install_codex_hooks(&root, &codex_home);
    let root_session_id = "019f126d-0000-7000-8000-000000000061";
    let child_session_id = "019f126d-0000-7000-8000-000000000161";
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

    let execution_marker = root.join("dispatch-execution-count");
    let argv = vec![
        "/bin/sh".to_string(),
        "-c".to_string(),
        "printf x >> \"$1\"".to_string(),
        "dispatch-receipt-test".to_string(),
        execution_marker.display().to_string(),
    ];
    let command_json = serde_json::to_string(&argv).expect("canonical command JSON");
    let run_session_command = |command: &str| {
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_asp"))
            .args([
                "agent",
                "session",
                command,
                "--name",
                "asp-explore",
                "--receipt-kind",
                "dispatch-execution-receipt.v1",
                "--command-json",
                command_json.as_str(),
                "--resident-bridge",
                "--json",
            ])
            .current_dir(&root)
            .env("CODEX_HOME", &codex_home)
            .env("CODEX_THREAD_ID", root_session_id)
            .env("ASP_STATE_HOME", root.join(".agent-semantic-protocols"))
            .env_remove("PRJ_CACHE_HOME")
            .output()
            .expect("run agent session dispatch command");
        assert!(
            output.status.success(),
            "command={command} stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
        serde_json::from_slice::<serde_json::Value>(&output.stdout).expect("dispatch command JSON")
    };

    let first_claim = run_session_command("dispatch-claim");
    assert_eq!(first_claim["action"].as_str(), Some("send"));
    assert_eq!(first_claim["lease"]["attemptCount"].as_u64(), Some(1));
    let first_identity = first_claim["lease"]["dispatchIdentity"]
        .as_str()
        .expect("derived dispatch identity")
        .to_string();

    let completed = run_session_command("dispatch-execute");
    assert_eq!(completed["status"].as_str(), Some("terminal"));
    assert_eq!(completed["attemptCount"].as_u64(), Some(1));
    assert_eq!(completed["evidenceRef"].as_str(), Some("parser-exit:0"));

    let recovered = run_session_command("dispatch-claim");
    assert_eq!(recovered["action"].as_str(), Some("complete"));
    assert_eq!(recovered["lease"]["status"].as_str(), Some("terminal"));
    assert_eq!(recovered["lease"]["attemptCount"].as_u64(), Some(1));
    assert_eq!(
        recovered["lease"]["dispatchIdentity"].as_str(),
        Some(first_identity.as_str())
    );
    assert_eq!(
        recovered["lease"]["evidenceRef"].as_str(),
        Some("parser-exit:0")
    );

    let replay = run_session_command("dispatch-execute");
    assert_eq!(replay["status"].as_str(), Some("terminal"));
    assert_eq!(replay["attemptCount"].as_u64(), Some(1));
    assert_eq!(
        std::fs::read_to_string(&execution_marker).expect("execution marker"),
        "x"
    );
}

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
use super::{force_activation_project_root_to_hook_state, prepend_path};
