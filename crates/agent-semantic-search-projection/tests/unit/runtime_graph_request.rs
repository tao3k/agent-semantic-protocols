use agent_semantic_search_projection::{adapt_graph_evaluate_payload, validate_graph_source_root};

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
        "seedIds": ["owner:src/lib.rs"],
        "cache": {"enabled": true},
    });

    let adapted = adapt_graph_evaluate_payload(&request).expect("adapt graph evaluation");
    assert_eq!(adapted["budget"], 3);
    assert_eq!(adapted["rankPayload"]["budget"], 3);
    assert_eq!(adapted["rankPayload"]["seedIds"][0], "owner:src/lib.rs");
    assert!(adapted.get("graph").is_none());
}
