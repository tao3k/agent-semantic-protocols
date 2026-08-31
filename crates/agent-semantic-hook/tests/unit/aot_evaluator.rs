use super::evaluate_pre_tool;

const GENERATION: &str = r#"{
  "schemaId":"agent.semantic-protocols.hook-policy-bundle",
  "schemaVersion":1,
  "generationDigest":"blake3-256:g1",
  "rules":[{
    "id":"route-read-to-asp-languages",
    "matchers":["Bash"],
    "wrappedCommand":true,
    "actions":["read"],
    "registeredExtensions":["rs","org","md"],
    "decision":"deny",
    "reasonKind":"registered-source-route-required",
    "message":"Use the parser-owned ASP route.",
    "profile":"rust",
    "language":"rust",
    "route":"asp languages search owner"
  }]
}"#;

const EDIT_GENERATION: &str = r#"{
  "schemaId":"agent.semantic-protocols.hook-policy-bundle",
  "schemaVersion":1,
  "generationDigest":"blake3-256:g2",
  "rules":[{
    "id":"deny-edit",
    "matchers":["Edit"],
    "wrappedCommand":false,
    "decision":"deny",
    "reasonKind":"edit-policy-denied",
    "message":"Edit is denied."
  }]
}"#;

const TESTING_GENERATION: &str = r#"{
  "schemaId":"agent.semantic-protocols.hook-policy-bundle",
  "schemaVersion":1,
  "generationDigest":"blake3-256:g3",
  "agentCallingPattern":"@{name}",
  "rules":[{
    "id":"testing-role-dispatch",
    "matchers":["Bash"],
    "wrappedCommand":true,
    "decision":"deny",
    "intent":"test-build-command",
    "reasonKind":"agent-choice-required",
    "message":"Use ASP Testing. {{agentDispatchMessage}}",
    "route":"asp_testing",
    "argvPrefixAny":[["cargo","test"]]
  }]
}"#;

#[test]
fn bash_registered_operand_without_behavior_fact_remains_allow() {
    let payload = r#"{"tool_name":"Bash","tool_input":{"command":"BATT -s src/a.rs"},"session_id":"s","tool_use_id":"t"}"#;
    assert_eq!(evaluate_pre_tool(GENERATION, payload, "Bash"), Ok(None));
}

#[test]
fn bash_without_registered_operand_does_not_match_source_rule() {
    let payload = r#"{"tool_name":"Bash","tool_input":{"command":"just --list | rg hook"}}"#;
    assert_eq!(evaluate_pre_tool(GENERATION, payload, "Bash"), Ok(None));
}

#[test]
fn escaped_codex_command_string_is_owned_when_json_unescaping_is_required() {
    let payload = serde_json::json!({
        "tool_name": "Bash",
        "tool_input": {"command": "just --list | rg \"hook\""}
    })
    .to_string();
    assert_eq!(evaluate_pre_tool(GENERATION, &payload, "Bash"), Ok(None));
}

#[test]
fn generation_schema_is_fail_closed() {
    let old = GENERATION.replace("\"schemaVersion\":1", "\"schemaVersion\":2");
    assert!(evaluate_pre_tool(&old, r#"{"tool_name":"Bash","tool_input":{}}"#, "Bash").is_err());
}

#[test]
fn mismatched_tool_name_is_fail_closed() {
    let payload = r#"{"tool_name":"apply_patch","tool_input":{}}"#;
    assert!(evaluate_pre_tool(GENERATION, payload, "Bash").is_err());
}

#[test]
fn native_edit_alias_is_authoritative_without_source_inference() {
    let payload = r#"{"tool_name":"apply_patch","tool_input":{"patch":"*** Begin Patch"}}"#;
    let decision = evaluate_pre_tool(EDIT_GENERATION, payload, "Edit")
        .expect("evaluate")
        .expect("deny");
    assert_eq!(decision.evidence, "host-matcher");
    assert_eq!(decision.subject, None);
    assert_eq!(decision.terminal, "host-matcher-decision");
}

#[test]
fn configured_testing_agent_role_makes_dispatch_idempotent() {
    let payload = serde_json::json!({
        "tool_name": "Bash",
        "tool_input": {"command": "cargo test -p fixture"},
        "agent_role": "asp_testing"
    })
    .to_string();
    assert_eq!(
        evaluate_pre_tool(TESTING_GENERATION, &payload, "Bash"),
        Ok(None)
    );
}

#[test]
fn codex_configured_agent_type_makes_dispatch_idempotent() {
    let payload = serde_json::json!({
        "tool_name": "Bash",
        "tool_input": {"command": "cargo test -p fixture"},
        "agent_type": "asp_testing"
    })
    .to_string();
    assert_eq!(
        evaluate_pre_tool(TESTING_GENERATION, &payload, "Bash"),
        Ok(None)
    );
}

#[test]
fn codex_child_identity_reaches_dispatch_fixed_point_on_first_pre_tool() {
    let payload = serde_json::json!({
        "tool_name": "Bash",
        "tool_input": {"command": "cargo test -p fixture"},
        "agent_id": "child-thread-id",
        "agent_type": "asp_testing",
        "session_id": "child-thread-id",
        "transcript_path": "/rollouts/child.jsonl"
    })
    .to_string();
    assert_eq!(
        evaluate_pre_tool(TESTING_GENERATION, &payload, "Bash"),
        Ok(None),
        "the configured child role must terminate dispatch in the same PreTool call"
    );
}

#[test]
fn temporary_subagent_identity_cannot_satisfy_registered_agent_dispatch() {
    let payload = serde_json::json!({
        "tool_name": "Bash",
        "tool_input": {"command": "cargo test -p fixture"},
        "agent_id": "child-testing",
        "agent_role": "temporary",
        "session_id": "parent-session-1"
    })
    .to_string();
    let decision = evaluate_pre_tool(TESTING_GENERATION, &payload, "Bash")
        .expect("evaluate unverified child")
        .expect("dispatch deny");
    assert_eq!(decision.reason_kind, "agent-choice-required");
    assert!(decision.message.contains("collaboration.spawn_agent({"));
    assert!(decision.message.contains("collaboration.list_agents({"));
    assert!(decision.message.contains(
        "asp session register-child --parent-thread-id parent-session-1 --agent-name asp_testing"
    ));
    assert!(decision.message.contains(
        "Invoke Host tool `Bash` exactly once with input {\\\"command\\\":\\\"cargo test -p fixture\\\"}"
    ));
    assert!(
        decision
            .message
            .contains("parent-authored task exactly as written")
    );
    assert!(!decision.message.contains("`@asp_testing`"));
    assert!(!decision.message.contains("{{agentDispatchMessage}}"));
}

#[test]
fn registered_org_and_markdown_operands_require_a_reader_fact() {
    for path in ["notes/spec.org", "README.md"] {
        let payload =
            format!(r#"{{"tool_name":"Bash","tool_input":{{"command":"unknown-reader {path}"}}}}"#);
        assert_eq!(evaluate_pre_tool(GENERATION, &payload, "Bash"), Ok(None));
    }
}
