mod canonical_materialization;
mod client;
mod context;
mod durability;
mod exact_segment;
mod lease;
mod memory_backend;
mod model;
mod owner_content_identity;
mod owner_identity_journal;
pub use canonical_materialization::{
    ValidatedWorkspaceCanonicalMaterialization, WORKSPACE_CANONICAL_MATERIALIZATION_SCHEMA_ID,
    WorkspaceCanonicalMaterialization, WorkspaceCanonicalMaterializationLoad,
};

pub(crate) use memory_backend::WorkspaceMemoryBackend;
pub(crate) use model::{
    RUNTIME_SERVER_SHUTDOWN_RECEIPT_SCHEMA_ID, WORKSPACE_RECOVERY_RECEIPT_SCHEMA_ID,
    validate_owners,
};
pub(crate) use resident_overlay::{ResidentOverlaySnapshot, ResidentOverlayStore};
mod pointer;
pub(crate) use pointer::ACTIVE_WORKSPACE_GENERATION_REQUIRED;
mod registry;
mod resident_overlay;
mod resident_ready;
mod restore;
mod scope_path;
mod search_generation_authority;
mod search_index_projection;
mod segment;
mod selector_overlay;
mod store;

pub use client::{
    WorkspaceGenerationDataPlaneCacheReceipt, WorkspaceGenerationDataPlaneClient,
    WorkspaceGenerationDataPlaneOpen,
};
pub use context::WorkspaceRuntimeContext;
pub use durability::{
    WORKSPACE_GENERATION_DURABILITY_RECEIPT_SCHEMA_ID, WorkspaceGenerationDurabilityReceipt,
    WorkspaceGenerationDurabilityState,
};
pub use exact_segment::{
    WorkspaceExactProjectionDataPlaneClient, WorkspaceExactProjectionDataPlaneOpen,
};
pub use lease::WorkspaceGenerationLease;
pub use memory_backend::WorkspaceProjectionLease;
pub use model::{
    ExactProjectionKind, RUNTIME_MERKLE_OWNER_READ_RECEIPT_SCHEMA_ID, RuntimeDataPlaneCounters,
    RuntimeProjectionScope, RuntimeServerShutdownReceipt, WORKSPACE_GENERATION_DELTA_SCHEMA_ID,
    WorkspaceDataPlanePerformanceReceipt, WorkspaceDerivedProjectionSnapshot,
    WorkspaceGenerationBuild, WorkspaceGenerationDelta, WorkspaceGenerationSnapshot,
    WorkspaceGenerationState, WorkspaceMemoryGeneration, WorkspaceOwnerSnapshot,
    WorkspaceRecoveryReceipt, WorkspaceRecoverySource, WorkspaceRuntimeMerkleOwnerRead,
    WorkspaceRuntimeOwnerRead, WorkspaceRuntimeSelectorOverlay,
    WorkspaceRuntimeSelectorOverlayReceipt, WorkspaceRuntimeSelectorRead,
    WorkspaceRuntimeSelectorRebind, WorkspaceSelectorSnapshot,
};
pub use pointer::WorkspaceGenerationPointerReader;
pub use registry::{PublishedWorkspaceGenerationState, RuntimeServerWorkspaceRegistry};
pub use restore::restore_active_turso_generation;
pub use scope_path::{workspace_generation_directory, workspace_generation_pointer_path};
pub use search_generation_authority::WorkspaceSearchGenerationAuthority;
#[doc(hidden)]
pub use search_generation_authority::read_search_generation_authority_fixture;
pub(crate) use search_generation_authority::{
    WorkspaceSearchGenerationAuthorityPublisher, WorkspaceSearchGenerationAuthorityReader,
    publish_search_generation_authority_segment, workspace_search_generation_authority_channel,
};
pub use search_index_projection::{
    WorkspaceSearchGenerationDataPlaneClient, encode_workspace_search_generation_segment,
    workspace_search_generation_segment_path,
};
pub use agent_semantic_search::{
    SearchGenerationSection, SearchGenerationSectionKind, SearchGenerationSectionRepresentation,
    ValidatedSearchGenerationSegment, encode_search_generation_segment,
    ValidatedSortedRecordTable, encode_sorted_record_table,
};
pub use segment::{MappedWorkspaceGeneration, WorkspaceGenerationPublisher};
pub use store::{
    RuntimeServerWorkspaceStore, prepare_runtime_server_workspace_store,
    prepare_runtime_server_workspace_store_at_root,
};
mod retirement;
pub use retirement::{
    RESIDENT_WORKSPACE_RETIREMENT_RECEIPT_SCHEMA_ID, ResidentWorkspaceRetirementReason,
    ResidentWorkspaceRetirementReceipt,
};
mod atomic_snapshot_pointer;
#[cfg(test)]
#[path = "../../tests/unit/runtime_server_search_index_cold_open.rs"]
mod runtime_server_search_index_cold_open;
