use crate::graph_candidate_projection::GraphProjectionCandidate;
use crate::graph_query_owner_seed::{
    graph_has_package_path_candidate, graph_query_owner_seed_paths,
};

#[test]
fn graph_query_owner_seed_prefers_explicit_package_path_candidates() {
    let candidates = vec![
        GraphProjectionCandidate::new(
            "packages/runtime/search/src/router.rs",
            1,
            1,
            "SearchRouter",
            "dynamic overlay",
            "package-path-query",
            "package-path",
        ),
        GraphProjectionCandidate::new(
            "src/lib.rs",
            1,
            1,
            "SearchRouter",
            "dynamic overlay",
            "source-index",
            "high",
        ),
    ];
    let owners = vec![
        "src/lib.rs".to_string(),
        "packages/runtime/search/src/router.rs".to_string(),
    ];
    let query_terms = vec!["runtime_search".to_string()];

    assert!(graph_has_package_path_candidate(&candidates, &query_terms));
    assert_eq!(
        graph_query_owner_seed_paths(&candidates, &owners, 1, &query_terms),
        vec!["packages/runtime/search/src/router.rs".to_string()]
    );
}

#[test]
fn graph_query_owner_seed_ranks_owner_evidence_by_identifier_axes() {
    let candidates = vec![
        GraphProjectionCandidate::new(
            "src/cache.rs",
            1,
            1,
            "CacheStatus",
            "cache status receipt",
            "source-index",
            "high",
        ),
        GraphProjectionCandidate::new(
            "src/runtime.rs",
            1,
            1,
            "RuntimeStatus",
            "runtime receipt",
            "source-index",
            "high",
        ),
    ];
    let owners = vec!["src/runtime.rs".to_string(), "src/cache.rs".to_string()];
    let query_terms = vec!["CacheStatus".to_string()];

    assert_eq!(
        graph_query_owner_seed_paths(&candidates, &owners, 1, &query_terms),
        vec!["src/cache.rs".to_string()]
    );
}
