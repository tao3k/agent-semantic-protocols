mod canonical_materialization;
mod client;
mod model;
pub use canonical_materialization::{
    WORKSPACE_CANONICAL_MATERIALIZATION_SCHEMA_ID, WorkspaceCanonicalMaterialization,
};
use model::validate_owners;
mod pointer;
mod registry;
mod restore;
mod segment;

pub use client::{WorkspaceGenerationDataPlaneClient, WorkspaceGenerationDataPlaneOpen};
pub use model::{
    RuntimeDataPlaneCounters, RuntimeServerShutdownReceipt, WorkspaceDataPlanePerformanceReceipt,
    WorkspaceGenerationSnapshot, WorkspaceGenerationState, WorkspaceMemoryGeneration,
    WorkspaceOwnerSnapshot, WorkspaceProjectionLease, WorkspaceRecoveryReceipt,
    WorkspaceRecoverySource, WorkspaceSelectorSnapshot,
};
pub use pointer::WorkspaceGenerationPointerReader;
pub use registry::{RuntimeServerWorkspaceRegistry, WorkspaceGenerationLease};
pub use restore::restore_active_turso_generation;
pub use segment::{MappedWorkspaceGeneration, WorkspaceGenerationPublisher};
