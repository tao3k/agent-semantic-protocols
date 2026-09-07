// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Runtime Server workspace model interface grouped by its owning implementation modules.

mod core;
mod mutation;
mod owner;
pub(crate) use core::validate_owners;
pub use core::{
    ExactProjectionKind, RUNTIME_MERKLE_OWNER_READ_RECEIPT_SCHEMA_ID,
    RUNTIME_SERVER_SHUTDOWN_RECEIPT_SCHEMA_ID, RuntimeDataPlaneCounters, RuntimeProjectionScope,
    RuntimeServerShutdownReceipt, WORKSPACE_GENERATION_SCHEMA_ID,
    WORKSPACE_RECOVERY_RECEIPT_SCHEMA_ID, WORKSPACE_RUNTIME_SELECTOR_OVERLAY_RECEIPT_SCHEMA_ID,
    WorkspaceDataPlanePerformanceReceipt, WorkspaceGenerationBuild, WorkspaceGenerationSnapshot,
    WorkspaceGenerationState, WorkspaceMemoryGeneration, WorkspaceRecoveryReceipt,
    WorkspaceRecoverySource, WorkspaceRuntimeMerkleOwnerRead, WorkspaceRuntimeOwnerRead,
};
pub use mutation::{
    WORKSPACE_GENERATION_DELTA_SCHEMA_ID, WorkspaceGenerationDelta,
    WorkspaceRuntimeSelectorOverlay, WorkspaceRuntimeSelectorOverlayReceipt,
    WorkspaceRuntimeSelectorRead, WorkspaceRuntimeSelectorRebind,
};
pub use owner::{
    WorkspaceDerivedProjectionSnapshot, WorkspaceOwnerProjection, WorkspaceOwnerSearchSeedSnapshot,
    WorkspaceOwnerSearchSnapshot, WorkspaceOwnerSnapshot, WorkspaceRuntimeOwnerSearchRead,
    WorkspaceSelectorSnapshot, WorkspaceTopologySourceSegment,
};
