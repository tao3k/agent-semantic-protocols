#![deny(dead_code)]

//! SQLite-backed cache database surface for `agent-semantic-client`.

pub mod db;
pub mod engine;
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
pub use pragmas::{ClientDbJournalMode, ClientDbRuntimePragmas};
pub use source_index::{
    CLIENT_DB_SOURCE_INDEX_PROVIDER_ID, CLIENT_DB_SOURCE_INDEX_SCHEMA_ID,
    CLIENT_DB_SOURCE_INDEX_SCHEMA_VERSION, CLIENT_DB_SOURCE_INDEX_SCOPE_DIR_EVIDENCE_PREFIX,
    CLIENT_DB_SOURCE_INDEX_SCOPE_REGISTRY_EVIDENCE_PATH,
    CLIENT_DB_SOURCE_INDEX_SCOPE_WITNESS_SHA256, ClientDbSourceIndexCandidate,
    ClientDbSourceIndexImport, ClientDbSourceIndexImportFile, ClientDbSourceIndexImportRequest,
    ClientDbSourceIndexLookup, ClientDbSourceIndexLookupResult, ClientDbSourceIndexLookupState,
    ClientDbSourceIndexOwner, ClientDbSourceIndexPath, ClientDbSourceIndexQueryKey,
    ClientDbSourceIndexRefreshReport, ClientDbSourceIndexRefreshRequest,
    ClientDbSourceIndexRefreshResult, ClientDbSourceIndexScopeFile, ClientDbSourceIndexSelector,
    ClientDbSourceIndexSelectorLookup, ClientDbSourceIndexSource, ClientDbSourceIndexSourceKind,
    ClientDbSourceIndexStats, build_source_index_import, client_db_source_index_file_count,
    client_db_source_index_generation_id, source_index_relative_path, source_index_scope_dirs,
};
pub use structural_index::{
    ClientDbStructuralDependencyUsage, ClientDbStructuralHash, ClientDbStructuralIndexImport,
    ClientDbStructuralIndexLookup, ClientDbStructuralIndexRefreshPlan,
    ClientDbStructuralIndexStats, ClientDbStructuralKind, ClientDbStructuralLocator,
    ClientDbStructuralName, ClientDbStructuralOwner, ClientDbStructuralPath,
    ClientDbStructuralQueryKey, ClientDbStructuralSource, ClientDbStructuralSymbol,
};
