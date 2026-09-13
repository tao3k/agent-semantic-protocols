// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::path::Path;

use agent_semantic_config::CodexCollaborationToolCall;
use agent_semantic_config::CollaborationDispatchState;
use agent_semantic_config::CollaborationHostResultKind;
use agent_semantic_config::CollaborationInterruptResult;
use agent_semantic_config::CollaborationLifecycleTool;
use agent_semantic_config::CollaborationSpawnResult;
use agent_semantic_config::CollaborationWaitResult;
use agent_semantic_config::state_after_interrupt;
use agent_semantic_hook::aot_evaluator::evaluate_pre_tool;

const TOOL_CALL_SCHEMA: &str =
    include_str!("../../../../schemas/codex-collaboration-tool-call.v1.schema.json");
const LIVE_AGENTS_SCHEMA: &str =
    include_str!("../../../../schemas/codex-collaboration-live-agents.v1.schema.json");
const SPAWN_RESULT_SCHEMA: &str =
    include_str!("../../../../schemas/codex-collaboration-spawn-result.v1.schema.json");
const INTERRUPT_RESULT_SCHEMA: &str =
    include_str!("../../../../schemas/codex-collaboration-interrupt-result.v1.schema.json");
const WAIT_RESULT_SCHEMA: &str =
    include_str!("../../../../schemas/codex-collaboration-wait-result.v1.schema.json");
const ORG_CONTRACT: &str =
    include_str!("../../../../org/contracts/agent.multi-agent-session-control-plane.v1.org");

const TESTING_DISPATCH_GENERATION: &str = r#"{
  "schemaId":"agent.semantic-protocols.hook-policy-bundle",
  "schemaVersion":1,
  "generationDigest":"blake3-256:collaboration-parent-message",
  "rules":[{
    "id":"testing-parent-message",
    "matchers":["Bash"],
    "wrappedCommand":true,
    "decision":"deny",
    "intent":"test-build-command",
    "reasonKind":"agent-choice-required",
    "message":"{{agentDispatchMessage}}",
    "route":"asp_testing",
    "argvPrefixAny":[["cargo","test"]]
  }]
}"#;

#[test]
fn all_native_collaboration_tool_fixtures_match_one_schema() {
    let schema: serde_json::Value = serde_json::from_str(TOOL_CALL_SCHEMA).expect("tool schema");
    let validator = jsonschema::validator_for(&schema).expect("compile collaboration schema");
    let fixture_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../schemas/fixtures/codex-collaboration-tool-call");

    for name in [
        "valid-list-agents.v1.json",
        "valid-list-agents-root.v1.json",
        "valid-spawn-agent.v1.json",
        "valid-followup-task.v1.json",
        "valid-send-message.v1.json",
        "valid-interrupt-agent.v1.json",
        "valid-wait-agent.v1.json",
    ] {
        let bytes = std::fs::read(fixture_root.join(name)).expect("read valid fixture");
        let value: serde_json::Value = serde_json::from_slice(&bytes).expect("decode fixture");
        assert!(validator.is_valid(&value), "fixture must be valid: {name}");
        let typed: CodexCollaborationToolCall =
            serde_json::from_value(value).expect("decode Config-owned typed call");
        typed
            .validate()
            .expect("Config-owned interface and JSON Schema must agree");
    }

    let invalid = std::fs::read(fixture_root.join("invalid-choice-plane.v1.json"))
        .expect("read invalid fixture");
    let invalid: serde_json::Value = serde_json::from_slice(&invalid).expect("decode fixture");
    assert!(!validator.is_valid(&invalid));

    let mut unsupported_spawn_field: serde_json::Value = serde_json::from_slice(
        &std::fs::read(fixture_root.join("valid-spawn-agent.v1.json")).expect("read spawn fixture"),
    )
    .expect("decode spawn fixture");
    unsupported_spawn_field["toolInput"]["service_tier"] = serde_json::json!("priority");
    assert!(
        !validator.is_valid(&unsupported_spawn_field),
        "the schema must reject fields absent from Codex MultiAgentV2 SpawnAgentArgs"
    );

    let mut root_followup: serde_json::Value = serde_json::from_slice(
        &std::fs::read(fixture_root.join("valid-followup-task.v1.json"))
            .expect("read followup fixture"),
    )
    .expect("decode followup fixture");
    root_followup["toolInput"]["target"] = serde_json::json!("/root");
    assert!(
        !validator.is_valid(&root_followup),
        "Codex rejects followup_task targeting the root Agent"
    );

    for fixture in [
        "valid-spawn-agent.v1.json",
        "valid-followup-task.v1.json",
        "valid-send-message.v1.json",
    ] {
        let mut value: serde_json::Value = serde_json::from_slice(
            &std::fs::read(fixture_root.join(fixture)).expect("read message fixture"),
        )
        .expect("decode message fixture");
        value["toolInput"]["message"] = serde_json::json!("   ");
        assert!(
            !validator.is_valid(&value),
            "Codex message_content rejects whitespace-only input: {fixture}"
        );
    }
}

