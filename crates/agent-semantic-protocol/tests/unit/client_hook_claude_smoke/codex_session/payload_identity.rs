use serde_json::json;

use super::{
    assert_configured_asp_explore_dispatch, claude_fixture, install_codex_hooks,
    register_asp_explore_session, run_codex_pre_tool_decision_with_env, show_agent_session_json,
    write_codex_asp_explore_rollout,
};

#[test]
fn codex_normal_task_routes_native_source_reads_by_activated_provider() {
    let root = claude_fixture();
    let codex_home = root.join(".codex-home");
    install_codex_hooks(&root, &codex_home);

    for (suffix, relative_path, expected_decision, expected_reason) in [
        ("rust", "src/core.rs", "deny", "direct-source-read"),
        ("hook", "src/hook_runtime.rs", "deny", "direct-source-read"),
        ("typescript", "src/component.ts", "allow", "none"),
        ("tsx", "src/view.tsx", "allow", "none"),
    ] {
        let path = root.join(relative_path);
        std::fs::write(&path, "// precise native read fixture\n").expect("write read fixture");
        let session_id = format!("019f5c0f-a653-7040-ab54-read-{suffix}");
        let decision = run_codex_pre_tool_decision_with_env(
            &root,
            json!({
                "session_id": session_id,
                "transcript_path": format!("/tmp/rollout-read-{suffix}.jsonl"),
                "cwd": root,
                "tool_name": "Read",
                "tool_input": {"file_path": path}
            }),
            &[],
        );

        assert_eq!(
            decision["decision"], expected_decision,
            "{relative_path}: {decision}"
        );
        assert_eq!(
            decision["reasonKind"], expected_reason,
            "{relative_path}: {decision}"
        );
        assert!(
            decision["fields"].get("executionLane").is_none(),
            "{relative_path}: {decision}"
        );
        if expected_decision == "deny" {
            assert_eq!(decision["routes"][0]["kind"], "owner");
        } else {
            assert!(
                decision["routes"].as_array().is_some_and(Vec::is_empty),
                "{relative_path}: {decision}"
            );
        }
    }
}

#[test]
fn codex_normal_task_routes_reasoning_from_payload_session_without_thread_env() {
    let root = claude_fixture();
    let codex_home = root.join(".codex-home");
    install_codex_hooks(&root, &codex_home);
    let session_id = "019f5c0f-a653-7040-ab54-514605a1c72a";
    let decision = run_codex_pre_tool_decision_with_env(
        &root,
        json!({
            "session_id": session_id,
            "transcript_path": format!("/tmp/rollout-{session_id}.jsonl"),
            "cwd": root,
            "tool_name": "Bash",
            "tool_input": {"command": "asp rust query --term guide --workspace ."}
        }),
        &[],
    );

    assert_configured_asp_explore_dispatch(&decision);
    assert_eq!(decision["fields"]["sessionId"], session_id);
}

#[test]
fn codex_proven_resident_child_executes_parser_command_without_self_routing() {
    let root = claude_fixture();
    let codex_home = root.join(".codex-home");
    install_codex_hooks(&root, &codex_home);
    let root_id = "019f5c0f-a653-7040-ab54-resident-root";
    let stale_child_id = "019f5c0f-a653-7040-ab54-resident-stale";
    let child_id = "019f5c0f-a653-7040-ab54-resident-terminal";
    register_asp_explore_session(&root, root_id, stale_child_id);
    write_codex_asp_explore_rollout(&root, root_id, child_id, "gpt-5.4-mini");
    let command = "asp gerbil-scheme search lexical package-source-stage-topology-request-specs owner tests --workspace . --view seeds";
    let decision = run_codex_pre_tool_decision_with_env(
        &root,
        json!({
            "session_id": root_id,
            "agent_id": child_id,
            "agent_type": "asp_explorer",
            "transcript_path": format!("/tmp/rollout-{child_id}.jsonl"),
            "cwd": root,
            "tool_name": "Bash",
            "tool_input": {"command": command}
        }),
        &[("CODEX_THREAD_ID", root_id)],
    );

    assert_eq!(decision["decision"], "allow", "{decision}");
    assert_eq!(decision["reasonKind"], "none", "{decision}");
    assert_eq!(
        decision["fields"]["agentSessionAction"],
        "active-hook-selected-resident"
    );
    assert_eq!(decision["fields"]["routingTerminal"], true);
    assert_eq!(decision["fields"]["redispatchAllowed"], false);
    assert_eq!(
        decision["fields"]["executionTransport"],
        "resident-child-terminal"
    );
    assert!(
        decision["fields"].get("agentSessionRoute").is_none(),
        "a resident terminal must not route back to itself: {decision}"
    );
    let rebound = show_agent_session_json(&root, child_id);
    assert_eq!(rebound["sessions"][0]["sessionId"], child_id, "{rebound}");
    assert_eq!(
        rebound["sessions"][0]["messageTargetId"], "/root/asp_explorer",
        "{rebound}"
    );
    assert_eq!(rebound["sessions"][0]["validation"]["status"], "passed");
}

