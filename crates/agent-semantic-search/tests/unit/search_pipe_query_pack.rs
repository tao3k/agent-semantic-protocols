use crate::{
    SearchPipeQueryPackCandidate, SearchPipeTermRole, search_pipe_clause_coverages,
    search_pipe_is_path_like_token, search_pipe_next_query_pack_hint,
    search_pipe_query_clause_texts, search_pipe_query_clauses, search_pipe_role_terms,
    search_pipe_unique_query_terms,
};

#[test]
fn search_pipe_query_pack_splits_broad_queries_into_stable_clauses() {
    let query =
        "src/runtime.rs packages/runtime-search SearchRouter CacheStatus concurrency through owner";
    let clauses = search_pipe_query_clauses(
        crate::search_pipe_query_pack::SearchPipeQueryClausesRequest::new(
            crate::search_pipe_query_pack::SearchPipeLanguageId::new("rust"),
            crate::search_pipe_query_pack::SearchPipeQueryText::new(query),
        ),
    );
    let clause_texts = search_pipe_query_clause_texts(
        crate::search_pipe_query_pack::SearchPipeQueryClausesRequest::new(
            crate::search_pipe_query_pack::SearchPipeLanguageId::new("rust"),
            crate::search_pipe_query_pack::SearchPipeQueryText::new(query),
        ),
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
fn semantic_facts_intent_requires_compound_typed_evidence() {
    let data_shape_terms = vec!["field".to_owned(), "fields".to_owned(), "scalar".to_owned()];
    let collection_terms = vec![
        "collection".to_owned(),
        "collections".to_owned(),
        "list".to_owned(),
    ];
    let intent_axes = [
        crate::SearchPipeSemanticFactsIntentAxis {
            axis: "data-shape",
            terms: &data_shape_terms,
            roles: &[],
        },
        crate::SearchPipeSemanticFactsIntentAxis {
            axis: "collection",
            terms: &collection_terms,
            roles: &[],
        },
    ];
    let descriptor = crate::SearchPipeSemanticFactsDescriptor {
        descriptor_id: "test.semantic-facts",
        descriptor_version: "1",
        intent_axes: &intent_axes,
    };

    let explicit_axes =
        crate::search_pipe_semantic_facts_intent("python", "list|collection fields", descriptor);
    assert!(explicit_axes.requested);
    assert_eq!(
        explicit_axes.matched_axes,
        vec!["collection".to_owned(), "data-shape".to_owned()]
    );

    let symbol_anchor =
        crate::search_pipe_semantic_facts_intent("rust", "Snapshot fields", descriptor);
    assert!(symbol_anchor.requested);
    assert_eq!(symbol_anchor.matched_terms, vec!["fields".to_owned()]);

    assert!(
        !crate::search_pipe_semantic_facts_intent(
            "rust",
            "low cohesion rg query set command scope package",
            descriptor,
        )
        .requested
    );
    assert!(
        !crate::search_pipe_semantic_facts_intent("python", "list workflow cache", descriptor,)
            .requested
    );

    assert!(
        !crate::search_pipe_semantic_facts_intent("rust", "vec collection", descriptor,).requested
    );
    let rust_collection_terms = vec!["vec".to_owned(), "collection".to_owned()];
    let rust_axes = [crate::SearchPipeSemanticFactsIntentAxis {
        axis: "collection",
        terms: &rust_collection_terms,
        roles: &[],
    }];
    assert!(
        crate::search_pipe_semantic_facts_intent(
            "rust",
            "vec collection",
            crate::SearchPipeSemanticFactsDescriptor {
                descriptor_id: "rust.semantic-facts",
                descriptor_version: "1",
                intent_axes: &rust_axes,
            },
        )
        .requested
    );
}

#[test]
fn search_pipe_query_pack_keeps_explicit_clauses_and_roles() {
    let role_overrides = [
        crate::search_pipe_query_pack::SearchPipeQueryPackTermRoleOverride {
            term: "Effect",
            role: "context",
            case_sensitive: true,
        },
    ];
    let trigger_terms = vec!["Queue".to_string(), "Stream".to_string()];
    let recipe_terms = vec![
        "Queue".to_string(),
        "Stream".to_string(),
        "backpressure".to_string(),
    ];
    let recipe_clauses = [crate::search_pipe_query_pack::SearchPipeQueryPackClause {
        terms: &recipe_terms,
        roles: &[],
        intent_axes: &[],
    }];
    let recipes = [crate::search_pipe_query_pack::SearchPipeQueryPackRecipe {
        recipe_id: "typescript.queue-stream-backpressure",
        trigger_terms: &trigger_terms,
        trigger_match: "all",
        clauses: &recipe_clauses,
    }];
    let descriptor = crate::search_pipe_query_pack::SearchPipeQueryPackDescriptor {
        descriptor_id: "typescript.query-pack",
        descriptor_version: "1",
        language_id: "typescript",
        term_role_overrides: &role_overrides,
        recipes: &recipes,
    };
    let request = crate::search_pipe_query_pack::SearchPipeQueryClausesRequest::new(
        crate::search_pipe_query_pack::SearchPipeLanguageId::new("typescript"),
        crate::search_pipe_query_pack::SearchPipeQueryText::new("Effect Stream|Queue backpressure"),
    );
    let generic_terms = search_pipe_unique_query_terms(&search_pipe_query_clauses(request));
    assert!(
        search_pipe_role_terms(&generic_terms, SearchPipeTermRole::Context).is_empty(),
        "provider-specific roles must not leak into the generic query composer"
    );
    let clauses = search_pipe_query_clauses(request.with_query_pack_descriptor(descriptor));
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
            descriptor,
            &["Effect".to_string()],
            &["Queue".to_string(), "Stream".to_string()],
            &["backpressure".to_string()]
        ),
        Some("Queue Stream|Queue Stream backpressure".to_string())
    );
}

#[test]
fn search_pipe_clause_coverage_matches_candidate_evidence() {
    let clauses = search_pipe_query_clauses(
        crate::search_pipe_query_pack::SearchPipeQueryClausesRequest::new(
            crate::search_pipe_query_pack::SearchPipeLanguageId::new("rust"),
            crate::search_pipe_query_pack::SearchPipeQueryText::new("SearchRouter CacheStatus"),
        ),
    );
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
