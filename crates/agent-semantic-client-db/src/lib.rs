#![deny(dead_code)]

//! ASP Server-owned DB Engine facade and in-process control adapters.

pub mod agent_session_registry;
mod artifact_event_builder;
pub use agent_session_registry::{
    AgentSessionModelObservationRef, AgentSessionModelObservationSource,
};
pub mod artifact_pointer_store;
pub mod codex_multi_agent_control_plane_owner;
pub mod context_run_mvcc;
mod dependency_index;
pub mod engine;
pub use engine::{
    SessionControlPlaneAgentRegistration, SessionControlPlaneDelegationProposal,
    SessionControlPlaneRuntime, SessionControlPlaneRuntimeMetricsSnapshot,
    SessionControlPlaneRuntimeRegistry, SessionControlPlaneSnapshot,
    SessionControlPlaneTransactionReceipt,
};
pub mod active_generation_projection_capability;
pub mod graph_turbo_cache;
pub mod parser_read_authority;
mod runtime_concurrency;
pub mod runtime_generation_cancellation;
pub mod runtime_merkle_owner_proof_qualification;
pub mod runtime_provider_catalog;
pub mod runtime_provider_register;
pub mod runtime_resident_read;
pub mod runtime_search_service;
pub mod runtime_server;
pub mod runtime_server_admission;
mod runtime_server_admission_builder_supervisor;
pub mod runtime_server_admission_catalog;
mod runtime_server_agent_control_plane;
mod runtime_server_agent_session_status;
pub mod runtime_server_control;
pub use runtime_server_control::call_runtime_server_for_state_home;
pub mod runtime_server_lifecycle;
pub mod runtime_server_lifecycle_coordinator;
pub mod runtime_server_owner_receipt;
pub mod runtime_server_publication;
pub mod runtime_server_supervisor;
pub mod workspace_generation_qualification;
pub use runtime_server_owner_receipt::{
    RUNTIME_SERVER_OWNER_SPAWN_SCHEMA_ID, RUNTIME_SERVER_OWNER_SPAWN_SCHEMA_VERSION,
    RuntimeServerActivationReadyReceipt, RuntimeServerDrainReceipt, RuntimeServerExitReceipt,
    RuntimeServerResidentTransactionReceipt, RuntimeServerSpawnReceipt,
    RuntimeServerSpawnReceiptRead, StaleRuntimeServerSpawnReceipt,
};
mod runtime_server_asp_python_graphs_status;
pub mod runtime_server_diagnostics;
mod runtime_server_generation_admission;
pub mod runtime_server_health;
pub mod runtime_server_hook_admission_locator;
pub mod runtime_server_observability;
pub mod runtime_server_opentelemetry;
pub mod runtime_server_runtime;
pub mod runtime_server_workspace;
pub mod runtime_telemetry_bus;
pub mod search_incident;
pub mod seqlock_json_memory;
pub mod server_source_index;
mod source_index;
pub mod storage_contract;
pub mod storage_performance_receipt;
mod syntax_query;
pub mod turso_agent_storage;
pub mod turso_cdc_storage;
pub mod turso_encrypted_storage;
mod turso_mvcc_keyset;
pub mod workspace_project_resolution;
pub use runtime_server_control::{
    AgentSessionControlPlaneState, RuntimeServerAgentSessionLifecycleState,
    RuntimeServerAgentSessionStatus, RuntimeServerControlReceipt, RuntimeServerEndpoint,
    RuntimeServerOperation, acquire_runtime_server_election, call_runtime_server,
    prepare_runtime_server_endpoint, prepare_runtime_server_endpoint_with_workspace_store,
    prepare_runtime_server_endpoint_with_workspace_store_and_identity,
    publish_runtime_server_endpoint, read_runtime_server_agent_sessions,
    read_runtime_server_endpoint, resolve_runtime_server_agent_session_status,
    runtime_server_endpoint_path, runtime_server_runtime_base, wait_for_runtime_server_election,
};
pub use turso_mvcc_keyset::{
    TursoMvccEventId, TursoMvccPageCursor, TursoMvccPageLimit, TursoMvccPartitionKey,
};
pub mod turso_mvcc_partition;
mod turso_mvcc_partition_sql;
pub mod turso_mvcc_store;
pub mod turso_sync_storage;
mod types;
pub mod workspace_db_ipc;
pub use types::ClientDbProviderCommandSelectionInput;
pub use workspace_db_ipc::WorkspaceDbIpcSession;

