use agent_semantic_search::{
    SearchPipeLanguageId, SearchPipeQueryPackDescriptor, SearchPipeQueryText, SearchPipeTermRole,
    SearchQueryBudgetRequest, search_pipe_source_index_query_gate, search_pipe_typed_query_terms,
    search_query_budget_block,
};

fn descriptor() -> SearchPipeQueryPackDescriptor<'static> {
    SearchPipeQueryPackDescriptor {
        descriptor_id: "test-query-pack.v1",
        descriptor_version: "1",
        language_id: "rust",
        term_role_overrides: &[],
        recipes: &[],
    }
}

#[test]
fn explicit_literal_and_diagnostic_prefixes_are_typed_anchors() {
    let terms = search_pipe_typed_query_terms(
        SearchPipeLanguageId::new("rust"),
        SearchPipeQueryText::new(
            "literal:item-not-in-live-owner diagnostic:snapshot-item-kind-mismatch",
        ),
        descriptor(),
    );

    assert_eq!(terms.len(), 2);
    assert_eq!(terms[0].raw, "item-not-in-live-owner");
    assert_eq!(terms[0].role, SearchPipeTermRole::Literal);
    assert_eq!(terms[1].raw, "snapshot-item-kind-mismatch");
    assert_eq!(terms[1].role, SearchPipeTermRole::DiagnosticCode);
}

#[test]
fn one_explicit_literal_bypasses_the_broad_query_budget() {
    let block = search_query_budget_block(SearchQueryBudgetRequest {
        language_id: "rust",
        query: "literal:item-not-in-live-owner",
        scopes: &[],
        explicit_filters: false,
        query_pack_descriptor: descriptor(),
    });

    assert!(block.is_none());
}

#[test]
fn typed_literal_set_reaches_source_index_acquisition() {
    let terms = search_pipe_typed_query_terms(
        SearchPipeLanguageId::new("rust"),
        SearchPipeQueryText::new(
            "literal:item-not-in-live-owner literal:snapshot-item-kind-mismatch",
        ),
        descriptor(),
    );

    assert_eq!(terms.len(), 2);
    assert!(
        search_pipe_source_index_query_gate(&terms).is_none(),
        "typed literal anchors must not be gated before source-index lookup"
    );
}
