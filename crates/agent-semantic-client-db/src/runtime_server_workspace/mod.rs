mod canonical_materialization;
mod client;
mod exact_segment;
mod lease;
mod memory_backend;
mod model;
mod owner_freshness;
pub use canonical_materialization::{
    WORKSPACE_CANONICAL_MATERIALIZATION_SCHEMA_ID, WorkspaceCanonicalMaterialization,
    WorkspaceCanonicalMaterializationLoad,
};
pub(crate) use memory_backend::WorkspaceMemoryBackend;
pub(crate) use model::{
    RUNTIME_SERVER_SHUTDOWN_RECEIPT_SCHEMA_ID, WORKSPACE_RECOVERY_RECEIPT_SCHEMA_ID,
    validate_owners,
};
pub use owner_freshness::{WorkspaceOwnerProjectionBuildFuture, WorkspaceOwnerProjectionBuilder};
pub(crate) use resident_overlay::{ResidentOverlaySnapshot, ResidentOverlayStore};
pub(crate) use selector_overlay::validate_projection_kind;
mod pointer;
mod registry;
mod resident_overlay;
mod resident_ready;
mod restore;
mod scope_path;
mod segment;
mod selector_overlay;
mod store;

pub use client::{WorkspaceGenerationDataPlaneClient, WorkspaceGenerationDataPlaneOpen};
pub use exact_segment::{
    WorkspaceExactProjectionDataPlaneClient, WorkspaceExactProjectionDataPlaneOpen,
};
pub use lease::WorkspaceGenerationLease;
pub use memory_backend::WorkspaceProjectionLease;
pub use model::{
    RuntimeDataPlaneCounters, RuntimeServerShutdownReceipt, WorkspaceDataPlanePerformanceReceipt,
    WorkspaceDerivedProjectionSnapshot, WorkspaceGenerationBuild, WorkspaceGenerationSnapshot,
    WorkspaceGenerationState, WorkspaceMemoryGeneration, WorkspaceOwnerSnapshot,
    WorkspaceRecoveryReceipt, WorkspaceRecoverySource, WorkspaceRuntimeOwnerFreshnessReceipt,
    WorkspaceRuntimeSelectorOverlay, WorkspaceRuntimeSelectorOverlayReceipt,
    WorkspaceRuntimeSelectorRead, WorkspaceSelectorSnapshot,
};
pub use pointer::WorkspaceGenerationPointerReader;
pub use registry::RuntimeServerWorkspaceRegistry;
pub use restore::restore_active_turso_generation;
pub use scope_path::{workspace_generation_directory, workspace_generation_pointer_path};
pub use segment::{MappedWorkspaceGeneration, WorkspaceGenerationPublisher};
pub use store::prepare_runtime_server_workspace_store;
