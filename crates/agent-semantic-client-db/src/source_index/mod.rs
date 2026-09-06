// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

//! ASP Server-owned source-index rows for workspace source discovery.
//! This module is the DB Engine contract; client code only consumes it through
//! Runtime Server operations.

mod generation_overlay;
mod import;
mod text;
mod types;
pub(crate) use text::source_query_keys;

#[cfg(test)]
#[path = "../../tests/unit/source_index_text.rs"]
mod text_tests;

pub use agent_semantic_content_identity::exact_selector_cache::{
    ExactSelectorMerkleLookupKeyV1, ExactSelectorMerkleMissV1,
    ExactSelectorProjectionRecordV1 as ClientDbExactSelectorProjectionV1,
    ExactSelectorWarmHitV1 as ClientDbExactSelectorWarmHitV1, ExactSelectorWarmSideEffectsV1,
};
pub use generation_overlay::{overlay_active_source_index_import, partial_source_index_import};
pub use import::{
    assemble_source_index_import, build_source_index_import, source_index_file_hashes,
    source_index_import_with_file_hashes, source_index_relative_path, source_index_scope_dirs,
};
pub use types::{
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
    ClientDbSourceIndexOwnedRelation, ClientDbSourceIndexOwner, ClientDbSourceIndexPath,
    ClientDbSourceIndexProjectLookupRequest, ClientDbSourceIndexProjectionCoverage,
    ClientDbSourceIndexQueryKey, ClientDbSourceIndexRefreshReport,
    ClientDbSourceIndexRefreshRequest, ClientDbSourceIndexRefreshResult,
    ClientDbSourceIndexScopeFile, ClientDbSourceIndexSelector, ClientDbSourceIndexSelectorId,
    ClientDbSourceIndexSelectorKind, ClientDbSourceIndexSelectorLookup,
    ClientDbSourceIndexSelectorSymbol, ClientDbSourceIndexSource, ClientDbSourceIndexSourceBlobs,
    ClientDbSourceIndexSourceKind, ClientDbSourceIndexStats, ClientDbSourceIndexStructuralSelector,
    client_db_source_index_file_count, client_db_source_index_generation_id_for_snapshot,
    client_db_source_index_registry_evidence_hash, client_db_source_index_scope_dir_evidence_hash,
};
