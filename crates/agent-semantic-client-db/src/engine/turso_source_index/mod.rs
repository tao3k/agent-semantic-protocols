mod canonical;
pub(in crate::engine) mod core;
pub(in crate::engine) mod generation_snapshot;
pub use generation_snapshot::{
    ClientDbSourceIndexGenerationOwnerV1, ClientDbSourceIndexGenerationSnapshotV1,
    ClientDbSourceIndexSelectorFactV1, latest_turso_source_index_generation_snapshot,
};
mod facts;
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
mod trace;

pub(in crate::engine) use core::bootstrap_turso_source_index_schema;
pub(super) use core::turso_source_index_access_lock;
pub use core::{
    latest_turso_source_index_file_hashes, latest_turso_source_index_scope_files,
    latest_turso_source_index_stats, lookup_reusable_turso_source_index_generation,
    refresh_turso_source_index_import,
};
pub use provider_incremental::{
    ProviderIncrementalOwnerWriteV1, ProviderIncrementalScopeV1, ProviderIncrementalWriteReceiptV1,
    ProviderOwnerDecisionV1, ProviderOwnerFingerprintV1, ProviderOwnerMetadataV1,
    ProviderOwnerProbeV1, ProviderSelectorProjectionV1,
};
pub use provider_incremental_probe_batch::{
    ProviderOwnerBatchProbeReceiptV1, ProviderOwnerBatchProbeRequestV1,
    ProviderOwnerBatchProbeResultV1,
};
pub use provider_treesitter::{
    ProviderOwnerInventoryEntryStateV1, ProviderOwnerInventoryEntryV1,
    ProviderOwnerInventoryStateV1, ProviderOwnerInventoryV1, ProviderOwnerInventoryWriteReceiptV1,
    ProviderOwnerInventoryWriteV1, ProviderRemainingOwnerCountKindV1,
    ProviderTreeSitterCaptureProjectionV1, ProviderTreeSitterContinuationV1,
    ProviderTreeSitterOwnerResultStateV1, ProviderTreeSitterOwnerResultV1,
    ProviderTreeSitterOwnerWriteReceiptV1, ProviderTreeSitterQueryCountersV1,
    ProviderTreeSitterQueryIdentityV1, ProviderTreeSitterQueryReadStateV1,
    ProviderTreeSitterQueryReadV1, ProviderTreeSitterQueryReceiptV1,
};
mod transaction;
mod workspace_db_registry;

pub use workspace_db_registry::{
    ProviderSearchWorkspaceSessionV1, WorkspaceDbRegistry, WorkspaceDbRegistryCountersV1,
};
