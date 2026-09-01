#![deny(dead_code)]

//! Search orchestration services for ASP agent-facing queries.

pub mod command_diagnostics;
mod dynamic_candidates;
mod dynamic_overlay;
mod evidence_graph_rank;
mod graph_action_frontier;
mod graph_candidate_projection;
pub use graph_action_frontier::{DependencyActionNodeV1, matched_dependency_action_targets};
mod graph_candidate_sparsity;
mod graph_evidence_projection;
pub mod graph_generation_authority;
mod graph_node_projection;
mod graph_owner_rank;
pub use graph_owner_rank::{
    GraphOwnerRankCandidate, GraphOwnerRankReport, GraphOwnerRankRequest, GraphOwnerRankScore,
    GraphOwnerRankedOwner, rank_graph_owner_report,
};
pub mod exact_selector_generation_fixture;
mod graph_query_owner_seed;
mod graph_seed_decision;
mod graph_selector_seed_projection;
mod graph_topology_projection;
mod lexical_overlay;
pub mod memory_search;
mod merkle_search_generation;
pub use memory_search::{
    MemorySearchGeneration, MemorySearchGenerationReceipt, MemorySearchItem,
    MemorySearchPerformanceReceipt, MemorySearchRequest, MemorySearchResolution,
    MemorySearchResolutionState, MemorySearchSourceLeaf,
};

mod lexical_search_frame;
mod pipe_candidates;
mod pipe_source;
mod pipe_source_index_acquisition;
mod pipe_source_index_projection;
mod pipe_source_lexical_frame;
mod prompt_output_replay;
mod provider_candidate_annotations;
pub mod provider_relation_memory;
pub mod recall_route_planner;
mod resident_graph_search;
mod resident_source_index;
mod runtime_search_receipt;
mod search_candidate;
mod search_generation_segment;
mod search_language_files;
mod search_lexical_replay;
mod search_packet_replay;
mod search_pipe_evidence;
pub mod search_pipe_quality;
mod search_pipe_query_pack;
mod search_query_budget;
mod search_subagent_receipt;
mod sorted_record_table;

mod source_index_rank;
pub use agent_semantic_search_projection::source_index_artifact_digest;
pub use source_index_rank::{
    SourceIndexRankReport, SourceIndexRankRequest, SourceIndexRankScore,
    SourceIndexRankedCandidate, rank_source_index_report,
};
pub mod syntax_query_replay;

#[cfg(test)]
#[path = "../tests/unit/resident_graph_search.rs"]
mod resident_graph_search_tests;
#[cfg(test)]
#[path = "../tests/unit/resident_source_index.rs"]
mod resident_source_index_tests;
#[cfg(test)]
#[path = "../tests/unit/runtime_search_receipt.rs"]
mod runtime_search_receipt_tests;

