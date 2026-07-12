use std::path::Path;

use agent_semantic_search::{
    SearchPipeSourceIndexAcquisitionRequest, SearchPipeSourceIndexDecision,
    collect_search_pipe_source_index_acquisition,
};

#[test]
fn concept_only_search_pipe_query_is_gated_before_index_lookup() {
    let acquisition =
        collect_search_pipe_source_index_acquisition(SearchPipeSourceIndexAcquisitionRequest {
            intent: "search performance provider startup preflight",
            project_root: Path::new("."),
            scopes: &[],
            lookup: None,
        })
        .expect("concept-only query should produce a gate without an index lookup");

    assert_eq!(
        acquisition.decision,
        SearchPipeSourceIndexDecision::QueryGate
    );
    assert!(acquisition.candidates.is_empty());
    let gate = acquisition.gate.expect("query gate receipt");
    assert_eq!(gate.term_count, 5);
}

#[test]
fn structurally_anchored_search_pipe_query_reaches_the_index_route() {
    let acquisition =
        collect_search_pipe_source_index_acquisition(SearchPipeSourceIndexAcquisitionRequest {
            intent: "search provider run_language_command",
            project_root: Path::new("."),
            scopes: &[],
            lookup: None,
        });

    assert!(
        acquisition.is_none(),
        "anchored query should not be preflight-gated"
    );
}
