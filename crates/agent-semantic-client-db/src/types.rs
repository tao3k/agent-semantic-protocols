//! Shared DB Engine DTOs used by Turso adapters and client-facing receipts.

use std::path::{Path, PathBuf};

use agent_semantic_client_core::{
    CacheArtifactId, CacheExportMethod, CacheGenerationId, ClientCacheFileHash, ClientDbStatus,
    LanguageId, ProviderId, SemanticSchemaId,
};
use serde::{Deserialize, Serialize};

/// Current Turso DB Engine schema version for the local agent semantic client DB.
pub const AGENT_SEMANTIC_CLIENT_DB_SCHEMA_VERSION: i64 = 1;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(transparent)]
struct ClientDbJournalMode(String);

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(transparent)]
struct ClientDbSynchronousLevel(i64);

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(transparent)]
struct ClientDbBusyTimeoutMillis(i64);

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(transparent)]
struct ClientDbForeignKeyState(bool);

/// Read-only diagnostic summary for the active DB Engine path.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientDbRuntimePragmas {
    journal_mode: ClientDbJournalMode,
    synchronous: ClientDbSynchronousLevel,
    busy_timeout_ms: ClientDbBusyTimeoutMillis,
    foreign_keys: ClientDbForeignKeyState,
}

impl ClientDbRuntimePragmas {
    pub fn new(
        journal_mode: impl Into<String>,
        synchronous: i64,
        busy_timeout_ms: i64,
        foreign_keys: bool,
    ) -> Result<Self, String> {
        let journal_mode = journal_mode.into();
        if journal_mode.is_empty() {
            return Err("client DB journal mode must be non-empty".to_string());
        }
        if !(0..=3).contains(&synchronous) {
            return Err(format!(
                "client DB synchronous level must be between 0 and 3, got {synchronous}"
            ));
        }
        if busy_timeout_ms < 0 {
            return Err(format!(
                "client DB busy timeout must be non-negative, got {busy_timeout_ms}"
            ));
        }
        Ok(Self {
            journal_mode: ClientDbJournalMode(journal_mode),
            synchronous: ClientDbSynchronousLevel(synchronous),
            busy_timeout_ms: ClientDbBusyTimeoutMillis(busy_timeout_ms),
            foreign_keys: ClientDbForeignKeyState(foreign_keys),
        })
    }

    #[must_use]
    pub fn journal_mode(&self) -> &str {
        &self.journal_mode.0
    }

    #[must_use]
    pub fn synchronous(&self) -> i64 {
        self.synchronous.0
    }

    #[must_use]
    pub fn busy_timeout_ms(&self) -> i64 {
        self.busy_timeout_ms.0
    }

    #[must_use]
    pub fn foreign_keys(&self) -> bool {
        self.foreign_keys.0
    }
}

/// Read-only diagnostic summary for the active DB Engine path.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientDbReport {
    pub db_path: PathBuf,
    #[serde(serialize_with = "serialize_client_db_status")]
    pub status: ClientDbStatus,
    pub generation_count: u32,
    pub syntax_row_generation_count: u32,
    pub syntax_row_match_count: u32,
    pub syntax_row_capture_count: u32,
    pub source_index_generation_count: u32,
    pub source_index_owner_count: u32,
    pub source_index_selector_count: u32,
    pub artifact_event_count: u32,
    pub raw_source_stored: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runtime_pragmas: Option<ClientDbRuntimePragmas>,
    pub reason: Option<String>,
}

fn serialize_client_db_status<S>(status: &ClientDbStatus, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_str(status.as_str())
}

#[derive(Clone, Debug, Eq, PartialEq)]
/// Aggregate persisted-row counts exposed by the client DB diagnostics surface.
pub struct ClientDbSummary {
    pub generation_count: u32,
    pub syntax_row_generation_count: u32,
    pub syntax_row_match_count: u32,
    pub syntax_row_capture_count: u32,
    pub structural_index_generation_count: u32,
    pub structural_index_owner_count: u32,
    pub structural_index_symbol_count: u32,
    pub structural_index_dependency_usage_count: u32,
    pub source_index_generation_count: u32,
    pub source_index_owner_count: u32,
    pub source_index_selector_count: u32,
    pub artifact_event_count: u32,
    pub raw_source_stored: bool,
}

