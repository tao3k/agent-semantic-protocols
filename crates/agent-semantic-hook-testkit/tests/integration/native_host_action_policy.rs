use agent_semantic_hook::aot_evaluator::evaluate_pre_tool;
use agent_semantic_hook::aot_evaluator::reader_probe_request;

use super::aot_evaluator_contract::canonical_generation;

#[test]
fn exact_native_host_actions_bypass_reader_probe_and_retain_policy_boundaries() {
    let generation = canonical_generation();
    let edit_payload = serde_json::json!({
        "session_id": "testkit-native-edit",
        "cwd": ".",
        "hook_event_name": "PreToolUse",
        "tool_name": "apply_patch",
        "tool_input": {"patch": "*** Begin Patch\n*** Update File: README.md\n*** End Patch"}
    })
    .to_string();
    let edit = evaluate_pre_tool(&generation, &edit_payload, "apply_patch")
        .expect("evaluate native Edit")
        .expect("owner-scoped native Edit decision");
    assert_eq!(edit.config_rule_id, "allow-owner-scoped-mutation");
    assert_eq!(edit.operation_intent, "owner-scoped-mutation");
    assert_eq!(edit.decision, "allow");

    for (tool_name, expected_decision, expected_rule) in [
        (
            "mcp__codex_app__send_message_to_thread",
            "allow",
            "allow-codex-thread-control",
        ),
        (
            "mcp__codex_app__automation_update",
            "allow",
            "allow-codex-automation-control",
        ),
    ] {
        let payload = serde_json::json!({
            "session_id": "testkit-native-codex-app",
            "cwd": ".",
            "hook_event_name": "PreToolUse",
            "tool_name": tool_name,
            "tool_input": {}
        })
        .to_string();
        assert!(
            reader_probe_request(&generation, &payload, tool_name)
                .expect("exact native Host action probe admission")
                .is_none(),
            "exact Host actions must never enter Reader Probe: {tool_name}"
        );
        let decision = evaluate_pre_tool(&generation, &payload, tool_name)
            .unwrap_or_else(|error| panic!("evaluate {tool_name}: {error}"))
            .unwrap_or_else(|| panic!("missing exact decision for {tool_name}"));
        assert_eq!(decision.config_rule_id, expected_rule);
        assert_eq!(decision.decision, expected_decision, "tool={tool_name}");
    }

    for tool_name in [
        "mcp__codex_app__consume_usage_reset",
        "mcp__codex_app__uninstall_plugin",
    ] {
        let payload = serde_json::json!({
            "session_id": "testkit-host-owned-sensitive-tool",
            "cwd": ".",
            "hook_event_name": "PreToolUse",
            "tool_name": tool_name,
            "tool_input": {}
        })
        .to_string();
        assert!(
            evaluate_pre_tool(&generation, &payload, tool_name)
                .unwrap_or_else(|error| panic!("evaluate {tool_name}: {error}"))
                .is_none(),
            "sensitive Host-owned tool must not have an ASP policy decision: {tool_name}"
        );
    }

    let unconfigured_payload = serde_json::json!({
        "session_id": "testkit-native-mcp",
        "cwd": ".",
        "hook_event_name": "PreToolUse",
        "tool_name": "mcp__filesystem__metadata",
        "tool_input": {}
    })
    .to_string();
    assert!(
        evaluate_pre_tool(
            &generation,
            &unconfigured_payload,
            "mcp__filesystem__metadata"
        )
        .expect("evaluate unconfigured exact MCP tool")
        .is_none(),
        "unconfigured MCP action must remain an explicit no-decision"
    );

    let spawn_payload = serde_json::json!({
        "session_id": "testkit-native-agent",
        "cwd": ".",
        "hook_event_name": "PreToolUse",
        "tool_name": "spawn_agent",
        "tool_input": {"agent_type": "asp_explorer"}
    })
    .to_string();
    assert!(
        evaluate_pre_tool(&generation, &spawn_payload, "spawn_agent")
            .expect("evaluate spawn_agent Host match")
            .is_none(),
        "spawn admission is owned by the lifecycle control plane"
    );
}
