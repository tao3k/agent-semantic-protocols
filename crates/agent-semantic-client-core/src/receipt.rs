//! Execution receipts emitted by the `agent-semantic-client` command layer.

use serde::{Deserialize, Serialize};

use crate::cache_manifest::{CacheManifestReport, CacheManifestStatus};
use crate::request::ClientMethod;
use crate::types::{
    ByteCount, CacheArtifactId, CacheStatus, ClientCachePath, ClientDbBackend,
    ClientDbEngineDurability, ClientDbFileName, ClientDbFutureBackend, ClientDbJournalMode,
    ClientDbStatus, ClientRepoId, ClientScopeId, ClientStateLayoutVersion, ClientWorkspaceId,
    CompactArtifactId, ElapsedMillis, LanguageId, ProviderId, SemanticProtocolId,
    SemanticProtocolVersion, SemanticSchemaId, SemanticSchemaVersion, SyntaxQueryAstAbiFingerprint,
    SyntaxQueryGrammarId, SyntaxQueryGrammarProfileVersion, SyntaxQuerySelector,
};

/// Schema id for `agent-semantic-client-receipt.v1`.
pub const AGENT_SEMANTIC_CLIENT_RECEIPT_SCHEMA_ID: &str = "agent.semantic-protocols.client-receipt";
/// Protocol id for agent semantic client receipts.
pub const AGENT_SEMANTIC_CLIENT_RECEIPT_PROTOCOL_ID: &str = "agent.semantic-protocols.client";
/// Schema version for `agent-semantic-client-receipt.v1`.
pub const AGENT_SEMANTIC_CLIENT_RECEIPT_SCHEMA_VERSION: &str = "1";
/// Protocol version for the agent semantic client receipt envelope.
pub const AGENT_SEMANTIC_CLIENT_RECEIPT_PROTOCOL_VERSION: &str = "1";

/// Execution route selected for a client request.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ExecutionRoute {
    LocalNative,
    LocalCache,
    CloudFlight,
    HybridReroute,
}

/// Native provider provenance attached to client outputs.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeProvenance {
    pub language_id: LanguageId,
    pub provider_id: ProviderId,
    pub provider_binary: String,
}

/// Provider process execution metrics captured for a local-native route.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderCommandReceipt {
    pub language_id: LanguageId,
    pub provider_id: ProviderId,
    pub argv: Vec<String>,
    pub exit_code: i32,
    pub stdout_bytes: ByteCount,
    pub stderr_bytes: ByteCount,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stdout_sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stderr_sha256: Option<String>,
    #[serde(skip_serializing_if = "is_false")]
    pub stdout_truncated: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub stderr_truncated: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub timed_out: bool,
    pub elapsed_ms: ElapsedMillis,
}

/// Structured DB Engine state embedded in client receipts.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientDbEngineReceipt {
    pub backend: ClientDbBackend,
    pub future_backend: ClientDbFutureBackend,
    pub layout_version: ClientStateLayoutVersion,
    pub db_file_name: ClientDbFileName,
    pub schema_version: i64,
    pub durability: ClientDbEngineDurability,
    pub features: ClientDbEngineFeaturesReceipt,
    pub client_dir: ClientCachePath,
    pub db_path: ClientCachePath,
    pub manifest_path: ClientCachePath,
    pub artifact_path: ClientCachePath,
    pub repo_id: ClientRepoId,
    pub workspace_id: ClientWorkspaceId,
    pub scope_id: ClientScopeId,
    pub sqlite_report: ClientDbSqliteReceipt,
}

/// Capability flags reported by the active DB Engine backend.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientDbEngineFeaturesReceipt {
    pub async_io: bool,
    pub concurrent_writes: bool,
    pub fts: bool,
    pub vector: bool,
    pub overlay_search: bool,
    pub sync: bool,
    pub encryption: bool,
}

/// SQLite transition-backend report nested under the DB Engine receipt.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientDbSqliteReceipt {
    pub db_path: ClientCachePath,
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
    pub runtime_pragmas: Option<ClientDbRuntimePragmasReceipt>,
    pub reason: Option<String>,
}

/// Runtime SQLite pragmas observed through the current DB Engine adapter.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientDbRuntimePragmasReceipt {
    pub journal_mode: ClientDbJournalMode,
    pub synchronous: i64,
    pub busy_timeout_ms: u64,
    pub foreign_keys: bool,
}

