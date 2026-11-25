#![deny(dead_code)]
// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Converged sub-millisecond pure-Rust search engine for ASP and Marlin.

pub mod converged_resident_index;
pub mod file_locator;
mod graph_candidate_projection;
pub mod graph_generation_authority;
mod graph_node_projection;
mod graph_owner_rank;
mod graph_topology_projection;
pub mod marlin_engine;
pub mod marlin_playbook_executor;
pub mod marlin_storage_port;
mod source_index_rank;
#[cfg(feature = "tantivy-accelerator")]
mod tantivy_ram_engine;
pub mod trigram_query;
pub mod trigram_store;

pub use agent_semantic_search_playbook::{
    GraphNativeBlock, NativeRgArgvAnalysis, NativeRgOutputAttribution, ProducerNativeBlock,
    ProgressiveSearchPlaybookError, ProgressiveSearchPlaybookRequest, SearchPlaybookClauseAxis,
    SearchPlaybookClauseRef, SearchPlaybookNormalizedComposition, SearchPlaybookPlanInput,
    SearchPlaybookProducerDeclaration, TantivyQueryAnalysis, TantivyQueryDiagnostic,
    TantivyQueryMetrics, admit_search_playbook_plan, analyze_native_rg_argv, analyze_tantivy_query,
    search_playbook_composition_abi_digest,
};
pub use converged_resident_index::{
    WorkspaceContextLine, WorkspaceDeltaReceipt, WorkspaceFileDelta, WorkspaceIndexConfig,
    WorkspaceSearchHit, WorkspaceSearchIndex, WorkspaceSearchQuery, WorkspaceSearchReport,
};
pub use graph_candidate_projection::{
    GraphCandidateHotNodesRequest, GraphCandidateItemNodesRequest, GraphProjectionCandidate,
    graph_candidate_hot_node_id, graph_candidate_hot_nodes, graph_candidate_item_node_id,
    graph_candidate_item_nodes,
};
pub use graph_node_projection::{owner_path_graph_nodes, stable_graph_node_id};
pub use graph_owner_rank::{
    GraphOwnerRankCandidate, GraphOwnerRankReport, GraphOwnerRankRequest, GraphOwnerRankScore,
    GraphOwnerRankedOwner, rank_graph_owner_report, ranked_graph_owner_paths_for_submodule_paths,
    ranked_graph_owner_paths_with_topology,
};
pub use graph_topology_projection::{
    GraphOwnerMissingTopologyRequest, GraphTopologyProjection, GraphTopologyProjectionRequest,
    graph_owner_missing_topology_projection, graph_path_is_under, graph_project_submodule_paths,
    graph_project_submodule_paths_from_content, graph_project_topology_projection,
    graph_submodule_owner_edges,
};
pub use marlin_engine::{DEFAULT_SEARCH_GENERATION_POINTER_KEY, MarlinConvergedSearchEngine};
pub use marlin_playbook_executor::{
    ConvergedLineMatch, ConvergedSearchExecutionError, ConvergedSearchExecutionReport,
    ConvergedSearchHit, execute_admitted_playbook,
};
pub use marlin_storage_port::{
    InMemorySearchStoragePort, SearchArtifactBlob, SearchArtifactHash, SearchEngineStoragePort,
    SearchGenerationDigest, SearchGenerationPointerRecord, SearchGenerationPointerSwapRequest,
    SearchPointerKey, SearchProjectId, SearchStoragePortError, SearchTopologyEdgeId,
    SearchTopologyEdgeRecord, SearchWorkspaceId,
};
pub use source_index_rank::{
    SourceIndexRankCandidate, SourceIndexRankReport, SourceIndexRankRequest, SourceIndexRankScore,
    SourceIndexRankedCandidate, rank_source_index_candidates, rank_source_index_report,
    reorder_source_index_candidates, source_index_lookup_terms,
};
pub use trigram_query::{TrigramQuery, compile_regex_trigram_query};
pub use trigram_store::{
    SnapshotFileRecord, TrigramLookupEntry, TrigramPosting, TrigramSnapshotError,
};

#[cfg(test)]
extern crate self as agent_semantic_search;

#[cfg(test)]
#[path = "../tests/unit/graph_candidate_projection.rs"]
mod graph_candidate_projection_tests;
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
#[path = "../tests/unit/marlin_converged_engine.rs"]
mod marlin_converged_engine_tests;
#[cfg(test)]
#[path = "../tests/unit/source_index_rank.rs"]
mod source_index_rank_tests;
#[cfg(test)]
#[path = "../tests/unit/source_snapshot_fixture.rs"]
mod source_snapshot_fixture;