/// Named lookup request for one provider cache generation probe.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientDbGenerationLookup {
    pub db_path: PathBuf,
    pub language_id: LanguageId,
    pub provider_id: ProviderId,
    pub project_root: PathBuf,
    pub export_method: CacheExportMethod,
    pub request_fingerprint: Option<String>,
}

/// Matching cache generation metadata returned by a DB lookup.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientDbGenerationHit {
    pub language_id: LanguageId,
    pub provider_id: ProviderId,
    pub project_root: PathBuf,
    pub export_method: CacheExportMethod,
    pub schema_ids: Vec<SemanticSchemaId>,
    pub request_fingerprint: Option<String>,
    pub file_hashes: Vec<ClientCacheFileHash>,
    pub artifact_ids: Vec<CacheArtifactId>,
}

macro_rules! client_db_provider_command_text {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl From<String> for $name {
            fn from(value: String) -> Self {
                Self(value)
            }
        }

        impl From<&str> for $name {
            fn from(value: &str) -> Self {
                Self(value.to_owned())
            }
        }
    };
}

client_db_provider_command_text!(
    /// Provider manifest identifier selected for execution.
    ClientDbProviderManifestId
);
client_db_provider_command_text!(
    /// Provider manifest digest selected for execution.
    ClientDbProviderManifestDigest
);
client_db_provider_command_text!(
    /// Provider binary identity selected for execution.
    ClientDbProviderBinary
);
client_db_provider_command_text!(
    /// Provider execution mode selected for execution.
    ClientDbProviderExecution
);
client_db_provider_command_text!(
    /// One provider command prefix argument.
    ClientDbProviderCommandArg
);
client_db_provider_command_text!(
    /// Resolved provider executable path.
    ClientDbProviderExecutablePath
);

/// Unvalidated provider-command values accepted at the DB Engine boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientDbProviderCommandSelectionInput {
    pub manifest_id: ClientDbProviderManifestId,
    pub manifest_digest: ClientDbProviderManifestDigest,
    pub language_id: LanguageId,
    pub provider_id: ProviderId,
    pub binary: ClientDbProviderBinary,
    pub execution: ClientDbProviderExecution,
    pub provider_command_prefix: Vec<ClientDbProviderCommandArg>,
    pub executable_path: Option<ClientDbProviderExecutablePath>,
    pub executable_len: Option<i64>,
    pub executable_mtime_ms: Option<i64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
/// Validated provider command identity persisted for Runtime Server activation.
pub struct ClientDbProviderCommandSelection {
    pub manifest_id: ClientDbProviderManifestId,
    pub manifest_digest: ClientDbProviderManifestDigest,
    pub language_id: LanguageId,
    pub provider_id: ProviderId,
    pub binary: ClientDbProviderBinary,
    pub execution: ClientDbProviderExecution,
    pub provider_command_prefix: Vec<ClientDbProviderCommandArg>,
    pub executable_path: Option<ClientDbProviderExecutablePath>,
    pub executable_len: Option<i64>,
    pub executable_mtime_ms: Option<i64>,
}

impl ClientDbProviderCommandSelection {
    #[must_use]
    pub fn new(input: ClientDbProviderCommandSelectionInput) -> Self {
        Self {
            manifest_id: input.manifest_id,
            manifest_digest: input.manifest_digest,
            language_id: input.language_id,
            provider_id: input.provider_id,
            binary: input.binary,
            execution: input.execution,
            provider_command_prefix: input.provider_command_prefix,
            executable_path: input.executable_path,
            executable_len: input.executable_len,
            executable_mtime_ms: input.executable_mtime_ms,
        }
    }

    #[must_use]
    pub fn manifest_id(&self) -> &str {
        self.manifest_id.as_str()
    }

