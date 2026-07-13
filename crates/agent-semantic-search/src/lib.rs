#![deny(dead_code)]

//! Search orchestration services for ASP agent-facing queries.

mod document_candidates;
mod dynamic_candidates;
mod dynamic_overlay;
mod dynamic_search;
mod evidence_graph_rank;
mod graph_candidate_projection;
mod graph_candidate_sparsity;
mod graph_evidence_projection;
mod graph_node_projection;
mod graph_owner_rank;
pub use graph_owner_rank::{
    GraphOwnerRankCandidate, GraphOwnerRankReport, GraphOwnerRankRequest, GraphOwnerRankScore,
    GraphOwnerRankedOwner, rank_graph_owner_report,
};
mod graph_query_owner_seed;
mod graph_seed_decision;
mod graph_topology_projection;
mod lexical_overlay;
mod lexical_search_frame;
mod owner_items_source_index_trace;
mod pipe_candidates;
mod pipe_source;
mod pipe_source_index_projection;
mod pipe_source_lexical_frame;
pub use pipe_source::SearchPipeSelectorPayloadProof;
mod graph_owner_item_route;
mod prompt_output_replay;
mod provider_candidate_annotations;
mod query_packet_replay;
mod query_wrapper_candidates;
mod query_wrapper_quality;
mod query_wrapper_scan;
mod query_wrapper_scan_source;
mod search_candidate;
mod search_language_files;
mod search_lexical_replay;
mod search_overlay;
mod search_packet_replay;
mod search_pipe_evidence;
mod search_pipe_quality;
mod search_pipe_query_pack;
mod search_query_budget;
mod search_subagent_receipt;
mod source_index_lookup;

pub use graph_owner_item_route::{
    GraphOwnerItemEvidence, GraphOwnerItemRenderRequest, GraphOwnerItemRoute,
    GraphOwnerItemRouteRequest, GraphSemanticKind, rank_graph_owner_items,
    render_graph_owner_item_frontier,
};

mod source_index_rank;
pub use source_index_rank::{
    SourceIndexRankReport, SourceIndexRankRequest, SourceIndexRankScore,
    SourceIndexRankedCandidate, rank_source_index_report,
};
mod structural_index_search;
pub mod syntax_query_replay;
mod turso_overlay_search;

