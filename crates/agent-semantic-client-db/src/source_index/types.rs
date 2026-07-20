//! Public value types for DB Engine-owned source index rows.

use std::path::{Path, PathBuf};

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
pub fn client_db_source_index_file_count(file_count: usize) -> u32 {
    file_count.min(u32::MAX as usize) as u32
}

/// Content address of the disposable source-index projection for one snapshot.
#[must_use]
pub fn client_db_source_index_artifact_digest(
    source_snapshot: &agent_semantic_content_identity::SourceSnapshotEvidence,
) -> String {
    agent_semantic_content_identity::hash_derived_artifact_key(
        agent_semantic_content_identity::DerivedArtifactKeyInput {
            artifact_kind: "source-index",
            schema_id: "asp.source-index-artifact.v1",
            snapshot_root: &source_snapshot.root_digest,
            provider_digest: &source_snapshot.provider_digest,
            parameters: &[],
        },
    )
    .value
}

/// Deterministic generation identity; there is deliberately no timestamp fallback.
#[must_use]
pub fn client_db_source_index_generation_id_for_snapshot(
    source_snapshot: &agent_semantic_content_identity::SourceSnapshotEvidence,
) -> CacheGenerationId {
    CacheGenerationId::from(format!(
        "source-index-{}",
        client_db_source_index_artifact_digest(source_snapshot)
    ))
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
    ColdRequired,
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
            Self::ColdRequired => "cold-required",
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
    /// Parser-owned item identity associated with the bounded selector proof.
    pub selector_symbol: Option<String>,
    /// Parser-owned item kind associated with the bounded selector proof.
    pub selector_kind: Option<String>,
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
            selector_symbol: None,
            selector_kind: None,
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
    pub source_snapshot: Option<agent_semantic_content_identity::SourceSnapshotEvidence>,
    pub index_artifact_digest: Option<String>,
}

/// Request for looking up source-index candidates through a project state root.
pub struct ClientDbSourceIndexProjectLookupRequest<'a> {
    pub cache_project_root: &'a Path,
    pub indexed_project_root: &'a Path,
    pub language_id: Option<&'a LanguageId>,
    pub query_keys: Vec<ClientDbSourceIndexQueryKey>,
    pub limit: u32,
    pub expected_snapshot_root: &'a str,
    pub expected_index_artifact_digest: &'a str,
}

/// Request for looking up source-index candidates from an already resolved
/// client cache directory.
pub struct ClientDbSourceIndexClientDirLookupRequest<'a> {
    pub client_dir: &'a Path,
    pub indexed_project_root: &'a Path,
    pub language_id: Option<&'a LanguageId>,
    pub query_keys: Vec<ClientDbSourceIndexQueryKey>,
    pub limit: u32,
    pub expected_snapshot_root: &'a str,
    pub expected_index_artifact_digest: &'a str,
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
    pub source_snapshot: agent_semantic_content_identity::SourceSnapshotEvidence,
}

impl ClientDbSourceIndexRefreshResult {
    pub fn source_snapshot(&self) -> &agent_semantic_content_identity::SourceSnapshotEvidence {
        &self.source_snapshot
    }

    /// Content address for the disposable index projection of this source snapshot.
    #[must_use]
    pub fn index_artifact_digest(&self) -> &str {
        &self.index_artifact_digest
    }
}

/// Request for applying a source-index import to the DB.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientDbSourceIndexRefreshRequest {
    pub import: ClientDbSourceIndexImport,
    pub file_count: u32,
    pub source_snapshot: agent_semantic_content_identity::SourceSnapshotEvidence,
}

/// DB-owned refresh result for source-index generation writes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientDbSourceIndexRefreshReport {
    pub generation_id: CacheGenerationId,
    pub reused_generation: bool,
    pub file_count: u32,
    pub owner_count: u32,
    pub selector_count: u32,
    /// Owners whose canonical rows were inserted or replaced by this refresh.
    pub changed_owner_count: u32,
    /// Owners removed from the canonical snapshot by this refresh.
    pub removed_owner_count: u32,
    /// Token-owner postings written after the changed-owner projection refresh.
    pub posting_write_count: u32,
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
    pub source_snapshot: agent_semantic_content_identity::SourceSnapshotEvidence,
    pub index_artifact_digest: String,
}

impl ClientDbSourceIndexRefreshResult {
    #[must_use]
    pub fn from_report(
        db_path: impl Into<PathBuf>,
        report: ClientDbSourceIndexRefreshReport,
        source_snapshot: agent_semantic_content_identity::SourceSnapshotEvidence,
    ) -> Self {
        let index_artifact_digest = agent_semantic_content_identity::hash_derived_artifact_key(
            agent_semantic_content_identity::DerivedArtifactKeyInput {
                artifact_kind: "source-index",
                schema_id: "asp.source-index-artifact.v1",
                snapshot_root: &source_snapshot.root_digest,
                provider_digest: &source_snapshot.provider_digest,
                parameters: &[],
            },
        )
        .value;
        Self {
            db_path: db_path.into(),
            generation_id: report.generation_id,
            reused_generation: report.reused_generation,
            file_count: report.file_count,
            owner_count: report.owner_count,
            selector_count: report.selector_count,
            source_snapshot,
            index_artifact_digest,
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