#[test]
fn codex_hook_child_with_profile_drift_cannot_replace_resident_owner() {
    let root = claude_fixture();
    let codex_home = root.join(".codex-home");
    install_codex_hooks(&root, &codex_home);
    let root_id = "019f5c0f-a653-7040-ab54-drift-root";
    let current_child_id = "019f5c0f-a653-7040-ab54-drift-current";
    let drifted_child_id = "019f5c0f-a653-7040-ab54-drifted-child";
    register_asp_explore_session(&root, root_id, current_child_id);
    write_codex_asp_explore_rollout(&root, root_id, drifted_child_id, "gpt-5.6-sol");

    let decision = run_codex_pre_tool_decision_with_env(
        &root,
        json!({
            "session_id": root_id,
            "agent_id": drifted_child_id,
            "agent_type": "asp_explorer",
            "transcript_path": format!("/tmp/rollout-{drifted_child_id}.jsonl"),
            "cwd": root,
            "tool_name": "Bash",
            "tool_input": {"command": "asp rust search lexical replacement owner tests --workspace . --view seeds"}
        }),
        &[],
    );

    assert_eq!(decision["decision"], "allow", "{decision}");
    assert_eq!(decision["fields"]["routingTerminal"], true, "{decision}");
    let registry = agent_semantic_client_db::AgentSessionRegistry::open_existing_state_root(
        root.join(".agent-semantic-protocols"),
    )
    .expect("open registry")
    .expect("registry exists");
    let project_id = agent_semantic_client_db::AgentSessionRegistry::project_scope_id(&root);
    let retained = registry
        .session_by_name(&project_id, root_id, "asp-explore")
        .expect("read resident route")
        .expect("resident route exists");
    assert_eq!(
        retained.session_id, current_child_id,
        "drifted child must not replace the resident owner: {retained:?}"
    );
}

#[test]
fn codex_generic_child_cannot_spoof_resident_identity_through_tool_input() {
    let root = claude_fixture();
    let codex_home = root.join(".codex-home");
    install_codex_hooks(&root, &codex_home);
    let root_id = "019f5c0f-a653-7040-ab54-generic-root";
    let decision = run_codex_pre_tool_decision_with_env(
        &root,
        json!({
            "session_id": root_id,
            "transcript_path": "/tmp/rollout-generic-child.jsonl",
            "cwd": root,
            "tool_name": "CustomTool",
            "tool_input": {
                "agent_id": "spoofed-child",
                "agent_type": "asp_explorer",
                "command": "asp rust search lexical owner tests --workspace . --view seeds"
            }
        }),
        &[("CODEX_THREAD_ID", root_id)],
    );

    assert_eq!(decision["decision"], "allow", "{decision}");
    assert_eq!(decision["reasonKind"], "none");
    assert!(
        decision["fields"]
            .get("residentChildIdentityProof")
            .is_none()
    );
    assert!(decision["fields"].get("routingTerminal").is_none());
}

