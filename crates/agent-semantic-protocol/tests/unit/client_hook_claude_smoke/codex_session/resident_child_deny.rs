use super::{
    claude_fixture, codex_asp_query_payload, install_codex_hooks, register_asp_explore_session,
    run_codex_pre_tool_decision_with_env, write_codex_asp_explore_rollout,
};
use serde_json::json;
#[test]
fn registered_resident_child_source_deny_never_enters_bootstrap() {
    let root = claude_fixture();
    let codex_home = root.join(".codex-home");
    install_codex_hooks(&root, &codex_home);
    let root_session_id = "019f126d-0000-7000-8000-000000000052";
    let child_session_id = "019f126d-0000-7000-8000-000000000152";
    register_asp_explore_session(&root, root_session_id, child_session_id);

    let decision = run_codex_pre_tool_decision_with_env(
        &root,
        codex_asp_query_payload("rg -n registry crates"),
        &[("CODEX_THREAD_ID", child_session_id)],
    );

    assert_eq!(decision["decision"].as_str(), Some("deny"));
    assert!(decision["fields"].get("agentSessionLoopCommand").is_none());
    assert!(decision["fields"].get("agentSessionBootstrap").is_none());
    assert!(decision["fields"].get("requiredAction").is_none());
    assert_eq!(decision["fields"]["registeredResidentChild"], true);
    let message = decision["message"].as_str().unwrap_or_default();
    assert!(message.contains("[asp-search-subagent]"));
    assert!(!message.contains("asp agent session bootstrap"));
}

#[test]
fn unregistered_host_attributed_resident_child_source_deny_never_enters_bootstrap() {
    let root = claude_fixture();
    let codex_home = root.join(".codex-home");
    install_codex_hooks(&root, &codex_home);
    let root_session_id = "019f126d-0000-7000-8000-000000000056";
    let child_session_id = "019f126d-0000-7000-8000-000000000156";
    write_codex_asp_explore_rollout(&root, root_session_id, child_session_id, "gpt-5.4-mini");

    let decision = run_codex_pre_tool_decision_with_env(
        &root,
        codex_asp_query_payload("rg -n registry crates"),
        &[("CODEX_THREAD_ID", child_session_id)],
    );

    assert_eq!(decision["decision"].as_str(), Some("deny"));
    assert!(decision["fields"].get("requiredAction").is_none());
    assert_eq!(decision["fields"]["registeredResidentChild"], false);
    assert_eq!(
        decision["fields"]["residentChildIdentityProof"],
        "codex-rollout-metadata"
    );
    let message = decision["message"].as_str().unwrap_or_default();
    assert!(message.contains("[asp-search-subagent]"));
    assert!(!message.contains("asp agent session bootstrap"));
}

#[test]
fn codex_hook_payload_resident_child_without_registry_or_rollout_never_enters_bootstrap() {
    let root = claude_fixture();
    let codex_home = root.join(".codex-home");
    install_codex_hooks(&root, &codex_home);
    let child_session_id = "019f126d-0000-7000-8000-000000000158";

    let decision = run_codex_pre_tool_decision_with_env(
        &root,
        json!({
            "agent_id": child_session_id,
            "agent_type": "asp_explorer",
            "tool_name": "Bash",
            "tool_input": {"command": "rg -n registry crates"}
        }),
        &[("CODEX_THREAD_ID", child_session_id)],
    );

    assert_eq!(decision["decision"].as_str(), Some("deny"));
    for field in [
        "requiredAction",
        "nextAction",
        "agentSessionLoopCommand",
        "agentSessionBootstrap",
        "agentSessionBootstrapGuideCommand",
        "agentSessionBootstrapCommand",
    ] {
        assert!(
            decision["fields"].get(field).is_none(),
            "{field}: {decision}"
        );
    }
    assert_eq!(
        decision["fields"]["residentChildIdentityProof"],
        "codex-hook-payload"
    );
    let message = decision["message"].as_str().unwrap_or_default();
    assert!(message.contains("[asp-search-subagent]"));
    assert!(!message.contains("asp agent session bootstrap"));
}