/// Machine-readable receipt for one `agent-semantic-client` command.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientReceipt {
    pub schema_id: SemanticSchemaId,
    pub schema_version: SemanticSchemaVersion,
    pub protocol_id: SemanticProtocolId,
    pub protocol_version: SemanticProtocolVersion,
    pub method: ClientMethod,
    pub route: ExecutionRoute,
    pub cache_status: CacheStatus,
    pub provider_command_count: u32,
    pub provider_processes_spawned: u32,
    pub provider_commands: Vec<ProviderCommandReceipt>,
    pub native_provenance: Vec<NativeProvenance>,
    pub compact_artifact_id: Option<CompactArtifactId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub syntax_artifact_id: Option<CacheArtifactId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub syntax_query_ast_abi_fingerprint: Option<SyntaxQueryAstAbiFingerprint>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub syntax_query_grammar_id: Option<SyntaxQueryGrammarId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub syntax_query_grammar_profile_version: Option<SyntaxQueryGrammarProfileVersion>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub syntax_query_selector: Option<SyntaxQuerySelector>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub packet_bytes: Option<ByteCount>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sqlite_read_count: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sqlite_write_count: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_writeback_provider_command_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_writeback_provider_processes_spawned: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_writeback_provider_elapsed_ms: Option<ElapsedMillis>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_writeback_provider_commands: Option<Vec<ProviderCommandReceipt>>,
    pub elapsed_ms: ElapsedMillis,
    pub stdout_bytes: ByteCount,
    pub stderr_bytes: ByteCount,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_root: Option<ClientCachePath>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_manifest_path: Option<ClientCachePath>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_manifest_status: Option<CacheManifestStatus>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_generation_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_source_stored: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub db_engine: Option<ClientDbEngineReceipt>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_db_path: Option<ClientCachePath>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_db_status: Option<ClientDbStatus>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_db_generation_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_db_syntax_row_generation_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_db_syntax_row_match_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_db_syntax_row_capture_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_db_structural_index_generation_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_db_structural_index_owner_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_db_structural_index_symbol_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_db_structural_index_dependency_usage_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_db_source_index_generation_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_db_source_index_owner_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_db_source_index_selector_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_db_artifact_event_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_db_raw_source_stored: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_db_journal_mode: Option<ClientDbJournalMode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_db_synchronous: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_db_busy_timeout_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_db_foreign_keys: Option<bool>,
}

fn is_false(value: &bool) -> bool {
    !*value
}

impl ClientReceipt {
    #[must_use]
    pub fn local_native(
        method: ClientMethod,
        provenance: NativeProvenance,
        provider_command: ProviderCommandReceipt,
    ) -> Self {
        Self {
            schema_id: AGENT_SEMANTIC_CLIENT_RECEIPT_SCHEMA_ID.into(),
            schema_version: AGENT_SEMANTIC_CLIENT_RECEIPT_SCHEMA_VERSION.into(),
            protocol_id: AGENT_SEMANTIC_CLIENT_RECEIPT_PROTOCOL_ID.into(),
            protocol_version: AGENT_SEMANTIC_CLIENT_RECEIPT_PROTOCOL_VERSION.into(),
            method,
            route: ExecutionRoute::LocalNative,
            cache_status: CacheStatus::Miss,
            provider_command_count: 1,
            provider_processes_spawned: 1,
            elapsed_ms: provider_command.elapsed_ms,
            stdout_bytes: provider_command.stdout_bytes,
            stderr_bytes: provider_command.stderr_bytes,
            provider_commands: vec![provider_command],
            native_provenance: vec![provenance],
            compact_artifact_id: None,
            syntax_artifact_id: None,
            syntax_query_ast_abi_fingerprint: None,
            syntax_query_grammar_id: None,
            syntax_query_grammar_profile_version: None,
            syntax_query_selector: None,
            packet_bytes: None,
            sqlite_read_count: None,
            sqlite_write_count: None,
            cache_writeback_provider_command_count: None,
            cache_writeback_provider_processes_spawned: None,
            cache_writeback_provider_elapsed_ms: None,
            cache_writeback_provider_commands: None,
            cache_root: None,
            cache_manifest_path: None,
            cache_manifest_status: None,
            cache_generation_count: None,
            raw_source_stored: None,
            db_engine: None,
            client_db_path: None,
            client_db_status: None,
            client_db_generation_count: None,
            client_db_syntax_row_generation_count: None,
            client_db_syntax_row_match_count: None,
            client_db_syntax_row_capture_count: None,
            client_db_structural_index_generation_count: None,
            client_db_structural_index_owner_count: None,
            client_db_structural_index_symbol_count: None,
            client_db_structural_index_dependency_usage_count: None,
            client_db_source_index_generation_count: None,
            client_db_source_index_owner_count: None,
            client_db_source_index_selector_count: None,
            client_db_artifact_event_count: None,
            client_db_raw_source_stored: None,
            client_db_journal_mode: None,
            client_db_synchronous: None,
            client_db_busy_timeout_ms: None,
            client_db_foreign_keys: None,
        }
    }

