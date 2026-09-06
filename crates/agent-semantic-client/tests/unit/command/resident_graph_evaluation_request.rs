// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

use super::decode_resident_graph_evaluation_request;

fn intent() -> serde_json::Value {
    serde_json::json!({
        "schemaId": "agent.semantic-protocols.semantic-graph-resident-evaluation-request",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.search",
        "protocolVersion": "1",
        "packetKind": "resident-graph-evaluation-request",
        "languageId": "rust",
        "surface": "search-pipe",
        "queryTerms": ["parser"],
        "profile": "structural",
        "entryNodeIds": [],
        "budget": {"maxDepth": 4, "maxNodes": 64, "maxEdges": 128, "maxResults": 32}
    })
}

#[test]
fn resident_graph_request_carries_only_intent_until_runtime_admission() {
    let packet = intent();
    let encoded = serde_json::to_vec(&packet).expect("encode fixture");
    decode_resident_graph_evaluation_request(&encoded).expect("decode resident intent");

    for forbidden in [
        "graph",
        "sourceSnapshot",
        "workspaceGeneration",
        "workspaceIdentity",
        "generationDigest",
        "providerId",
        "algorithm",
    ] {
        assert!(
            packet.get(forbidden).is_none(),
            "forbidden field {forbidden}"
        );
    }
}

#[test]
fn resident_graph_request_rejects_client_supplied_generation_state() {
    let mut packet = intent();
    packet["graph"] = serde_json::json!({"nodes": [], "edges": []});
    let encoded = serde_json::to_vec(&packet).expect("encode fixture");
    let error = decode_resident_graph_evaluation_request(&encoded)
        .expect_err("client graph state must be rejected");
    assert!(error.contains("unknown resident graph evaluation request field"));
}
