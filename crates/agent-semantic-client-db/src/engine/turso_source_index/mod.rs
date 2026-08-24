mod active_generation;
mod canonical;
mod contract;
pub(in crate::engine) mod core;
pub(in crate::engine) mod generation_snapshot;
pub use active_generation::{
    ClientDbActiveSourceIndexGeneration, active_turso_source_index_generation,
};
pub(in crate::engine) use contract::TURSO_SOURCE_INDEX_TERM_PROJECTION_VERSION;
pub use generation_snapshot::{
    ClientDbActiveGenerationSourceBlob, ClientDbActiveGenerationSourceBlobs,
    ClientDbSourceIndexGenerationOwner, ClientDbSourceIndexGenerationRelation,
    ClientDbSourceIndexGenerationSnapshot, ClientDbSourceIndexSelectorFact,
    active_turso_source_index_generation_blobs, latest_turso_source_index_generation_snapshot,
};
mod facts;
mod generation_clone;
mod materialization;
mod membership;
mod prepare;
mod projection;
mod provider_incremental;
mod provider_incremental_probe_batch;
mod provider_incremental_schema;
mod provider_treesitter;
mod provider_treesitter_read;
mod provider_treesitter_write;
mod publish;
mod readiness;
mod relation;
mod resident_selector;
mod schema;
mod selector_identity;
mod source_blob;
mod trace;

pub(super) use core::turso_source_index_access_lock;
pub use core::{
    latest_turso_source_index_file_hashes, latest_turso_source_index_scope_files,
    latest_turso_source_index_stats, lookup_reusable_turso_source_index_generation,
};
pub use provider_incremental::{
    ProviderIncrementalOwnerSnapshot, ProviderIncrementalOwnerWrite, ProviderIncrementalScoped,
    ProviderIncrementalWriteReceipt, ProviderOwnerDecision, ProviderOwnerFingerprint,
    ProviderOwnerMetadata, ProviderOwnerProbe, ProviderSelectorProjection,
};
pub use provider_incremental_probe_batch::{
    ProviderOwnerBatchProbeReceipt, ProviderOwnerBatchProbeRequest, ProviderOwnerBatchProbeResult,
};
pub use provider_treesitter::{
    ProviderOwnerInventory, ProviderOwnerInventoryEntry, ProviderOwnerInventoryEntryState,
    ProviderOwnerInventoryState, ProviderOwnerInventoryWrite, ProviderOwnerInventoryWriteReceipt,
    ProviderRemainingOwnerCountKind, ProviderTreeSitterCaptureProjection,
    ProviderTreeSitterContinuation, ProviderTreeSitterOwnerResult,
    ProviderTreeSitterOwnerResultState, ProviderTreeSitterOwnerWriteReceipt,
    ProviderTreeSitterQueryCounters, ProviderTreeSitterQueryIdentity, ProviderTreeSitterQueryRead,
    ProviderTreeSitterQueryReadState, ProviderTreeSitterQueryReceipt,
};
pub use resident_selector::{
    TursoResidentSelectorCandidate, TursoResidentSelectorQuery, TursoResidentSelectorRead,
};
pub(in crate::engine) use schema::bootstrap_turso_source_index_schema;
mod transaction;
mod workspace_db_owner;
#[cfg(test)]
#[path = "../../../tests/unit/workspace_db_owner.rs"]
mod workspace_db_owner_tests;
mod workspace_db_registry;

#[cfg(test)]
#[path = "../../../tests/unit/workspace_db_mvcc_capability.rs"]
mod tests;

pub use workspace_db_registry::{
    ProviderSearchWorkspaceSession, WorkspaceDbRegistry, WorkspaceDbRegistryCounters,
    WorkspaceDbWriteFinishMode, WorkspaceDbWriteFinishReceipt,
};
