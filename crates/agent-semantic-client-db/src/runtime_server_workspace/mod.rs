mod canonical_materialization;
mod canonical_snapshot;
mod client;
mod context;
mod durability;
mod exact_segment;
mod lease;
mod memory_backend;
mod model;
mod owner_content_identity;
mod owner_identity_journal;
mod owner_search_admission;
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

pub use agent_semantic_search::{
    SearchGenerationSection, SearchGenerationSectionKind, SearchGenerationSectionRepresentation,
    ValidatedSearchGenerationSegment, ValidatedSortedRecordTable, encode_search_generation_segment,
    encode_sorted_record_table,
};
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
    WorkspaceGenerationState, WorkspaceMemoryGeneration, WorkspaceOwnerProjection,
    WorkspaceOwnerSearchSeedSnapshot, WorkspaceOwnerSearchSnapshot, WorkspaceOwnerSnapshot,
    WorkspaceRecoveryReceipt, WorkspaceRecoverySource, WorkspaceRuntimeMerkleOwnerRead,
    WorkspaceRuntimeOwnerRead, WorkspaceRuntimeOwnerSearchRead, WorkspaceRuntimeSelectorOverlay,
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

#[cfg(test)]
pub(crate) fn test_content_search_generation_receipt(
    project_id: &str,
    workspace_identity: &str,
    source_snapshot: &agent_semantic_content_identity::SourceSnapshotEvidence,
) -> agent_semantic_search::ContentSearchGenerationReceipt {
    let identity = agent_semantic_search::SearchGenerationIdentity {
        project_id: project_id.to_owned(),
        workspace_id: workspace_identity.to_owned(),
        source_root_digest: agent_semantic_search::canonical_blake3_digest(
            &source_snapshot.root_digest,
        )
        .expect("test source root digest"),
        provider_digest: agent_semantic_search::canonical_blake3_digest(
            &source_snapshot.provider_digest,
        )
        .expect("test provider digest"),
        schema_digest: agent_semantic_search::canonical_blake3_digest(
            &agent_semantic_content_identity::project_resolution_schema_digest(),
        )
        .expect("test schema digest"),
        generation_candidate_digest: format!("blake3-256:{}", "c".repeat(64)),
    };
    let stage =
        |stage, worker_id: &str, byte: u8| agent_semantic_search::SearchGenerationStageReceipt {
            stage,
            identity: identity.clone(),
            artifact_digest: format!("blake3-256:{}", char::from(byte).to_string().repeat(64)),
            worker_id: worker_id.to_owned(),
            complete: true,
        };
    agent_semantic_search::ContentSearchGenerationReceipt::new(stage(
        agent_semantic_search::SearchGenerationConstructionStage::SourceByteAcquisition,
        "test-source-byte-acquisition",
        b'd',
    ))
    .expect("test content search generation")
}
pub use search_index_projection::{
    RuntimeDerivedAttachmentBuildTiming, WorkspaceSearchGenerationDataPlaneClient,
    encode_workspace_search_generation_segment, workspace_search_generation_segment_path,
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
pub mod content_binding;
