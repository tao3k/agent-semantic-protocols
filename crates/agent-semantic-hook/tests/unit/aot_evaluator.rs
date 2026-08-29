use super::evaluate_pre_tool;

const GENERATION: &str = r#"{
  "schemaId":"agent.semantic-protocols.hook-generation",
  "schemaVersion":1,
  "generationDigest":"blake3-256:g1",
  "rules":[{
    "id":"allow-explicit-no-agent",
    "priority":200000,
    "matchers":["Bash"],
    "decision":"allow",
    "intent":"no-agent",
    "reasonKind":"hook-policy-bypass",
    "message":"Allowed explicit no-agent process.",
    "processEnvironmentAssignmentAny":["ASP_NO_AGENT=1"]
  },{
    "id":"route-read-to-asp-languages",
    "matchers":["Bash"],
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
  "schemaId":"agent.semantic-protocols.hook-generation",
  "schemaVersion":1,
  "generationDigest":"blake3-256:g2",
  "rules":[{
    "id":"deny-edit",
    "matchers":["Edit"],
    "decision":"deny",
    "reasonKind":"edit-policy-denied",
    "message":"Edit is denied."
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
fn process_bound_no_agent_assignment_is_a_terminal_allow() {
    for command in [
        "ASP_NO_AGENT=1 arbitrary-command src/a.rs",
        "/usr/bin/env ASP_NO_AGENT=1 arbitrary-command src/a.rs",
        "export ASP_NO_AGENT=1; arbitrary-command src/a.rs",
        "export ASP_NO_AGENT=1; exec arbitrary-command src/a.rs",
    ] {
        let payload = serde_json::json!({
            "tool_name": "Bash",
            "tool_input": {"command": command}
        })
        .to_string();
        let decision = evaluate_pre_tool(GENERATION, &payload, "Bash")
            .expect("evaluate")
            .expect("terminal allow");
        assert_eq!(decision.config_rule_id, "allow-explicit-no-agent");
        assert_eq!(decision.decision, "allow");
    }
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
fn registered_org_and_markdown_operands_require_a_reader_fact() {
    for path in ["notes/spec.org", "README.md"] {
        let payload =
            format!(r#"{{"tool_name":"Bash","tool_input":{{"command":"unknown-reader {path}"}}}}"#);
        assert_eq!(evaluate_pre_tool(GENERATION, &payload, "Bash"), Ok(None));
    }
}
