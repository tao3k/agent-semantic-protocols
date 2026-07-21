#![deny(dead_code)]

//! DB Engine facade and control adapters for `agent-semantic-client`.

pub mod agent_session_registry;
pub use agent_session_registry::{
    AgentSessionModelObservationRef, AgentSessionModelObservationSource,
};
pub mod artifact_pointer_store;
mod dependency_index;
pub mod engine;
mod source_index;
pub mod storage_contract;
pub mod storage_performance_receipt;
mod structural_index;
mod syntax_query;
pub mod turso_agent_storage;
pub mod turso_cdc_storage;
pub mod turso_encrypted_storage;
mod turso_mvcc_keyset;
mod turso_mvcc_maintenance;
pub mod turso_mvcc_store;
mod turso_mvcc_typed;
pub mod turso_sync_storage;
mod types;

pub use agent_semantic_client_core::ClientDbStatus;
pub use agent_session_registry::{
    AGENT_SESSION_REGISTRY_DB_NAME, AGENT_SESSION_STATUS_ACTIVE, AGENT_SESSION_STATUS_ARCHIVED,
    AGENT_SESSION_STATUS_IDLE, AGENT_SESSION_STATUS_INVALID, AgentSessionDispatchClaimRequest,
    AgentSessionDispatchClaimResult, AgentSessionDispatchCompleteRequest,
    AgentSessionDispatchLeaseRecord, AgentSessionLookupRequest, AgentSessionRecord,
    AgentSessionRegisterRequest, AgentSessionRegistry, AgentSessionToolEventRequest,
    agent_session_message_target_is_currently_routable, agent_session_message_target_is_live_bound,
    agent_session_normalized_metadata_json, agent_session_status_is_routable,
    agent_session_unix_timestamp,
};
pub use dependency_index::{
    DEFAULT_GERBIL_DEPS_SEARCH_LIMIT, GerbilDepsQueryRequest, GerbilDepsQueryResult,
    GerbilDepsSearchRequest, GerbilDepsSearchResult, gerbil_deps_minimal_import,
    gerbil_deps_query_export, gerbil_deps_query_terms, gerbil_deps_search_exports,
    gerbil_deps_selector_for, gerbil_deps_validate_module_id, gerbil_deps_validate_symbol,
};
pub use engine::{
    ClientDbBackend, ClientDbEngine, ClientDbEngineDurability, ClientDbEngineFeatures,
    ClientDbEngineReadSession, ClientDbEngineReport, ClientDbEngineWriteSession,
};
pub use engine::{
    ClientDbEngineSourceIndexReadModelReport, ClientDbEngineStructuralIndexReadModelReport,
    TURSO_BOOTSTRAP_TABLE, TursoClientDbSearchDocument, TursoClientDbSearchHit,
    TursoClientDbSearchResult, TursoClientDbSearchState,
};
pub use source_index::{
    CLIENT_DB_LANGUAGE_PROJECTION_PROTOCOL_ID, CLIENT_DB_LANGUAGE_PROJECTION_PROTOCOL_VERSION,
    CLIENT_DB_LANGUAGE_PROJECTION_SCHEMA_ID, CLIENT_DB_LANGUAGE_PROJECTION_SCHEMA_VERSION,
    CLIENT_DB_SOURCE_INDEX_PROVIDER_ID, CLIENT_DB_SOURCE_INDEX_SCHEMA_ID,
    CLIENT_DB_SOURCE_INDEX_SCHEMA_VERSION, CLIENT_DB_SOURCE_INDEX_SCOPE_DIR_EVIDENCE_PREFIX,
    CLIENT_DB_SOURCE_INDEX_SCOPE_REGISTRY_EVIDENCE_PATH,
    CLIENT_DB_SOURCE_INDEX_SCOPE_WITNESS_SHA256, ClientDbLanguageProjection,
    ClientDbLanguageProjectionHarness, ClientDbLanguageProjectionImport,
    ClientDbLanguageProjectionImportRequest, ClientDbLanguageProjectionItem,
    ClientDbLanguageProjectionNodeKind, ClientDbLanguageProjectionNodeRef,
    ClientDbLanguageProjectionOwner, ClientDbLanguageProjectionRelation,
    ClientDbLanguageProjectionSource, ClientDbLanguageProjectionSourceKind,
    ClientDbSourceIndexCandidate, ClientDbSourceIndexCandidateLookup,
    ClientDbSourceIndexCandidateLookupResult, ClientDbSourceIndexClientDirLookupRequest,
    ClientDbSourceIndexImport, ClientDbSourceIndexImportAssemblyRequest,
    ClientDbSourceIndexImportFile, ClientDbSourceIndexImportRequest, ClientDbSourceIndexLookup,
    ClientDbSourceIndexLookupResult, ClientDbSourceIndexLookupState, ClientDbSourceIndexOwner,
    ClientDbSourceIndexPath, ClientDbSourceIndexProjectLookupRequest, ClientDbSourceIndexQueryKey,
    ClientDbSourceIndexRefreshReport, ClientDbSourceIndexRefreshRequest,
    ClientDbSourceIndexRefreshResult, ClientDbSourceIndexScopeFile, ClientDbSourceIndexSelector,
    ClientDbSourceIndexSelectorLookup, ClientDbSourceIndexSelectorPayloadProof,
    ClientDbSourceIndexSource, ClientDbSourceIndexSourceKind, ClientDbSourceIndexStats,
    assemble_source_index_import, build_source_index_import,
    client_db_source_index_artifact_digest, client_db_source_index_file_count,
    client_db_source_index_generation_id_for_snapshot,
    client_db_source_index_registry_evidence_hash, client_db_source_index_scope_dir_evidence_hash,
    source_index_file_hashes, source_index_import_from_language_projection,
    source_index_import_with_file_hashes, source_index_relative_path, source_index_scope_dirs,
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
