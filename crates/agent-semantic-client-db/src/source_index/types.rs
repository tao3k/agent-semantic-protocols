//! Public value types for DB Engine-owned source index rows.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use agent_semantic_client_core::{
    CacheGenerationId, ClientCacheFileHash, LanguageId, ProviderId, SemanticSchemaId,
    SemanticSchemaVersion,
};
use sha2::{Digest, Sha256};

pub const CLIENT_DB_SOURCE_INDEX_SCHEMA_ID: &str = "agent.semantic-protocols.semantic-source-index";
pub const CLIENT_DB_SOURCE_INDEX_SCHEMA_VERSION: &str = "1";
pub const CLIENT_DB_SOURCE_INDEX_PROVIDER_ID: &str = "db-engine-source-index";
pub const CLIENT_DB_SOURCE_INDEX_SCOPE_DIR_EVIDENCE_PREFIX: &str = "@scope/dir/";
pub const CLIENT_DB_SOURCE_INDEX_SCOPE_REGISTRY_EVIDENCE_PATH: &str = "@scope/registry";
pub const CLIENT_DB_SOURCE_INDEX_SCOPE_WITNESS_SHA256: &str =
    "0000000000000000000000000000000000000000000000000000000000000000";

#[must_use]
pub fn client_db_source_index_generation_id() -> CacheGenerationId {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    CacheGenerationId::from(format!("source-index-{nanos}"))
}

#[must_use]
pub fn client_db_source_index_file_count(file_count: usize) -> u32 {
    file_count.min(u32::MAX as usize) as u32
}

#[must_use]
pub fn client_db_source_index_registry_evidence_hash(
    registry_fingerprint: &str,
) -> ClientCacheFileHash {
    ClientCacheFileHash {
        path: CLIENT_DB_SOURCE_INDEX_SCOPE_REGISTRY_EVIDENCE_PATH.to_string(),
        sha256: format!("{:x}", Sha256::digest(registry_fingerprint.as_bytes())),
        byte_len: registry_fingerprint.len().min(u64::MAX as usize) as u64,
        mtime_ms: 0,
    }
}

#[must_use]
pub fn client_db_source_index_scope_dir_evidence_hash(
    relative_dir: &str,
    byte_len: u64,
    mtime_ms: u64,
) -> ClientCacheFileHash {
    ClientCacheFileHash {
        path: format!("{CLIENT_DB_SOURCE_INDEX_SCOPE_DIR_EVIDENCE_PREFIX}{relative_dir}"),
        sha256: CLIENT_DB_SOURCE_INDEX_SCOPE_WITNESS_SHA256.to_string(),
        byte_len,
        mtime_ms,
    }
}

macro_rules! source_index_value_type {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
        pub struct $name(String);

        impl $name {
            /// Create a source index scalar.
            #[must_use]
            pub fn new(value: impl Into<String>) -> Self {
                Self(value.into())
            }

            /// Return the stored scalar text.
            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl From<&str> for $name {
            fn from(value: &str) -> Self {
                Self(value.to_string())
            }
        }

        impl From<String> for $name {
            fn from(value: String) -> Self {
                Self(value)
            }
        }
    };
}

source_index_value_type!(
    /// Project-relative path retained by the DB Engine source index.
    ClientDbSourceIndexPath
);
source_index_value_type!(
    /// Query key used for index-first owner recall.
    ClientDbSourceIndexQueryKey
);
source_index_value_type!(
    /// Source authority for a selector or owner row.
    ClientDbSourceIndexSource
);

/// One DB Engine-owned source index generation imported into the client DB.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientDbSourceIndexImport {
    pub generation_id: CacheGenerationId,
    pub project_root: PathBuf,
    pub schema_id: SemanticSchemaId,
    pub schema_version: SemanticSchemaVersion,
    pub file_hashes: Vec<ClientCacheFileHash>,
    pub owners: Vec<ClientDbSourceIndexOwner>,
    pub selectors: Vec<ClientDbSourceIndexSelector>,
}

/// Source file projection used to assemble a DB-owned source-index import
/// packet without storing raw source text.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientDbSourceIndexImportFile {
    pub relative_path: String,
    pub language_id: LanguageId,
    pub provider_id: ProviderId,
    pub text: String,
    pub selectors: Vec<ClientDbSourceIndexSelector>,
}

/// Request for building one Rust-owned source-index import packet.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientDbSourceIndexImportRequest {
    pub generation_id: CacheGenerationId,
    pub project_root: PathBuf,
    pub schema_id: SemanticSchemaId,
    pub schema_version: SemanticSchemaVersion,
    pub selector_source: ClientDbSourceIndexSource,
    pub file_hashes: Vec<ClientCacheFileHash>,
    pub files: Vec<ClientDbSourceIndexImportFile>,
}

