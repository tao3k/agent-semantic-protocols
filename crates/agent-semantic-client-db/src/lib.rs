#![deny(dead_code)]

//! SQLite-backed cache database surface for `agent-semantic-client`.

pub mod db;
pub mod engine;
mod evidence_graph;
pub mod pragmas;
mod source_index;
mod structural_index;
mod syntax_query;

pub use agent_semantic_client_core::ClientDbStatus;
pub use db::{
    AGENT_SEMANTIC_CLIENT_DB_FILE, AGENT_SEMANTIC_CLIENT_DB_SCHEMA_VERSION, ClientDb,
    ClientDbArtifactEvent, ClientDbGenerationHit, ClientDbGenerationLookup,
    ClientDbProviderCommandSelection, ClientDbReport, ClientDbSummary, ClientDbSyntaxCaptureReplay,
    ClientDbSyntaxNodeType, ClientDbSyntaxQueryInputKind, ClientDbSyntaxQueryLookup,
    ClientDbSyntaxQueryReplay,
};
pub use engine::{
    ClientDbBackend, ClientDbEngine, ClientDbEngineDurability, ClientDbEngineFeatures,
    ClientDbEngineReport,
};
#[cfg(feature = "turso-backend")]
pub use engine::{
    TURSO_BOOTSTRAP_TABLE, TURSO_ENTITY_TABLE, TURSO_OVERLAY_DOCUMENT_TABLE,
    TURSO_SEARCH_DOCUMENT_TABLE, TursoClientDbOverlayDocument, TursoClientDbSearchDocument,
    TursoClientDbSearchHit, bootstrap_turso_client_db, search_turso_documents,
    upsert_turso_overlay_document, upsert_turso_search_document,
};
pub use evidence_graph::{
    CLIENT_DB_EVIDENCE_GRAPH_SCHEMA_ID, CLIENT_DB_EVIDENCE_GRAPH_SCHEMA_VERSION,
    ClientDbEvidenceGraph, ClientDbEvidenceGraphEdge, ClientDbEvidenceGraphNode,
    source_index_evidence_graph, structural_index_evidence_graph,
};
pub use pragmas::{ClientDbJournalMode, ClientDbRuntimePragmas};
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
    ClientDbSourceIndexSelectorLookup, ClientDbSourceIndexSource, ClientDbSourceIndexSourceKind,
    ClientDbSourceIndexStats, assemble_source_index_import, build_source_index_import,
    client_db_source_index_file_count, client_db_source_index_generation_id,
    client_db_source_index_registry_evidence_hash, client_db_source_index_scope_dir_evidence_hash,
    lookup_source_index_from_client_dir, lookup_source_index_from_project,
    source_index_file_hashes, source_index_import_with_file_hashes, source_index_relative_path,
    source_index_scope_dirs,
};
pub use structural_index::{
    ClientDbStructuralDependencyUsage, ClientDbStructuralHash, ClientDbStructuralIndexImport,
    ClientDbStructuralIndexLookup, ClientDbStructuralIndexRefreshPlan,
    ClientDbStructuralIndexStats, ClientDbStructuralKind, ClientDbStructuralLocator,
    ClientDbStructuralName, ClientDbStructuralOwner, ClientDbStructuralPath,
    ClientDbStructuralQueryKey, ClientDbStructuralSource, ClientDbStructuralSymbol,
};