#[test]
fn live_agent_status_schema_matches_codex_multi_agent_v2_exactly() {
    let schema: serde_json::Value =
        serde_json::from_str(LIVE_AGENTS_SCHEMA).expect("live agents schema");
    let validator = jsonschema::validator_for(&schema).expect("compile live agents schema");

    for status in [
        serde_json::json!("pending_init"),
        serde_json::json!("running"),
        serde_json::json!("interrupted"),
        serde_json::json!({"completed": null}),
        serde_json::json!({"completed": "done"}),
        serde_json::json!({"errored": "failed"}),
        serde_json::json!("shutdown"),
        serde_json::json!("not_found"),
    ] {
        let value = serde_json::json!({
            "agents": [{"agent_name": "/root", "agent_status": status}]
        });
        assert!(
            validator.is_valid(&value),
            "status must match Codex: {value}"
        );
    }

    for status in [
        serde_json::json!("imaginary"),
        serde_json::json!({"completed": 1}),
        serde_json::json!({"errored": null}),
        serde_json::json!({"running": true}),
    ] {
        let value = serde_json::json!({
            "agents": [{"agent_name": "/root", "agent_status": status}]
        });
        assert!(
            !validator.is_valid(&value),
            "invented status admitted: {value}"
        );
    }
}

#[test]
fn structured_host_results_match_codex_v2_without_inventing_message_envelopes() {
    let cases = [
        (
            SPAWN_RESULT_SCHEMA,
            serde_json::json!({"task_name": "/root/asp_testing", "nickname": null}),
        ),
        (
            INTERRUPT_RESULT_SCHEMA,
            serde_json::json!({"previous_status": "running"}),
        ),
        (
            WAIT_RESULT_SCHEMA,
            serde_json::json!({"message": "Wait completed.", "timed_out": false}),
        ),
    ];
    for (schema, value) in &cases {
        let schema: serde_json::Value = serde_json::from_str(schema).expect("result schema");
        let validator = jsonschema::validator_for(&schema).expect("compile result schema");
        assert!(validator.is_valid(value), "result={value}");
    }

    let spawn: CollaborationSpawnResult =
        serde_json::from_value(cases[0].1.clone()).expect("decode typed spawn result");
    spawn.validate().expect("validate typed spawn result");
    let _: CollaborationInterruptResult =
        serde_json::from_value(cases[1].1.clone()).expect("decode typed interrupt result");
    let wait: CollaborationWaitResult =
        serde_json::from_value(cases[2].1.clone()).expect("decode typed wait result");
    wait.validate().expect("validate typed wait result");

    assert!(ORG_CONTRACT.contains("successful empty tool output plus Host Agent activity"));
    assert!(!ORG_CONTRACT.contains("send_message result envelope"));
    assert!(!ORG_CONTRACT.contains("followup_task result envelope"));
}