pub use document_candidates::{
    DocumentSearchCandidate, DocumentSearchCandidateCollection, DocumentSearchCandidateRequest,
    collect_document_search_candidates,
};
pub use dynamic_candidates::{
    DynamicSearchCandidate, DynamicSearchCandidateRequest, DynamicSearchRootCandidateRequest,
    IngestSearchCandidate, collect_dynamic_lexical_overlay_candidates,
    collect_dynamic_lexical_overlay_candidates_from_roots, collect_ingest_search_candidates,
};
pub use dynamic_overlay::{
    DynamicOverlayLane, QUERY_OVERLAY_ROUTE_SOURCE, SEARCH_OVERLAY_ROUTE_SOURCE,
};
pub use dynamic_search::{
    DynamicOwnerItem, DynamicOwnerItemsRequest, DynamicOwnerPath, DynamicOwnerQuery,
    DynamicSearchLanguage, DynamicSearchRoots, render_dynamic_owner_items_code,
    render_dynamic_owner_items_frontier,
};
pub use evidence_graph_rank::{
    EvidenceGraphRankNode, EvidenceGraphRankScore, EvidenceGraphRankedNode,
    evidence_graph_rank_terms, rank_evidence_graph_nodes,
};
pub use graph_candidate_projection::{
    GraphCandidateHotNodesRequest, GraphCandidateItemNodesRequest, GraphProjectionCandidate,
    graph_candidate_hot_node_id, graph_candidate_hot_nodes, graph_candidate_item_node_id,
    graph_candidate_item_nodes,
};
pub use graph_candidate_sparsity::{
    GraphCandidateSparsityInput, select_sparse_graph_candidate_indices,
};
pub use graph_evidence_projection::graph_frontier_has_only_owner_or_topology_nodes;
pub use graph_node_projection::{owner_path_graph_nodes, stable_graph_node_id};
pub use graph_owner_rank::{
    ranked_graph_owner_paths_for_submodule_paths, ranked_graph_owner_paths_with_topology,
};
pub use graph_query_owner_seed::{graph_has_package_path_candidate, graph_query_owner_seed_paths};
pub use graph_seed_decision::{
    GraphTurboSeedPlanInput, SearchActionSelection, SearchEvidenceState, SeedActionIntent,
    SeedPhaseDecision, graph_turbo_seed_plan, recommended_action_for_seed_risk,
};
pub use graph_topology_projection::{
    GraphTopologyProjection, GraphTopologyProjectionRequest, graph_path_is_under,
    graph_project_submodule_paths, graph_project_submodule_paths_from_content,
    graph_project_topology_projection, graph_submodule_owner_edges,
};
pub use lexical_overlay::{
    LexicalOverlayCandidateHit, LexicalOverlayDocument, LexicalOverlaySearchHit,
    LexicalOverlaySearchRequest, search_lexical_overlay, search_lexical_overlay_candidates,
};
pub use lexical_search_frame::{
    LexicalAcquisitionRoute, LexicalEvidenceState, LexicalQueryRelation,
    LexicalSearchFrameCandidate, LexicalSearchFrameRequest, LexicalSearchFrameRoute,
    plan_lexical_search_frame,
};
pub use owner_items_source_index_trace::{
    OwnerItemsSourceIndexTrace, OwnerItemsSourceIndexTraceRender, OwnerItemsSourceIndexTraceStream,
    owner_items_source_index_trace, render_owner_items_source_index_lookup_trace,
    render_owner_items_source_index_trace, source_index_owner_query,
};
pub use pipe_candidates::{
    SearchPipeCandidate, SearchPipeCandidateRequest, collect_search_pipe_candidates,
};
pub use pipe_source::{
    SearchPipeAutoAcquisitionRequest, SearchPipeDocumentAcquisitionRequest,
    SearchPipeFailureAcquisitionRequest, SearchPipeSearchOverlayAcquisition,
    SearchPipeSearchOverlayAcquisitionRequest, SearchPipeSourceAcquisition,
    SearchPipeSourceAcquisitionTrace, SearchPipeSourceIndexAcquisition,
    SearchPipeSourceIndexAcquisitionRequest, SearchPipeSourceIndexCandidate,
    SearchPipeSourceIndexDecision, SearchPipeSourceIndexGate, SearchPipeSourceIndexLookup,
    SearchPipeSourceMode, collect_search_pipe_auto_acquisition,
    collect_search_pipe_document_acquisition, collect_search_pipe_failure_acquisition,
    collect_search_pipe_search_overlay_acquisition, collect_search_pipe_source_index_acquisition,
    failure_candidate_query,
};
pub use prompt_output_replay::{
    PromptOutputFingerprintRequest, PromptOutputReplayRequest, is_prime_seed_search_request,
    prompt_output_artifact_replay_safe, prompt_output_request_fingerprint,
};
pub use provider_candidate_annotations::{
    ProviderFactsEnvelope, compact_provider_fact_nodes, compact_provider_fact_value,
    provider_candidate_annotation_nodes, provider_facts_envelope_from_stdout,
    provider_facts_envelope_from_value,
};
pub use query_packet_replay::{
    QueryPacketReplayRequest, query_packet_matches_request, render_query_packet_stdout,
};
pub use query_wrapper_candidates::{
    QueryWrapperCandidateCollection, QueryWrapperSearchRequest, QueryWrapperSearchSourceIndexTrace,
    QueryWrapperSearchStageTraceProjection, QueryWrapperSearchSurface,
    QueryWrapperSourceIndexTraceProjection, collect_query_wrapper_candidate_collection,
    query_wrapper_ranked_search_candidates, query_wrapper_search_stage_trace_projection,
    query_wrapper_source_index_trace_projection,
};
pub use query_wrapper_quality::{
    QueryWrapperClauseCoverage, QueryWrapperQuality, QueryWrapperQualityCandidate,
    QueryWrapperSearchClause, analyze_query_wrapper_quality, query_wrapper_axis_terms,
    query_wrapper_candidate_matches_term, query_wrapper_clauses, query_wrapper_owner_candidates,
    query_wrapper_package_clusters, query_wrapper_package_clusters_from_paths,
    query_wrapper_package_key, query_wrapper_rg_scope_next, query_wrapper_terms,
    query_wrapper_unique_clause_terms,
};
pub use query_wrapper_scan::{
    QUERY_WRAPPER_CANDIDATE_LIMIT, QueryCandidateAppend, QueryWrapperCandidate,
    QueryWrapperCandidateSurface, QueryWrapperScanConfig, QueryWrapperSearchCandidateCollection,
    QueryWrapperSearchCandidateRequest, QueryWrapperSourceIndexCandidate,
    QueryWrapperSourceIndexCandidateRequest, QueryWrapperSourceIndexCollection,
    QueryWrapperSourceIndexLookup, QueryWrapperSourceIndexRequest, append_query_candidates,
    augment_package_path_candidates,
};
pub use query_wrapper_scan_source::{
    collect_query_wrapper_search_candidates, collect_query_wrapper_source_index_candidates,
};
pub use search_candidate::structural_index_hit_to_search_candidate;
pub use search_candidate::{
    FieldHit, RankFeature, RankedSearchCandidate, SearchCandidate, SearchCandidateMergeReceipt,
    SearchStageReceipt, lexical_overlay_hit_to_search_candidate, merge_search_candidates,
    merge_search_candidates_with_receipt, search_candidate_has_executable_line_identity,
    source_index_candidate_to_search_candidate,
};
pub use search_language_files::{
    LanguageFileSpec, language_file_spec, language_neutral_search_file_spec,
};
pub use search_lexical_replay::{
    SearchLexicalReplayRequest, search_lexical_packet_matches_request,
};
pub use search_overlay::{
    SearchOverlayCandidate, SearchOverlayCandidates, SearchOverlayCollectionRequest,
    SearchOverlayConfig, SearchOverlayProvenance, SearchOverlaySurface,
    collect_search_overlay_candidates,
};
pub use search_packet_replay::{
    output_with_delegation_hint_lines, search_output_artifact_replay_safe,
};
pub use search_pipe_evidence::{
    SearchPipeEvidenceCandidate, search_pipe_declaration_header_match, search_pipe_handle_paths,
    search_pipe_high_value_matches, search_pipe_high_value_missing, search_pipe_is_high_value_term,
    search_pipe_parser_handles, search_pipe_path_exact_match, search_pipe_search_overlay_handles,
    search_pipe_strong_match, search_pipe_weak_match, search_pipe_weak_reason,
};
pub use search_pipe_quality::{
    SearchPipeCohesionTerm, is_search_pipe_package_axis_term, search_pipe_candidate_packages,
    search_pipe_fd_owner_axis_term, search_pipe_fd_query_terms, search_pipe_missing_path_terms,
    search_pipe_owner_seed_terms, search_pipe_package_cohesion, search_pipe_package_key,
    search_pipe_quality_risks, search_pipe_query_pack_quality,
};
pub use search_pipe_query_pack::{
    SearchPipeClauseCoverage, SearchPipeLanguageId, SearchPipeQueryClause,
    SearchPipeQueryClausesRequest, SearchPipeQueryPackCandidate, SearchPipeQueryTerm,
    SearchPipeQueryText, SearchPipeTermRole, search_pipe_clause_coverages,
    search_pipe_is_path_like_token, search_pipe_next_query_pack_hint,
    search_pipe_query_candidate_matches_term, search_pipe_query_clause_texts,
    search_pipe_query_clauses, search_pipe_role_terms, search_pipe_unique_query_terms,
};
pub use search_query_budget::{
    SearchQueryBudgetBlock, search_query_budget_block, search_query_terms,
    search_rg_terms_budget_block, search_terms_budget_block, specific_search_term,
};
pub use search_subagent_receipt::{
    SEARCH_SUBAGENT_GRAPH_ROUTE_RECEIPT_SCHEMA, search_subagent_graph_route_receipt,
    search_subagent_graph_route_receipt_is_compact,
};
pub use source_index_lookup::{
    SourceIndexClientCacheLookupRequest, SourceIndexClientCachePlannerLookupRequest,
    SourceIndexLookupRequest, lookup_source_index, lookup_source_index_for_language,
    lookup_source_index_in_cache, lookup_source_index_in_client_cache_dir,
    lookup_source_index_in_client_cache_dir_with_planner,
};
pub use source_index_rank::{
    SourceIndexRankCandidate, rank_source_index_candidates, reorder_source_index_candidates,
    source_index_lookup_terms,
};
pub use structural_index_search::{
    TursoStructuralIndexCandidateRequest, TursoStructuralIndexSearchHit,
    collect_turso_structural_index_ranked_candidates,
    collect_turso_structural_index_ranked_candidates_async,
    collect_turso_structural_index_ranked_candidates_from_engine_async,
    search_turso_structural_index_documents,
};
pub use syntax_query_replay::{
    SyntaxQueryReplayCapture, SyntaxQueryRowsReplay, render_semantic_tree_sitter_query_rows_stdout,
    render_semantic_tree_sitter_query_stdout,
};
pub use turso_overlay_search::{
    TursoOverlaySearchDocument, TursoOverlaySearchHit, bootstrap_turso_overlay_search_store,
    search_turso_overlay_documents, upsert_turso_overlay_search_document,
};

