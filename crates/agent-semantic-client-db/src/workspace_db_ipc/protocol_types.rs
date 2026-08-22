//! Serializable request, response, operation, and receipt types for workspace IPC.

use std::path::PathBuf;

use agent_semantic_client_core::LanguageId;
use serde::{Deserialize, Serialize};

use crate::{
    ClientDbSourceIndexLookupResult, ProviderIncrementalOwnerSnapshot,
    ProviderIncrementalOwnerWrite, ProviderIncrementalScoped, ProviderIncrementalWriteReceipt,
    ProviderOwnerBatchProbeReceipt, ProviderOwnerBatchProbeRequest, ProviderOwnerInventoryWrite,
    ProviderOwnerInventoryWriteReceipt, ProviderOwnerProbe, ProviderTreeSitterContinuation,
    ProviderTreeSitterOwnerResult, ProviderTreeSitterOwnerWriteReceipt,
    ProviderTreeSitterQueryIdentity, ProviderTreeSitterQueryRead, TursoResidentSelectorQuery,
    TursoResidentSelectorRead, WorkspaceDbWriteFinishMode, WorkspaceDbWriteFinishReceipt,
};

use crate::workspace_db_ipc::{
    AgentSessionRegistryIpcOperation, AgentSessionRegistryIpcResult, RuntimeGraphFactSource,
    RuntimeGraphFactsRead, deserialize_changed_paths, deserialize_mutation_id,
};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeResidentReadWorkCounters {
    pub database_opens: u64,
    pub filesystem_reads: u64,
    pub provider_spawns: u64,
    pub control_socket_roundtrips: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeResidentReadEvidence {
    pub schema_id: String,
    pub schema_version: String,
    pub operation_id: String,
    pub workspace_identity: String,
    pub generation_digest: String,
    pub root_digest: String,
    pub read_state: String,
    pub elapsed_micros: u64,
    pub work_counters: RuntimeResidentReadWorkCounters,
    pub terminal_state: String,
    pub telemetry_digest: String,
}

pub fn resident_read_terminal_digest(
    operation_id: &str,
    surface: &str,
    workspace_identity: &str,
    generation_digest: &str,
    root_digest: &str,
    read_state: &str,
    elapsed_micros: u64,
    terminal_state: &str,
) -> String {
    use sha2::Digest;
    let canonical = format!(
        "{operation_id}|{surface}|{workspace_identity}|{generation_digest}|{root_digest}|{read_state}|{elapsed_micros}|{terminal_state}"
    );
    format!("sha256:{:x}", sha2::Sha256::digest(canonical.as_bytes()))
}

impl RuntimeResidentReadEvidence {
    pub fn validate(&self) -> Result<(), String> {
        let valid = self.telemetry_digest.len() == 71
            && self.telemetry_digest.starts_with("sha256:")
            && self.telemetry_digest[7..]
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit());
        if !valid {
            return Err("runtime resident read telemetryDigest is invalid".to_owned());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RuntimeResidentReadResult<T> {
    pub value: T,
    pub evidence: RuntimeResidentReadEvidence,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "action",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum RuntimeCacheControlRequest {
    Status {
        project_root: String,
    },
    RefreshSourceIndex {
        project_root: String,
        expected_generation: Option<String>,
    },
    RebuildSourceIndex {
        project_root: String,
        #[serde(deserialize_with = "deserialize_mutation_id")]
        mutation_id: String,
    },
    Invalidate {
        project_root: String,
        #[serde(deserialize_with = "deserialize_mutation_id")]
        mutation_id: String,
        scope: RuntimeCacheInvalidationScope,
    },
    ApplyOwnerDelta {
        project_root: String,
        #[serde(deserialize_with = "deserialize_mutation_id")]
        mutation_id: String,
        changed_paths: Vec<String>,
        removed_paths: Vec<String>,
        fallback_policy: RuntimeCacheOwnerDeltaFallbackPolicy,
    },
}

impl RuntimeCacheControlRequest {
    #[must_use]
    pub fn project_root(&self) -> &str {
        match self {
            Self::Status { project_root }
            | Self::RefreshSourceIndex { project_root, .. }
            | Self::RebuildSourceIndex { project_root, .. }
            | Self::Invalidate { project_root, .. }
            | Self::ApplyOwnerDelta { project_root, .. } => project_root,
        }
    }

    #[must_use]
    pub fn mutation_id(&self) -> Option<&str> {
        match self {
            Self::RebuildSourceIndex { mutation_id, .. }
            | Self::Invalidate { mutation_id, .. }
            | Self::ApplyOwnerDelta { mutation_id, .. } => Some(mutation_id),
            Self::Status { .. } | Self::RefreshSourceIndex { .. } => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeCacheInvalidationScope {
    WorkspaceGeneration,
    ProviderOwners,
    SyntaxRows,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeCacheOwnerDeltaFallbackPolicy {
    FailClosed,
    FullGeneration,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeCacheGenerationState {
    Missing,
    Ready,
    Stale,
    Rebuilding,
    Invalidated,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeCacheControlReceipt {
    pub action: String,
    pub authority: String,
    pub generation_state: RuntimeCacheGenerationState,
    pub generation_digest: Option<String>,
    pub database_opens_by_client: u64,
    pub writer_queue_owner: String,
    pub mutation_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure: Option<String>,
}

pub const RUNTIME_MERKLE_OWNER_READ_REQUEST_SCHEMA_ID: &str =
    "agent.semantic-protocols.runtime-merkle-owner-read-request";

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimeMerkleOwnerReadRequest {
    pub schema_id: String,
    pub schema_version: String,
    pub project_root: String,
    pub owner_path: String,
}

impl RuntimeMerkleOwnerReadRequest {
    pub fn new(project_root: impl Into<String>, owner_path: impl Into<String>) -> Self {
        Self {
            schema_id: RUNTIME_MERKLE_OWNER_READ_REQUEST_SCHEMA_ID.to_owned(),
            schema_version: "1".to_owned(),
            project_root: project_root.into(),
            owner_path: owner_path.into(),
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema_id != RUNTIME_MERKLE_OWNER_READ_REQUEST_SCHEMA_ID
            || self.schema_version != "1"
        {
            return Err("runtime Merkle owner read request schema mismatch".to_owned());
        }
        if self.project_root.trim().is_empty() || self.owner_path.trim().is_empty() {
            return Err(
                "runtime Merkle owner read request requires project root and owner path".to_owned(),
            );
        }
        let owner_path = std::path::Path::new(&self.owner_path);
        if owner_path.is_absolute()
            || owner_path.components().any(|component| {
                matches!(
                    component,
                    std::path::Component::CurDir | std::path::Component::ParentDir
                )
            })
        {
            return Err("runtime Merkle owner path must be normalized and relative".to_owned());
        }
        Ok(())
    }
}

/// Typed workspace operation accepted by the Runtime Server data-plane protocol.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase"
)]
pub enum WorkspaceDbIpcOperation {
    Health,
    Shutdown,
    CacheControl {
        request: RuntimeCacheControlRequest,
    },
    ReadSourceIndex {
        request: WorkspaceDbSourceIndexLookupRequest,
    },
    ReadRuntimeSourceIndex {
        request: WorkspaceDbSourceIndexLookupRequest,
    },
    ReadTreeSitterInventory {
        request: WorkspaceDbSourceIndexLookupRequest,
    },
    ReadRuntimeGraphFacts {
        project_root: String,
        sources: Vec<RuntimeGraphFactSource>,
    },
    ReadRuntimeSelector {
        project_root: String,
        language_id: LanguageId,
        projection_kind: crate::runtime_server_workspace::ExactProjectionKind,
        structural_selector: String,
    },
    ReadRuntimeExactProjection {
        project_root: String,
        language_id: LanguageId,
        projection_kind: crate::runtime_server_workspace::ExactProjectionKind,
        structural_selector: String,
    },
    ProviderSearch {
        operation_id: String,
        project_root: String,
        language_id: LanguageId,
        args: Vec<String>,
    },
    ReadRuntimeOwner {
        project_root: String,
        owner_path: String,
    },
    ReadRuntimeMerkleOwner {
        request: RuntimeMerkleOwnerReadRequest,
    },
    ProjectProviderOwner {
        project_root: String,
        language_id: LanguageId,
        owner_path: String,
    },
    ResolveProviderRuntime {
        project_root: String,
        language_id: LanguageId,
    },
    ProjectTreeSitterQuery {
        project_root: String,
        language_id: LanguageId,
        args: Vec<String>,
    },
    ReadRuntimeSearchGenerationAuthority {
        project_root: String,
    },
    PublishRuntimeSelectorOverlay {
        project_root: String,
        overlay: crate::runtime_server_workspace::WorkspaceRuntimeSelectorOverlay,
    },
    RebindRuntimeSelectorOverlay {
        project_root: String,
        rebind: crate::runtime_server_workspace::WorkspaceRuntimeSelectorRebind,
    },
    RequireRuntimeGeneration {
        project_root: String,
    },
    RestoreRuntimeGenerationFromPointer {
        project_root: String,
    },
    AdmitRuntimeGenerationForRead {
        project_root: String,
        language_id: String,
        provider_id: String,
    },
    AdmitRuntimeGeneration {
        #[serde(deserialize_with = "deserialize_mutation_id")]
        mutation_id: String,
        project_root: String,
        #[serde(deserialize_with = "deserialize_changed_paths")]
        changed_paths: Vec<String>,
        candidate: crate::runtime_server_admission::WorkspaceGenerationCandidateIdentity,
    },
    SubmitRuntimeGenerationMutation {
        #[serde(deserialize_with = "deserialize_mutation_id")]
        mutation_id: String,
        project_root: String,
        #[serde(deserialize_with = "deserialize_changed_paths")]
        changed_paths: Vec<String>,
    },
    ReadRuntimeGenerationDurability {
        project_root: String,
    },
    EvaluateGraphTurbo {
        project_root: String,
        message: serde_json::Value,
    },
    AgentSessionRegistry {
        project_root: String,
        operation: AgentSessionRegistryIpcOperation,
    },
    RefreshCodexMultiAgentControlPlane {
        project_id: String,
        root_session_id: String,
    },
    ReadCodexMultiAgentControlPlane {
        root_session_id: String,
    },
    WriteProviderIncrementalOwner {
        request: ProviderIncrementalOwnerWrite,
    },
    ReadProviderTreeSitterQuery {
        query: ProviderTreeSitterQueryIdentity,
        incremental_budget: u32,
        continuation: Option<ProviderTreeSitterContinuation>,
    },
    ReadProviderOwnerSnapshot {
        scope: ProviderIncrementalScoped,
        owner_path: String,
    },
    ReadProviderOwnerWarm {
        scope: ProviderIncrementalScoped,
        owner: ProviderOwnerBatchProbeRequest,
    },
    ReadResidentSelector {
        request: TursoResidentSelectorQuery,
    },
    WriteProviderTreeSitterOwnerResult {
        query: ProviderTreeSitterQueryIdentity,
        result: ProviderTreeSitterOwnerResult,
    },
    ProbeProviderOwners {
        scope: ProviderIncrementalScoped,
        owners: Vec<ProviderOwnerBatchProbeRequest>,
    },
    UpsertProviderInventory {
        request: ProviderOwnerInventoryWrite,
    },
    FinishWrites {
        scope: ProviderIncrementalScoped,
        mode: WorkspaceDbWriteFinishMode,
    },
    PublishRuntimeOwner {
        project_root: String,
        owner: crate::runtime_server_workspace::WorkspaceOwnerSnapshot,
    },
    TombstoneRuntimeOwner {
        project_root: String,
        owner_path: String,
    },
    RelocateRuntimeOwner {
        project_root: String,
        previous_owner_path: String,
        owner: crate::runtime_server_workspace::WorkspaceOwnerSnapshot,
    },
}

/// Versioned request envelope; it cannot carry SQL, paths, or Turso handles.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceDbIpcRequest {
    pub schema_id: String,
    pub schema_version: String,
    pub workspace_identity: String,
    pub transport_contract_digest: String,
    pub owner_epoch: u64,
    pub binding_token: String,
    pub request_id: String,
    pub operation: WorkspaceDbIpcOperation,
}

/// Typed workspace result returned by the Runtime Server data plane.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "state",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase"
)]
pub enum WorkspaceDbIpcResult {
    Healthy,
    ShutdownAccepted,
    CacheControl {
        receipt: RuntimeCacheControlReceipt,
    },
    SourceIndex {
        lookup: ClientDbSourceIndexLookupResult,
        evidence: RuntimeResidentReadEvidence,
    },
    RuntimeGraphFacts {
        read: RuntimeGraphFactsRead,
    },
    SourceIndexGeneration {
        receipt: crate::ClientDbSourceIndexRefreshReport,
    },
    ProviderIncrementalOwner {
        receipt: ProviderIncrementalWriteReceipt,
    },
    ProviderTreeSitterQuery {
        read: ProviderTreeSitterQueryRead,
    },
    ProviderOwnerSnapshot {
        snapshot: Option<ProviderIncrementalOwnerSnapshot>,
    },
    ProviderOwnerWarm {
        probe: ProviderOwnerProbe,
        snapshot: Option<ProviderIncrementalOwnerSnapshot>,
    },
    ResidentSelector {
        read: Option<TursoResidentSelectorRead>,
    },
    ProviderTreeSitterOwner {
        receipt: ProviderTreeSitterOwnerWriteReceipt,
    },
    ProviderOwners {
        receipt: ProviderOwnerBatchProbeReceipt,
    },
    ProviderInventory {
        receipt: ProviderOwnerInventoryWriteReceipt,
    },
    WriteFinish {
        receipt: WorkspaceDbWriteFinishReceipt,
    },
    RuntimeGeneration {
        receipt: crate::runtime_server_workspace::WorkspaceRecoveryReceipt,
    },
    RuntimeGenerationResidentReady {
        receipt: crate::runtime_server_workspace::WorkspaceRecoveryReceipt,
    },
    RuntimeGenerationDurability {
        receipt: Option<crate::runtime_server_workspace::WorkspaceGenerationDurabilityReceipt>,
    },
    RuntimeGenerationMutationAdmission {
        receipt: crate::runtime_server_admission::WorkspaceGenerationMutationAdmissionReceipt,
    },
    RuntimeGenerationMutationSubmission {
        receipt: crate::runtime_server_admission::WorkspaceGenerationMutationSubmissionReceipt,
    },
    GraphTurboEvaluation {
        workspace_identity: String,
        project_root: String,
        receipt: serde_json::Value,
    },
    AgentSessionRegistry {
        result: AgentSessionRegistryIpcResult,
    },
    CodexMultiAgentControlPlanePublication {
        receipt: crate::codex_multi_agent_control_plane_owner::CodexControlPlanePublicationReceipt,
    },
    CodexMultiAgentControlPlane {
        projection: Option<agent_semantic_context_product::codex_multi_agent_v2_control_plane::CodexMultiAgentV2ControlPlaneProjection>,
    },
    RuntimeSelector {
        read: crate::runtime_server_workspace::WorkspaceRuntimeSelectorRead,
        evidence: RuntimeResidentReadEvidence,
    },
    RuntimeOwner {
        read: crate::runtime_server_workspace::WorkspaceRuntimeOwnerRead,
        evidence: RuntimeResidentReadEvidence,
    },
    RuntimeMerkleOwner {
        read: crate::runtime_server_workspace::WorkspaceRuntimeMerkleOwnerRead,
        evidence: RuntimeResidentReadEvidence,
    },
    ProviderOwnerProjection {
        owner: crate::runtime_server_workspace::WorkspaceOwnerSnapshot,
    },
    ProviderRuntime {
        runtime: serde_json::Value,
    },
    ProviderSearch {
        receipt: crate::runtime_search_service::RuntimeProviderSearchReceipt,
    },
    TreeSitterQuery {
        rendered: Option<String>,
    },
    RuntimeSearchGenerationAuthority {
        authority: Option<crate::runtime_server_workspace::WorkspaceSearchGenerationAuthority>,
    },
    RuntimeSelectorOverlay {
        receipt: crate::runtime_server_workspace::WorkspaceRuntimeSelectorOverlayReceipt,
    },
    Failed {
        code: String,
        message: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceDbSourceIndexLookupRequest {
    pub project_root: PathBuf,
    pub indexed_project_root: PathBuf,
    pub query: String,
    pub language_id: Option<LanguageId>,
    pub limit: u32,
}

/// Versioned response bound to the serving workspace and owner epoch.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceDbIpcResponse {
    pub schema_id: String,
    pub schema_version: String,
    pub workspace_identity: String,
    pub transport_contract_digest: String,
    pub owner_epoch: u64,
    pub request_id: String,
    pub result: WorkspaceDbIpcResult,
}

pub const WORKSPACE_DB_OWNER_ENDPOINT_SCHEMA_ID: &str =
    "agent.semantic-protocols.workspace-db-owner-endpoint.v1";
pub const WORKSPACE_DB_OWNER_REQUEST_SCHEMA_ID: &str =
    "agent.semantic-protocols.workspace-db-owner-request.v1";
pub const WORKSPACE_DB_OWNER_RESPONSE_SCHEMA_ID: &str =
    "agent.semantic-protocols.workspace-db-owner-response.v1";
pub const WORKSPACE_DB_OWNER_SCHEMA_VERSION: &str = "1";
