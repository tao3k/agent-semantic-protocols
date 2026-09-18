// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use agent_semantic_search_projection::adapt_graph_evaluate_payload;
use agent_semantic_search_projection::bind_graph_generation_identity;
use agent_semantic_search_projection::validate_graph_generation_receipt_identity;
use agent_semantic_search_projection::validate_graph_source_root;

#[test]
fn graph_request_binds_source_root_not_runtime_generation_digest() {
    let source_root = "a".repeat(64);
    let runtime_generation = format!("blake3-256:{}", "b".repeat(64));
    let request = serde_json::json!({
        "workspaceGeneration": {
            "rootDigest": source_root,
        }
    });

    validate_graph_source_root(&request, &"a".repeat(64))
        .expect("the admitted source root is the graph content authority");
    let error = validate_graph_source_root(&request, &runtime_generation)
        .expect_err("the Runtime generation digest is not a source Merkle root alias");
    assert!(error.contains("graph-source-root-mismatch"), "{error}");
}

#[test]
fn graph_adapter_preserves_the_admitted_query_budget_inside_rank_controls() {
    let request = serde_json::json!({
        "queryTerms": ["compare"],
        "profile": "owner-query",
        "budget": 3,
        "entryNodeIds": ["owner:src/lib.rs"],
        "cache": {"enabled": true},
    });

    let adapted = adapt_graph_evaluate_payload(&request).expect("adapt graph evaluation");
    assert_eq!(adapted["budget"], 3);
    assert_eq!(adapted["rankPayload"]["budget"], 3);
    assert_eq!(
        adapted["rankPayload"]["entryNodeIds"][0],
        "owner:src/lib.rs"
    );
    assert!(adapted.get("graph").is_none());
}

#[test]
fn generation_identity_is_bound_from_the_exact_graph_payload() {
    let mut request = serde_json::json!({
        "payload": {
            "identity": {
                "workspaceId": "workspace-a",
                "generationCandidateDigest": "blake3-256:abc"
            }
        }
    });

    bind_graph_generation_identity(&mut request).unwrap();

    assert_eq!(request["workspaceIdentity"], "workspace-a");
    assert_eq!(request["generationDigest"], "blake3-256:abc");
}

#[test]
fn generation_receipt_rejects_cross_generation_replay() {
    let receipt = serde_json::json!({
        "requestId": "request-a",
        "payload": { "state": "completed" },
        "workspaceIdentity": "workspace-a",
        "generationDigest": "blake3-256:old"
    });

    assert!(
        validate_graph_generation_receipt_identity(
            &receipt,
            "request-a",
            "completed",
            "workspace-a",
            "blake3-256:new",
        )
        .is_err()
    );
}