#[test]
fn codex_hook_payload_wrong_agent_id_keeps_main_bootstrap() {
    let root = claude_fixture();
    let codex_home = root.join(".codex-home");
    install_codex_hooks(&root, &codex_home);
    let child_session_id = "019f126d-0000-7000-8000-000000000159";

    let decision = run_codex_pre_tool_decision_with_env(
        &root,
        json!({
            "agent_id": "019f126d-0000-7000-8000-000000000259",
            "agent_type": "asp_explorer",
            "tool_name": "Bash",
            "tool_input": {"command": "rg -n registry crates"}
        }),
        &[("CODEX_THREAD_ID", child_session_id)],
    );

    assert_eq!(decision["decision"].as_str(), Some("deny"));
    assert_eq!(
        decision["fields"]["requiredAction"],
        "enter-asp-explore-choice-pane"
    );
    assert!(
        decision["message"]
            .as_str()
            .unwrap_or_default()
            .contains("asp agent session bootstrap")
    );
}

#[test]
fn codex_hook_payload_wrong_agent_type_keeps_main_bootstrap() {
    let root = claude_fixture();
    let codex_home = root.join(".codex-home");
    install_codex_hooks(&root, &codex_home);
    let child_session_id = "019f126d-0000-7000-8000-000000000160";

    let decision = run_codex_pre_tool_decision_with_env(
        &root,
        json!({
            "agent_id": child_session_id,
            "agent_type": "explorer",
            "tool_name": "Bash",
            "tool_input": {"command": "rg -n registry crates"}
        }),
        &[("CODEX_THREAD_ID", child_session_id)],
    );

    assert_eq!(decision["decision"].as_str(), Some("deny"));
    assert_eq!(
        decision["fields"]["requiredAction"],
        "enter-asp-explore-choice-pane"
    );
    assert!(
        decision["message"]
            .as_str()
            .unwrap_or_default()
            .contains("asp agent session bootstrap")
    );
}

#[test]
fn codex_root_current_session_mismatch_never_enters_bootstrap() {
    let root = claude_fixture();
    let codex_home = root.join(".codex-home");
    install_codex_hooks(&root, &codex_home);
    let root_session_id = "019f126d-0000-7000-8000-000000000061";
    let child_session_id = "019f126d-0000-7000-8000-000000000161";

    let decision = run_codex_pre_tool_decision_with_env(
        &root,
        json!({
            "session_id": root_session_id,
            "tool_name": "Bash",
            "tool_input": {"command": "rg -n registry crates"}
        }),
        &[("CODEX_THREAD_ID", child_session_id)],
    );

    assert_eq!(decision["decision"].as_str(), Some("deny"));
    assert_eq!(decision["fields"]["registeredResidentChild"], false);
    assert_eq!(
        decision["fields"]["subagentIdentityProof"],
        "codex-root-current-session-mismatch"
    );
    assert!(decision["fields"].get("requiredAction").is_none());
    assert!(
        !decision["message"]
            .as_str()
            .unwrap_or_default()
            .contains("asp agent session bootstrap")
    );
}

#[test]
fn codex_root_current_session_match_keeps_main_bootstrap() {
    let root = claude_fixture();
    let codex_home = root.join(".codex-home");
    install_codex_hooks(&root, &codex_home);
    let root_session_id = "019f126d-0000-7000-8000-000000000062";

    let decision = run_codex_pre_tool_decision_with_env(
        &root,
        json!({
            "session_id": root_session_id,
            "tool_name": "Bash",
            "tool_input": {"command": "rg -n registry crates"}
        }),
        &[("CODEX_THREAD_ID", root_session_id)],
    );

    assert_eq!(decision["decision"].as_str(), Some("deny"));
    assert_eq!(
        decision["fields"]["requiredAction"],
        "enter-asp-explore-choice-pane"
    );
    assert!(
        decision["message"]
            .as_str()
            .unwrap_or_default()
            .contains("asp agent session bootstrap")
    );
}

#[test]
fn registered_resident_child_transcript_allows_parser_owned_rust_search() {
    let root = claude_fixture();
    let codex_home = root.join(".codex-home");
    install_codex_hooks(&root, &codex_home);
    let root_session_id = "019f126d-0000-7000-8000-000000000063";
    let child_session_id = "019f126d-0000-7000-8000-000000000163";
    register_asp_explore_session(&root, root_session_id, child_session_id);

    let decision = run_codex_pre_tool_decision_with_env(
        &root,
        json!({
            "session_id": root_session_id,
            "transcript_path": format!(
                "/tmp/rollout-2026-07-14T12-56-15-{child_session_id}.jsonl"
            ),
            "tool_name": "Bash",
            "tool_input": {
                "command": "direnv exec . asp rust search ingest items tests --workspace . --view seeds"
            }
        }),
        &[("CODEX_THREAD_ID", root_session_id)],
    );

    assert_eq!(decision["decision"].as_str(), Some("allow"), "{decision}");
    assert_eq!(
        decision["fields"]["agentSessionAction"],
        "active-resident-child"
    );
    assert_eq!(decision["fields"]["routingTerminal"], true);
    assert_eq!(decision["fields"]["redispatchAllowed"], false);
    assert!(decision["fields"].get("requiredAction").is_none());
}

