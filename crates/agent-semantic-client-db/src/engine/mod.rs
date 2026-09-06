// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

//! DB Engine backend facade, control adapter, and Turso backend boundary.

mod contract;
pub(crate) mod facade;
mod facade_turso_report;
mod search_facade;
mod session_control_plane;
mod session_facade;
mod source_index_candidate_projection;
mod source_index_candidate_selection;
mod source_index_candidate_types;
mod source_index_facade;
mod source_index_query_scoring;
mod turso;
use turso::connect_turso_client_db;
mod turso_artifact;
mod turso_artifact_graph;
mod turso_batch;
mod turso_bootstrap;
mod turso_cache;
mod turso_cache_key;
mod turso_legacy_migration;
pub(crate) mod turso_lock_policy;
mod turso_migration;
mod turso_provider_command;
#[cfg(test)]
#[path = "../../tests/unit/db/engine/turso_schema_lifecycle_private.rs"]
mod turso_schema_lifecycle_tests;
mod turso_search;
mod turso_source_index;
pub(crate) use turso_source_index::active_turso_workspace_generation_materialization;
pub use turso_source_index::{
    ClientDbActiveGenerationSourceBlob, ClientDbActiveGenerationSourceBlobs,
    ClientDbActiveSourceIndexGeneration, ClientDbSourceIndexGenerationOwner,
    ClientDbSourceIndexGenerationRelation, ClientDbSourceIndexGenerationSnapshot,
    ClientDbSourceIndexSelectorFact, ProviderIncrementalOwnerSnapshot,
    ProviderIncrementalOwnerWrite, ProviderIncrementalScoped, ProviderIncrementalWriteReceipt,
    ProviderOwnerDecision, ProviderOwnerFingerprint, ProviderOwnerInventory,
    ProviderOwnerInventoryEntry, ProviderOwnerInventoryEntryState, ProviderOwnerInventoryState,
    ProviderOwnerInventoryWrite, ProviderOwnerInventoryWriteReceipt, ProviderOwnerMetadata,
    ProviderOwnerProbe, ProviderRemainingOwnerCountKind, ProviderSelectorProjection,
    ProviderTreeSitterCaptureProjection, ProviderTreeSitterContinuation,
    ProviderTreeSitterOwnerResult, ProviderTreeSitterOwnerResultState,
    ProviderTreeSitterOwnerWriteReceipt, ProviderTreeSitterQueryCounters,
    ProviderTreeSitterQueryIdentity, ProviderTreeSitterQueryReceipt,
    active_turso_source_index_generation_blobs, latest_turso_source_index_generation_snapshot,
};
pub(crate) mod turso_statement;
mod turso_syntax;

pub use contract::{ClientDbBackend, ClientDbEngineDurability, ClientDbEngineFeatures};
pub use facade::ClientDbEngineSourceIndexReadModelReport;
pub use facade::{
    ClientDbEngine, ClientDbEngineReadSession, ClientDbEngineReport, ClientDbEngineWriteSession,
};
pub use session_control_plane::{
    SessionControlPlaneAgentRegistration, SessionControlPlaneDelegationProposal,
    SessionControlPlaneRuntime, SessionControlPlaneRuntimeMetricsSnapshot,
    SessionControlPlaneRuntimeRegistry, SessionControlPlaneSnapshot,
    SessionControlPlaneTransactionReceipt,
};
pub use turso::TURSO_BOOTSTRAP_TABLE;
pub use turso::TursoClientDbEngineReport;
pub(crate) use turso::shared_turso_database;
pub use turso_migration::{
    ClientDbTurso07ActiveMigration, ClientDbTurso07MigrationReport, ClientDbTurso07ReplayCoverage,
    ClientDbTurso07ReplayFamilyReceipt, ClientDbTurso07RetiredDerivedReceipt,
};
pub use turso_search::{
    TursoClientDbSearchDocument, TursoClientDbSearchHit, TursoClientDbSearchResult,
    TursoClientDbSearchState,
};
pub use turso_source_index::{
    ProviderOwnerBatchProbeReceipt, ProviderOwnerBatchProbeRequest, ProviderOwnerBatchProbeResult,
    ProviderTreeSitterQueryRead, ProviderTreeSitterQueryReadState,
    active_turso_source_index_generation,
};
pub use turso_source_index::{
    ProviderSearchWorkspaceSession, TursoResidentSelectorCandidate, TursoResidentSelectorQuery,
    TursoResidentSelectorRead, WorkspaceDbRegistry, WorkspaceDbRegistryCounters,
    WorkspaceDbWriteFinishMode, WorkspaceDbWriteFinishReceipt,
};
