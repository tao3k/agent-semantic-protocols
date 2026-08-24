use agent_semantic_search::{
    EvidenceGraphRankNode, evidence_graph_rank_terms, rank_evidence_graph_nodes,
};

#[test]
fn evidence_graph_rank_prefers_selector_and_query_key_hits() {
    let ranked = rank_evidence_graph_nodes(
        vec![
            EvidenceGraphRankNode {
                ordinal: 0,
                id: "structural-owner:generation:src/lib.rs".to_string(),
                kind: "structural-owner".to_string(),
                label: "src/lib.rs".to_string(),
                path: Some("src/lib.rs".to_string()),
                selector: None,
                query_keys: vec!["lib".to_string()],
                outgoing_edge_count: 4,
            },
            EvidenceGraphRankNode {
                ordinal: 1,
                id: "selector:rust://src/lib.rs#item/struct/EvidenceFixture".to_string(),
                kind: "selector".to_string(),
                label: "EvidenceFixture".to_string(),
                path: Some("src/lib.rs".to_string()),
                selector: Some("rust://src/lib.rs#item/struct/EvidenceFixture".to_string()),
                query_keys: vec!["EvidenceFixture".to_string(), "serde".to_string()],
                outgoing_edge_count: 0,
            },
        ],
        "serde EvidenceFixture",
    );

    assert_eq!(
        ranked[0].node.selector.as_deref(),
        Some("rust://src/lib.rs#item/struct/EvidenceFixture")
    );
    assert_eq!(ranked[0].score.term_hits, 2);
    assert_eq!(ranked[0].score.selector_bonus, 1);
}

#[test]
fn evidence_graph_rank_uses_topology_as_tiebreaker_without_reordering_hits() {
    let ranked = rank_evidence_graph_nodes(
        vec![
            EvidenceGraphRankNode {
                ordinal: 0,
                id: "node:a".to_string(),
                kind: "symbol".to_string(),
                label: "alpha".to_string(),
                path: None,
                selector: None,
                query_keys: vec!["alpha".to_string()],
                outgoing_edge_count: 1,
            },
            EvidenceGraphRankNode {
                ordinal: 1,
                id: "node:b".to_string(),
                kind: "symbol".to_string(),
                label: "alpha".to_string(),
                path: None,
                selector: None,
                query_keys: vec!["alpha".to_string()],
                outgoing_edge_count: 7,
            },
        ],
        "alpha",
    );

    assert_eq!(ranked[0].node.id, "node:b");
    assert_eq!(ranked[0].score.topology_bonus, 7);
    assert_eq!(ranked[1].node.id, "node:a");
}

#[test]
fn evidence_graph_rank_terms_are_language_neutral_identifier_axes() {
    assert_eq!(
        evidence_graph_rank_terms("serde::Serialize EvidenceFixture src/lib.rs"),
        vec![
            "evidencefixture".to_string(),
            "rs".to_string(),
            "serde::serialize".to_string(),
            "src/lib".to_string()
        ]
    );
}
