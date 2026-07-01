use crate::{
    SearchPipeQueryPackCandidate, SearchPipeTermRole, search_pipe_clause_coverages,
    search_pipe_is_path_like_token, search_pipe_next_query_pack_hint,
    search_pipe_query_clause_texts, search_pipe_query_clauses, search_pipe_role_terms,
    search_pipe_unique_query_terms,
};

#[test]
fn search_pipe_query_pack_splits_broad_queries_into_stable_clauses() {
    let clauses = search_pipe_query_clauses(
        "rust",
        "src/runtime.rs packages/runtime-search SearchRouter CacheStatus concurrency through owner",
    );
    let clause_texts = search_pipe_query_clause_texts(
        "rust",
        "src/runtime.rs packages/runtime-search SearchRouter CacheStatus concurrency through owner",
    );

    assert_eq!(
        clause_texts,
        vec![
            "src/runtime.rs packages/runtime-search",
            "SearchRouter CacheStatus",
            "concurrency"
        ]
    );
    assert_eq!(clauses.len(), 3);
}

#[test]
fn search_pipe_query_pack_keeps_explicit_clauses_and_roles() {
    let clauses = search_pipe_query_clauses("typescript", "Effect Stream|Queue backpressure");
    let terms = search_pipe_unique_query_terms(&clauses);

    assert_eq!(clauses.len(), 2);
    assert_eq!(
        search_pipe_role_terms(&terms, SearchPipeTermRole::Context),
        vec!["Effect".to_string()]
    );
    assert_eq!(
        search_pipe_role_terms(&terms, SearchPipeTermRole::Symbol),
        vec!["Stream".to_string(), "Queue".to_string()]
    );
    assert_eq!(
        search_pipe_next_query_pack_hint(
            &["Effect".to_string()],
            &["Queue".to_string(), "Stream".to_string()],
            &["backpressure".to_string()]
        ),
        Some("Queue Stream|backpressure|Queue Stream backpressure".to_string())
    );
}

#[test]
fn search_pipe_clause_coverage_matches_candidate_evidence() {
    let clauses = search_pipe_query_clauses("rust", "SearchRouter CacheStatus");
    let candidates = vec![SearchPipeQueryPackCandidate {
        path: "src/router.rs".to_string(),
        symbol: "SearchRouter".to_string(),
        text: "pub struct SearchRouter".to_string(),
    }];
    let coverages = search_pipe_clause_coverages(&clauses, &candidates);

    assert_eq!(coverages[0].matched, vec!["searchrouter".to_string()]);
    assert_eq!(coverages[0].missing, vec!["cachestatus".to_string()]);
    assert!(search_pipe_is_path_like_token("src/router.rs"));
    assert!(!search_pipe_is_path_like_token("SearchRouter"));
}
