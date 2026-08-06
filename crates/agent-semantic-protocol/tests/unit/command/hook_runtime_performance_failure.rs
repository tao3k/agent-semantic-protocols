use super::wall_failure_value;

#[test]
fn extracts_one_typed_wall_failure_from_post_tool_output() {
    let failure = serde_json::json!({
        "schemaId": "agent.semantic-protocols.agent-facing-search-wall-failure",
        "schemaVersion": "1",
        "surface": "search",
        "stage": "graph-turbo-generation-open",
        "state": "unavailable",
        "reasonKind": "agent-facing-search-wall-budget-exceeded",
        "budgetMicros": 800000,
        "elapsedMicros": 802357,
        "retryAfterMs": 250
    });
    let payload = serde_json::json!({
        "tool_response": format!("diagnostic\n{failure}\n{failure}\n")
    });
    assert_eq!(
        wall_failure_value(&payload)
            .expect("typed output should parse")
            .expect("one wall failure should be present"),
        failure
    );
}

#[test]
fn ignores_untyped_post_tool_text() {
    let payload = serde_json::json!({"tool_response": "budget exceeded"});
    assert!(
        wall_failure_value(&payload)
            .expect("untyped output should not fail")
            .is_none()
    );
}
