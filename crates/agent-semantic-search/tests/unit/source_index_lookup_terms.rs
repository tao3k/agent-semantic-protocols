use agent_semantic_search::{
    SourceIndexRankCandidate, SourceIndexRankRequest, rank_source_index_report,
    source_index_lookup_terms,
};

#[test]
fn source_index_lookup_terms_include_path_segments_suffixes_and_stem() {
    let terms = source_index_lookup_terms("crates/agent-semantic-search/src/search_planner.rs");

    for expected in [
        "crates/agent-semantic-search/src/search_planner.rs",
        "agent-semantic-search/src/search_planner.rs",
        "src/search_planner.rs",
        "search_planner.rs",
        "search_planner",
        "rs",
        "src",
    ] {
        assert!(
            terms.iter().any(|term| term == expected),
            "expected {expected:?} in terms {terms:?}"
        );
    }
}

#[test]
fn source_index_lookup_terms_normalize_backslash_paths() {
    let terms = source_index_lookup_terms("src\\search_planner.rs");

    assert!(terms.iter().any(|term| term == "src/search_planner.rs"));
    assert!(terms.iter().any(|term| term == "search_planner.rs"));
    assert!(terms.iter().any(|term| term == "search_planner"));
}

#[test]
fn rank_report_attaches_rust_computed_score_breakdown() {
    let report = rank_source_index_report(SourceIndexRankRequest {
        query: "search_planner.rs".to_string(),
        candidates: vec![
            SourceIndexRankCandidate {
                ordinal: 0,
                path: "src/lib.rs".to_string(),
                query_keys: vec!["lib".to_string()],
            },
            SourceIndexRankCandidate {
                ordinal: 1,
                path: "crates/agent-semantic-search/src/search_planner.rs".to_string(),
                query_keys: vec!["search_planner".to_string()],
            },
        ],
    });

    let first = &report.ranked_candidates[0];
    assert_eq!(
        first.candidate.path,
        "crates/agent-semantic-search/src/search_planner.rs"
    );
    assert_eq!(first.score.basename, 1);
    assert!(first.score.total > report.ranked_candidates[1].score.total);
}
