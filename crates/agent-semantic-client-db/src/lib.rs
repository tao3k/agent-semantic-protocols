#![deny(dead_code)]

//! DB Engine facade and control adapters for `agent-semantic-client`.

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
pub mod graph_turbo_cache;
mod runtime_concurrency;
pub mod runtime_server;
pub mod runtime_server_admission;
pub mod runtime_server_admission_catalog;
mod runtime_server_agent_control_plane;
pub mod runtime_server_control;
pub mod runtime_server_diagnostics;
pub mod runtime_server_observability;
pub mod runtime_server_runtime;
pub mod runtime_server_workspace;
mod source_index;
pub mod storage_contract;
pub mod storage_performance_receipt;
mod structural_index;
mod syntax_query;
pub mod turso_agent_storage;
pub mod turso_cdc_storage;
pub mod turso_encrypted_storage;
mod turso_mvcc_keyset;
pub mod workspace_project_resolution;
pub use runtime_server_control::{
    RuntimeServerControlReceipt, RuntimeServerEndpoint, RuntimeServerOperation,
    acquire_runtime_server_election, call_runtime_server, prepare_runtime_server_endpoint,
    read_runtime_server_endpoint, runtime_server_endpoint_path, runtime_server_runtime_base,
};
pub use turso_mvcc_keyset::{
    TursoMvccEventId, TursoMvccPageCursor, TursoMvccPageLimit, TursoMvccPartitionKey,
};
mod turso_mvcc_maintenance;
pub mod turso_mvcc_partition;
mod turso_mvcc_partition_sql;
pub mod turso_mvcc_store;
mod turso_mvcc_typed;
pub mod turso_sync_storage;
mod types;
mod workspace_db_endpoint;
pub use workspace_db_endpoint::WorkspaceDbOwnerEndpoint;
pub mod workspace_db_ipc;
mod workspace_db_ipc_server;
pub mod workspace_db_owner_election;
pub use types::ClientDbProviderCommandSelectionInput;
pub use workspace_db_ipc::{WorkspaceDbIpcSession, serve_workspace_db_session_until_shutdown};

pub use agent_semantic_client_core::ClientDbStatus;
pub use agent_session_registry::{
    AGENT_SESSION_REGISTRY_DB_NAME, AGENT_SESSION_STATUS_ACTIVE, AGENT_SESSION_STATUS_ARCHIVED,
    AGENT_SESSION_STATUS_IDLE, AGENT_SESSION_STATUS_INVALID, AgentSessionDispatchClaimRequest,
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
    ClientDbBackend, ClientDbEngine, ClientDbEngineDurability, ClientDbEngineFeatures,
    ClientDbEngineReadSession, ClientDbEngineReport, ClientDbEngineWriteSession,
    ProviderIncrementalOwnerSnapshot, ProviderIncrementalOwnerWrite, ProviderIncrementalScoped,
    ProviderIncrementalWriteReceipt, ProviderOwnerBatchProbeReceipt,
    ProviderOwnerBatchProbeRequest, ProviderOwnerBatchProbeResult, ProviderOwnerDecision,
    ProviderOwnerFingerprint, ProviderOwnerInventory, ProviderOwnerInventoryEntry,
    ProviderOwnerInventoryEntryState, ProviderOwnerInventoryState, ProviderOwnerInventoryWrite,
    ProviderOwnerInventoryWriteReceipt, ProviderOwnerMetadata, ProviderOwnerProbe,
    ProviderRemainingOwnerCountKind, ProviderSelectorProjection,
    ProviderTreeSitterCaptureProjection, ProviderTreeSitterContinuation,
    ProviderTreeSitterOwnerResult, ProviderTreeSitterOwnerResultState,
    ProviderTreeSitterOwnerWriteReceipt, ProviderTreeSitterQueryCounters,
    ProviderTreeSitterQueryIdentity, ProviderTreeSitterQueryRead, ProviderTreeSitterQueryReadState,
    ProviderTreeSitterQueryReceipt,
};
pub use engine::{
    ClientDbEngineSourceIndexReadModelReport, ClientDbEngineStructuralIndexReadModelReport,
    TURSO_BOOTSTRAP_TABLE, TursoClientDbSearchDocument, TursoClientDbSearchHit,
    TursoClientDbSearchResult, TursoClientDbSearchState,
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
    build_source_index_import, client_db_source_index_artifact_digest,
    client_db_source_index_file_count, client_db_source_index_generation_id_for_snapshot,
    client_db_source_index_registry_evidence_hash, client_db_source_index_scope_dir_evidence_hash,
    source_index_file_hashes, source_index_import_with_file_hashes, source_index_relative_path,
    source_index_scope_dirs,
};
pub use source_index::{
    ClientDbExactSelectorProjectionV1, ClientDbExactSelectorWarmHitV1,
    ExactSelectorMerkleLookupKeyV1, ExactSelectorMerkleMissV1, ExactSelectorWarmSideEffectsV1,
};
pub use structural_index::{
    ClientDbStructuralDependencyUsage, ClientDbStructuralHash, ClientDbStructuralIndexImport,
    ClientDbStructuralIndexLookup, ClientDbStructuralIndexRefreshPlan,
    ClientDbStructuralIndexStats, ClientDbStructuralKind, ClientDbStructuralLocator,
    ClientDbStructuralName, ClientDbStructuralOwner, ClientDbStructuralPath,
    ClientDbStructuralQueryKey, ClientDbStructuralSource, ClientDbStructuralSymbol,
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
#[path = "../tests/unit/test_support_common.rs"]
mod test_support;

pub use engine::{
    ProviderSearchWorkspaceSession, TursoResidentSelectorCandidate, TursoResidentSelectorQuery,
    TursoResidentSelectorRead, WorkspaceDbRegistry, WorkspaceDbRegistryCounters,
    WorkspaceDbWriteFinishMode, WorkspaceDbWriteFinishReceipt,
};
pub mod fixture;
