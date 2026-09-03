#![deny(dead_code)]

//! Search orchestration services for ASP agent-facing queries.

mod cold_rg_corpus;
pub mod command_diagnostics;
mod content_generation;
mod derived_attachment_lifecycle;
mod dynamic_candidates;
mod dynamic_overlay;
mod evidence_graph_rank;
mod generation_graph_protocol;
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
mod graph_topology_projection;
mod lexical_accelerator;
mod lexical_generation_plan;
mod lexical_overlay;
pub mod memory_search;
#[cfg(feature = "tantivy-accelerator")]
mod merkle_search_generation;
pub use memory_search::{
    MemorySearchGeneration, MemorySearchGenerationReceipt, MemorySearchItem,
    MemorySearchPerformanceReceipt, MemorySearchRequest, MemorySearchResolution,
    MemorySearchResolutionState, MemorySearchSourceLeaf,
};

mod playbook_receipt;
mod provider_candidate_annotations;
pub mod provider_relation_memory;
mod public_playbook;
#[cfg(feature = "tantivy-accelerator")]
mod resident_byte_coverage;
mod resident_graph_search;
mod resident_search_execution;
#[cfg(feature = "tantivy-accelerator")]
mod resident_source_index;
mod runtime_search_receipt;
mod search_candidate;
mod search_generation_segment;
mod search_language_files;
mod sorted_record_table;
#[cfg(feature = "tantivy-accelerator")]
mod tantivy_lexical;
mod workspace_playbook_plan;

mod source_index_rank;
pub use agent_semantic_search_projection::source_index_artifact_digest;
pub use source_index_rank::{
    SourceIndexRankReport, SourceIndexRankRequest, SourceIndexRankScore,
    SourceIndexRankedCandidate, rank_source_index_report,
};
pub mod syntax_query_replay;

#[cfg(test)]
#[path = "../tests/unit/content_generation.rs"]
mod content_generation_tests;
#[cfg(test)]
#[path = "../tests/unit/generation_graph_protocol.rs"]
mod generation_graph_protocol_tests;
#[cfg(test)]
#[path = "../tests/unit/lexical_accelerator.rs"]
mod lexical_accelerator_tests;
#[cfg(test)]
#[path = "../tests/unit/lexical_generation_plan.rs"]
mod lexical_generation_plan_tests;
#[cfg(test)]
#[path = "../tests/unit/playbook_pipeline_performance.rs"]
#[cfg(feature = "tantivy-accelerator")]
mod playbook_pipeline_performance_tests;
#[cfg(test)]
#[path = "../tests/unit/playbook_receipt.rs"]
mod playbook_receipt_tests;
#[cfg(test)]
#[path = "../tests/unit/public_playbook.rs"]
mod public_playbook_tests;
#[cfg(test)]
#[path = "../tests/unit/resident_graph_search.rs"]
mod resident_graph_search_tests;
#[cfg(test)]
#[path = "../tests/unit/resident_search_execution.rs"]
#[cfg(feature = "tantivy-accelerator")]
mod resident_search_execution_tests;
#[cfg(test)]
#[path = "../tests/unit/resident_source_index.rs"]
#[cfg(feature = "tantivy-accelerator")]
mod resident_source_index_tests;
#[cfg(test)]
#[path = "../tests/unit/runtime_search_receipt.rs"]
mod runtime_search_receipt_tests;
#[cfg(test)]
#[path = "../tests/unit/workspace_playbook_plan.rs"]
mod workspace_playbook_plan_tests;