pub use agent_semantic_client_core::ClientDbStatus;
pub use agent_session_registry::{
    AGENT_SESSION_REGISTRY_DB_NAME, AGENT_SESSION_STATUS_ACTIVE, AGENT_SESSION_STATUS_ARCHIVED,
    AGENT_SESSION_STATUS_IDLE, AGENT_SESSION_STATUS_INVALID, AgentSessionDispatchClaimRequest,
    CollaborationAgentObservation, CollaborationSnapshotPersistenceReceipt,
    run_collaboration_snapshot_inbox,
    AgentSessionDispatchClaimResult, AgentSessionDispatchCompleteRequest,
    AgentSessionDispatchLeaseRecord, AgentSessionLookupRequest, AgentSessionRecord,
    AgentSessionRegisterRequest, AgentSessionRegistry, AgentSessionResidentName,
    AgentSessionRootSessionId, AgentSessionToolEventRequest,
    agent_session_message_target_is_currently_routable, agent_session_message_target_is_live_bound,
    agent_session_normalized_metadata_json, agent_session_status_is_routable,
    agent_session_unix_timestamp,
};
pub use artifact_event_builder::ClientDbArtifactEventBuilder;
pub use dependency_index::{
    DEFAULT_GERBIL_DEPS_SEARCH_LIMIT, GerbilDepsQueryRequest, GerbilDepsQueryResult,
    GerbilDepsSearchRequest, GerbilDepsSearchResult, gerbil_deps_minimal_import,
    gerbil_deps_query_export, gerbil_deps_query_terms, gerbil_deps_search_exports,
    gerbil_deps_selector_for, gerbil_deps_validate_module_id, gerbil_deps_validate_symbol,
};
pub use engine::{
    ClientDbActiveGenerationSourceBlob, ClientDbActiveGenerationSourceBlobs, ClientDbBackend,
    ClientDbEngine, ClientDbEngineDurability, ClientDbEngineFeatures, ClientDbEngineReadSession,
    ClientDbEngineReport, ClientDbEngineWriteSession, ClientDbSourceIndexGenerationOwner,
    ClientDbSourceIndexGenerationRelation, ClientDbSourceIndexGenerationSnapshot,
    ClientDbSourceIndexSelectorFact, ProviderIncrementalOwnerSnapshot,
    ProviderIncrementalOwnerWrite, ProviderIncrementalScoped, ProviderIncrementalWriteReceipt,
    ProviderOwnerBatchProbeReceipt, ProviderOwnerBatchProbeRequest, ProviderOwnerBatchProbeResult,
    ProviderOwnerDecision, ProviderOwnerFingerprint, ProviderOwnerInventory,
    ProviderOwnerInventoryEntry, ProviderOwnerInventoryEntryState, ProviderOwnerInventoryState,
    ProviderOwnerInventoryWrite, ProviderOwnerInventoryWriteReceipt, ProviderOwnerMetadata,
    ProviderOwnerProbe, ProviderRemainingOwnerCountKind, ProviderSelectorProjection,
    ProviderTreeSitterCaptureProjection, ProviderTreeSitterContinuation,
    ProviderTreeSitterOwnerResult, ProviderTreeSitterOwnerResultState,
    ProviderTreeSitterOwnerWriteReceipt, ProviderTreeSitterQueryCounters,
    ProviderTreeSitterQueryIdentity, ProviderTreeSitterQueryRead, ProviderTreeSitterQueryReadState,
    ProviderTreeSitterQueryReceipt,
};
pub use engine::{
    ClientDbEngineSourceIndexReadModelReport, TURSO_BOOTSTRAP_TABLE, TursoClientDbSearchDocument,
    TursoClientDbSearchHit, TursoClientDbSearchResult, TursoClientDbSearchState,
};
pub use source_index::{
    CLIENT_DB_SOURCE_INDEX_PROVIDER_ID, CLIENT_DB_SOURCE_INDEX_SCHEMA_ID,
    CLIENT_DB_SOURCE_INDEX_SCHEMA_VERSION, CLIENT_DB_SOURCE_INDEX_SCOPE_DIR_EVIDENCE_PREFIX,
    CLIENT_DB_SOURCE_INDEX_SCOPE_REGISTRY_EVIDENCE_PATH,
    CLIENT_DB_SOURCE_INDEX_SCOPE_WITNESS_SHA256, ClientDbLiveSourceIndexFacts,
    ClientDbSourceIndexCandidate, ClientDbSourceIndexCandidateLookup,
    ClientDbSourceIndexCandidateLookupResult, ClientDbSourceIndexCandidatePath,
    ClientDbSourceIndexClientDirLookupRequest, ClientDbSourceIndexImport,
    ClientDbSourceIndexImportAssemblyRequest, ClientDbSourceIndexImportFile,
    ClientDbSourceIndexImportRequest, ClientDbSourceIndexLookup, ClientDbSourceIndexLookupResult,
    ClientDbSourceIndexLookupState, ClientDbSourceIndexMembershipChangeSet,
    ClientDbSourceIndexOwner, ClientDbSourceIndexPath, ClientDbSourceIndexProjectLookupRequest,
    ClientDbSourceIndexProjectionCoverage, ClientDbSourceIndexQueryKey,
    ClientDbSourceIndexRefreshReport, ClientDbSourceIndexRefreshRequest,
    ClientDbSourceIndexRefreshResult, ClientDbSourceIndexScopeFile, ClientDbSourceIndexSelector,
    ClientDbSourceIndexSelectorId, ClientDbSourceIndexSelectorKind,
    ClientDbSourceIndexSelectorLookup, ClientDbSourceIndexSelectorSymbol,
    ClientDbSourceIndexSource, ClientDbSourceIndexSourceBlobs, ClientDbSourceIndexSourceKind,
    ClientDbSourceIndexStats, ClientDbSourceIndexStructuralSelector, assemble_source_index_import,
    build_source_index_import, client_db_source_index_file_count,
    client_db_source_index_generation_id_for_snapshot,
    client_db_source_index_registry_evidence_hash, client_db_source_index_scope_dir_evidence_hash,
    overlay_active_source_index_import, source_index_file_hashes,
    source_index_import_with_file_hashes, source_index_relative_path, source_index_scope_dirs,
};
pub use source_index::{
    ClientDbExactSelectorProjectionV1, ClientDbExactSelectorWarmHitV1,
    ExactSelectorMerkleLookupKeyV1, ExactSelectorMerkleMissV1, ExactSelectorWarmSideEffectsV1,
};
pub use types::{
    AGENT_SEMANTIC_CLIENT_DB_SCHEMA_VERSION, ClientDbArtifactEdge, ClientDbArtifactEvent,
    ClientDbArtifactHash, ClientDbArtifactRepairChainFrame, ClientDbArtifactRoot,
    ClientDbGenerationHit, ClientDbGenerationLookup, ClientDbProofReceipt,
    ClientDbProviderCommandSelection, ClientDbReport, ClientDbSummary, ClientDbSyntaxCaptureReplay,
    ClientDbSyntaxNodeType, ClientDbSyntaxQueryInputKind, ClientDbSyntaxQueryLookup,
    ClientDbSyntaxQueryReplay,
};
extern crate self as agent_semantic_client_db;

#[cfg(test)]
#[path = "../tests/unit/runtime_server_endpoint_generation.rs"]
mod runtime_server_endpoint_generation_tests;
#[cfg(test)]
#[path = "../tests/unit/test_support_common.rs"]
mod test_support;
#[cfg(test)]
#[path = "../tests/unit/workspace_generation_qualification.rs"]
mod workspace_generation_qualification_tests;

pub use engine::{
    ProviderSearchWorkspaceSession, TursoResidentSelectorCandidate, TursoResidentSelectorQuery,
    TursoResidentSelectorRead, WorkspaceDbRegistry, WorkspaceDbRegistryCounters,
    WorkspaceDbWriteFinishMode, WorkspaceDbWriteFinishReceipt,
};
pub mod fixture;

pub use engine::{
    active_turso_source_index_generation, active_turso_source_index_generation_blobs,
    latest_turso_source_index_generation_snapshot,
};