#[cfg(test)]
#[path = "../tests/unit/document_auto_lexical_overlay_scenario.rs"]
mod document_auto_lexical_overlay_scenario_tests;
#[cfg(test)]
#[path = "../tests/unit/dynamic_search_candidates.rs"]
mod dynamic_search_candidates_tests;
pub mod file_locator;
#[cfg(test)]
#[path = "../tests/unit/graph_candidate_projection.rs"]
mod graph_candidate_projection_tests;
#[cfg(test)]
#[path = "../tests/unit/graph_candidate_sparsity.rs"]
mod graph_candidate_sparsity_tests;
#[cfg(test)]
#[path = "../tests/unit/graph_evidence_projection.rs"]
mod graph_evidence_projection_tests;
#[cfg(test)]
#[path = "../tests/unit/graph_node_projection.rs"]
mod graph_node_projection_tests;
#[cfg(test)]
#[path = "../tests/unit/graph_owner_rank.rs"]
mod graph_owner_rank_tests;
#[cfg(test)]
#[path = "../tests/unit/graph_query_owner_seed.rs"]
mod graph_query_owner_seed_tests;
#[cfg(test)]
#[path = "../tests/unit/graph_seed_decision.rs"]
mod graph_seed_decision_tests;
#[cfg(test)]
#[path = "../tests/unit/graph_topology_projection.rs"]
mod graph_topology_projection_tests;
#[cfg(test)]
#[path = "../tests/unit/owner_items_source_index_trace.rs"]
mod owner_items_source_index_trace_tests;
#[cfg(test)]
#[path = "../tests/unit/pipe_candidates.rs"]
mod pipe_candidates_tests;
#[cfg(test)]
#[path = "../tests/unit/prompt_output_replay.rs"]
mod prompt_output_replay_tests;
#[cfg(test)]
#[path = "../tests/unit/provider_candidate_annotations.rs"]
mod provider_candidate_annotations_tests;
#[cfg(test)]
#[path = "../tests/unit/query_packet_replay.rs"]
mod query_packet_replay_tests;
#[cfg(test)]
#[path = "../tests/unit/query_wrapper_candidates.rs"]
mod query_wrapper_candidates_tests;
#[cfg(test)]
#[path = "../tests/unit/search_candidate.rs"]
mod search_candidate_tests;
pub mod search_command_preflight;
#[cfg(test)]
#[path = "../tests/unit/search_language_files.rs"]
mod search_language_files_tests;
#[cfg(test)]
#[path = "../tests/unit/search_lexical_replay.rs"]
mod search_lexical_replay_tests;
#[cfg(test)]
#[path = "../tests/unit/search_package_scenarios.rs"]
mod search_package_scenarios_tests;
#[cfg(test)]
#[path = "../tests/unit/search_packet_replay.rs"]
mod search_packet_replay_tests;
#[cfg(test)]
#[path = "../tests/unit/search_pipe_evidence.rs"]
mod search_pipe_evidence_tests;
#[cfg(test)]
#[path = "../tests/unit/search_pipe_quality.rs"]
mod search_pipe_quality_tests;
#[cfg(test)]
#[path = "../tests/unit/search_pipe_query_pack.rs"]
mod search_pipe_query_pack_tests;
pub mod search_planner;
#[cfg(test)]
#[path = "../tests/unit/search_query_budget.rs"]
mod search_query_budget_tests;
#[cfg(test)]
#[path = "../tests/unit/source_index_rank.rs"]
mod source_index_rank_tests;
#[cfg(test)]
#[path = "../tests/unit/syntax_query_replay.rs"]
mod syntax_query_replay_tests;
