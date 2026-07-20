use agent_semantic_search::{
    GraphOwnerRankCandidate, GraphOwnerRankRequest, rank_graph_owner_report,
};

#[test]
fn graph_owner_rank_report_is_public_and_constructible() {
    let fixture = crate::source_snapshot_fixture::canonical_test_snapshot();
    let report = rank_graph_owner_report(GraphOwnerRankRequest {
        candidates: vec![
            GraphOwnerRankCandidate::new(
                "src/lib.rs",
                "SearchRouter",
                "dynamic overlay graph ranking",
                "source-index",
                "high",
            ),
            GraphOwnerRankCandidate::new(
                "languages/rust/src/lib.rs",
                "SearchRouter",
                "dynamic overlay graph ranking",
                "source-index",
                "high",
            ),
        ],
        query_terms: vec!["dynamicOverlay".to_string()],
        submodule_paths: vec!["languages/rust".to_string()],
        source_snapshot: fixture.evidence.clone(),
    });

    let top = report
        .ranked_owners
        .first()
        .expect("public graph owner rank report should include owners");
    assert_eq!(top.path, "languages/rust/src/lib.rs");
    assert_eq!(
        top.topology_submodule_path.as_deref(),
        Some("languages/rust")
    );
    assert_eq!(top.score.query_axis_count, 2);
    assert!(top.score.total > 0);
}