/// Request for assembling source-index file hashes and import rows from
/// workspace files without storing raw source text in durable rows.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientDbSourceIndexImportAssemblyRequest {
    pub generation_id: CacheGenerationId,
    pub project_root: PathBuf,
    pub schema_id: SemanticSchemaId,
    pub schema_version: SemanticSchemaVersion,
    pub selector_source: ClientDbSourceIndexSource,
    pub file_text_bytes_limit: u64,
    pub previous_file_hashes: Option<Vec<ClientCacheFileHash>>,
    pub registry_fingerprint: String,
    pub extra_scope_dirs: Vec<String>,
    pub files: Vec<ClientDbSourceIndexScopeFile>,
}

/// Rust-owned owner row retained for index-first broad search.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientDbSourceIndexOwner {
    pub owner_path: ClientDbSourceIndexPath,
    pub language_id: Option<LanguageId>,
    pub provider_id: Option<ProviderId>,
    pub source_kind: ClientDbSourceIndexSource,
    pub line_count: Option<u32>,
    pub query_keys: Vec<ClientDbSourceIndexQueryKey>,
}

/// Source file scope used by source-index refresh and reuse checks.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientDbSourceIndexScopeFile {
    pub path: PathBuf,
    pub language_id: LanguageId,
    pub provider_id: ProviderId,
    pub selector_receipts: Vec<ClientDbSourceIndexSelector>,
}

/// Source-index lookup state for agent-facing search fallbacks.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ClientDbSourceIndexLookupState {
    MissingDb,
    EmptyIndex,
    Busy,
    Hit,
    Miss,
}

impl ClientDbSourceIndexLookupState {
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::MissingDb => "missing-db",
            Self::EmptyIndex => "empty-index",
            Self::Busy => "busy",
            Self::Hit => "hit",
            Self::Miss => "miss",
        }
    }
}

/// Agent-facing source-index candidate row returned by the DB facade.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientDbSourceIndexCandidate {
    pub path: String,
    pub language_id: Option<LanguageId>,
    pub provider_id: Option<ProviderId>,
    pub source_kind: ClientDbSourceIndexSourceKind,
    pub line_count: Option<u32>,
    pub query_keys: Vec<String>,
    pub selector_proof: Option<ClientDbSourceIndexSelectorPayloadProof>,
}

/// Provider/parser proof that a source-index candidate has a bounded payload.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientDbSourceIndexSelectorPayloadProof {
    pub structural_selector: String,
    pub payload_kind: String,
    pub bounded: bool,
}

/// Typed source category for source-index candidate rows.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ClientDbSourceIndexSourceKind {
    File,
    Other(String),
}

impl ClientDbSourceIndexSourceKind {
    #[must_use]
    pub fn as_str(&self) -> &str {
        match self {
            Self::File => "file",
            Self::Other(value) => value.as_str(),
        }
    }
}

impl From<ClientDbSourceIndexSource> for ClientDbSourceIndexSourceKind {
    fn from(value: ClientDbSourceIndexSource) -> Self {
        match value.as_str() {
            "file" => Self::File,
            other => Self::Other(other.to_string()),
        }
    }
}

impl From<ClientDbSourceIndexOwner> for ClientDbSourceIndexCandidate {
    fn from(owner: ClientDbSourceIndexOwner) -> Self {
        Self {
            selector_proof: None,
            path: owner.owner_path.as_str().to_string(),
            language_id: owner.language_id,
            provider_id: owner.provider_id,
            source_kind: owner.source_kind.into(),
            line_count: owner.line_count,
            query_keys: owner
                .query_keys
                .into_iter()
                .map(|key| key.as_str().to_string())
                .collect(),
        }
    }
}

/// Lookup result from the DB Engine-owned source index.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientDbSourceIndexLookupResult {
    pub db_path: PathBuf,
    pub state: ClientDbSourceIndexLookupState,
    pub candidates: Vec<ClientDbSourceIndexCandidate>,
}

/// Request for looking up source-index candidates through a project state root.
pub struct ClientDbSourceIndexProjectLookupRequest<'a> {
    pub cache_project_root: &'a Path,
    pub indexed_project_root: &'a Path,
    pub language_id: Option<&'a LanguageId>,
    pub query_keys: Vec<ClientDbSourceIndexQueryKey>,
    pub limit: u32,
}