#[test]
fn org_babel_owns_the_exact_syntax_for_every_native_collaboration_tool() {
    for syntax in [
        "collaboration.list_agents({",
        "collaboration.spawn_agent({",
        "collaboration.followup_task({",
        "collaboration.send_message({",
        "collaboration.interrupt_agent({",
        "collaboration.wait_agent({",
    ] {
        assert!(
            ORG_CONTRACT.contains(syntax),
            "missing Org syntax: {syntax}"
        );
    }
    assert!(!ORG_CONTRACT.contains("choice_plane("));
    assert!(!ORG_CONTRACT.contains("mcp__codex_app__"));
    assert!(!ORG_CONTRACT.contains("multi_agent_v1"));
    assert!(!ORG_CONTRACT.contains("register-delivered-child"));
    assert!(ORG_CONTRACT.contains("asp session register-child"));
    assert!(ORG_CONTRACT.contains("successful empty tool output plus Host Agent activity"));
    assert!(ORG_CONTRACT.contains("belong to a separate thread-management surface"));
    assert!(ORG_CONTRACT.contains("message: {{{MESSAGE}}}"));
    assert!(ORG_CONTRACT.contains("parent-authored task exactly as written"));
    assert!(!ORG_CONTRACT.contains("AGENT_PATH_JSON"));
    assert!(!ORG_CONTRACT.contains("AGENT_TYPE_JSON"));
    assert!(!ORG_CONTRACT.contains("MESSAGE_JSON"));
}

#[test]
fn lifecycle_semantics_prevent_duplicate_turns_and_agent_recreation() {
    assert!(CollaborationLifecycleTool::SpawnAgent.starts_turn());
    assert!(CollaborationLifecycleTool::FollowupTask.starts_turn());
    assert!(!CollaborationLifecycleTool::SendMessage.starts_turn());
    assert!(CollaborationLifecycleTool::WaitAgent.observes_only());
    assert!(CollaborationLifecycleTool::ListAgents.observes_only());
    assert_eq!(
        state_after_interrupt(CollaborationDispatchState::Running),
        CollaborationDispatchState::Reusable
    );
    assert!(ORG_CONTRACT.contains("prevents a Host snapshot race"));
    assert!(ORG_CONTRACT.contains("it is not a dispatch path"));
    assert_eq!(
        CollaborationLifecycleTool::SendMessage.host_result_kind(),
        CollaborationHostResultKind::EmptyActivity
    );
    assert_eq!(
        CollaborationLifecycleTool::FollowupTask.host_result_kind(),
        CollaborationHostResultKind::EmptyActivity
    );
    assert!(ORG_CONTRACT.contains(
        "The Codex MultiAgentV2 implementation does not return an invented uniform JSON"
    ));
    assert!(ORG_CONTRACT.contains("no result JSON is invented"));
}

#[test]
fn aot_dispatch_escapes_the_parent_message_without_exposing_encoding_placeholders() {
    let payload = serde_json::json!({
        "tool_name": "Bash",
        "tool_input": {"command": "cargo test -p \"fixture\""},
        "session_id": "parent-thread-1"
    })
    .to_string();
    let decision = evaluate_pre_tool(TESTING_DISPATCH_GENERATION, &payload, "Bash")
        .expect("evaluate parent dispatch")
        .expect("dispatch must deny in the parent");

    assert!(decision.message.contains("agent_type: \"asp_testing\""));
    assert!(decision.message.contains("target: \"/root/asp_testing\""));
    assert!(decision.message.contains(
        "asp session register-child --parent-thread-id parent-thread-1 --agent-name asp_testing"
    ));
    assert!(decision.message.contains(
        "Invoke Host tool `Bash` exactly once with input {\\\"command\\\":\\\"cargo test -p \\\\\\\"fixture\\\\\\\"\\\"}"
    ));
    assert!(decision.message.contains("message: \"First run"));
    assert!(!decision.message.contains("{{{"));
    assert!(!decision.message.contains("MESSAGE_JSON"));
    assert!(!decision.message.contains("AGENT_PATH_JSON"));
}