#[test]
fn codex_unobservable_child_can_enter_only_the_validating_dispatch_wrapper() {
    let root = claude_fixture();
    let codex_home = root.join(".codex-home");
    install_codex_hooks(&root, &codex_home);
    let child_id = "019f5c0f-a653-7040-ab54-unobservable-child";
    let decision = run_codex_pre_tool_decision_with_env(
        &root,
        json!({
            "session_id": child_id,
            "transcript_path": "/tmp/rollout-dispatch-wrapper.jsonl",
            "cwd": root,
            "tool_name": "Bash",
            "tool_input": {"command": "direnv exec . asp agent session dispatch-execute --name asp-explore --root-session-id root --dispatch-identity once --command-digest digest --command-json '[]'"}
        }),
        &[("CODEX_THREAD_ID", child_id)],
    );

    assert_eq!(decision["decision"], "allow", "{decision}");
    assert_eq!(
        decision["fields"]["agentSessionAction"],
        "resident-command-bridge"
    );
    assert_eq!(
        decision["fields"]["executionLane"],
        "hook-selected-resident"
    );
    assert_eq!(decision["fields"]["routingTerminal"], true);
    assert_eq!(decision["fields"]["redispatchAllowed"], false);
}

#[test]
fn codex_normal_task_preserves_root_language_guide_spelling() {
    let root = claude_fixture();
    let codex_home = root.join(".codex-home");
    install_codex_hooks(&root, &codex_home);
    let session_id = "019f5c0f-a653-7040-ab54-root-guide";
    let decision = run_codex_pre_tool_decision_with_env(
        &root,
        json!({
            "session_id": session_id,
            "transcript_path": "/tmp/rollout-root-guide.jsonl",
            "cwd": root,
            "tool_name": "Bash",
            "tool_input": {"command": "asp guide --language rust"}
        }),
        &[],
    );

    assert_eq!(decision["decision"], "allow", "{decision}");
    assert_eq!(decision["reasonKind"], "none", "{decision}");
    assert!(decision["fields"].get("residentName").is_none());
}

#[test]
fn codex_normal_task_preserves_non_reasoning_intents_from_payload_session() {
    let root = claude_fixture();
    let codex_home = root.join(".codex-home");
    install_codex_hooks(&root, &codex_home);

    for (suffix, command) in [
        (
            "invalid",
            "asp rust query --selector src/lib.rs --workspace . --code",
        ),
        (
            "exact",
            "asp rust query --selector rust://src/lib.rs#item/function/run --workspace . --code",
        ),
        (
            "fallback",
            "asp rust query --from-hook direct-source-read --selector src/lib.rs:1:10 --workspace . --code --fallback-reason bounded",
        ),
    ] {
        let session_id = format!("019f5c0f-a653-7040-ab54-{suffix}");
        let decision = run_codex_pre_tool_decision_with_env(
            &root,
            json!({
                "session_id": session_id,
                "transcript_path": format!("/tmp/rollout-{suffix}.jsonl"),
                "cwd": root,
                "tool_name": "Bash",
                "tool_input": {"command": command}
            }),
            &[],
        );

        assert_eq!(decision["decision"], "allow", "{command}: {decision}");
        assert_eq!(decision["reasonKind"], "none", "{command}: {decision}");
        assert!(
            decision["fields"].get("residentName").is_none(),
            "{command}: {decision}"
        );
    }
}

#[test]
fn codex_normal_task_does_not_route_configured_control_plane_command() {
    let root = claude_fixture();
    let codex_home = root.join(".codex-home");
    install_codex_hooks(&root, &codex_home);
    let session_id = "019f5c0f-a653-7040-ab54-control";
    let command = "asp install plugin --codex .";
    let decision = run_codex_pre_tool_decision_with_env(
        &root,
        json!({
            "session_id": session_id,
            "transcript_path": "/tmp/rollout-control.jsonl",
            "cwd": root,
            "tool_name": "Bash",
            "tool_input": {"command": command}
        }),
        &[],
    );

    assert_eq!(decision["decision"], "allow", "{decision}");
    assert!(
        decision["fields"].get("agentSessionRoute").is_none(),
        "{decision}"
    );
    assert!(
        decision["fields"].get("aspCommandIntent").is_none(),
        "{decision}"
    );
}