/// Request for looking up source-index candidates from an already resolved
/// client cache directory.
pub struct ClientDbSourceIndexClientDirLookupRequest<'a> {
    pub client_dir: &'a Path,
    pub indexed_project_root: &'a Path,
    pub language_id: Option<&'a LanguageId>,
    pub query_keys: Vec<ClientDbSourceIndexQueryKey>,
    pub limit: u32,
}

/// DB-owned source-index candidate lookup result without path projection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientDbSourceIndexCandidateLookupResult {
    pub state: ClientDbSourceIndexLookupState,
    pub candidates: Vec<ClientDbSourceIndexCandidate>,
}

/// Rust-owned selector row retained for exact owner-local expansion.
///
/// `selector_id` is the stable structural selector identity. Line fields are
/// compatibility/display hints and must not be used as selector identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientDbSourceIndexSelector {
    pub owner_path: ClientDbSourceIndexPath,
    pub selector_id: String,
    pub symbol: Option<String>,
    pub kind: Option<String>,
    pub start_line: u32,
    pub end_line: u32,
    pub source: ClientDbSourceIndexSource,
    pub query_keys: Vec<ClientDbSourceIndexQueryKey>,
    pub payload_proof: Option<ClientDbSourceIndexSelectorPayloadProof>,
}

/// Aggregate row counts for one source index generation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientDbSourceIndexStats {
    pub generation_id: CacheGenerationId,
    pub owner_count: u32,
    pub selector_count: u32,
}

/// Request for applying a source-index import to the DB.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientDbSourceIndexRefreshRequest {
    pub import: ClientDbSourceIndexImport,
    pub file_count: u32,
}

/// DB-owned refresh result for source-index generation writes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientDbSourceIndexRefreshReport {
    pub generation_id: CacheGenerationId,
    pub reused_generation: bool,
    pub file_count: u32,
    pub owner_count: u32,
    pub selector_count: u32,
}

/// Source-index refresh result projected with the concrete DB path.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientDbSourceIndexRefreshResult {
    pub db_path: PathBuf,
    pub generation_id: CacheGenerationId,
    pub reused_generation: bool,
    pub file_count: u32,
    pub owner_count: u32,
    pub selector_count: u32,
}

impl ClientDbSourceIndexRefreshResult {
    #[must_use]
    pub fn from_stats(
        db_path: impl Into<PathBuf>,
        stats: ClientDbSourceIndexStats,
        file_count: usize,
        reused_generation: bool,
    ) -> Self {
        Self {
            db_path: db_path.into(),
            generation_id: stats.generation_id,
            reused_generation,
            file_count: client_db_source_index_file_count(file_count),
            owner_count: stats.owner_count,
            selector_count: stats.selector_count,
        }
    }

    #[must_use]
    pub fn from_report(
        db_path: impl Into<PathBuf>,
        report: ClientDbSourceIndexRefreshReport,
    ) -> Self {
        Self {
            db_path: db_path.into(),
            generation_id: report.generation_id,
            reused_generation: report.reused_generation,
            file_count: report.file_count,
            owner_count: report.owner_count,
            selector_count: report.selector_count,
        }
    }
}

impl ClientDbSourceIndexRefreshResult {
    #[must_use]
    pub fn db_path(&self) -> &Path {
        &self.db_path
    }

    #[must_use]
    pub fn generation_id(&self) -> &CacheGenerationId {
        &self.generation_id
    }

    #[must_use]
    pub fn reused_generation(&self) -> bool {
        self.reused_generation
    }

    #[must_use]
    pub fn file_count(&self) -> u32 {
        self.file_count
    }

    #[must_use]
    pub fn owner_count(&self) -> u32 {
        self.owner_count
    }

    #[must_use]
    pub fn selector_count(&self) -> u32 {
        self.selector_count
    }
}

/// Lookup request for DB Engine-owned source index rows.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientDbSourceIndexLookup {
    pub project_root: PathBuf,
    pub language_id: Option<LanguageId>,
    pub query: ClientDbSourceIndexQueryKey,
    pub limit: u32,
}

/// Lookup request for a multi-key Rust-owned source-index candidate query.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientDbSourceIndexCandidateLookup {
    pub project_root: PathBuf,
    pub language_id: Option<LanguageId>,
    pub query_keys: Vec<ClientDbSourceIndexQueryKey>,
    pub limit: u32,
}

/// Lookup request for DB Engine-owned source index selector rows.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientDbSourceIndexSelectorLookup {
    pub project_root: PathBuf,
    pub language_id: Option<LanguageId>,
    pub kind: Option<String>,
    pub query: Option<ClientDbSourceIndexQueryKey>,
    pub limit: u32,
}
