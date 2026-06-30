#![deny(dead_code)]

//! Search orchestration services for ASP agent-facing queries.

mod document_candidates;
mod dynamic_candidates;
mod dynamic_overlay;
mod dynamic_search;
mod lexical_overlay;
mod native_finder;
mod pipe_candidates;
mod pipe_source;
mod prompt_output_replay;
mod query_packet_replay;
mod query_wrapper_candidates;
mod query_wrapper_scan;
mod search_language_files;
mod search_lexical_replay;
mod search_packet_replay;
mod source_index_rank;
mod syntax_query_replay;

pub use document_candidates::{
    DocumentSearchCandidate, DocumentSearchCandidateCollection, DocumentSearchCandidateRequest,
    collect_document_search_candidates,
};
pub use dynamic_candidates::{
    DynamicSearchCandidate, DynamicSearchCandidateRequest, DynamicSearchRootCandidateRequest,
    IngestSearchCandidate, collect_dynamic_lexical_overlay_candidates,
    collect_dynamic_lexical_overlay_candidates_from_roots, collect_ingest_search_candidates,
};
pub use dynamic_search::{
    DynamicOwnerItem, DynamicOwnerItemsRequest, DynamicOwnerPath, DynamicOwnerQuery,
    DynamicSearchLanguage, DynamicSearchRoots, render_dynamic_owner_items_code,
    render_dynamic_owner_items_frontier,
};
pub use lexical_overlay::{
    LexicalOverlayCandidateHit, LexicalOverlayDocument, LexicalOverlaySearchHit,
    LexicalOverlaySearchRequest, search_lexical_overlay, search_lexical_overlay_candidates,
};
pub use native_finder::{
    NativeFinderCandidate, NativeFinderCandidates, NativeFinderCollectionRequest,
    NativeFinderConfig, NativeFinderProvenance, NativeFinderSurface,
    collect_native_finder_candidates,
};
pub use pipe_candidates::{
    SearchPipeCandidate, SearchPipeCandidateRequest, collect_search_pipe_candidates,
};
pub use pipe_source::{
    SearchPipeDocumentAcquisitionRequest, SearchPipeFailureAcquisitionRequest,
    SearchPipeFinderAcquisition, SearchPipeFinderAcquisitionRequest, SearchPipeSourceAcquisition,
    SearchPipeSourceAcquisitionTrace, SearchPipeSourceIndexAcquisition,
    SearchPipeSourceIndexAcquisitionRequest, SearchPipeSourceIndexCandidate,
    SearchPipeSourceIndexDecision, SearchPipeSourceIndexGate, SearchPipeSourceIndexLookup,
    SearchPipeSourceMode, collect_search_pipe_document_acquisition,
    collect_search_pipe_failure_acquisition, collect_search_pipe_finder_acquisition,
    collect_search_pipe_source_index_acquisition, failure_candidate_query,
};
pub use prompt_output_replay::{
    PromptOutputFingerprintRequest, PromptOutputReplayRequest, is_prime_seed_search_request,
    prompt_output_artifact_replay_safe, prompt_output_request_fingerprint,
};
pub use query_packet_replay::{
    QueryPacketReplayRequest, query_packet_matches_request, render_query_packet_stdout,
};
pub use query_wrapper_candidates::{
    QueryWrapperCandidateCollection, QueryWrapperSearchClause, QueryWrapperSearchRequest,
    QueryWrapperSearchSourceIndexTrace, QueryWrapperSearchSurface,
    collect_query_wrapper_candidate_collection,
};
pub use query_wrapper_scan::{
    QUERY_WRAPPER_CANDIDATE_LIMIT, QueryCandidateAppend, QueryWrapperCandidate,
    QueryWrapperCandidateSurface, QueryWrapperScanConfig, QueryWrapperSourceIndexCandidate,
    QueryWrapperSourceIndexCollection, QueryWrapperSourceIndexLookup,
    QueryWrapperSourceIndexRequest, append_query_candidates, augment_package_path_candidates,
    collect_query_wrapper_source_index_candidates, query_candidate_priority,
};
pub use search_language_files::{
    LanguageFileSpec, language_file_spec, language_neutral_search_file_spec,
};
pub use search_lexical_replay::{
    SearchLexicalReplayRequest, search_lexical_packet_matches_request,
};
pub use search_packet_replay::{
    output_with_delegation_hint_lines, search_output_artifact_replay_safe,
};
pub use source_index_rank::{
    SourceIndexRankCandidate, rank_source_index_candidates, source_index_lookup_terms,
};
pub use syntax_query_replay::{
    SyntaxQueryReplayCapture, SyntaxQueryRowsReplay, render_semantic_tree_sitter_query_rows_stdout,
    render_semantic_tree_sitter_query_stdout,
};

#[cfg(test)]
#[path = "../tests/unit/dynamic_search_candidates.rs"]
mod dynamic_search_candidates_tests;
#[cfg(test)]
#[path = "../tests/unit/pipe_candidates.rs"]
mod pipe_candidates_tests;
#[cfg(test)]
#[path = "../tests/unit/prompt_output_replay.rs"]
mod prompt_output_replay_tests;
#[cfg(test)]
#[path = "../tests/unit/query_packet_replay.rs"]
mod query_packet_replay_tests;
#[cfg(test)]
#[path = "../tests/unit/query_wrapper_candidates.rs"]
mod query_wrapper_candidates_tests;
#[cfg(test)]
#[path = "../tests/unit/search_language_files.rs"]
mod search_language_files_tests;
#[cfg(test)]
#[path = "../tests/unit/search_lexical_replay.rs"]
mod search_lexical_replay_tests;
#[cfg(test)]
#[path = "../tests/unit/search_packet_replay.rs"]
mod search_packet_replay_tests;
#[cfg(test)]
#[path = "../tests/unit/source_index_rank.rs"]
mod source_index_rank_tests;
#[cfg(test)]
#[path = "../tests/unit/syntax_query_replay.rs"]
mod syntax_query_replay_tests;
