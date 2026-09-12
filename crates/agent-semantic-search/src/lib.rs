#![deny(dead_code)]
// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Search orchestration services for ASP agent-facing queries.

pub mod command_diagnostics;
mod content_generation;
mod derived_attachment_lifecycle;
mod dynamic_candidates;
mod dynamic_overlay;
mod evidence_graph_rank;
mod generation_graph_protocol;
mod graph_action_frontier;
mod graph_candidate_projection;
mod resident_grep_corpus;
pub use graph_action_frontier::DependencyActionNodeV1;
pub use graph_action_frontier::matched_dependency_action_targets;
mod graph_candidate_sparsity;
mod graph_evidence_projection;
pub mod graph_generation_authority;
mod graph_node_projection;
mod graph_owner_rank;
pub use graph_owner_rank::GraphOwnerRankCandidate;
pub use graph_owner_rank::GraphOwnerRankReport;
pub use graph_owner_rank::GraphOwnerRankRequest;
pub use graph_owner_rank::GraphOwnerRankScore;
pub use graph_owner_rank::GraphOwnerRankedOwner;
pub use graph_owner_rank::rank_graph_owner_report;
pub mod exact_selector_generation_fixture;
mod graph_topology_projection;
mod lexical_accelerator;
mod lexical_generation_plan;
mod lexical_overlay;
pub mod memory_search;
#[cfg(feature = "tantivy-accelerator")]
mod merkle_search_generation;
pub use memory_search::MemorySearchGeneration;
pub use memory_search::MemorySearchGenerationReceipt;
pub use memory_search::MemorySearchItem;
pub use memory_search::MemorySearchPerformanceReceipt;
pub use memory_search::MemorySearchRequest;
pub use memory_search::MemorySearchResolution;
pub use memory_search::MemorySearchResolutionState;
pub use memory_search::MemorySearchSourceLeaf;

mod progressive_query;
mod provider_candidate_annotations;
pub mod provider_relation_memory;
#[cfg(feature = "tantivy-accelerator")]
mod resident_byte_coverage;
mod resident_graph_search;
#[cfg(feature = "tantivy-accelerator")]
mod resident_grep_candidate_plan;
#[cfg(feature = "tantivy-accelerator")]
mod resident_source_index;
mod runtime_search_receipt;
mod scheme_playbook_source;
mod search_candidate;
mod search_generation_segment;
mod search_language_files;
mod sorted_record_table;
#[cfg(feature = "tantivy-accelerator")]
mod tantivy_lexical;
mod workspace_playbook_plan;
mod workspace_playbook_result;

mod source_index_rank;
pub use agent_semantic_search_projection::source_index_artifact_digest;
pub use agent_semantic_shell_parser::TantivyQueryAnalysis;
pub use agent_semantic_shell_parser::TantivyQueryDiagnostic;
pub use agent_semantic_shell_parser::TantivyQueryMetrics;
pub use agent_semantic_shell_parser::analyze_tantivy_query;
pub use source_index_rank::SourceIndexRankReport;
pub use source_index_rank::SourceIndexRankRequest;
pub use source_index_rank::SourceIndexRankScore;
pub use source_index_rank::SourceIndexRankedCandidate;
pub use source_index_rank::rank_source_index_report;
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
#[path = "../tests/unit/resident_graph_search.rs"]
mod resident_graph_search_tests;
#[cfg(test)]
#[path = "../tests/unit/resident_source_index.rs"]
#[cfg(feature = "tantivy-accelerator")]
mod resident_source_index_tests;
#[cfg(test)]
#[path = "../tests/unit/runtime_search_receipt.rs"]
mod runtime_search_receipt_tests;
#[cfg(test)]
#[path = "../tests/unit/scheme_playbook_source.rs"]
mod scheme_playbook_source_tests;
#[cfg(test)]
#[path = "../tests/unit/workspace_playbook_plan.rs"]
mod workspace_playbook_plan_tests;
#[cfg(test)]
#[path = "../tests/unit/workspace_playbook_result.rs"]
mod workspace_playbook_result_tests;

