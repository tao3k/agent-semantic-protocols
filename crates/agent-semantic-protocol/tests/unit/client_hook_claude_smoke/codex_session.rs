use serde_json::json;

use super::{
    claude_fixture, codex_asp_query_payload, install_codex_hooks, register_asp_explore_session,
    register_expired_asp_explore_session, run_codex_hook_decision_with_env,
    run_codex_pre_tool_decision_with_env,
};

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
        Some("query-host-status-then-resume")
    );
    assert_eq!(
        decision["fields"]["agentSessionAction"].as_str(),
        Some("reuse-resident-child")
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
    assert!(message.contains("Reuse or resume the registered resident asp-explore child session"));
    assert!(message.contains("do not spawn another asp-explore session"));
    assert!(message.contains("do not close it after the result"));
    assert!(message.contains("query the host session status"));
    assert!(message.contains("If the host reports active/running"));
    assert!(message.contains("Only create a replacement when the host reports"));
    assert!(message.contains("019f126d-0000-7000-8000-000000000101"));
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
    assert!(message.contains("no registered active asp-explore child session"));
    assert!(message.contains("do not create duplicate asp-explore sessions"));
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
        Some("query-host-status-then-resume")
    );
    assert_eq!(
        decision["fields"]["agentSessionAction"].as_str(),
        Some("start-resident-child")
    );
    assert_eq!(
        decision["fields"]["agentSessionBootstrap"].as_str(),
        Some("session-start-reminder")
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
    assert!(message.contains("no registered active asp-explore child session"));
    assert!(message.contains("asp agent session reuse --name asp-explore --json"));
    assert!(message.contains("agent `asp_explorer`"));
    assert!(message.contains("do not use an ad-hoc natural-language subagent"));
    assert!(message.contains("--child-session-id <child-session-id>"));
    assert!(message.contains("do not create duplicate asp-explore sessions"));
    assert!(message.contains("After registration, retry the original tool command"));
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
    assert!(message.contains("asp agent session reuse --name asp-explore --json"));
    assert!(message.contains("agent `asp_explorer`"));
    assert!(message.contains("do not use an ad-hoc natural-language subagent"));
    assert!(message.contains("--child-session-id <child-session-id>"));
    assert!(message.contains("Do not create duplicate asp-explore sessions"));
}

#[test]
fn codex_main_session_does_not_require_asp_explore_before_non_asp_tool() {
    let root = claude_fixture();
    let codex_home = root.join(".codex-home");
    install_codex_hooks(&root, &codex_home);

    let decision = run_codex_pre_tool_decision_with_env(
        &root,
        codex_asp_query_payload("cargo test -p agent-semantic-protocol codex_"),
        &[("CODEX_THREAD_ID", "019f126d-0000-7000-8000-000000000004")],
    );

    assert_eq!(decision["decision"].as_str(), Some("allow"));
}
