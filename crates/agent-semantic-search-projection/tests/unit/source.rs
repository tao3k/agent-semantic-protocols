use agent_semantic_search_projection::GraphTurboEvaluationRequest;

#[test]
fn request_schema_rejects_partial_boundary_packets() {
    let error = GraphTurboEvaluationRequest::from_value(serde_json::json!({
        "schemaId": "agent.semantic-protocols.semantic-graph-turbo-request",
        "schemaVersion": "1"
    }))
    .expect_err("partial request must fail shared schema validation");
    assert!(!error.to_string().is_empty(), "{error}");
}

fn complete_request() -> serde_json::Value {
    serde_json::json!({
        "schemaId": "agent.semantic-protocols.semantic-graph-turbo-request",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.semantic-language",
        "protocolVersion": "1",
        "packetKind": "graph-turbo-request",
        "surface": "search-pipe",
        "sourceSnapshot": {},
        "workspaceGeneration": {},
        "queryTerms": ["owner"],
        "profile": "owner-query",
        "algorithm": "typed-ppr-diverse",
        "seedIds": ["owner:src/lib.rs"],
        "budget": 32
    })
}

#[test]
fn request_admission_rejects_unknown_fields_without_compiling_a_schema() {
    let mut value = complete_request();
    value["legacyFallback"] = serde_json::json!(true);
    let error = GraphTurboEvaluationRequest::from_value(value)
        .expect_err("unknown fields must fail closed");
    assert!(error.to_string().contains("unknown"), "{error}");
}

#[test]
fn request_admission_rejects_non_positive_budget() {
    let mut value = complete_request();
    value["budget"] = serde_json::json!(0);
    let error =
        GraphTurboEvaluationRequest::from_value(value).expect_err("zero budget must fail closed");
    assert!(error.to_string().contains("budget"), "{error}");
}