pub use agent_semantic_search_playbook::{
    GraphNativeBlock, ProducerNativeBlock, ProgressiveSearchPlaybookError,
    ProgressiveSearchPlaybookRequest, SearchPlaybookClauseAxis, SearchPlaybookClauseRef,
    SearchPlaybookProducerDeclaration, parse_progressive_search_playbook_args,
    parse_search_playbook_producer_declaration,
};
pub use agent_semantic_search_projection::WORKSPACE_SEARCH_PLAYBOOK_V1_EVIDENCE_ITEM_LIMIT;
pub use content_generation::CONTENT_SEARCH_GENERATION_RECEIPT_SCHEMA_ID;
pub use content_generation::ContentSearchGenerationReceipt;
pub use content_generation::NativeSyntaxDiagnostic;
pub use content_generation::NativeSyntaxProjection;
pub use content_generation::NativeSyntaxRelation;
pub use content_generation::NativeSyntaxSelector;
pub use content_generation::SearchGenerationConstructionStage;
pub use content_generation::SearchGenerationIdentity;
pub use content_generation::SearchGenerationStageReceipt;
pub use content_generation::SourceByteOwner;
pub use content_generation::build_native_syntax_stage;
pub use content_generation::build_native_syntax_stage_with_diagnostics;
pub use content_generation::build_source_byte_acquisition_stage;
pub use content_generation::canonical_blake3_digest;
pub use derived_attachment_lifecycle::RUNTIME_SEARCH_ATTACHMENT_EVENT_CAPACITY;
pub use derived_attachment_lifecycle::RuntimeSearchDerivedAttachmentEvent;
pub use derived_attachment_lifecycle::RuntimeSearchDerivedAttachmentHub;
pub use derived_attachment_lifecycle::RuntimeSearchDerivedAttachmentIdentity;
pub use derived_attachment_lifecycle::RuntimeSearchDerivedAttachmentKey;
pub use derived_attachment_lifecycle::RuntimeSearchDerivedAttachmentKind;
pub use derived_attachment_lifecycle::RuntimeSearchDerivedAttachmentSnapshot;
pub use derived_attachment_lifecycle::RuntimeSearchDerivedAttachmentState;
pub use dynamic_candidates::DynamicSearchCandidate;
pub use dynamic_candidates::DynamicSearchCandidateCollection;
pub use dynamic_candidates::DynamicSearchRootCandidateRequest;
pub use dynamic_candidates::IngestSearchCandidate;
pub use dynamic_candidates::RgCoverageBudget;
pub use dynamic_candidates::RgCoverageOwner;
pub use dynamic_candidates::RgCoverageReceipt;
pub use dynamic_candidates::RgCoverageRequest;
pub use dynamic_candidates::RgCoverageResult;
pub use dynamic_candidates::collect_dynamic_lexical_overlay_candidates_from_roots;
pub use dynamic_candidates::collect_rg_coverage_candidates;
pub use dynamic_overlay::DynamicOverlayLane;
pub use dynamic_overlay::QUERY_OVERLAY_ROUTE_SOURCE;
pub use dynamic_overlay::SEARCH_OVERLAY_ROUTE_SOURCE;
pub use evidence_graph_rank::EvidenceGraphNodeId;
pub use evidence_graph_rank::EvidenceGraphNodeKind;
pub use evidence_graph_rank::EvidenceGraphRankNode;
pub use evidence_graph_rank::EvidenceGraphRankScore;
pub use evidence_graph_rank::EvidenceGraphRankedNode;
pub use evidence_graph_rank::evidence_graph_rank_terms;
pub use evidence_graph_rank::rank_evidence_graph_nodes;
pub use generation_graph_protocol::SEARCH_GENERATION_GRAPH_RECEIPT_SCHEMA_ID;
pub use generation_graph_protocol::SEARCH_GENERATION_GRAPH_REQUEST_SCHEMA_ID;
pub use generation_graph_protocol::SearchGenerationGraphReceipt;
pub use generation_graph_protocol::SearchGenerationGraphRequest;
pub use graph_candidate_projection::GraphCandidateHotNodesRequest;
pub use graph_candidate_projection::GraphCandidateItemNodesRequest;
pub use graph_candidate_projection::GraphProjectionCandidate;
pub use graph_candidate_projection::graph_candidate_hot_node_id;
pub use graph_candidate_projection::graph_candidate_hot_nodes;
pub use graph_candidate_projection::graph_candidate_item_node_id;
pub use graph_candidate_projection::graph_candidate_item_nodes;
pub use graph_candidate_sparsity::GraphCandidateSparsityInput;
pub use graph_candidate_sparsity::select_sparse_graph_candidate_indices;
pub use graph_evidence_projection::graph_frontier_has_only_owner_or_topology_nodes;
pub use graph_node_projection::owner_path_graph_nodes;
pub use graph_node_projection::stable_graph_node_id;
pub use graph_owner_rank::ranked_graph_owner_paths_for_submodule_paths;
pub use graph_owner_rank::ranked_graph_owner_paths_with_topology;
pub use graph_topology_projection::GraphOwnerMissingTopologyRequest;
pub use graph_topology_projection::GraphTopologyProjection;
pub use graph_topology_projection::GraphTopologyProjectionRequest;
pub use graph_topology_projection::graph_owner_missing_topology_projection;
pub use graph_topology_projection::graph_path_is_under;
pub use graph_topology_projection::graph_project_submodule_paths;
pub use graph_topology_projection::graph_project_submodule_paths_from_content;
pub use graph_topology_projection::graph_project_topology_projection;
pub use graph_topology_projection::graph_submodule_owner_edges;
pub use lexical_accelerator::LEXICAL_ACCELERATOR_RECEIPT_SCHEMA_ID;
pub use lexical_accelerator::LexicalAcceleratorReceipt;
pub use lexical_accelerator::LexicalRecallRoute;
pub use lexical_accelerator::LexicalRouteEquivalenceCase;
pub use lexical_accelerator::plan_lexical_recall_route;
pub use lexical_generation_plan::AdmittedLexicalOwner;
pub use lexical_generation_plan::LEXICAL_GENERATION_PLAN_SCHEMA_ID;
pub use lexical_generation_plan::LexicalGenerationPlan;
pub use lexical_generation_plan::LexicalOwnerFact;
pub use lexical_generation_plan::LexicalShardArtifact;
pub use lexical_generation_plan::LexicalShardDisposition;
pub use lexical_generation_plan::LexicalShardPlanEntry;
pub use lexical_generation_plan::plan_lexical_generation;
pub use lexical_overlay::LexicalOverlayCandidateHit;
pub use lexical_overlay::LexicalOverlayDocument;
pub use lexical_overlay::LexicalOverlaySearchHit;
pub use lexical_overlay::LexicalOverlaySearchRequest;
pub use lexical_overlay::search_lexical_overlay;
pub use lexical_overlay::search_lexical_overlay_candidates;
#[cfg(feature = "tantivy-accelerator")]
pub use merkle_search_generation::MerkleSearchGeneration;
#[cfg(feature = "tantivy-accelerator")]
pub use merkle_search_generation::SearchOwnerChange;
#[cfg(feature = "tantivy-accelerator")]
pub use merkle_search_generation::SearchOwnerFragment;
#[cfg(feature = "tantivy-accelerator")]
pub use merkle_search_generation::SearchProjectionIdentity;
#[cfg(feature = "tantivy-accelerator")]
pub use merkle_search_generation::search_owner_graph_fragment_digest;
#[cfg(feature = "tantivy-accelerator")]
pub use merkle_search_generation::search_projection_analyzer_digest;
pub use progressive_query::{
    ProgressiveQueryRequest, QueryOutputFormat, parse_progressive_query_args, query_output_format,
};
pub use provider_candidate_annotations::ProviderFactsEnvelope;
pub use provider_candidate_annotations::compact_provider_fact_nodes;
pub use provider_candidate_annotations::compact_provider_fact_value;
pub use provider_candidate_annotations::provider_candidate_annotation_nodes;
pub use provider_candidate_annotations::provider_facts_envelope_from_stdout;
pub use provider_candidate_annotations::provider_facts_envelope_from_value;
#[cfg(feature = "tantivy-accelerator")]
pub use resident_byte_coverage::RESIDENT_BYTE_GRAM_WIDTH;
#[cfg(feature = "tantivy-accelerator")]
pub use resident_byte_coverage::ResidentByteCoverageIndex;
#[cfg(feature = "tantivy-accelerator")]
pub use resident_byte_coverage::ResidentByteCoverageInput;
#[cfg(feature = "tantivy-accelerator")]
pub use resident_byte_coverage::ResidentByteCoverageOwner;
#[cfg(feature = "tantivy-accelerator")]
pub use resident_byte_coverage::ResidentByteCoverageQueryReceipt;
#[cfg(feature = "tantivy-accelerator")]
pub use resident_byte_coverage::ResidentByteCoverageStats;
pub use resident_graph_search::ResidentGraphBuildMetrics;
pub use resident_graph_search::ResidentGraphEvaluatedEdge;
pub use resident_graph_search::ResidentGraphEvaluation;
pub use resident_graph_search::ResidentGraphEvaluationBudget;
pub use resident_graph_search::ResidentGraphEvaluationRequest;
pub use resident_graph_search::ResidentGraphGeneration;
pub use resident_graph_search::ResidentGraphRankedNode;
pub use resident_graph_search::ResidentGraphRelationDirection;
pub use resident_graph_search::ResidentGraphRelationPattern;
pub use resident_graph_search::ResidentGraphSearchBudget;
pub use resident_graph_search::ResidentGraphSearchRequest;
pub use resident_graph_search::ResidentGraphSearchStage;
pub use resident_graph_search::ResidentGraphSearchWork;
pub use resident_graph_search::build_resident_graph_generation;
pub use resident_graph_search::canonical_resident_graph_generation_digest;
pub use resident_graph_search::evaluate_resident_graph_generation;
pub use resident_graph_search::evaluate_resident_graph_relation_patterns;
pub use resident_graph_search::open_resident_graph_generation;
pub use resident_graph_search::rank_resident_graph_generation;
#[cfg(feature = "tantivy-accelerator")]
pub use resident_grep_candidate_plan::{
    ResidentGrepCandidatePlan, build_resident_grep_candidate_plan,
};
pub use resident_grep_corpus::RESIDENT_GREP_CORPUS_RECEIPT_SCHEMA_ID;
pub use resident_grep_corpus::ResidentGrepCorpusArtifact;
pub use resident_grep_corpus::ResidentGrepCorpusOwner;
pub use resident_grep_corpus::ResidentGrepCorpusReceipt;
pub use resident_grep_corpus::ResidentGrepMappedCorpusOwner;
pub use resident_grep_corpus::ResidentGrepOwnerSpan;
pub use resident_grep_corpus::build_resident_grep_corpus;
pub use resident_grep_corpus::open_mapped_resident_grep_corpus;
pub use resident_grep_corpus::owner_for_corpus_line;
#[cfg(feature = "tantivy-accelerator")]
pub use resident_source_index::ResidentIndexBuildResources;
#[cfg(feature = "tantivy-accelerator")]
pub use resident_source_index::ResidentIndexBuildStrategy;
#[cfg(feature = "tantivy-accelerator")]
pub use resident_source_index::ResidentLexicalCoverageInput;
#[cfg(feature = "tantivy-accelerator")]
pub use resident_source_index::ResidentSearchAuthority;
#[cfg(feature = "tantivy-accelerator")]
pub use resident_source_index::ResidentSourceDocument;
#[cfg(feature = "tantivy-accelerator")]
pub use resident_source_index::ResidentSourceIndex;
#[cfg(feature = "tantivy-accelerator")]
pub use resident_source_index::resident_index_engine_digest;
#[cfg(feature = "tantivy-accelerator")]
pub use resident_source_index::resident_lexical_coverage_batch;
#[cfg(feature = "tantivy-accelerator")]
pub use resident_source_index::resident_lexical_coverage_keys;
#[cfg(feature = "tantivy-accelerator")]
pub use resident_source_index::resident_navigation_keys;
pub use runtime_search_receipt::RUNTIME_SEARCH_SELECTOR_OWNER_LIMIT;
pub use runtime_search_receipt::RUNTIME_SEARCH_SOURCE_CAPACITY;
pub use runtime_search_receipt::RUNTIME_SEARCH_SOURCE_LIMIT;
pub use runtime_search_receipt::RuntimeSearchResult;
pub use runtime_search_receipt::RuntimeSearchSource;
pub use runtime_search_receipt::bounded_ranked_selector_owner_paths;
pub use runtime_search_receipt::bounded_runtime_search_source;
pub use runtime_search_receipt::build_runtime_provider_search_receipt;
pub use runtime_search_receipt::build_runtime_provider_search_receipt_with_graph;
pub use scheme_playbook_source::{
    DEFAULT_SEARCH_PLAYBOOK_SOURCE, admit_default_search_playbook_source,
    admit_search_playbook_source,
};
pub use search_candidate::FieldHit;
pub use search_candidate::RankFeature;
pub use search_candidate::RankedSearchCandidate;
pub use search_candidate::SearchCandidate;
pub use search_candidate::SearchCandidateMergeReceipt;
pub use search_candidate::SearchStageReceipt;
pub use search_candidate::StructuralIndexSearchHit;
pub use search_candidate::lexical_overlay_hit_to_search_candidate;
pub use search_candidate::merge_search_candidates;
pub use search_candidate::merge_search_candidates_with_receipt;
pub use search_candidate::search_candidate_has_executable_line_identity;
pub use search_candidate::source_index_candidate_to_search_candidate;
pub use search_candidate::structural_index_hit_to_search_candidate;
pub use search_generation_segment::SearchGenerationSection;
pub use search_generation_segment::SearchGenerationSectionKind;
pub use search_generation_segment::SearchGenerationSectionRepresentation;
pub use search_generation_segment::ValidatedSearchGenerationSegment;
pub use search_generation_segment::encode_search_generation_segment;
pub use search_language_files::LanguageFileSpec;
pub use search_language_files::language_file_spec;
pub use search_language_files::language_neutral_search_file_spec;
pub use sorted_record_table::ValidatedSortedRecordTable;
pub use sorted_record_table::encode_sorted_record_table;
pub use source_index_rank::SourceIndexRankCandidate;
pub use source_index_rank::rank_source_index_candidates;
pub use source_index_rank::reorder_source_index_candidates;
pub use source_index_rank::source_index_lookup_terms;
pub use syntax_query_replay::SyntaxQueryReplayCapture;
pub use syntax_query_replay::SyntaxQueryRowsReplay;
pub use syntax_query_replay::render_semantic_tree_sitter_query_rows_stdout;
pub use syntax_query_replay::render_semantic_tree_sitter_query_stdout;
pub use workspace_playbook_plan::NormalizedWorkspaceSearchPlaybookRequest;
pub use workspace_playbook_plan::WorkspaceSearchPlanBinding;
pub use workspace_playbook_plan::WorkspaceSearchPlaybookPlan;
pub use workspace_playbook_plan::WorkspaceSearchPlaybookRoute;
pub use workspace_playbook_plan::WorkspaceSearchProducerAxis;
pub use workspace_playbook_plan::WorkspaceSearchProgressivePlan;
pub use workspace_playbook_plan::WorkspaceSearchProvider;
pub use workspace_playbook_plan::build_workspace_search_playbook_plan;
pub use workspace_playbook_result::{
    WORKSPACE_SEARCH_PLAYBOOK_RESULT_SCHEMA_ID, WorkspaceSearchAxisKind,
    WorkspaceSearchClauseReceipt, WorkspaceSearchGraphFanIn, WorkspaceSearchHitProjection,
    WorkspaceSearchPlaybookEvidence, WorkspaceSearchPlaybookResult,
    WorkspaceSearchPlaybookResultKind, WorkspaceSearchProgressiveExecutionWitness,
    WorkspaceSearchSyntaxCandidate, build_workspace_search_playbook_result,
    synthesize_workspace_search_playbook_result,
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
pub use exact_selector_fixture_memory::ExactSelectorFixtureArtifactV1;
pub use exact_selector_fixture_memory::ExactSelectorFixtureBackendV1;
pub use exact_selector_fixture_memory::ExactSelectorFixtureFileBackendV1;
pub use exact_selector_fixture_memory::ExactSelectorFixtureProjectionV1;
pub use exact_selector_fixture_memory::ExactSelectorFixtureResidentV1;
pub use exact_selector_fixture_memory::exact_selector_fixture_lookup_v1;
pub use exact_selector_fixture_memory::exact_selector_fixture_projection_range_v1;
pub use exact_selector_fixture_memory::exact_selector_fixture_projection_v1;
pub mod exact_selector_fixture_publication;
pub use exact_selector_fixture_publication::EXACT_SELECTOR_FIXTURE_ARTIFACT_KIND;
pub use exact_selector_fixture_publication::ExactSelectorFixturePublicationReceiptV1;
pub use exact_selector_fixture_publication::build_exact_selector_fixture_from_projection_records_v1;
pub use exact_selector_fixture_publication::publish_exact_selector_fixture_v1;
