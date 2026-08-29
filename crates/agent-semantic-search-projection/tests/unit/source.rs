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