#[test]
fn codex_normal_task_matches_only_supported_asp_invocations() {
    let root = claude_fixture();
    let codex_home = root.join(".codex-home");
    install_codex_hooks(&root, &codex_home);

    for (suffix, command, expected_decision, expected_reason) in [
        ("semicolon", "printf ready; asp rust guide", "allow", "none"),
        ("data", "printf '%s' 'asp rust guide'", "allow", "none"),
        ("invalid-facade", "asp bananas guide", "allow", "none"),
    ] {
        let session_id = format!("019f5c0f-a653-7040-ab54-{suffix}");
        let decision = run_codex_pre_tool_decision_with_env(
            &root,
            json!({
                "session_id": session_id,
                "transcript_path": format!("/tmp/rollout-{suffix}.jsonl"),
                "cwd": root,
                "tool_name": "Bash",
                "tool_input": {"command": command}
            }),
            &[],
        );

        assert_eq!(decision["decision"], expected_decision, "{decision}");
        assert_eq!(decision["reasonKind"], expected_reason, "{decision}");
        if suffix == "invalid-facade" {
            assert!(
                decision["fields"].get("residentName").is_none()
                    && decision["fields"].get("agentSessionRoute").is_none(),
                "{decision}"
            );
        }
    }
}

#[test]
fn codex_normal_task_does_not_treat_asp_path_inspection_as_an_invocation() {
    let root = claude_fixture();
    let codex_home = root.join(".codex-home");
    install_codex_hooks(&root, &codex_home);
    let session_id = "019f5c0f-a653-7040-ab54-path-inspection";
    let command =
        "command -v asp; readlink \"$(command -v asp)\"; shasum -a 256 \"$(command -v asp)\"";
    let decision = run_codex_pre_tool_decision_with_env(
        &root,
        json!({
            "session_id": session_id,
            "transcript_path": "/tmp/rollout-path-inspection.jsonl",
            "cwd": root,
            "tool_name": "Bash",
            "tool_input": {"command": command}
        }),
        &[],
    );

    assert_eq!(decision["decision"], "allow", "{decision}");
    assert_eq!(decision["reasonKind"], "none", "{decision}");
    assert!(
        decision["fields"].get("aspCommandIntent").is_none(),
        "{decision}"
    );
    assert!(
        decision["fields"].get("agentSessionRoute").is_none(),
        "{decision}"
    );
}

#[test]
fn codex_normal_task_routes_testing_execution_to_configured_resident() {
    let root = claude_fixture();
    let codex_home = root.join(".codex-home");
    install_codex_hooks(&root, &codex_home);
    let session_id = "019f5c0f-a653-7040-ab54-testing";
    let command = "cargo test -p agent-semantic-config";
    let decision = run_codex_pre_tool_decision_with_env(
        &root,
        json!({
            "session_id": session_id,
            "transcript_path": "/tmp/rollout-testing.jsonl",
            "cwd": root,
            "tool_name": "Bash",
            "tool_input": {"command": command}
        }),
        &[],
    );

    assert_eq!(decision["decision"], "deny", "{decision}");
    assert_eq!(
        decision["reasonKind"], "subagent-receipt-required",
        "{decision}"
    );
    assert_eq!(decision["fields"]["executionLane"], "testing");
    assert_eq!(decision["fields"]["transport"], "resident-agent");
    assert_eq!(decision["fields"]["residentChildName"], "asp-testing");
    assert_eq!(decision["fields"]["targetAgentName"], "asp_testing");
    assert_eq!(decision["fields"]["canonicalTarget"], "/root/asp_testing");
    assert!(
        decision["message"].as_str().is_some_and(
            |message| message.contains("asp-testing") && !message.contains("asp-explore")
        ),
        "{decision}"
    );
    assert!(
        decision["fields"]
            .get("sourceAccessCompactMessage")
            .is_none(),
        "{decision}"
    );
    assert_eq!(
        decision["fields"]["receiptKind"],
        "asp-testing-execution-v1"
    );
    assert_eq!(decision["fields"]["sessionId"], session_id);
    assert!(
        decision["fields"]["commandDigest"]
            .as_str()
            .is_some_and(|digest| digest.starts_with("sha256:") && digest.len() == 71),
        "{decision}"
    );
    assert!(
        decision["fields"].get("agentSessionRoute").is_none(),
        "{decision}"
    );
}

