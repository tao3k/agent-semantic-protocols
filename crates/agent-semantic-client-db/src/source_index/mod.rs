//! Rust-owned SQL source index rows for workspace source discovery.

mod api;
mod import;
mod lookup;
mod storage;
mod text;
mod types;

pub use api::{lookup_source_index_from_client_dir, lookup_source_index_from_project};
pub use import::{
    assemble_source_index_import, build_source_index_import, source_index_file_hashes,
    source_index_import_with_file_hashes, source_index_relative_path, source_index_scope_dirs,
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
    ClientDbSourceIndexSelectorLookup, ClientDbSourceIndexSource, ClientDbSourceIndexSourceKind,
    ClientDbSourceIndexStats, client_db_source_index_file_count,
    client_db_source_index_generation_id, client_db_source_index_registry_evidence_hash,
    client_db_source_index_scope_dir_evidence_hash,
};