    #[must_use]
    pub fn cache_status(
        provenance: Vec<NativeProvenance>,
        cache_report: &CacheManifestReport,
    ) -> Self {
        Self::cache_report(ClientMethod::CacheStatus, provenance, cache_report)
    }

    #[must_use]
    pub fn cache_report(
        method: ClientMethod,
        provenance: Vec<NativeProvenance>,
        cache_report: &CacheManifestReport,
    ) -> Self {
        Self {
            schema_id: AGENT_SEMANTIC_CLIENT_RECEIPT_SCHEMA_ID.into(),
            schema_version: AGENT_SEMANTIC_CLIENT_RECEIPT_SCHEMA_VERSION.into(),
            protocol_id: AGENT_SEMANTIC_CLIENT_RECEIPT_PROTOCOL_ID.into(),
            protocol_version: AGENT_SEMANTIC_CLIENT_RECEIPT_PROTOCOL_VERSION.into(),
            method,
            route: ExecutionRoute::LocalCache,
            cache_status: cache_status_for_report(cache_report),
            provider_command_count: 0,
            provider_processes_spawned: 0,
            provider_commands: Vec::new(),
            native_provenance: provenance,
            compact_artifact_id: None,
            syntax_artifact_id: None,
            syntax_query_ast_abi_fingerprint: None,
            syntax_query_grammar_id: None,
            syntax_query_grammar_profile_version: None,
            syntax_query_selector: None,
            packet_bytes: None,
            sqlite_read_count: None,
            sqlite_write_count: None,
            cache_writeback_provider_command_count: None,
            cache_writeback_provider_processes_spawned: None,
            cache_writeback_provider_elapsed_ms: None,
            cache_writeback_provider_commands: None,
            elapsed_ms: ElapsedMillis::new(0),
            stdout_bytes: ByteCount::new(0),
            stderr_bytes: ByteCount::new(0),
            cache_root: cache_report
                .cache_root
                .as_ref()
                .map(|path| ClientCachePath::from_path(path)),
            cache_manifest_path: cache_report
                .manifest_path
                .as_ref()
                .map(|path| ClientCachePath::from_path(path)),
            cache_manifest_status: Some(cache_report.status.clone()),
            cache_generation_count: Some(cache_report.generation_count),
            raw_source_stored: Some(cache_report.raw_source_stored),
            db_engine: None,
            client_db_path: None,
            client_db_status: None,
            client_db_generation_count: None,
            client_db_syntax_row_generation_count: None,
            client_db_syntax_row_match_count: None,
            client_db_syntax_row_capture_count: None,
            client_db_structural_index_generation_count: None,
            client_db_structural_index_owner_count: None,
            client_db_structural_index_symbol_count: None,
            client_db_structural_index_dependency_usage_count: None,
            client_db_source_index_generation_count: None,
            client_db_source_index_owner_count: None,
            client_db_source_index_selector_count: None,
            client_db_artifact_event_count: None,
            client_db_raw_source_stored: None,
            client_db_journal_mode: None,
            client_db_synchronous: None,
            client_db_busy_timeout_ms: None,
            client_db_foreign_keys: None,
        }
    }
}

fn cache_status_for_report(cache_report: &CacheManifestReport) -> CacheStatus {
    match cache_report.status {
        CacheManifestStatus::Unavailable | CacheManifestStatus::Missing => CacheStatus::Miss,
        CacheManifestStatus::Present => CacheStatus::WarmProvider,
        CacheManifestStatus::Invalid => CacheStatus::Stale,
    }
}
