use super::resident_graph_turbo_message;

#[test]
fn search_packet_remains_an_identity_free_rank_intent_until_runtime_admission() {
    let packet = serde_json::json!({
        "schemaId": "agent.semantic-protocols.semantic-graph-turbo-request",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.semantic-language",
        "protocolVersion": "1",
        "packetKind": "graph-turbo-request",
        "surface": "search-pipe",
        "sourceSnapshot": {
            "schemaId": "asp.source-snapshot.v1",
            "algorithm": "blake3-merkle-v1",
            "rootDigest": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "sourceKind": "derived-overlay",
            "leafCount": 1,
            "providerDigest": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
        },
        "workspaceGeneration": {
            "rootDigest": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "rootDepth": 0,
            "leafCount": 1,
            "ownerCount": 1
        },
        "queryTerms": ["parser"],
        "profile": "owner-query",
        "algorithm": "typed-ppr-diverse",
        "seedIds": [],
        "budget": 1,
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
