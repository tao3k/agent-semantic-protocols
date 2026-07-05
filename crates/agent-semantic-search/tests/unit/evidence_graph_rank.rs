use agent_semantic_client_db::{
    ClientDbSourceIndexCandidate, ClientDbSourceIndexLookupResult, ClientDbSourceIndexLookupState,
    ClientDbSourceIndexSourceKind,
};
use agent_semantic_search::{
    EvidenceGraphRankNode, evidence_graph_rank_terms, rank_evidence_graph_nodes,
    render_owner_items_source_index_lookup_trace,
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

#[test]
fn owner_items_source_index_trace_reports_missing_db_before_parser_fallback() {
    let line = render_owner_items_source_index_lookup_trace(
        "crates/agent-semantic-hook/build.rs",
        &ClientDbSourceIndexLookupResult {
            db_path: "live/client/client.turso".into(),
            state: ClientDbSourceIndexLookupState::MissingDb,
            candidates: Vec::new(),
        },
    );

    assert!(line.contains("status=missing-db"), "{line}");
    assert!(line.contains("source=source-index"), "{line}");
    assert!(line.contains("reason=sourceIndex:missing-db"), "{line}");
    assert!(
        line.contains("next=asp_cache_source-index_refresh"),
        "{line}"
    );
}

#[test]
fn owner_items_source_index_trace_reports_hit_before_path_only_fallback() {
    let line = render_owner_items_source_index_lookup_trace(
        "crates/agent-semantic-hook/build.rs",
        &ClientDbSourceIndexLookupResult {
            db_path: "live/client/client.turso".into(),
            state: ClientDbSourceIndexLookupState::Hit,
            candidates: vec![ClientDbSourceIndexCandidate {
                path: "crates/agent-semantic-hook/build.rs".to_string(),
                language_id: None,
                provider_id: None,
                source_kind: ClientDbSourceIndexSourceKind::Other("turso-source-index".to_string()),
                line_count: Some(8),
                query_keys: vec!["build".to_string(), "build.rs".to_string()],
            }],
        },
    );

    assert!(line.contains("status=hit"), "{line}");
    assert!(line.contains("source=source-index"), "{line}");
    assert!(
        line.contains("path=crates/agent-semantic-hook/build.rs"),
        "{line}"
    );
    assert!(!line.contains("owner-not-found"), "{line}");
    assert!(!line.contains("path-only"), "{line}");
}
