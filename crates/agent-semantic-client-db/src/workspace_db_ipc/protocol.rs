//! Typed Unix-domain transport for one workspace database owner epoch.

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
use agent_semantic_client_core::LanguageId;
use std::path::{Path, PathBuf};
use tokio::net::UnixStream;

use super::transport::{read_frame, write_frame};

pub use crate::workspace_db_endpoint::{
    WorkspaceDbOwnerEndpoint, bind_workspace_db_owner, prepare_workspace_db_owner_endpoint,
    workspace_db_owner_runtime_base, workspace_db_owner_transport_contract_digest,
};
pub use crate::workspace_db_owner_election::{
    WorkspaceDbOwnerRetirement, remove_stale_workspace_db_owner_socket,
    try_retire_workspace_db_owner_endpoint,
};

pub(super) const MAX_FRAME_BYTES: usize = 8 * 1024 * 1024;

fn workspace_db_ipc_read_lane_capacity() -> usize {
    crate::runtime_concurrency::RuntimeConcurrencyPlan::current().reader_limit()
}

fn deserialize_changed_paths<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let paths = Vec::<String>::deserialize(deserializer)?;
    if paths.is_empty() {
        return Err(serde::de::Error::custom(
            "changedPaths must contain at least one normalized path",
        ));
    }
    let mut unique = std::collections::BTreeSet::new();
    for path in &paths {
        if path.trim().is_empty() {
            return Err(serde::de::Error::custom(
                "changedPaths must not contain empty paths",
            ));
        }
        if !unique.insert(path) {
            return Err(serde::de::Error::custom(
                "changedPaths must not contain duplicate paths",
            ));
        }
    }
    Ok(paths)
}

fn deserialize_mutation_id<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let mutation_id = String::deserialize(deserializer)?;
    if mutation_id.trim().is_empty() {
        return Err(serde::de::Error::custom(
            "mutationId must be non-empty text",
        ));
    }
    Ok(mutation_id)
}

