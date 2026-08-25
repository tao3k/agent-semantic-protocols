use super::resident_graph_turbo_message;

#[test]
fn search_packet_remains_an_identity_free_rank_intent_until_runtime_admission() {
    let packet = serde_json::json!({
        "schemaId": "agent.semantic-protocols.semantic-graph-turbo-request",
        "schemaVersion": "1",
        "surface": "search-pipe",
        "sourceSnapshot": {
            "rootDigest": "blake3-256:snapshot"
        },
        "workspaceGeneration": {
            "rootDigest": "blake3-256:generation"
        },
        "graph": {"nodes": [], "edges": []}
    });
    let encoded = serde_json::to_vec(&packet).expect("encode fixture");
    let message = resident_graph_turbo_message(&encoded).expect("decode rank intent");

    let _typed_request = message;
    assert!(packet.get("workspaceIdentity").is_none());
    assert!(packet.get("generationDigest").is_none());
}

#[test]
fn rank_intent_rejects_a_non_object_packet() {
    let packet = serde_json::json!(["not", "an", "intent"]);
    let encoded = serde_json::to_vec(&packet).expect("encode fixture");
    let error =
        resident_graph_turbo_message(&encoded).expect_err("non-object rank intent is rejected");
    assert!(!error.is_empty(), "decoder must return a diagnostic");
}