    #[must_use]
    pub fn manifest_digest(&self) -> &str {
        self.manifest_digest.as_str()
    }

    #[must_use]
    pub fn language_id(&self) -> &LanguageId {
        &self.language_id
    }

    #[must_use]
    pub fn provider_id(&self) -> &ProviderId {
        &self.provider_id
    }

    #[must_use]
    pub fn binary(&self) -> &str {
        self.binary.as_str()
    }

    #[must_use]
    pub fn execution(&self) -> &str {
        self.execution.as_str()
    }

    #[must_use]
    pub fn provider_command_prefix(&self) -> &[ClientDbProviderCommandArg] {
        &self.provider_command_prefix
    }

    #[must_use]
    pub fn executable_path(&self) -> Option<&str> {
        self.executable_path
            .as_ref()
            .map(ClientDbProviderExecutablePath::as_str)
    }

    #[must_use]
    pub fn executable_len(&self) -> Option<i64> {
        self.executable_len
    }

    #[must_use]
    pub fn executable_mtime_ms(&self) -> Option<i64> {
        self.executable_mtime_ms
    }
}

impl ClientDbArtifactEvent {
    pub(crate) fn from_storage_columns(
        columns: ClientDbArtifactEventStorageColumns,
    ) -> Result<Self, String> {
        let event_ordinal = u32::try_from(columns.event_ordinal).map_err(|_| {
            format!(
                "artifact event ordinal is outside the u32 domain: {}",
                columns.event_ordinal
            )
        })?;
        let bytes = u64::try_from(columns.bytes).map_err(|_| {
            format!(
                "artifact event byte count must be non-negative: {}",
                columns.bytes
            )
        })?;
        if columns.timestamp_ms < 0 {
            return Err(format!(
                "artifact event timestamp must be non-negative: {}",
                columns.timestamp_ms
            ));
        }
        if columns.artifact_path.is_empty()
            || columns.kind.is_empty()
            || columns.language.is_empty()
            || columns.method.is_empty()
            || columns.project_root.is_empty()
        {
            return Err(
                "artifact event identity, kind, language, method, and project root must be non-empty"
                    .to_string(),
            );
        }
        Ok(Self {
            artifact_path: ClientDbArtifactPath(columns.artifact_path),
            event_ordinal: ClientDbEventOrdinal(event_ordinal),
            timestamp_ms: ClientDbEventTimestampMillis(columns.timestamp_ms),
            kind: ClientDbArtifactEventKind(columns.kind),
            language: LanguageId::new(columns.language),
            method: ClientDbArtifactEventMethod(columns.method),
            target: ClientDbArtifactEventTarget(columns.target),
            query: ClientDbArtifactEventQuery(columns.query),
            project_root: ClientDbArtifactEventProjectRoot(columns.project_root),
            project_root_arg: ClientDbArtifactEventProjectRootArg(columns.project_root_arg),
            bytes: ClientDbArtifactEventByteCount(bytes),
        })
    }

    #[must_use]
    pub fn artifact_path(&self) -> &str {
        &self.artifact_path.0
    }

    #[must_use]
    pub fn event_ordinal(&self) -> u32 {
        self.event_ordinal.0
    }

    #[must_use]
    pub fn timestamp_ms(&self) -> i64 {
        self.timestamp_ms.0
    }

    #[must_use]
    pub fn kind(&self) -> &str {
        &self.kind.0
    }

    #[must_use]
    pub fn language(&self) -> &str {
        self.language.as_str()
    }

    #[must_use]
    pub fn method(&self) -> &str {
        &self.method.0
    }

    #[must_use]
    pub fn target(&self) -> &str {
        &self.target.0
    }

    #[must_use]
    pub fn query(&self) -> &str {
        &self.query.0
    }

    #[must_use]
    pub fn project_root(&self) -> &str {
        &self.project_root.0
    }

    #[must_use]
    pub fn project_root_arg(&self) -> &str {
        &self.project_root_arg.0
    }