#[test]
fn codex_host_proven_testing_resident_executes_its_lane_as_terminal() {
    let root = claude_fixture();
    let codex_home = root.join(".codex-home");
    install_codex_hooks(&root, &codex_home);
    let decision = run_codex_pre_tool_decision_with_env(
        &root,
        json!({
            "session_id": "019f5c0f-a653-7040-ab54-testing-root",
            "transcript_path": "/tmp/rollout-testing-child.jsonl",
            "agent_id": "019f5c0f-a653-7040-ab54-testing-child",
            "agent_type": "asp_testing",
            "cwd": root,
            "tool_name": "Bash",
            "tool_input": {"command": "cargo check -p agent-semantic-config"}
        }),
        &[],
    );

    assert_eq!(decision["decision"], "allow", "{decision}");
    assert_eq!(
        decision["fields"]["agentSessionAction"],
        "active-hook-selected-resident"
    );
    assert_eq!(decision["fields"]["executionLane"], "testing");
    assert_eq!(
        decision["fields"]["executionTransport"],
        "resident-child-terminal"
    );
    assert_eq!(decision["fields"]["routingTerminal"], true);
    assert_eq!(decision["fields"]["redispatchAllowed"], false);
    assert_eq!(decision["fields"]["targetAgentName"], "asp_testing");
}

#[test]
fn codex_configured_resident_spawn_requires_isolated_canonical_context() {
    let root = claude_fixture();
    let codex_home = root.join(".codex-home");
    install_codex_hooks(&root, &codex_home);

    for (task_name, fork_turns) in [
        ("asp_testing", serde_json::Value::Null),
        ("asp_testing", serde_json::json!("all")),
        ("testing-copy", serde_json::json!("none")),
    ] {
        let decision = run_codex_pre_tool_decision_with_env(
            &root,
            json!({
                "session_id": "019f5c0f-a653-7040-ab54-spawn-root",
                "cwd": root,
                "tool_name": "collaboration.spawn_agent",
                "tool_input": {
                    "agent_type": "asp_testing",
                    "task_name": task_name,
                    "fork_turns": fork_turns,
                    "message": "run the routed test"
                }
            }),
            &[],
        );
        assert_eq!(decision["decision"], "deny", "{decision}");
        assert_eq!(
            decision["fields"]["requiredAction"],
            "spawn-configured-resident-with-isolated-context"
        );
        assert_eq!(decision["fields"]["targetAgentName"], "asp_testing");
        assert_eq!(decision["fields"]["canonicalTarget"], "/root/asp_testing");
        assert_eq!(decision["fields"]["requiredForkTurns"], "none");
    }

    let valid = run_codex_pre_tool_decision_with_env(
        &root,
        json!({
            "session_id": "019f5c0f-a653-7040-ab54-spawn-root",
            "cwd": root,
            "tool_name": "collaboration.spawn_agent",
            "tool_input": {
                "agent_type": "asp_testing",
                "task_name": "asp_testing",
                "fork_turns": "none",
                "message": "run the routed test"
            }
        }),
        &[],
    );
    assert_ne!(valid["decision"], "deny", "{valid}");

    let generic = run_codex_pre_tool_decision_with_env(
        &root,
        json!({
            "session_id": "019f5c0f-a653-7040-ab54-spawn-root",
            "cwd": root,
            "tool_name": "collaboration.spawn_agent",
            "tool_input": {
                "agent_type": "worker",
                "task_name": "ordinary-worker",
                "fork_turns": "all",
                "message": "ordinary work"
            }
        }),
        &[],
    );
    assert_ne!(generic["decision"], "deny", "{generic}");
}

