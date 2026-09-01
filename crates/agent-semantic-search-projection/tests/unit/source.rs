use agent_semantic_search_projection::ResidentGraphEvaluationRequestV1;

#[test]
fn request_schema_rejects_partial_boundary_packets() {
    let error = ResidentGraphEvaluationRequestV1::from_value(serde_json::json!({
        "schemaId": "agent.semantic-protocols.semantic-graph-resident-evaluation-request",
        "schemaVersion": "1"
    }))
    .expect_err("partial request must fail shared schema validation");
    assert!(!error.to_string().is_empty(), "{error}");
}

fn complete_request() -> serde_json::Value {
    serde_json::json!({
        "schemaId": "agent.semantic-protocols.semantic-graph-resident-evaluation-request",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.search",
        "protocolVersion": "1",
        "packetKind": "resident-graph-evaluation-request",
        "languageId": "rust",
        "surface": "search-pipe",
        "queryTerms": ["owner"],
        "profile": "structural",
        "seedIds": ["owner:src/lib.rs"],
        "budget": {"maxDepth": 4, "maxNodes": 32, "maxEdges": 64, "maxResults": 16}
    })
}

#[test]
fn request_admission_rejects_unknown_fields_without_compiling_a_schema() {
    let mut value = complete_request();
    value["legacyFallback"] = serde_json::json!(true);
    let error = ResidentGraphEvaluationRequestV1::from_value(value)
        .expect_err("unknown fields must fail closed");
    assert!(error.to_string().contains("unknown"), "{error}");
}

#[test]
fn request_admission_rejects_an_oversized_budget() {
    let mut value = complete_request();
    value["budget"]["maxNodes"] = serde_json::json!(257);
    let error = ResidentGraphEvaluationRequestV1::from_value(value)
        .expect_err("oversized budget must fail closed");
    assert!(error.to_string().contains("budget"), "{error}");
}