    #[must_use]
    pub fn bytes(&self) -> u64 {
        self.bytes.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ClientDbArtifactPath(String);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ClientDbEventOrdinal(u32);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ClientDbEventTimestampMillis(i64);

#[derive(Clone, Debug, Eq, PartialEq)]
struct ClientDbArtifactEventKind(String);

#[derive(Clone, Debug, Eq, PartialEq)]
struct ClientDbArtifactEventMethod(String);

#[derive(Clone, Debug, Eq, PartialEq)]
struct ClientDbArtifactEventTarget(String);

#[derive(Clone, Debug, Eq, PartialEq)]
struct ClientDbArtifactEventQuery(String);

#[derive(Clone, Debug, Eq, PartialEq)]
struct ClientDbArtifactEventProjectRoot(String);

#[derive(Clone, Debug, Eq, PartialEq)]
struct ClientDbArtifactEventProjectRootArg(String);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ClientDbArtifactEventByteCount(u64);

pub(crate) struct ClientDbArtifactEventStorageColumns {
    pub(crate) artifact_path: String,
    pub(crate) event_ordinal: i64,
    pub(crate) timestamp_ms: i64,
    pub(crate) kind: String,
    pub(crate) language: String,
    pub(crate) method: String,
    pub(crate) target: String,
    pub(crate) query: String,
    pub(crate) project_root: String,
    pub(crate) project_root_arg: String,
    pub(crate) bytes: i64,
}

/// Graph-turbo artifact event row stored in the active DB Engine.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientDbArtifactEvent {
    artifact_path: ClientDbArtifactPath,
    event_ordinal: ClientDbEventOrdinal,
    timestamp_ms: ClientDbEventTimestampMillis,
    kind: ClientDbArtifactEventKind,
    language: LanguageId,
    method: ClientDbArtifactEventMethod,
    target: ClientDbArtifactEventTarget,
    query: ClientDbArtifactEventQuery,
    project_root: ClientDbArtifactEventProjectRoot,
    project_root_arg: ClientDbArtifactEventProjectRootArg,
    bytes: ClientDbArtifactEventByteCount,
}

/// Merkle hash value used by artifact graph roots, edges, and proof receipts.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientDbArtifactHash {
    pub(crate) algorithm: String,
    pub(crate) value: String,
}

/// Queryable Merkle artifact root stored in the active DB Engine.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientDbArtifactRoot {
    pub(crate) repo_id: String,
    pub(crate) workspace_id: String,
    pub(crate) scope_id: String,
    pub(crate) generation: String,
    pub(crate) root_kind: String,
    pub(crate) root_hash: ClientDbArtifactHash,
    pub(crate) node_hash: ClientDbArtifactHash,
    pub(crate) producer_hash: Option<ClientDbArtifactHash>,
    pub(crate) schema_hash: Option<ClientDbArtifactHash>,
    pub(crate) content_hash: Option<ClientDbArtifactHash>,
}

/// Queryable edge between two Merkle artifact roots.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientDbArtifactEdge {
    pub edge_hash: ClientDbArtifactHash,
    pub role: String,
    pub ordinal: u32,
    pub parent: ClientDbArtifactRoot,
    pub child: ClientDbArtifactRoot,
}

/// Repair-chain frame linking howFrom/howFix/change/proof artifacts.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientDbArtifactRepairChainFrame {
    pub(crate) frame_kind: String,
    pub(crate) root: ClientDbArtifactRoot,
    pub(crate) content_hash: ClientDbArtifactHash,
    pub(crate) parents: Vec<ClientDbArtifactEdge>,
}

/// Compact proof receipt summary persisted for artifact graph queries.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientDbProofReceipt {
    pub(crate) receipt_id: String,
    pub(crate) obligation_id: String,
    pub(crate) recipe_id: String,
    pub(crate) checker: String,
    pub(crate) environment: String,
    pub(crate) okay: bool,
    pub(crate) trust_level: String,
    pub(crate) summary_for_agent: String,
    pub(crate) root: ClientDbArtifactRoot,
}

