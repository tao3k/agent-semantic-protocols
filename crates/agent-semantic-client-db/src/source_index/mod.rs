//! DB Engine-owned source index rows for workspace source discovery.

mod import;
pub(crate) mod language_projection;
mod text;
mod types;

pub use import::{
    assemble_source_index_import, build_source_index_import, source_index_file_hashes,
    source_index_import_from_language_projection, source_index_import_with_file_hashes,
    source_index_relative_path, source_index_scope_dirs,
};
pub use language_projection::{
    CLIENT_DB_LANGUAGE_PROJECTION_PROTOCOL_ID, CLIENT_DB_LANGUAGE_PROJECTION_PROTOCOL_VERSION,
    CLIENT_DB_LANGUAGE_PROJECTION_SCHEMA_ID, CLIENT_DB_LANGUAGE_PROJECTION_SCHEMA_VERSION,
    ClientDbLanguageProjection, ClientDbLanguageProjectionHarness,
    ClientDbLanguageProjectionImportRequest, ClientDbLanguageProjectionItem,
    ClientDbLanguageProjectionNodeKind, ClientDbLanguageProjectionNodeRef,
    ClientDbLanguageProjectionOwner, ClientDbLanguageProjectionRelation,
    ClientDbLanguageProjectionSource, ClientDbLanguageProjectionSourceKind,
};
pub use types::{
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
    client_db_source_index_file_count, client_db_source_index_generation_id,
    client_db_source_index_registry_evidence_hash, client_db_source_index_scope_dir_evidence_hash,
};
