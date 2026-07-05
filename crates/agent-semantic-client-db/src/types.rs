//! Shared DB Engine DTOs used by Turso adapters and client-facing receipts.

use std::path::{Path, PathBuf};

use agent_semantic_client_core::{
    CacheArtifactId, CacheExportMethod, CacheGenerationId, ClientCacheFileHash,
    ClientDbJournalMode, ClientDbStatus, LanguageId, ProviderId, SemanticSchemaId,
};
use serde::{Deserialize, Serialize};

/// Current Turso DB Engine schema version for the local agent semantic client DB.
pub const AGENT_SEMANTIC_CLIENT_DB_SCHEMA_VERSION: i64 = 1;

/// Runtime DB pragmas retained for receipt shape compatibility.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientDbRuntimePragmas {
    pub journal_mode: ClientDbJournalMode,
    pub synchronous: i64,
    pub busy_timeout_ms: i64,
    pub foreign_keys: bool,
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
    pub structural_index_generation_count: u32,
    pub structural_index_owner_count: u32,
    pub structural_index_symbol_count: u32,
    pub structural_index_dependency_usage_count: u32,
    pub source_index_generation_count: u32,
    pub source_index_owner_count: u32,
    pub source_index_selector_count: u32,
    pub artifact_event_count: u32,
    pub raw_source_stored: bool,
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

/// Cached provider command selection for one activation context.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientDbProviderCommandSelection {
    pub manifest_id: String,
    pub manifest_digest: String,
    pub language_id: String,
    pub provider_id: String,
    pub binary: String,
    pub execution: String,
    pub provider_command_prefix: Vec<String>,
    pub executable_path: Option<String>,
    pub executable_len: Option<i64>,
    pub executable_mtime_ms: Option<i64>,
}

impl ClientDbProviderCommandSelection {
    #[must_use]
    pub fn new(
        manifest_id: String,
        manifest_digest: String,
        language_id: String,
        provider_id: String,
        binary: String,
        execution: String,
        provider_command_prefix: Vec<String>,
        executable_path: Option<String>,
        executable_len: Option<i64>,
        executable_mtime_ms: Option<i64>,
    ) -> Self {
        Self {
            manifest_id,
            manifest_digest,
            language_id,
            provider_id,
            binary,
            execution,
            provider_command_prefix,
            executable_path,
            executable_len,
            executable_mtime_ms,
        }
    }

    #[must_use]
    pub fn manifest_id(&self) -> &str {
        &self.manifest_id
    }

    #[must_use]
    pub fn manifest_digest(&self) -> &str {
        &self.manifest_digest
    }

    #[must_use]
    pub fn language_id(&self) -> &str {
        &self.language_id
    }

    #[must_use]
    pub fn provider_id(&self) -> &str {
        &self.provider_id
    }

    #[must_use]
    pub fn binary(&self) -> &str {
        &self.binary
    }

    #[must_use]
    pub fn execution(&self) -> &str {
        &self.execution
    }

    #[must_use]
    pub fn provider_command_prefix(&self) -> &[String] {
        &self.provider_command_prefix
    }

    #[must_use]
    pub fn executable_path(&self) -> Option<&str> {
        self.executable_path.as_deref()
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
    #[must_use]
    pub fn artifact_path(&self) -> &str {
        &self.artifact_path
    }

    #[must_use]
    pub fn event_ordinal(&self) -> u32 {
        self.event_ordinal
    }

    #[must_use]
    pub fn timestamp_ms(&self) -> i64 {
        self.timestamp_ms
    }

    #[must_use]
    pub fn kind(&self) -> &str {
        &self.kind
    }

    #[must_use]
    pub fn language(&self) -> &str {
        &self.language
    }

    #[must_use]
    pub fn method(&self) -> &str {
        &self.method
    }

    #[must_use]
    pub fn target(&self) -> &str {
        &self.target
    }

    #[must_use]
    pub fn query(&self) -> &str {
        &self.query
    }

    #[must_use]
    pub fn project_root(&self) -> &str {
        &self.project_root
    }

    #[must_use]
    pub fn project_root_arg(&self) -> &str {
        &self.project_root_arg
    }

    #[must_use]
    pub fn bytes(&self) -> u64 {
        self.bytes
    }
}

/// Graph-turbo artifact event row stored in the active DB Engine.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientDbArtifactEvent {
    pub artifact_path: String,
    pub event_ordinal: u32,
    pub timestamp_ms: i64,
    pub kind: String,
    pub language: String,
    pub method: String,
    pub target: String,
    pub query: String,
    pub project_root: String,
    pub project_root_arg: String,
    pub bytes: u64,
}

/// Merkle hash value used by artifact graph roots, edges, and proof receipts.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientDbArtifactHash {
    pub algorithm: String,
    pub value: String,
}

/// Queryable Merkle artifact root stored in the active DB Engine.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientDbArtifactRoot {
    pub repo_id: String,
    pub workspace_id: String,
    pub scope_id: String,
    pub generation: String,
    pub root_kind: String,
    pub root_hash: ClientDbArtifactHash,
    pub node_hash: ClientDbArtifactHash,
    pub producer_hash: Option<ClientDbArtifactHash>,
    pub schema_hash: Option<ClientDbArtifactHash>,
    pub content_hash: Option<ClientDbArtifactHash>,
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
    pub frame_kind: String,
    pub root: ClientDbArtifactRoot,
    pub content_hash: ClientDbArtifactHash,
    pub parents: Vec<ClientDbArtifactEdge>,
}

/// Compact proof receipt summary persisted for artifact graph queries.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientDbProofReceipt {
    pub receipt_id: String,
    pub obligation_id: String,
    pub recipe_id: String,
    pub checker: String,
    pub environment: String,
    pub okay: bool,
    pub trust_level: String,
    pub summary_for_agent: String,
    pub root: ClientDbArtifactRoot,
}

/// Compact agent-facing render of queryable artifact graph facts.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientDbArtifactGraphCompactRender {
    pub frame_count: u32,
    pub proof_receipt_count: u32,
    pub lines: Vec<String>,
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

/// Normalize a project root into the DB Engine wire path form.
#[must_use]
pub fn normalized_project_root(project_root: &Path) -> String {
    project_root
        .canonicalize()
        .unwrap_or_else(|_| project_root.to_path_buf())
        .to_string_lossy()
        .into_owned()
}