/// Compact agent-facing render of queryable artifact graph facts.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientDbArtifactGraphCompactRender {
    pub(crate) frame_count: u32,
    pub(crate) proof_receipt_count: u32,
    pub(crate) lines: Vec<String>,
}

impl ClientDbArtifactGraphCompactRender {
    /// Render as newline-delimited compact graph facts.
    #[must_use]
    pub fn to_text(&self) -> String {
        self.lines.join("\n")
    }
}

impl From<agent_semantic_artifacts::ArtifactHash> for ClientDbArtifactHash {
    fn from(hash: agent_semantic_artifacts::ArtifactHash) -> Self {
        Self {
            algorithm: hash.algorithm,
            value: hash.value,
        }
    }
}

impl From<&agent_semantic_artifacts::ArtifactHash> for ClientDbArtifactHash {
    fn from(hash: &agent_semantic_artifacts::ArtifactHash) -> Self {
        Self {
            algorithm: hash.algorithm.clone(),
            value: hash.value.clone(),
        }
    }
}

impl From<agent_semantic_artifacts::ArtifactRootRef> for ClientDbArtifactRoot {
    fn from(root: agent_semantic_artifacts::ArtifactRootRef) -> Self {
        Self {
            repo_id: root.repo_id.as_str().to_string(),
            workspace_id: root.workspace_id.as_str().to_string(),
            scope_id: root.scope_id.as_str().to_string(),
            generation: root.generation.as_str().to_string(),
            root_kind: root.root_kind.as_str().to_string(),
            root_hash: root.root_hash.into(),
            node_hash: root.node_hash.into(),
            producer_hash: root.producer_hash.map(Into::into),
            schema_hash: root.schema_hash.map(Into::into),
            content_hash: root.content_hash.map(Into::into),
        }
    }
}

impl From<&agent_semantic_artifacts::ArtifactRootRef> for ClientDbArtifactRoot {
    fn from(root: &agent_semantic_artifacts::ArtifactRootRef) -> Self {
        Self {
            repo_id: root.repo_id.as_str().to_string(),
            workspace_id: root.workspace_id.as_str().to_string(),
            scope_id: root.scope_id.as_str().to_string(),
            generation: root.generation.as_str().to_string(),
            root_kind: root.root_kind.as_str().to_string(),
            root_hash: (&root.root_hash).into(),
            node_hash: (&root.node_hash).into(),
            producer_hash: root.producer_hash.as_ref().map(Into::into),
            schema_hash: root.schema_hash.as_ref().map(Into::into),
            content_hash: root.content_hash.as_ref().map(Into::into),
        }
    }
}

impl From<agent_semantic_artifacts::ArtifactRootEdge> for ClientDbArtifactEdge {
    fn from(edge: agent_semantic_artifacts::ArtifactRootEdge) -> Self {
        Self {
            edge_hash: edge.edge_hash.into(),
            role: edge.role,
            ordinal: u32::try_from(edge.ordinal).unwrap_or(u32::MAX),
            parent: edge.parent.into(),
            child: edge.child.into(),
        }
    }
}

impl From<&agent_semantic_artifacts::ArtifactRootEdge> for ClientDbArtifactEdge {
    fn from(edge: &agent_semantic_artifacts::ArtifactRootEdge) -> Self {
        Self {
            edge_hash: (&edge.edge_hash).into(),
            role: edge.role.clone(),
            ordinal: u32::try_from(edge.ordinal).unwrap_or(u32::MAX),
            parent: (&edge.parent).into(),
            child: (&edge.child).into(),
        }
    }
}

impl From<agent_semantic_artifacts::RepairChainFrame> for ClientDbArtifactRepairChainFrame {
    fn from(frame: agent_semantic_artifacts::RepairChainFrame) -> Self {
        Self {
            frame_kind: frame.frame_kind.as_str().to_string(),
            root: (&frame.root).into(),
            content_hash: frame.content_hash.into(),
            parents: repair_chain_parent_edges(&frame.root, &frame.parents),
        }
    }
}