pub use cold_rg_corpus::{
    COLD_RG_CORPUS_RECEIPT_SCHEMA_ID, ColdRgCorpusArtifact, ColdRgCorpusOwner, ColdRgCorpusReceipt,
    ColdRgOwnerSpan, build_cold_rg_corpus, owner_for_corpus_line,
};
pub use content_generation::{
    CONTENT_SEARCH_GENERATION_RECEIPT_SCHEMA_ID, ContentSearchGenerationReceipt,
    NativeSyntaxProjection, NativeSyntaxRelation, NativeSyntaxSelector,
    SearchGenerationConstructionStage, SearchGenerationIdentity, SearchGenerationStageReceipt,
    SourceByteOwner, build_native_syntax_stage, build_source_byte_acquisition_stage,
    canonical_blake3_digest,
};
pub use derived_attachment_lifecycle::{
    RUNTIME_SEARCH_ATTACHMENT_EVENT_CAPACITY, RuntimeSearchDerivedAttachmentEvent,
    RuntimeSearchDerivedAttachmentHub, RuntimeSearchDerivedAttachmentIdentity,
    RuntimeSearchDerivedAttachmentKey, RuntimeSearchDerivedAttachmentKind,
    RuntimeSearchDerivedAttachmentSnapshot, RuntimeSearchDerivedAttachmentState,
};
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
pub use generation_graph_protocol::{
    SEARCH_GENERATION_GRAPH_RECEIPT_SCHEMA_ID, SEARCH_GENERATION_GRAPH_REQUEST_SCHEMA_ID,
    SearchGenerationGraphReceipt, SearchGenerationGraphRequest,
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
pub use graph_topology_projection::{
    GraphOwnerMissingTopologyRequest, GraphTopologyProjection, GraphTopologyProjectionRequest,
    graph_owner_missing_topology_projection, graph_path_is_under, graph_project_submodule_paths,
    graph_project_submodule_paths_from_content, graph_project_topology_projection,
    graph_submodule_owner_edges,
};
pub use lexical_accelerator::{
    COLD_RG_QUERY_RECEIPT_SCHEMA_ID, ColdRgQueryReceipt, LEXICAL_ACCELERATOR_RECEIPT_SCHEMA_ID,
    LexicalAcceleratorReceipt, LexicalRecallRoute, LexicalRouteEquivalenceCase,
    plan_lexical_recall_route,
};
pub use lexical_generation_plan::{
    AdmittedLexicalOwner, LEXICAL_GENERATION_PLAN_SCHEMA_ID, LexicalGenerationPlan,
    LexicalOwnerFact, LexicalShardArtifact, LexicalShardDisposition, LexicalShardPlanEntry,
    plan_lexical_generation,
};
pub use lexical_overlay::{
    LexicalOverlayCandidateHit, LexicalOverlayDocument, LexicalOverlaySearchHit,
    LexicalOverlaySearchRequest, search_lexical_overlay, search_lexical_overlay_candidates,
};
#[cfg(feature = "tantivy-accelerator")]
pub use merkle_search_generation::{
    MerkleSearchGeneration, SearchOwnerChange, SearchOwnerFragment, SearchProjectionIdentity,
    search_owner_graph_fragment_digest, search_projection_analyzer_digest,
};
pub use playbook_receipt::{
    SEARCH_PLAYBOOK_RECEIPT_SCHEMA_ID, SearchPlaybookColdRgExecution,
    SearchPlaybookPythonGraphExecution, SearchPlaybookReceipt, SearchPlaybookReceiptInput,
    build_search_playbook_receipt,
};
pub use provider_candidate_annotations::{
    ProviderFactsEnvelope, compact_provider_fact_nodes, compact_provider_fact_value,
    provider_candidate_annotation_nodes, provider_facts_envelope_from_stdout,
    provider_facts_envelope_from_value,
};
pub use public_playbook::{SearchPlaybookRequest, parse_search_playbook_args};
#[cfg(feature = "tantivy-accelerator")]
pub use resident_byte_coverage::{
    RESIDENT_BYTE_GRAM_WIDTH, ResidentByteCoverageIndex, ResidentByteCoverageInput,
};
pub use resident_graph_search::{
    ResidentGraphBuildMetrics, ResidentGraphEvaluatedEdge, ResidentGraphEvaluation,
    ResidentGraphEvaluationBudget, ResidentGraphEvaluationRequest, ResidentGraphGeneration,
    ResidentGraphRankedNode, ResidentGraphSearchBudget, ResidentGraphSearchRequest,
    ResidentGraphSearchStage, ResidentGraphSearchWork, build_resident_graph_generation,
    canonical_resident_graph_generation_digest, evaluate_resident_graph_generation,
    open_resident_graph_generation, rank_resident_graph_generation,
};
pub use resident_search_execution::{
    ResidentSearchExecutionPlan, ResidentSearchFusionCapabilities, ResidentSearchIntent,
    plan_resident_search_execution,
};
#[cfg(feature = "tantivy-accelerator")]
pub use resident_source_index::{
    ResidentIndexBuildResources, ResidentIndexBuildStrategy, ResidentLexicalCoverageInput,
    ResidentSearchAuthority, ResidentSourceDocument, ResidentSourceIndex,
    resident_index_engine_digest, resident_lexical_coverage_batch, resident_lexical_coverage_keys,
    resident_navigation_keys,
};
pub use runtime_search_receipt::{
    RUNTIME_SEARCH_SELECTOR_OWNER_LIMIT, RUNTIME_SEARCH_SOURCE_CAPACITY,
    RUNTIME_SEARCH_SOURCE_LIMIT, RuntimeSearchResult, RuntimeSearchSource,
    bounded_ranked_selector_owner_paths, bounded_runtime_search_source,
    build_runtime_provider_search_receipt, build_runtime_provider_search_receipt_with_graph,
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
pub use sorted_record_table::{ValidatedSortedRecordTable, encode_sorted_record_table};
pub use source_index_rank::{
    SourceIndexRankCandidate, rank_source_index_candidates, reorder_source_index_candidates,
    source_index_lookup_terms,
};
pub use syntax_query_replay::{
    SyntaxQueryReplayCapture, SyntaxQueryRowsReplay, render_semantic_tree_sitter_query_rows_stdout,
    render_semantic_tree_sitter_query_stdout,
};
pub use workspace_playbook_plan::{
    WORKSPACE_SEARCH_PLAYBOOK_PLAN_SCHEMA_ID, WorkspaceSearchPlanBinding,
    WorkspaceSearchPlaybookPlan, WorkspaceSearchPlaybookRoute, WorkspaceSearchProvider,
    WorkspaceSearchRouteBudget, WorkspaceSearchSkipReason, WorkspaceSearchSkippedLanguage,
    WorkspaceSearchStageKind, WorkspaceSearchStagePlan, WorkspaceSearchStagePolicy,
    WorkspaceSearchWarmWork, build_workspace_search_playbook_plan,
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
#[path = "../tests/unit/graph_topology_projection.rs"]
mod graph_topology_projection_tests;
#[cfg(test)]
#[path = "../tests/unit/merkle_search_generation.rs"]
#[cfg(feature = "tantivy-accelerator")]
mod merkle_search_generation_tests;
#[cfg(test)]
#[path = "../tests/unit/provider_candidate_annotations.rs"]
mod provider_candidate_annotations_tests;
#[cfg(test)]
#[path = "../tests/unit/search_candidate.rs"]
mod search_candidate_tests;
#[cfg(test)]
#[path = "../tests/unit/search_language_files.rs"]
mod search_language_files_tests;
#[cfg(test)]
#[path = "../tests/unit/search_package_scenarios.rs"]
mod search_package_scenarios_tests;
#[cfg(test)]
#[path = "../tests/unit/source_index_rank.rs"]
mod source_index_rank_tests;
#[cfg(test)]
#[path = "../tests/unit/source_snapshot_fixture.rs"]
mod source_snapshot_fixture;

#[cfg(test)]
extern crate self as agent_semantic_search;

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
