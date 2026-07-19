use std::path::PathBuf;

use agent_semantic_search::{
    search_query_budget_block, search_query_terms, search_terms_budget_block, specific_search_term,
};

#[test]
fn search_query_budget_blocks_generic_broad_queries() {
    let block = search_query_budget_block(agent_semantic_search::SearchQueryBudgetRequest {
        language_id: "rust",
        query: "search query budget block generic provider",
        scopes: &[PathBuf::from(".")],
        explicit_filters: false,
    })
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
        search_query_budget_block(agent_semantic_search::SearchQueryBudgetRequest {
            language_id: "rust",
            query: "search query budget block generic provider",
            scopes: &[PathBuf::from(".")],
            explicit_filters: true,
        })
        .is_none()
    );
}

#[test]
fn search_query_budget_uses_typed_evidence_instead_of_character_anchors() {
    let terms = search_query_terms("search performance provider startup preflight");
    assert!(search_terms_budget_block(&terms, &[PathBuf::from(".")], false).is_none());
    assert!(
        search_query_budget_block(agent_semantic_search::SearchQueryBudgetRequest {
            language_id: "rust",
            query: "compiler trace module resolution|project references",
            scopes: &[PathBuf::from(".")],
            explicit_filters: false,
        })
        .is_none()
    );
    assert!(
        search_query_budget_block(agent_semantic_search::SearchQueryBudgetRequest {
            language_id: "rust",
            query: "Tokio runtime Handle::enter context guard lifecycle owner frontier",
            scopes: &[PathBuf::from(".")],
            explicit_filters: false,
        })
        .is_none()
    );
}

#[test]
fn search_query_budget_blocks_short_all_generic_queries() {
    let terms = search_query_terms("search provider");
    let block = search_terms_budget_block(&terms, &[PathBuf::from(".")], false)
        .expect("all-generic query should be blocked");

    assert_eq!(block.reason, "query-too-broad");
    assert_eq!(block.term_count, 2);
}
