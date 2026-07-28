//! DB Engine backend facade, control adapter, and Turso backend boundary.

mod contract;
mod facade;
mod facade_turso_report;
mod search_facade;
mod session_facade;
mod source_index_candidate_projection;
mod source_index_candidate_selection;
mod source_index_candidate_types;
mod source_index_facade;
mod source_index_query_scoring;
mod turso;
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
pub use turso_source_index::{
    ClientDbSourceIndexGenerationOwnerV1, ClientDbSourceIndexGenerationSnapshotV1,
    ClientDbSourceIndexSelectorFactV1, ProviderIncrementalOwnerWriteV1, ProviderIncrementalScopeV1,
    ProviderIncrementalWriteReceiptV1, ProviderOwnerDecisionV1, ProviderOwnerFingerprintV1,
    ProviderOwnerInventoryEntryStateV1, ProviderOwnerInventoryEntryV1,
    ProviderOwnerInventoryStateV1, ProviderOwnerInventoryV1, ProviderOwnerInventoryWriteReceiptV1,
    ProviderOwnerInventoryWriteV1, ProviderOwnerMetadataV1, ProviderOwnerProbeV1,
    ProviderRemainingOwnerCountKindV1, ProviderSelectorProjectionV1,
    ProviderTreeSitterCaptureProjectionV1, ProviderTreeSitterContinuationV1,
    ProviderTreeSitterOwnerResultStateV1, ProviderTreeSitterOwnerResultV1,
    ProviderTreeSitterOwnerWriteReceiptV1, ProviderTreeSitterQueryCountersV1,
    ProviderTreeSitterQueryIdentityV1, ProviderTreeSitterQueryReceiptV1,
    latest_turso_source_index_generation_snapshot,
};
pub(crate) mod turso_statement;
mod turso_syntax;

pub use contract::{ClientDbBackend, ClientDbEngineDurability, ClientDbEngineFeatures};
pub use facade::{
    ClientDbEngine, ClientDbEngineReadSession, ClientDbEngineReport, ClientDbEngineWriteSession,
};
pub use facade::{
    ClientDbEngineSourceIndexReadModelReport, ClientDbEngineStructuralIndexReadModelReport,
};
pub use turso::TURSO_BOOTSTRAP_TABLE;
pub use turso::TursoClientDbEngineReport;
pub use turso_migration::{
    ClientDbTurso07ActiveMigration, ClientDbTurso07MigrationReport, ClientDbTurso07ReplayCoverage,
    ClientDbTurso07ReplayFamilyReceipt, ClientDbTurso07RetiredDerivedReceipt,
};
pub use turso_search::{
    TursoClientDbSearchDocument, TursoClientDbSearchHit, TursoClientDbSearchResult,
    TursoClientDbSearchState,
};
pub use turso_source_index::{
    ProviderOwnerBatchProbeReceiptV1, ProviderOwnerBatchProbeRequestV1,
    ProviderOwnerBatchProbeResultV1, ProviderTreeSitterQueryReadStateV1,
    ProviderTreeSitterQueryReadV1,
};
pub use turso_source_index::{
    ProviderSearchWorkspaceSessionV1, WorkspaceDbRegistry, WorkspaceDbRegistryCountersV1,
};
