use super::evaluate_pre_tool;

const GENERATION: &str = r#"{
  "schemaId":"agent.semantic-protocols.hook-generation",
  "schemaVersion":1,
  "generationDigest":"blake3-256:g1",
  "rules":[{
    "id":"route-unresolved-source-access-to-asp-languages",
    "matchers":["Bash"],
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
fn bash_registered_operand_is_unknown_source_access_without_probe() {
    let payload = r#"{"tool_name":"Bash","tool_input":{"command":"BATT -s src/a.rs"},"session_id":"s","tool_use_id":"t"}"#;
    let decision = evaluate_pre_tool(GENERATION, payload, "Bash")
        .expect("evaluate")
        .expect("deny");
    assert_eq!(decision.evidence, "unknown-source-access");
    assert_eq!(decision.subject, Some("src/a.rs"));
    assert_eq!(decision.reader_observation_micros, 0);
}

#[test]
fn bash_without_registered_operand_does_not_match_source_rule() {
    let payload = r#"{"tool_name":"Bash","tool_input":{"command":"just --list | rg hook"}}"#;
    assert_eq!(evaluate_pre_tool(GENERATION, payload, "Bash"), Ok(None));
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
fn payload_no_agent_assignment_is_not_a_bypass() {
    let payload = r#"{"tool_name":"Bash","tool_input":{"command":"ASP_NO_AGENT=1 BATT src/a.rs"}}"#;
    assert!(evaluate_pre_tool(GENERATION, payload, "Bash").expect("evaluate").is_some());
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
fn registered_org_and_markdown_operands_are_fail_closed() {
    for path in ["notes/spec.org", "README.md"] {
        let payload = format!(r#"{{"tool_name":"Bash","tool_input":{{"command":"unknown-reader {path}"}}}}"#);
        let decision = evaluate_pre_tool(GENERATION, &payload, "Bash")
            .expect("evaluate")
            .expect("deny");
        assert_eq!(decision.subject, Some(path));
        assert_eq!(decision.access, "unknown");
        assert_eq!(decision.elapsed_micros, 0);
        assert!(!decision.process_launched);
    }
}
