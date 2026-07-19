#[test]
fn concept_only_search_pipe_query_is_gated_before_index_lookup() {
    let terms = agent_semantic_search::search_pipe_typed_query_terms(
        "rust",
        "search performance provider startup preflight",
    );
    let gate = agent_semantic_search::search_pipe_source_index_query_gate(&terms)
        .expect("concept-only query should produce a gate without an index lookup");
    assert_eq!(gate.term_count, 5);
}

#[test]
fn hyphenated_concept_clauses_are_gated_before_index_lookup() {
    let terms =
        agent_semantic_search::search_pipe_typed_query_terms("rust", "owner-items|selector-code");
    let gate = agent_semantic_search::search_pipe_source_index_query_gate(&terms)
        .expect("hyphenated concepts must not masquerade as structural anchors");

    assert_eq!(gate.term_count, 2);
    assert_eq!(gate.generic_term_count, 2);
}

#[test]
fn structurally_anchored_search_pipe_query_reaches_the_index_route() {
    let terms = agent_semantic_search::search_pipe_typed_query_terms(
        "rust",
        "search provider run_language_command",
    );
    let gate = agent_semantic_search::search_pipe_source_index_query_gate(&terms);

    assert!(
        gate.is_none(),
        "anchored query should not be preflight-gated"
    );
}
