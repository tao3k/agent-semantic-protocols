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
            "hook_event_name": "SubagentStart",
            "session_id": root_session_id,
            "agent_id": child_session_id,
            "agent_type": "asp_explorer",
            "model": "gpt-5.4-mini",
            "permission_mode": "default",
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
            "hook_event_name": "SubagentStop",
            "session_id": root_session_id,
            "agent_id": child_session_id,
            "agent_type": "asp_explorer",
            "model": "gpt-5.4-mini",
            "reasoning_effort": "low",
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
            "hook_event_name": "SubagentStart",
            "session_id": root_session_id,
            "agent_id": "019f126d-0000-7000-8000-000000000234",
            "agent_type": "asp_explorer",
            "model": "gpt-5.4-mini",
            "reasoning_effort": "low",
            "permission_mode": "default",
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
    let report = show_agent_session_json(&root, existing_child_id);
    assert_eq!(
        report["sessions"][0]["sessionId"].as_str(),
        Some(existing_child_id)
    );
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
        Some("enter-bootstrap-pane-for-existing-child")
    );
    assert_eq!(
        decision["fields"]["childSessionId"].as_str(),
        Some("019f126d-0000-7000-8000-000000000132")
    );
    assert!(decision["fields"].get("agentSessionBootstrap").is_none());
    let message = decision["message"].as_str().unwrap_or_default();
    assert!(message.contains("Enter the resident-child choice pane"));
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
        "asp search owner src/lib.rs items --workspace . --view seeds",
        "asp query --selector src/lib.rs --workspace . --code",
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
        "asp rust query --selector src/lib.rs --workspace . --code",
        "asp org query --selector org://docs/spec.org#heading/run --workspace . --content",
        "asp org elements-query --packet '{\"selector\":\"docs/spec.org\"}'",
        "asp org contract trace docs/spec.org",
        "asp org contract query-surface docs/spec.org",
        "asp fd -query example .",
        "asp rg -query example src",
    ];

    for command in commands {
        let decision = run_codex_pre_tool_decision_with_env(
            &root,
            codex_asp_query_payload(command),
            &[("CODEX_THREAD_ID", "019f126d-0000-7000-8000-000000000003")],
        );

        assert_eq!(
            decision["decision"].as_str(),
            Some("deny"),
            "command={command} decision={decision}"
        );
        assert_eq!(
            decision["fields"]["agentSessionRoute"].as_str(),
            Some("asp-explore"),
            "command={command} decision={decision}"
        );
        assert_eq!(
            decision["fields"]["agentSessionLoopCommand"].as_str(),
            Some("asp agent session bootstrap --name asp-explore"),
            "command={command} decision={decision}"
        );
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
        codex_asp_query_payload("asp rust query src/lib.rs --workspace . --code"),
        &[("CODEX_THREAD_ID", "019f126d-0000-7000-8000-000000000001")],
    );

    assert_eq!(decision["decision"].as_str(), Some("deny"));
    assert_eq!(
        decision["reasonKind"].as_str(),
        Some("asp-reasoning-routed")
    );
    assert_eq!(
        decision["fields"]["agentSessionRoute"].as_str(),
        Some("asp-explore")
    );
    assert_eq!(
        decision["fields"]["residentChildName"].as_str(),
        Some("asp-explore")
    );
    assert_eq!(
        decision["fields"]["residentCodexAgentName"].as_str(),
        Some("asp_explorer")
    );
    assert_eq!(
        decision["fields"]["targetAgentName"].as_str(),
        Some("asp_explorer")
    );
    assert_eq!(
        decision["fields"]["targetAgentRole"].as_str(),
        Some("asp_explorer")
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
        decision["fields"]["agentSessionLifecycle"].as_str(),
        Some("resident")
    );
    assert_eq!(
        decision["fields"]["agentSessionLoopCommand"].as_str(),
        Some("asp agent session bootstrap --name asp-explore")
    );
    assert!(decision["fields"]["agentSessionStatusCheck"].is_null());
    assert!(decision["fields"]["agentSessionStatusCommand"].is_null());
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
        Some("use-existing-asp-explore-through-pane")
    );
    assert_eq!(
        decision["fields"]["nextAction"].as_str(),
        Some("enter-bootstrap-pane-if-transport-is-not-ready")
    );
    assert_eq!(
        decision["fields"]["targetAgentName"].as_str(),
        Some("asp_explorer")
    );
    assert_eq!(
        decision["fields"]["targetAgentRole"].as_str(),
        Some("asp_explorer")
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
    let message = decision["message"].as_str().unwrap_or_default();
    assert!(message.contains("ASP denied main-session ASP exploration"));
    assert!(message.contains("asp-explore"));
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
    assert_eq!(
        decision["reasonKind"].as_str(),
        Some("asp-reasoning-routed")
    );
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
        Some("use-existing-asp-explore-through-pane")
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
            Some("asp-reasoning-routed"),
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

    assert_eq!(decision["decision"].as_str(), Some("deny"));
    assert_eq!(
        decision["reasonKind"].as_str(),
        Some("asp-reasoning-routed")
    );
    assert_eq!(
        decision["fields"]["agentSessionRoute"].as_str(),
        Some("asp-explore")
    );
    assert_eq!(
        decision["fields"]["requiredAction"].as_str(),
        Some("use-existing-asp-explore-through-pane")
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
        Some(child_session_id)
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
            "tool_input": {
                "command": "asp rust search lexical resident owner --view seeds"
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
        Some("asp rust search lexical resident owner --view seeds")
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
    assert!(message.contains("resident-child interactive pane"));
    assert!(message.contains("asp agent session bootstrap --name asp-explore"));
    assert!(message.contains("choose one number"));
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
    assert!(decision["fields"].get("childSessionId").is_none());
    let message = decision["message"].as_str().unwrap_or_default();
    assert!(message.contains("asp agent session bootstrap --name asp-explore"));
    assert!(!message.contains("asp agent session bootstrap --name asp-explore --json"));
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
        decision["fields"]["agentSessionLoopCommand"].as_str(),
        Some("asp agent session bootstrap --name asp-explore")
    );
    assert!(decision["fields"]["agentSessionStatusCheck"].is_null());
    assert!(decision["fields"]["agentSessionStatusCommand"].is_null());
    assert_eq!(
        decision["fields"]["agentSessionTimeoutPolicy"].as_str(),
        Some("timeout-is-not-duplicate-worker-trigger")
    );
    assert_eq!(
        decision["fields"]["agentSessionAction"].as_str(),
        Some("enter-resident-child-bootstrap-pane")
    );
    assert_eq!(
        decision["fields"]["requiredAction"].as_str(),
        Some("enter-asp-explore-choice-pane")
    );
    assert_eq!(
        decision["fields"]["nextAction"].as_str(),
        Some("choose-one-bootstrap-pane-option")
    );
    assert_eq!(
        decision["fields"]["targetAgentName"].as_str(),
        Some("asp_explorer")
    );
    assert_eq!(
        decision["fields"]["targetAgentRole"].as_str(),
        Some("asp_explorer")
    );
    assert_eq!(
        decision["fields"]["forbiddenUntilResolved"].as_str(),
        Some("raw-source-fallback")
    );
    assert_eq!(
        decision["fields"]["completionReceipt"].as_str(),
        Some("asp-explore-choice-pane-receipt")
    );
    assert_eq!(
        decision["fields"]["agentSessionBootstrap"].as_str(),
        Some("session-start-reminder")
    );
    assert_eq!(
        decision["fields"]["agentSessionBootstrapGuideCommand"].as_str(),
        Some("asp agent session bootstrap --name asp-explore")
    );
    assert_eq!(
        decision["fields"]["residentCodexAgentName"].as_str(),
        Some("asp_explorer")
    );
    assert!(decision["fields"]["agentSessionLookupCommand"].is_null());
    assert!(decision["fields"]["agentSessionRegisterCommandTemplate"].is_null());
    assert_eq!(
        decision["fields"]["rootSessionId"].as_str(),
        Some("019f126d-0000-7000-8000-000000000002")
    );
    assert!(decision["fields"].get("childSessionId").is_none());
    let message = decision["message"].as_str().unwrap_or_default();
    assert!(message.contains("asp agent session bootstrap --name asp-explore"));
    assert!(!message.contains("asp agent session bootstrap --name asp-explore --json"));
    assert!(!message.contains("asp agent session register --guide"));
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

    assert_eq!(decision["decision"].as_str(), Some("deny"));
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
    assert!(message.contains("ASP resident-child interactive loop"));
    assert!(message.contains("asp agent session bootstrap --name asp-explore"));
    assert!(!message.contains("asp agent session bootstrap --name asp-explore --json"));
    assert!(message.contains("asp-explore"));
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
        Some("ignore-unmanaged-native-subagent")
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
