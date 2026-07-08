#![deny(dead_code)]

//! DB Engine facade and control adapters for `agent-semantic-client`.

pub mod agent_session_registry;
pub mod engine;
mod evidence_graph;
mod source_index;
mod structural_index;
mod syntax_query;
mod types;

pub use agent_semantic_client_core::ClientDbStatus;
pub use agent_session_registry::{
    AGENT_SESSION_REGISTRY_DB_NAME, AGENT_SESSION_STATUS_ACTIVE, AGENT_SESSION_STATUS_ARCHIVED,
    AGENT_SESSION_STATUS_IDLE, AGENT_SESSION_STATUS_INVALID, AgentSessionLookupRequest,
    AgentSessionRecord, AgentSessionRegisterRequest, AgentSessionRegistry,
    AgentSessionToolEventRequest, agent_session_normalized_metadata_json,
    agent_session_status_is_routable, agent_session_unix_timestamp,
};
pub use engine::{
    ClientDbBackend, ClientDbEngine, ClientDbEngineDurability, ClientDbEngineFeatures,
    ClientDbEngineReadSession, ClientDbEngineReport, ClientDbEngineWriteSession,
};
pub use engine::{
    ClientDbEngineSourceIndexReadModelReport, ClientDbEngineStructuralIndexReadModelReport,
    TURSO_BOOTSTRAP_TABLE, TURSO_EDGE_TABLE, TURSO_ENTITY_TABLE, TURSO_OVERLAY_DOCUMENT_TABLE,
    TURSO_ROUTE_RECEIPT_TABLE, TURSO_SEARCH_DOCUMENT_TABLE,
    TursoClientDbEvidenceGraphPersistReport, TursoClientDbGraphEdge, TursoClientDbGraphEntity,
    TursoClientDbOverlayDocument, TursoClientDbRouteReceipt, TursoClientDbSearchDocument,
    TursoClientDbSearchHit, list_turso_graph_edges, list_turso_graph_entities,
    persist_turso_evidence_graph, upsert_turso_graph_edge, upsert_turso_graph_entity,
    upsert_turso_route_receipt,
};
pub use evidence_graph::{
    CLIENT_DB_EVIDENCE_GRAPH_SCHEMA_ID, CLIENT_DB_EVIDENCE_GRAPH_SCHEMA_VERSION,
    ClientDbEvidenceGraph, ClientDbEvidenceGraphEdge, ClientDbEvidenceGraphNode,
    source_index_evidence_graph, structural_index_evidence_graph,
};
pub use source_index::{
    CLIENT_DB_SOURCE_INDEX_PROVIDER_ID, CLIENT_DB_SOURCE_INDEX_SCHEMA_ID,
    CLIENT_DB_SOURCE_INDEX_SCHEMA_VERSION, CLIENT_DB_SOURCE_INDEX_SCOPE_DIR_EVIDENCE_PREFIX,
    CLIENT_DB_SOURCE_INDEX_SCOPE_REGISTRY_EVIDENCE_PATH,
    CLIENT_DB_SOURCE_INDEX_SCOPE_WITNESS_SHA256, ClientDbSourceIndexCandidate,
    ClientDbSourceIndexCandidateLookup, ClientDbSourceIndexCandidateLookupResult,
    ClientDbSourceIndexClientDirLookupRequest, ClientDbSourceIndexImport,
    ClientDbSourceIndexImportAssemblyRequest, ClientDbSourceIndexImportFile,
    ClientDbSourceIndexImportRequest, ClientDbSourceIndexLookup, ClientDbSourceIndexLookupResult,
    ClientDbSourceIndexLookupState, ClientDbSourceIndexOwner, ClientDbSourceIndexPath,
    ClientDbSourceIndexProjectLookupRequest, ClientDbSourceIndexQueryKey,
    ClientDbSourceIndexRefreshReport, ClientDbSourceIndexRefreshRequest,
    ClientDbSourceIndexRefreshResult, ClientDbSourceIndexScopeFile, ClientDbSourceIndexSelector,
    ClientDbSourceIndexSelectorLookup, ClientDbSourceIndexSelectorPayloadProof,
    ClientDbSourceIndexSource, ClientDbSourceIndexSourceKind, ClientDbSourceIndexStats,
    assemble_source_index_import, build_source_index_import, client_db_source_index_file_count,
    client_db_source_index_generation_id, client_db_source_index_registry_evidence_hash,
    client_db_source_index_scope_dir_evidence_hash, source_index_file_hashes,
    source_index_import_with_file_hashes, source_index_relative_path, source_index_scope_dirs,
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