#[test]
fn codex_normal_task_routes_arbitrary_execution_lane_to_its_configured_resident() {
    let root = claude_fixture();
    let codex_home = root.join(".codex-home");
    install_codex_hooks(&root, &codex_home);
    let state_home = root.join(".agent-semantic-protocols");
    let profile_path = state_home.join("agents/release-builder_codex.toml");
    std::fs::write(
        &profile_path,
        r#"name = "release_builder"
description = "Release build execution lane."
model = "gpt-5.4-mini"
model_reasoning_effort = "low"
sandbox_mode = "workspace-write"
developer_instructions = "Run only routed release build commands."
"#,
    )
    .expect("write custom resident profile");
    let hook_config_path = state_home.join("hooks/config.toml");
    std::fs::create_dir_all(hook_config_path.parent().expect("hook config parent"))
        .expect("create hook config parent");
    let mut hook_config = agent_semantic_config::default_hook_client_config_template();
    hook_config.push_str(
        r#"

[[agents.residentAgents]]
enabled = true
name = "release-build"
role = "release_builder"
roles = ["subagent", "release", "build"]
permissions = ["workspace-write"]
codexAgentName = "release_builder"
sessionLifetime = "resident"

[[rules]]
id = "resident-release-dispatch"
priority = 90001
intent = "release-command"
decision = "deny"
reasonKind = "subagent-receipt-required"
message = "Release commands are dispatched to the configured release-build resident agent."
event = "pre-tool"

[rules.fields]
executionLane = "release"

[rules.match]
argvPrefixAny = [["just", "release-probe"]]

[rules.dispatch]
transport = "resident-agent"
residentName = "release-build"
receiptKind = "release-build-execution-v1"
"#,
    );
    std::fs::write(&hook_config_path, hook_config).expect("write custom hook lane");

    let decision = run_codex_pre_tool_decision_with_env(
        &root,
        json!({
            "session_id": "019f5c0f-a653-7040-ab54-release",
            "transcript_path": "/tmp/rollout-release.jsonl",
            "cwd": root,
            "tool_name": "Bash",
            "tool_input": {"command": "just release-probe"}
        }),
        &[],
    );

    assert_eq!(decision["decision"], "deny", "{decision}");
    assert_eq!(decision["fields"]["executionLane"], "release");
    assert_eq!(decision["fields"]["residentChildName"], "release-build");
    assert_eq!(decision["fields"]["targetAgentName"], "release_builder");
    assert_eq!(
        decision["fields"]["canonicalTarget"],
        "/root/release_builder"
    );
    assert!(
        decision["message"].as_str().is_some_and(
            |message| message.contains("release-build") && !message.contains("asp-explore")
        ),
        "{decision}"
    );
    assert_eq!(
        decision["fields"]["receiptKind"],
        "release-build-execution-v1"
    );

    for (task_name, fork_turns) in [("release_builder", "all"), ("release-builder-copy", "none")] {
        let rejected_spawn = run_codex_pre_tool_decision_with_env(
            &root,
            json!({
                "session_id": "019f5c0f-a653-7040-ab54-release",
                "cwd": root,
                "tool_name": "collaboration_v2.spawn_agent",
                "tool_input": {
                    "agent_type": "release_builder",
                    "task_name": task_name,
                    "fork_turns": fork_turns,
                    "message": "run the routed release build"
                }
            }),
            &[],
        );
        assert_eq!(rejected_spawn["decision"], "deny", "{rejected_spawn}");
        assert_eq!(
            rejected_spawn["fields"]["residentChildName"],
            "release-build"
        );
        assert_eq!(
            rejected_spawn["fields"]["canonicalTarget"],
            "/root/release_builder"
        );
        assert_eq!(
            rejected_spawn["fields"]["requiredAction"],
            "spawn-configured-resident-with-isolated-context"
        );
    }

    let canonical_spawn = run_codex_pre_tool_decision_with_env(
        &root,
        json!({
            "session_id": "019f5c0f-a653-7040-ab54-release",
            "cwd": root,
            "tool_name": "collaboration_v2.spawn_agent",
            "tool_input": {
                "agent_type": "release_builder",
                "task_name": "release_builder",
                "fork_turns": "none",
                "message": "run the routed release build"
            }
        }),
        &[],
    );
    assert_ne!(canonical_spawn["decision"], "deny", "{canonical_spawn}");
}