#[path = "client.rs"]
mod client;
#[path = "session.rs"]
mod session;
pub use client::{connect_runtime_server_workspace_session, read_source_index_via_runtime_server};

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
    ReadRuntimeSelector {
        project_root: String,
        projection_kind: String,
        structural_selector: String,
    },
    EnsureRuntimeOwner {
        project_root: String,
        language_id: String,
        owner_path: String,
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
    EnsureRuntimeGeneration {
        project_root: String,
        candidate: crate::runtime_server_admission::WorkspaceGenerationCandidateIdentity,
    },
    RepairRuntimeGenerationLocator {
        project_root: String,
    },
    ReadRuntimeGenerationDurability {
        project_root: String,
    },
    EvaluateHook {
        project_root: String,
        arguments: Vec<String>,
        input: String,
    },
    EvaluateGraphTurbo {
        project_root: String,
        message: serde_json::Value,
    },
    AgentSessionRegistry {
        project_root: String,
        operation: AgentSessionRegistryIpcOperation,
    },
    PublishCodexMultiAgentControlPlane {
        projection: agent_semantic_context_product::codex_multi_agent_v2_control_plane::CodexMultiAgentV2ControlPlaneProjection,
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
#[serde(tag = "state", rename_all = "kebab-case")]
pub enum WorkspaceDbIpcResult {
    Healthy,
    ShutdownAccepted,
    CacheControl {
        receipt: RuntimeCacheControlReceipt,
    },
    SourceIndex {
        lookup: ClientDbSourceIndexLookupResult,
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
    HookEvaluation {
        workspace_identity: String,
        project_root: String,
        output: String,
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
    RuntimeOwnerFreshness {
        receipt: crate::runtime_server_workspace::WorkspaceRuntimeOwnerFreshnessReceipt,
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

pub use crate::workspace_db_ipc_server::{
    serve_one_workspace_db_ipc_request, serve_one_workspace_db_session_request,
};

static NEXT_WORKSPACE_DB_IPC_CLIENT_ID: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(1);

#[derive(Debug)]
pub struct WorkspaceDbIpcSession {
    endpoint: WorkspaceDbSessionBinding,
    pub(super) shared: std::sync::Arc<WorkspaceDbIpcSessionState>,
}

#[derive(Clone, Debug)]
struct WorkspaceDbSessionBinding {
    workspace_identity: String,
    project_root: Option<PathBuf>,
    transport_contract_digest: String,
    owner_epoch: u64,
    runtime_binary_path: String,
    runtime_binary_digest: String,
    binding_token: String,
    socket_path: String,
}

#[derive(Debug)]
pub(super) struct WorkspaceDbIpcSessionState {
    client_id: u64,
    next_request_id: std::sync::atomic::AtomicU64,
    lanes: Vec<tokio::sync::Mutex<Option<UnixStream>>>,
    pub(super) runtime_generation_mutations:
        std::sync::Arc<super::runtime_generation::MutationWorkspaceLane>,
}

impl Clone for WorkspaceDbIpcSession {
    fn clone(&self) -> Self {
        Self {
            endpoint: self.endpoint.clone(),
            shared: std::sync::Arc::clone(&self.shared),
        }
    }
}

impl WorkspaceDbIpcSession {
    pub fn new(endpoint: WorkspaceDbOwnerEndpoint) -> Self {
        let runtime_generation_mutations =
            super::runtime_generation::runtime_generation_mutation_lane(
                &endpoint.socket_path,
                endpoint.owner_epoch,
                &endpoint.workspace_identity,
            );
        Self {
            endpoint: WorkspaceDbSessionBinding {
                workspace_identity: endpoint.workspace_identity,
                project_root: None,
                transport_contract_digest: endpoint.transport_contract_digest,
                owner_epoch: endpoint.owner_epoch,
                runtime_binary_path: endpoint.runtime_binary_path,
                runtime_binary_digest: endpoint.runtime_binary_digest,
                binding_token: endpoint.binding_token,
                socket_path: endpoint.socket_path,
            },
            shared: std::sync::Arc::new(WorkspaceDbIpcSessionState {
                client_id: NEXT_WORKSPACE_DB_IPC_CLIENT_ID
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed),
                next_request_id: std::sync::atomic::AtomicU64::new(1),
                lanes: (0..workspace_db_ipc_read_lane_capacity())
                    .map(|_| tokio::sync::Mutex::new(None))
                    .collect(),
                runtime_generation_mutations,
            }),
        }
    }

    pub fn for_runtime_server(
        endpoint: &crate::runtime_server_control::RuntimeServerEndpoint,
        workspace_identity: impl Into<String>,
        project_root: PathBuf,
    ) -> Self {
        let workspace_identity = workspace_identity.into();
        let runtime_generation_mutations =
            super::runtime_generation::runtime_generation_mutation_lane(
                &endpoint.data_plane_socket_path,
                endpoint.owner_epoch,
                &workspace_identity,
            );
        Self {
            endpoint: WorkspaceDbSessionBinding {
                workspace_identity,
                project_root: Some(project_root),
                transport_contract_digest: endpoint.transport_contract_digest.clone(),
                owner_epoch: endpoint.owner_epoch,
                runtime_binary_path: endpoint.runtime_artifact_path.clone(),
                runtime_binary_digest: endpoint.runtime_artifact_digest.clone(),
                binding_token: endpoint.binding_token.clone(),
                socket_path: endpoint.data_plane_socket_path.clone(),
            },
            shared: std::sync::Arc::new(WorkspaceDbIpcSessionState {
                client_id: NEXT_WORKSPACE_DB_IPC_CLIENT_ID
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed),
                next_request_id: std::sync::atomic::AtomicU64::new(1),
                lanes: (0..workspace_db_ipc_read_lane_capacity())
                    .map(|_| tokio::sync::Mutex::new(None))
                    .collect(),
                runtime_generation_mutations,
            }),
        }
    }

    pub(super) fn runtime_project_root(&self) -> Result<&Path, String> {
        self.endpoint.project_root.as_deref().ok_or_else(|| {
            "Runtime Server operation requires a session-bound project root".to_owned()
        })
    }

    pub(super) async fn call_runtime_generation_admission(
        &self,
        mutation_id: String,
        project_root: String,
        changed_paths: Vec<String>,
    ) -> Result<crate::runtime_server_admission::WorkspaceGenerationMutationAdmissionReceipt, String>
    {
        if changed_paths.is_empty() {
            return Err("runtime generation admission requires changed paths".to_owned());
        }
        if mutation_id.trim().is_empty() {
            return Err("runtime generation admission requires a mutation id".to_owned());
        }
        let expected_mutation_id = mutation_id.clone();
        let candidate = crate::runtime_server_admission::discover_workspace_generation_candidate(
            std::path::Path::new(&project_root),
        )
        .await?;
        match self
            .call_operation(WorkspaceDbIpcOperation::AdmitRuntimeGeneration {
                mutation_id,
                project_root,
                changed_paths,
                candidate,
            })
            .await
        {
            Ok(WorkspaceDbIpcResult::RuntimeGenerationMutationAdmission { receipt }) => {
                receipt.validate()?;
                if receipt.mutation_id != expected_mutation_id {
                    return Err(format!(
                        "Runtime Server returned a mutation admission identity mismatch: expectedMutationId={} actualMutationId={}",
                        expected_mutation_id, receipt.mutation_id
                    ));
                }
                Ok(receipt)
            }
            Ok(_) => {
                Err("Runtime Server returned an unexpected generation admission result".to_owned())
            }
            Err(error) => Err(error),
        }
    }

    pub async fn runtime_generation_durability(
        &self,
        project_root: impl Into<String>,
    ) -> Result<Option<crate::runtime_server_workspace::WorkspaceGenerationDurabilityReceipt>, String>
    {
        match self
            .call_operation(WorkspaceDbIpcOperation::ReadRuntimeGenerationDurability {
                project_root: project_root.into(),
            })
            .await?
        {
            WorkspaceDbIpcResult::RuntimeGenerationDurability { receipt } => {
                if let Some(receipt) = &receipt {
                    receipt.validate()?;
                }
                Ok(receipt)
            }
            _ => {
                Err("Runtime Server returned an unexpected generation durability result".to_owned())
            }
        }
    }

    pub(super) async fn call_operation(
        &self,
        operation: WorkspaceDbIpcOperation,
    ) -> Result<WorkspaceDbIpcResult, String> {
        let sequence = self
            .shared
            .next_request_id
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let request_id = format!("workspace-db-ipc-{}-{sequence}", self.shared.client_id);
        let request = WorkspaceDbIpcRequest {
            schema_id: WORKSPACE_DB_OWNER_REQUEST_SCHEMA_ID.to_owned(),
            schema_version: WORKSPACE_DB_OWNER_SCHEMA_VERSION.to_owned(),
            workspace_identity: self.endpoint.workspace_identity.clone(),
            transport_contract_digest: self.endpoint.transport_contract_digest.clone(),
            owner_epoch: self.endpoint.owner_epoch,
            binding_token: self.endpoint.binding_token.clone(),
            request_id: request_id.clone(),
            operation,
        };
        let first_lane = sequence as usize % self.shared.lanes.len();
        let mut available_lane = None;
        for offset in 0..self.shared.lanes.len() {
            let lane_index = (first_lane + offset) % self.shared.lanes.len();
            if let Ok(lane) = self.shared.lanes[lane_index].try_lock() {
                available_lane = Some(lane);
                break;
            }
        }
        let mut lane = match available_lane {
            Some(lane) => lane,
            None => self.shared.lanes[first_lane].lock().await,
        };
        if lane.is_none() {
            *lane = Some(
                UnixStream::connect(&self.endpoint.socket_path)
                    .await
                    .map_err(|error| {
                        format!("failed to connect workspace owner endpoint: {error}")
                    })?,
            );
        }
        let response: WorkspaceDbIpcResponse = match lane.as_mut() {
            Some(stream) => {
                if let Err(error) = write_frame(stream, &request).await {
                    *lane = None;
                    return Err(error);
                }
                match read_frame(stream).await {
                    Ok(response) => response,
                    Err(error) => {
                        *lane = None;
                        return Err(error);
                    }
                }
            }
            None => return Err("workspace owner IPC lane was not initialized".to_owned()),
        };
        if response.schema_id != WORKSPACE_DB_OWNER_RESPONSE_SCHEMA_ID {
            return Err(format!(
                "workspace owner IPC response schema_id mismatch: expected {:?}, got {:?}",
                WORKSPACE_DB_OWNER_RESPONSE_SCHEMA_ID, response.schema_id
            ));
        }
        if response.schema_version != WORKSPACE_DB_OWNER_SCHEMA_VERSION {
            return Err(format!(
                "workspace owner IPC response schema_version mismatch: expected {:?}, got {:?}",
                WORKSPACE_DB_OWNER_SCHEMA_VERSION, response.schema_version
            ));
        }
        if response.workspace_identity != self.endpoint.workspace_identity {
            return Err(format!(
                "workspace owner IPC response workspace_identity mismatch: expected {:?}, got {:?}",
                self.endpoint.workspace_identity, response.workspace_identity
            ));
        }
        if response.transport_contract_digest != self.endpoint.transport_contract_digest {
            return Err(format!(
                "workspace owner IPC response transport_contract_digest mismatch: expected {:?}, got {:?}",
                self.endpoint.transport_contract_digest, response.transport_contract_digest
            ));
        }
        if response.owner_epoch != self.endpoint.owner_epoch {
            return Err(format!(
                "workspace owner IPC response owner_epoch mismatch: expected {}, got {}",
                self.endpoint.owner_epoch, response.owner_epoch
            ));
        }
        if response.request_id != request_id {
            return Err(format!(
                "workspace owner IPC response request_id mismatch: expected {:?}, got {:?}",
                request_id, response.request_id
            ));
        }
        match response.result {
            WorkspaceDbIpcResult::Failed { code, message } => Err(format!("{code}: {message}")),
            result => Ok(result),
        }
    }
}

#[cfg(test)]
#[path = "../../tests/unit/workspace_db_ipc.rs"]
mod tests;
pub const WORKSPACE_DB_OWNER_ENDPOINT_SCHEMA_ID: &str =
    "agent.semantic-protocols.workspace-db-owner-endpoint.v1";
pub const WORKSPACE_DB_OWNER_REQUEST_SCHEMA_ID: &str =
    "agent.semantic-protocols.workspace-db-owner-request.v1";
pub const WORKSPACE_DB_OWNER_RESPONSE_SCHEMA_ID: &str =
    "agent.semantic-protocols.workspace-db-owner-response.v1";
pub const WORKSPACE_DB_OWNER_SCHEMA_VERSION: &str = "1";
use super::agent_session_registry::{
    AgentSessionRegistryIpcOperation, AgentSessionRegistryIpcResult,
};
