use std::path::PathBuf;

use agent_semantic_search::{
    search_query_budget_block, search_query_terms, search_rg_terms_budget_block,
    search_terms_budget_block, specific_search_term,
};

#[test]
fn search_query_budget_blocks_generic_broad_queries() {
    let block = search_query_budget_block(
        "search query budget block generic provider",
        &[PathBuf::from(".")],
        false,
    )
    .expect("broad generic query should be blocked");

    assert_eq!(block.reason, "query-too-broad");
    assert_eq!(block.term_count, 6);
    assert_eq!(
        block.generic_terms,
        vec![
            "search".to_string(),
            "query".to_string(),
            "budget".to_string(),
            "block".to_string(),
            "generic".to_string(),
            "provider".to_string(),
        ]
    );
}

#[test]
fn search_query_budget_allows_specific_or_filtered_queries() {
    let terms = search_query_terms("CacheStatus cache_status src/lib.rs");

    assert!(specific_search_term("cache_status"));
    assert!(specific_search_term("src/lib.rs"));
    assert!(search_terms_budget_block(&terms, &[PathBuf::from(".")], false).is_none());
    assert!(
        search_query_budget_block(
            "search query budget block generic provider",
            &[PathBuf::from(".")],
            true,
        )
        .is_none()
    );
}

#[test]
fn search_query_budget_requires_a_specific_anchor_for_concept_bundles() {
    let terms = search_query_terms("search performance provider startup preflight");
    let block = search_terms_budget_block(&terms, &[PathBuf::from(".")], false)
        .expect("concept-only bundle should be blocked before acquisition");

    assert_eq!(block.reason, "query-needs-specific-anchor");
    assert_eq!(block.term_count, 5);

    let anchored = search_query_terms("search provider run_language_command");
    assert!(search_terms_budget_block(&anchored, &[PathBuf::from(".")], false).is_none());
}

#[test]
fn search_query_budget_blocks_short_all_generic_queries() {
    let terms = search_query_terms("search provider");
    let block = search_terms_budget_block(&terms, &[PathBuf::from(".")], false)
        .expect("all-generic query should be blocked");

    assert_eq!(block.reason, "query-too-broad");
    assert_eq!(block.term_count, 2);
}

#[test]
fn rg_budget_blocks_many_terms_on_broad_scope() {
    let terms = vec![
        "alpha".to_string(),
        "beta".to_string(),
        "gamma".to_string(),
        "delta".to_string(),
        "epsilon".to_string(),
    ];
    let block = search_rg_terms_budget_block(&terms, &[], false)
        .expect("rg query with many broad terms should be blocked");

    assert_eq!(block.reason, "query-too-broad");
    assert_eq!(block.generic_terms, terms);
    assert_eq!(block.term_count, 5);
}
