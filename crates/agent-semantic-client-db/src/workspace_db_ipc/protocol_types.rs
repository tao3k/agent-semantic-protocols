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
}

impl RuntimeCacheControlRequest {
    #[must_use]
    pub fn project_root(&self) -> &str {
        match self {
            Self::Status { project_root }
            | Self::RefreshSourceIndex { project_root, .. }
            | Self::RebuildSourceIndex { project_root, .. }
            | Self::Invalidate { project_root, .. } => project_root,
        }
    }

    #[must_use]
    pub fn mutation_id(&self) -> Option<&str> {
        match self {
            Self::RebuildSourceIndex { mutation_id, .. } | Self::Invalidate { mutation_id, .. } => {
                Some(mutation_id)
            }
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
    ReadRuntimeOwner {
        project_root: String,
        owner_path: String,
    },
    ReadRuntimeSearchGenerationAuthority {
        project_root: String,
    },
    PublishRuntimeSelectorOverlay {
        project_root: String,
        overlay: crate::runtime_server_workspace::WorkspaceRuntimeSelectorOverlay,
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
    EnsureRuntimeGeneration {
        project_root: String,
    },
    EnsureRuntimeGenerationReady {
        project_root: String,
    },
    RepairRuntimeGenerationLocator {
        project_root: String,
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
    RuntimeGenerationAdmission {
        receipt: crate::runtime_server_admission::WorkspaceGenerationAdmissionReceipt,
    },
    RuntimeGenerationReadiness {
        receipt: crate::runtime_server_admission::WorkspaceGenerationReadinessReceipt,
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
    },
    RuntimeOwner {
        read: crate::runtime_server_workspace::WorkspaceRuntimeOwnerRead,
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
