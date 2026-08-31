use agent_semantic_config::{
    CodexThreadOperation, CodexThreadReference, CodexThreadToolCall, SendMessageToThreadInput,
};

const THREAD_ID: &str = "01a055c3-6a84-7332-b76c-70b07801f029";
const REFERENCE_SCHEMA: &str =
    include_str!("../../../../schemas/codex-thread-reference.v1.schema.json");
const TOOL_SCHEMA: &str =
    include_str!("../../../../schemas/codex-thread-management-tool-call.v1.schema.json");
const ORG_CONTRACT: &str = include_str!("../../../../org/contracts/codex.thread-management.v1.org");

#[test]
fn deeplink_and_thread_id_validate_as_one_reference() {
    let reference = CodexThreadReference::new(THREAD_ID).expect("canonical thread reference");
    reference.validate().expect("reference validates");

    let schema: serde_json::Value =
        serde_json::from_str(REFERENCE_SCHEMA).expect("reference schema");
    let validator = jsonschema::validator_for(&schema).expect("compile reference schema");
    let value = serde_json::to_value(&reference).expect("serialize reference");
    assert!(validator.is_valid(&value));

    let mut mismatched = value;
    mismatched["deeplink"] =
        serde_json::json!("codex://threads/019fc51d-36ec-7890-b809-fcbd8f48fc28");
    let mismatched: CodexThreadReference =
        serde_json::from_value(mismatched).expect("wire shape remains decodable");
    assert!(mismatched.validate().is_err());
}

#[test]
fn standard_thread_calls_match_schema_and_typed_config() {
    let schema: serde_json::Value = serde_json::from_str(TOOL_SCHEMA).expect("tool schema");
    let validator = jsonschema::validator_for(&schema).expect("compile tool schema");
    let fixtures = [
        serde_json::json!({
            "schemaId": "agent.semantic-protocols.codex-thread-management-tool-call",
            "schemaVersion": "1",
            "namespace": "codex_app",
            "toolName": "navigate_to_codex_page",
            "toolInput": {"threadId": THREAD_ID}
        }),
        serde_json::json!({
            "schemaId": "agent.semantic-protocols.codex-thread-management-tool-call",
            "schemaVersion": "1",
            "namespace": "codex_app",
            "toolName": "read_thread",
            "toolInput": {"threadId": THREAD_ID, "turnLimit": 3, "includeOutputs": false}
        }),
        serde_json::json!({
            "schemaId": "agent.semantic-protocols.codex-thread-management-tool-call",
            "schemaVersion": "1",
            "namespace": "codex_app",
            "toolName": "send_message_to_thread",
            "toolInput": {"threadId": THREAD_ID, "prompt": "Continue the existing task."}
        }),
        serde_json::json!({
            "schemaId": "agent.semantic-protocols.codex-thread-management-tool-call",
            "schemaVersion": "1",
            "namespace": "codex_app",
            "toolName": "wait_threads",
            "toolInput": {"targets": [{"threadId": THREAD_ID}], "timeoutMs": 60000}
        }),
    ];

    for fixture in fixtures {
        assert!(validator.is_valid(&fixture), "fixture={fixture}");
        let call: CodexThreadToolCall =
            serde_json::from_value(fixture).expect("typed thread tool call");
        call.validate().expect("validated thread tool call");
    }
}

#[test]
fn thread_message_is_not_an_agent_dispatch_or_deeplink_side_effect() {
    let call = CodexThreadToolCall {
        schema_id: "agent.semantic-protocols.codex-thread-management-tool-call".to_owned(),
        schema_version: "1".to_owned(),
        namespace: "codex_app".to_owned(),
        operation: CodexThreadOperation::SendMessageToThread(SendMessageToThreadInput {
            thread_id: THREAD_ID.to_owned(),
            prompt: "Run the focused lifecycle validation.".to_owned(),
            host_id: None,
            model: None,
            thinking: None,
        }),
    };
    call.validate().expect("valid send call");

    assert!(ORG_CONTRACT.contains("codex://threads/{{{THREAD_ID}}}"));
    assert!(ORG_CONTRACT.contains("mcp__codex_app__send_message_to_thread({"));
    assert!(ORG_CONTRACT.contains("Only =send_message_to_thread= appends"));
    assert!(ORG_CONTRACT.contains("None is a\n=collaboration.*= Agent lifecycle call"));
    assert!(!ORG_CONTRACT.contains("AGENT_PATH_JSON"));
    assert!(!ORG_CONTRACT.contains("THREAD_ID_JSON"));
}