pub use dynamic_candidates::{
    DynamicSearchCandidate, DynamicSearchCandidateCollection, DynamicSearchRootCandidateRequest,
    IngestSearchCandidate, RgCoverageBudget, RgCoverageOwner, RgCoverageReceipt, RgCoverageRequest,
    RgCoverageResult, collect_dynamic_lexical_overlay_candidates_from_roots,
    collect_rg_coverage_candidates,
};
pub use dynamic_overlay::{
    DynamicOverlayLane, QUERY_OVERLAY_ROUTE_SOURCE, SEARCH_OVERLAY_ROUTE_SOURCE,
};
pub use evidence_graph_rank::{
    EvidenceGraphNodeId, EvidenceGraphNodeKind, EvidenceGraphRankNode, EvidenceGraphRankScore,
    EvidenceGraphRankedNode, evidence_graph_rank_terms, rank_evidence_graph_nodes,
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
pub use graph_selector_seed_projection::{
    GraphSelectorSeedProjection, GraphSelectorSeedProjectionRequest, graph_selector_seed_projection,
};
pub use graph_topology_projection::{
    GraphOwnerMissingTopologyRequest, GraphTopologyProjection, GraphTopologyProjectionRequest,
    graph_owner_missing_topology_projection, graph_path_is_under, graph_project_submodule_paths,
    graph_project_submodule_paths_from_content, graph_project_topology_projection,
    graph_submodule_owner_edges,
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
pub use merkle_search_generation::{
    MerkleSearchGeneration, SearchOwnerChange, SearchOwnerFragment, SearchProjectionIdentity,
    search_owner_graph_fragment_digest, search_projection_analyzer_digest,
};
pub use pipe_candidates::{
    SearchPipeCandidate, SearchPipeCandidateCollection, SearchPipeCandidateRequest,
    collect_search_pipe_candidates,
};
pub use pipe_source::{
    SearchPipeAutoAcquisitionRequest, SearchPipeFailureAcquisitionRequest,
    SearchPipeSearchOverlayAcquisition, SearchPipeSearchOverlayAcquisitionRequest,
    SearchPipeSourceAcquisition, SearchPipeSourceAcquisitionTrace, SearchPipeSourceArtifactDigest,
    SearchPipeSourceTraceSource, SearchPipeSourceTraceStatus, collect_search_pipe_auto_acquisition,
    collect_search_pipe_failure_acquisition, collect_search_pipe_search_overlay_acquisition,
    failure_candidate_query,
};
pub use pipe_source_index_acquisition::{
    SearchPipeSourceIndexAcquisition, SearchPipeSourceIndexAcquisitionRequest,
    SearchPipeSourceIndexCandidate, SearchPipeSourceIndexDecision, SearchPipeSourceIndexGate,
    SearchPipeSourceIndexLookup, collect_search_pipe_source_index_acquisition,
    search_pipe_source_index_query_gate,
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
pub use resident_graph_search::{
    ResidentGraphGeneration, ResidentGraphSearchRequest, ResidentGraphSearchStage,
    build_resident_graph_generation, build_resident_graph_search_request,
    project_resident_graph_search_result,
};
pub use resident_source_index::{
    ResidentSearchAuthority, ResidentSourceIndex, ResidentSourceIndexSeed,
    resident_lexical_coverage_keys, resident_navigation_keys,
};
pub use runtime_search_receipt::{
    RUNTIME_SEARCH_SOURCE_CAPACITY, RUNTIME_SEARCH_SOURCE_LIMIT, RuntimeSearchResult,
    RuntimeSearchSource, bounded_runtime_search_source, build_runtime_provider_search_receipt,
    build_runtime_provider_search_receipt_with_graph,
};
pub use search_candidate::{
    FieldHit, RankFeature, RankedSearchCandidate, SearchCandidate, SearchCandidateMergeReceipt,
    SearchStageReceipt, lexical_overlay_hit_to_search_candidate, merge_search_candidates,
    merge_search_candidates_with_receipt, search_candidate_has_executable_line_identity,
    source_index_candidate_to_search_candidate,
};
pub use search_candidate::{StructuralIndexSearchHit, structural_index_hit_to_search_candidate};
pub use search_generation_segment::{
    SearchGenerationSection, SearchGenerationSectionKind, SearchGenerationSectionRepresentation,
    ValidatedSearchGenerationSegment, encode_search_generation_segment,
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
pub use search_pipe_evidence::{
    SearchPipeEvidenceCandidate, search_pipe_declaration_header_match, search_pipe_handle_paths,
    search_pipe_high_value_matches, search_pipe_high_value_missing, search_pipe_is_high_value_term,
    search_pipe_parser_handles, search_pipe_path_exact_match, search_pipe_search_overlay_handles,
    search_pipe_strong_match, search_pipe_weak_match, search_pipe_weak_reason,
};
pub use search_pipe_quality::{
    SearchPipeCohesionTerm, is_search_pipe_package_axis_term, search_pipe_candidate_packages,
    search_pipe_missing_path_terms, search_pipe_owner_seed_terms, search_pipe_package_cohesion,
    search_pipe_package_key, search_pipe_quality_risks, search_pipe_query_pack_quality,
};
pub use search_pipe_query_pack::{
    SearchPipeClauseCoverage, SearchPipeLanguageId, SearchPipeQueryClause,
    SearchPipeQueryClausesRequest, SearchPipeQueryPackCandidate, SearchPipeQueryPackClause,
    SearchPipeQueryPackDescriptor, SearchPipeQueryPackRecipe, SearchPipeQueryPackTermRoleOverride,
    SearchPipeQueryTerm, SearchPipeQueryText, SearchPipeSemanticFactsDescriptor,
    SearchPipeSemanticFactsIntentAxis, SearchPipeSemanticFactsIntentDecision, SearchPipeTermRole,
    search_pipe_clause_coverages, search_pipe_is_path_like_token, search_pipe_next_query_pack_hint,
    search_pipe_query_candidate_matches_term, search_pipe_query_clause_texts,
    search_pipe_query_clauses, search_pipe_role_terms, search_pipe_typed_query_terms,
    search_pipe_unique_query_terms,
};
pub use search_query_budget::{
    SearchQueryBudgetBlock, SearchQueryBudgetRequest, search_query_budget_block,
    search_query_terms, search_terms_budget_block, specific_search_term,
};
pub use search_subagent_receipt::{
    SEARCH_SUBAGENT_GRAPH_ROUTE_RECEIPT_SCHEMA, search_subagent_graph_route_receipt,
    search_subagent_graph_route_receipt_is_compact,
};
pub use sorted_record_table::{ValidatedSortedRecordTable, encode_sorted_record_table};
pub use source_index_rank::{
    SourceIndexRankCandidate, rank_source_index_candidates, reorder_source_index_candidates,
    source_index_lookup_terms,
};
pub use syntax_query_replay::{
    SyntaxQueryReplayCapture, SyntaxQueryRowsReplay, render_semantic_tree_sitter_query_rows_stdout,
    render_semantic_tree_sitter_query_stdout,
};

#[cfg(test)]
#[path = "../tests/unit/dynamic_overlay_index.rs"]
mod dynamic_overlay_index_tests;
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
#[path = "../tests/unit/merkle_search_generation.rs"]
mod merkle_search_generation_tests;
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
#[path = "../tests/unit/recall_route_planner.rs"]
mod recall_route_planner_tests;
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
#[path = "../tests/unit/source_index_rank.rs"]
mod source_index_rank_tests;
pub use search_pipe_query_pack::search_pipe_semantic_facts_intent;
#[cfg(test)]
#[path = "../tests/unit/source_snapshot_fixture.rs"]
mod source_snapshot_fixture;

#[cfg(test)]
extern crate self as agent_semantic_search;

#[cfg(test)]
#[path = "../tests/unit/query_pack_fixture.rs"]
mod query_pack_fixture;
pub use search_pipe_evidence::SearchPipeEvidenceLanguageId;
pub mod load_once_generation;
pub use load_once_generation::LoadOnceGenerationV1;
pub mod active_exact_selector_fixture;
pub mod exact_selector_fixture_memory;
pub use exact_selector_fixture_memory::{
    ExactSelectorFixtureArtifactV1, ExactSelectorFixtureBackendV1,
    ExactSelectorFixtureFileBackendV1, ExactSelectorFixtureProjectionV1,
    ExactSelectorFixtureResidentV1, exact_selector_fixture_lookup_v1,
    exact_selector_fixture_projection_range_v1, exact_selector_fixture_projection_v1,
};
pub mod exact_selector_fixture_publication;
pub use exact_selector_fixture_publication::{
    EXACT_SELECTOR_FIXTURE_ARTIFACT_KIND, ExactSelectorFixturePublicationReceiptV1,
    build_exact_selector_fixture_from_projection_records_v1, publish_exact_selector_fixture_v1,
};
