//! Execution receipts emitted by the `agent-semantic-client` command layer.

use serde::{Deserialize, Serialize};

use crate::cache_manifest::{CacheManifestReport, CacheManifestStatus};
use crate::request::ClientMethod;
use crate::types::{
    ByteCount, CacheStatus, ClientCachePath, ClientDbStatus, CompactArtifactId, ElapsedMillis,
    LanguageId, ProviderId, SemanticProtocolId, SemanticProtocolVersion, SemanticSchemaId,
    SemanticSchemaVersion,
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
    pub elapsed_ms: ElapsedMillis,
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
    pub client_db_path: Option<ClientCachePath>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_db_status: Option<ClientDbStatus>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_db_generation_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_db_raw_source_stored: Option<bool>,
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
            cache_root: None,
            cache_manifest_path: None,
            cache_manifest_status: None,
            cache_generation_count: None,
            raw_source_stored: None,
            client_db_path: None,
            client_db_status: None,
            client_db_generation_count: None,
            client_db_raw_source_stored: None,
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
            cache_status: CacheStatus::Disabled,
            provider_command_count: 0,
            provider_processes_spawned: 0,
            provider_commands: Vec::new(),
            native_provenance: provenance,
            compact_artifact_id: None,
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
            client_db_path: None,
            client_db_status: None,
            client_db_generation_count: None,
            client_db_raw_source_stored: None,
        }
    }
}
