//! Runtime Server workspace model interface grouped by its owning implementation modules.

mod core;
pub(crate) use core::validate_owners;
pub use core::{
    ExactProjectionKind, RUNTIME_MERKLE_OWNER_READ_RECEIPT_SCHEMA_ID,
    RUNTIME_SERVER_SHUTDOWN_RECEIPT_SCHEMA_ID, RuntimeDataPlaneCounters, RuntimeProjectionScope,
    RuntimeServerShutdownReceipt, WORKSPACE_GENERATION_DELTA_SCHEMA_ID,
    WORKSPACE_GENERATION_SCHEMA_ID, WORKSPACE_RECOVERY_RECEIPT_SCHEMA_ID,
    WORKSPACE_RUNTIME_SELECTOR_OVERLAY_RECEIPT_SCHEMA_ID, WorkspaceDataPlanePerformanceReceipt,
    WorkspaceDerivedProjectionSnapshot, WorkspaceGenerationBuild, WorkspaceGenerationDelta,
    WorkspaceGenerationSnapshot, WorkspaceGenerationState, WorkspaceMemoryGeneration,
    WorkspaceOwnerSnapshot, WorkspaceRecoveryReceipt, WorkspaceRecoverySource,
    WorkspaceRuntimeMerkleOwnerRead, WorkspaceRuntimeOwnerRead, WorkspaceRuntimeSelectorOverlay,
    WorkspaceRuntimeSelectorOverlayReceipt, WorkspaceRuntimeSelectorRead,
    WorkspaceRuntimeSelectorRebind, WorkspaceSelectorSnapshot,
};