#[test]
fn registered_resident_child_with_root_env_keeps_main_bootstrap() {
    let root = claude_fixture();
    let codex_home = root.join(".codex-home");
    install_codex_hooks(&root, &codex_home);
    let root_session_id = "019f126d-0000-7000-8000-000000000053";
    let child_session_id = "019f126d-0000-7000-8000-000000000153";
    register_asp_explore_session(&root, root_session_id, child_session_id);

    let decision = run_codex_pre_tool_decision_with_env(
        &root,
        codex_asp_query_payload("rg -n registry crates"),
        &[("CODEX_THREAD_ID", root_session_id)],
    );

    assert_eq!(decision["decision"].as_str(), Some("deny"));
    assert_eq!(
        decision["fields"]["requiredAction"].as_str(),
        Some("enter-asp-explore-choice-pane")
    );
    assert!(
        decision["message"]
            .as_str()
            .unwrap_or_default()
            .contains("asp agent session bootstrap")
    );
}

#[test]
fn registered_resident_child_with_other_env_keeps_main_bootstrap() {
    let root = claude_fixture();
    let codex_home = root.join(".codex-home");
    install_codex_hooks(&root, &codex_home);
    let root_session_id = "019f126d-0000-7000-8000-000000000055";
    let child_session_id = "019f126d-0000-7000-8000-000000000155";
    let other_child_session_id = "019f126d-0000-7000-8000-000000000255";
    register_asp_explore_session(&root, root_session_id, child_session_id);
    write_codex_asp_explore_rollout(
        &root,
        root_session_id,
        other_child_session_id,
        "gpt-5.4-mini",
    );

    let decision = run_codex_pre_tool_decision_with_env(
        &root,
        codex_asp_query_payload("rg -n registry crates"),
        &[("CODEX_THREAD_ID", other_child_session_id)],
    );

    assert_eq!(decision["decision"].as_str(), Some("deny"));
    assert_eq!(
        decision["fields"]["requiredAction"].as_str(),
        Some("enter-asp-explore-choice-pane")
    );
    assert!(
        decision["message"]
            .as_str()
            .unwrap_or_default()
            .contains("asp agent session bootstrap")
    );
}

#[test]
fn resident_child_bootstrap_cli_fails_fast_with_main_owner_receipt() {
    let root = claude_fixture();
    let codex_home = root.join(".codex-home");
    install_codex_hooks(&root, &codex_home);
    let root_session_id = "019f126d-0000-7000-8000-000000000054";
    let child_session_id = "019f126d-0000-7000-8000-000000000154";
    register_asp_explore_session(&root, root_session_id, child_session_id);

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
        .env("CODEX_THREAD_ID", child_session_id)
        .env("ASP_STATE_HOME", root.join(".agent-semantic-protocols"))
        .env_remove("PRJ_CACHE_HOME")
        .output()
        .expect("run child bootstrap");

    assert!(!output.status.success());
    let receipt = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        receipt.contains("bootstrap-owner-main-session-only"),
        "{receipt}"
    );
}

#[test]
fn unregistered_host_attributed_child_bootstrap_cli_fails_fast() {
    let root = claude_fixture();
    let codex_home = root.join(".codex-home");
    install_codex_hooks(&root, &codex_home);
    let root_session_id = "019f126d-0000-7000-8000-000000000057";
    let child_session_id = "019f126d-0000-7000-8000-000000000157";
    write_codex_asp_explore_rollout(&root, root_session_id, child_session_id, "gpt-5.4-mini");

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
        .env("CODEX_THREAD_ID", child_session_id)
        .env("ASP_STATE_HOME", root.join(".agent-semantic-protocols"))
        .env_remove("PRJ_CACHE_HOME")
        .output()
        .expect("run host-attributed child bootstrap");

    assert!(!output.status.success());
    let receipt = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        receipt.contains("bootstrap-owner-main-session-only"),
        "{receipt}"
    );
}
