#![deny(dead_code)]
// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Converged sub-millisecond pure-Rust search engine for ASP and Marlin.

pub mod converged_resident_index;
pub mod file_locator;
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
    SEMANTIC_OPEN_EDB_SCHEMA_ID, SEMANTIC_OPEN_EDB_SCHEMA_VERSION, WorkspaceContextLine,
    WorkspaceDeltaReceipt, WorkspaceEdbAttr, WorkspaceEdbEdgeFact, WorkspaceEdbEntityFact,
    WorkspaceEdbFileFact, WorkspaceEdbPrefilter, WorkspaceEdbSnapshot, WorkspaceFileDelta,
    WorkspaceIndexConfig, WorkspaceOpenEdbIngestBatch, WorkspaceOpenEdbIngestReceipt,
    WorkspaceSearchHit, WorkspaceSearchIndex, WorkspaceSearchQuery, WorkspaceSearchReport,
    WorkspaceSymbolKind, WorkspaceSymbolMatch,
};
pub use marlin_engine::{DEFAULT_SEARCH_GENERATION_POINTER_KEY, MarlinConvergedSearchEngine};
pub use marlin_playbook_executor::{
    ConvergedLineMatch, ConvergedSearchExecutionError, ConvergedSearchExecutionReport,
    ConvergedSearchHit, execute_admitted_playbook, execute_admitted_playbook_with_index,
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
#[path = "../tests/unit/marlin_converged_engine.rs"]
mod marlin_converged_engine_tests;
#[cfg(test)]
#[path = "../tests/unit/source_index_rank.rs"]
mod source_index_rank_tests;