impl From<&agent_semantic_artifacts::RepairChainFrame> for ClientDbArtifactRepairChainFrame {
    fn from(frame: &agent_semantic_artifacts::RepairChainFrame) -> Self {
        Self {
            frame_kind: frame.frame_kind.as_str().to_string(),
            root: (&frame.root).into(),
            content_hash: (&frame.content_hash).into(),
            parents: repair_chain_parent_edges(&frame.root, &frame.parents),
        }
    }
}

fn repair_chain_parent_edges(
    child: &agent_semantic_artifacts::ArtifactRootRef,
    parents: &[agent_semantic_artifacts::RepairChainParentRef],
) -> Vec<ClientDbArtifactEdge> {
    parents
        .iter()
        .map(|parent| {
            agent_semantic_artifacts::build_artifact_root_edge(
                agent_semantic_artifacts::ArtifactRootEdgeInput::new(
                    parent.role.clone(),
                    parent.root.clone(),
                    child.clone(),
                )
                .with_ordinal(parent.ordinal),
            )
            .into()
        })
        .collect()
}

/// Named lookup request for normalized syntax query replay rows.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientDbSyntaxQueryLookup {
    pub db_path: PathBuf,
    pub language_id: LanguageId,
    pub provider_id: ProviderId,
    pub project_root: PathBuf,
    pub query_ast_fingerprint: String,
    pub selector: Option<String>,
}

/// Semantic tree-sitter query input form captured in a replay row.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ClientDbSyntaxQueryInputKind {
    Inline,
    Catalog,
}

impl ClientDbSyntaxQueryInputKind {
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Inline => "inline",
            Self::Catalog => "catalog",
        }
    }

    pub(crate) fn from_wire(value: &str) -> Self {
        if value == "catalog" {
            Self::Catalog
        } else {
            Self::Inline
        }
    }
}

/// One syntax capture row returned by a replay lookup.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ClientDbSyntaxCaptureReplay {
    pub match_locator: String,
    pub capture_locator: String,
    pub capture_name: String,
    pub capture_node_type: ClientDbSyntaxNodeType,
    pub item_node_type: ClientDbSyntaxNodeType,
    pub field: Option<String>,
    pub text: String,
}

/// Typed syntax node kind observed in replayable syntax query rows.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ClientDbSyntaxNodeType(String);

impl ClientDbSyntaxNodeType {
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for ClientDbSyntaxNodeType {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl PartialEq<&str> for ClientDbSyntaxNodeType {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

impl PartialEq<ClientDbSyntaxNodeType> for &str {
    fn eq(&self, other: &ClientDbSyntaxNodeType) -> bool {
        *self == other.as_str()
    }
}

/// Normalized syntax query rows that can render compact locator/capture output.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ClientDbSyntaxQueryReplay {
    pub generation_id: CacheGenerationId,
    pub language_id: LanguageId,
    pub grammar_id: String,
    pub grammar_profile_version: String,
    pub input_form: String,
    pub input_kind: ClientDbSyntaxQueryInputKind,
    pub compiled_source: String,
    pub captures: Vec<String>,
    pub query_ast_fingerprint: String,
    pub artifact_id: Option<CacheArtifactId>,
    pub packet_bytes: Option<u64>,
    pub file_hashes: Vec<ClientCacheFileHash>,
    pub rows: Vec<ClientDbSyntaxCaptureReplay>,
}

/// Resolve a project root into the canonical DB Engine wire identity.
pub fn normalized_project_root(project_root: &Path) -> Result<String, String> {
    project_root
        .canonicalize()
        .map_err(|error| {
            format!(
                "failed to canonicalize DB Engine project root {}: {error}",
                project_root.display()
            )
        })?
        .into_os_string()
        .into_string()
        .map_err(|_| {
            format!(
                "DB Engine project root is not UTF-8: {}",
                project_root.display()
            )
        })
}
