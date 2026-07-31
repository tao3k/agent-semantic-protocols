//! Typed Unix-domain transport for one workspace database owner epoch.

use serde::{Deserialize, Serialize};

use crate::{
    ClientDbSourceIndexLookupResult, ProviderIncrementalOwnerWrite, ProviderIncrementalScoped,
    ProviderIncrementalWriteReceipt, ProviderOwnerBatchProbeReceipt,
    ProviderOwnerBatchProbeRequest, ProviderOwnerInventoryWrite,
    ProviderOwnerInventoryWriteReceipt, ProviderSelectorProjection, ProviderTreeSitterContinuation,
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
    try_acquire_workspace_db_owner_election, try_retire_workspace_db_owner_endpoint,
};

pub(super) const MAX_FRAME_BYTES: usize = 8 * 1024 * 1024;

fn workspace_db_ipc_read_lane_capacity() -> usize {
    crate::runtime_concurrency::RuntimeConcurrencyPlan::current().reader_limit()
}

pub async fn connect_runtime_server_workspace_session(
    project_root: &Path,
) -> Result<WorkspaceDbIpcSession, String> {
    let workspace_identity =
        agent_semantic_client_core::state_core::ResolvedState::resolve(project_root)?
            .workspace
            .workspace_id
            .to_string();
    let state_home =
        if let Some(path) = std::env::var_os("ASP_STATE_HOME").filter(|value| !value.is_empty()) {
            PathBuf::from(path)
        } else {
            let home = std::env::var_os("HOME")
                .filter(|value| !value.is_empty())
                .ok_or_else(|| "ASP_STATE_HOME and HOME are both unset".to_owned())?;
            PathBuf::from(home).join(".agent-semantic-protocols")
        };
    let endpoint_path = crate::runtime_server_endpoint_path(&state_home);
    let endpoint_bytes = tokio::fs::read(&endpoint_path).await.map_err(|error| {
        format!(
            "Runtime Server endpoint is unavailable at {}: {error}",
            endpoint_path.display()
        )
    })?;
    let endpoint: crate::RuntimeServerEndpoint = serde_json::from_slice(&endpoint_bytes)
        .map_err(|error| format!("failed to decode Runtime Server endpoint: {error}"))?;
    endpoint.validate()?;
    Ok(WorkspaceDbIpcSession::for_runtime_server(
        &endpoint,
        workspace_identity,
    ))
}

pub fn commit_source_index_generation_via_runtime_server(
    request: crate::ClientDbSourceIndexRefreshRequest,
    materialization: crate::runtime_server_workspace::WorkspaceCanonicalMaterialization,
) -> Result<crate::ClientDbSourceIndexRefreshReport, String> {
    crate::engine::facade::block_on_db_engine_async(async move {
        let project_root = request.import.project_root.clone();
        let session = connect_runtime_server_workspace_session(&project_root).await?;
        session
            .commit_source_index_generation(&request, materialization)
            .await
    })
}

pub fn read_source_index_via_runtime_server(
    request: WorkspaceDbSourceIndexLookupRequest,
) -> Result<ClientDbSourceIndexLookupResult, String> {
    crate::engine::facade::block_on_db_engine_async(async move {
        let session = connect_runtime_server_workspace_session(&request.project_root).await?;
        session.read_source_index(&request).await
    })
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
    ReadSourceIndex {
        request: WorkspaceDbSourceIndexLookupRequest,
    },
    CommitSourceIndexGeneration {
        request: crate::ClientDbSourceIndexRefreshRequest,
        materialization: crate::runtime_server_workspace::WorkspaceCanonicalMaterialization,
    },
    EnsureRuntimeGeneration {
        project_root: String,
    },
    AdmitRuntimeGeneration {
        project_root: String,
    },
    WriteProviderIncrementalOwner {
        request: ProviderIncrementalOwnerWrite,
    },
    ReadProviderTreeSitterQuery {
        query: ProviderTreeSitterQueryIdentity,
        incremental_budget: u32,
        continuation: Option<ProviderTreeSitterContinuation>,
    },
    ReadProviderOwnerProjections {
        scope: ProviderIncrementalScoped,
        owner_path: String,
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
        owner: crate::runtime_server_workspace::WorkspaceOwnerSnapshot,
    },
    TombstoneRuntimeOwner {
        owner_path: String,
    },
    RelocateRuntimeOwner {
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
    ProviderOwnerProjections {
        projections: Vec<ProviderSelectorProjection>,
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
    Failed {
        code: String,
        message: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeGenerationEnsure {
    Ready(crate::runtime_server_workspace::WorkspaceRecoveryReceipt),
    Admission(crate::runtime_server_admission::WorkspaceGenerationAdmissionReceipt),
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceDbSourceIndexLookupRequest {
    pub project_root: PathBuf,
    pub indexed_project_root: PathBuf,
    pub source_snapshot: agent_semantic_content_identity::SourceSnapshotEvidence,
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
    serve_workspace_db_session_until_shutdown,
};

static NEXT_WORKSPACE_DB_IPC_CLIENT_ID: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(1);

#[derive(Debug)]
pub struct WorkspaceDbIpcSession {
    endpoint: WorkspaceDbSessionBinding,
    shared: std::sync::Arc<WorkspaceDbIpcSessionState>,
}

#[derive(Clone, Debug)]
struct WorkspaceDbSessionBinding {
    workspace_identity: String,
    transport_contract_digest: String,
    owner_epoch: u64,
    runtime_binary_path: String,
    runtime_binary_digest: String,
    binding_token: String,
    socket_path: String,
}

#[derive(Debug)]
struct WorkspaceDbIpcSessionState {
    client_id: u64,
    next_request_id: std::sync::atomic::AtomicU64,
    lanes: Vec<tokio::sync::Mutex<Option<UnixStream>>>,
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
        Self {
            endpoint: WorkspaceDbSessionBinding {
                workspace_identity: endpoint.workspace_identity,
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
            }),
        }
    }

    pub fn for_runtime_server(
        endpoint: &crate::runtime_server_control::RuntimeServerEndpoint,
        workspace_identity: impl Into<String>,
    ) -> Self {
        Self {
            endpoint: WorkspaceDbSessionBinding {
                workspace_identity: workspace_identity.into(),
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
            }),
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

    pub fn workspace_identity(&self) -> &str {
        self.endpoint.workspace_identity.as_str()
    }

    /// Digest of the typed transport contract bound to this resident owner.
    pub fn transport_contract_digest(&self) -> &str {
        self.endpoint.transport_contract_digest.as_str()
    }

    /// Epoch of the resident owner that accepted this IPC session.
    pub fn owner_epoch(&self) -> u64 {
        self.endpoint.owner_epoch
    }

    /// Immutable ASP artifact used to start the resident.
    pub fn runtime_binary_path(&self) -> &str {
        &self.endpoint.runtime_binary_path
    }

    /// Content digest of the ASP artifact used to start the resident.
    pub fn runtime_binary_digest(&self) -> &str {
        &self.endpoint.runtime_binary_digest
    }

    pub async fn health(&self) -> Result<(), String> {
        let result = tokio::time::timeout(
            std::time::Duration::from_millis(50),
            self.call_operation(WorkspaceDbIpcOperation::Health),
        )
        .await
        .map_err(|_| "workspace resident DB service health exceeded 50ms".to_owned())??;
        match result {
            WorkspaceDbIpcResult::Healthy => Ok(()),
            result => Err(format!(
                "workspace resident DB service returned an unexpected health result: {result:?}"
            )),
        }
    }

    pub async fn shutdown(&self) -> Result<(), String> {
        match self
            .call_operation(WorkspaceDbIpcOperation::Shutdown)
            .await?
        {
            WorkspaceDbIpcResult::ShutdownAccepted => Ok(()),
            result => Err(format!(
                "workspace resident DB service returned an unexpected shutdown result: {result:?}"
            )),
        }
    }

    pub async fn read_source_index(
        &self,
        request: &WorkspaceDbSourceIndexLookupRequest,
    ) -> Result<ClientDbSourceIndexLookupResult, String> {
        match self
            .call_operation(WorkspaceDbIpcOperation::ReadSourceIndex {
                request: request.clone(),
            })
            .await?
        {
            WorkspaceDbIpcResult::SourceIndex { lookup } => Ok(lookup),
            _ => Err("workspace owner IPC returned an unexpected source-index result".to_owned()),
        }
    }

    pub async fn probe_provider_owners(
        &self,
        scope: &ProviderIncrementalScoped,
        owners: &[ProviderOwnerBatchProbeRequest],
    ) -> Result<ProviderOwnerBatchProbeReceipt, String> {
        match self
            .call_operation(WorkspaceDbIpcOperation::ProbeProviderOwners {
                scope: scope.clone(),
                owners: owners.to_vec(),
            })
            .await?
        {
            WorkspaceDbIpcResult::ProviderOwners { receipt } => Ok(receipt),
            _ => Err("workspace owner IPC returned an unexpected probe result".to_owned()),
        }
    }

    pub async fn write_provider_incremental_owner(
        &self,
        request: &ProviderIncrementalOwnerWrite,
    ) -> Result<ProviderIncrementalWriteReceipt, String> {
        match self
            .call_operation(WorkspaceDbIpcOperation::WriteProviderIncrementalOwner {
                request: request.clone(),
            })
            .await?
        {
            WorkspaceDbIpcResult::ProviderIncrementalOwner { receipt } => Ok(receipt),
            _ => Err(
                "workspace owner IPC returned an unexpected incremental write result".to_owned(),
            ),
        }
    }

    pub async fn read_provider_treesitter_query(
        &self,
        query: &ProviderTreeSitterQueryIdentity,
        incremental_budget: u32,
        continuation: Option<&ProviderTreeSitterContinuation>,
    ) -> Result<ProviderTreeSitterQueryRead, String> {
        match self
            .call_operation(WorkspaceDbIpcOperation::ReadProviderTreeSitterQuery {
                query: query.clone(),
                incremental_budget,
                continuation: continuation.cloned(),
            })
            .await?
        {
            WorkspaceDbIpcResult::ProviderTreeSitterQuery { read } => Ok(read),
            _ => {
                Err("workspace owner IPC returned an unexpected Tree-sitter read result".to_owned())
            }
        }
    }

    pub async fn read_provider_owner_projections(
        &self,
        scope: &ProviderIncrementalScoped,
        owner_path: &str,
    ) -> Result<Vec<ProviderSelectorProjection>, String> {
        match self
            .call_operation(WorkspaceDbIpcOperation::ReadProviderOwnerProjections {
                scope: scope.clone(),
                owner_path: owner_path.to_owned(),
            })
            .await?
        {
            WorkspaceDbIpcResult::ProviderOwnerProjections { projections } => Ok(projections),
            _ => {
                Err("workspace owner IPC returned an unexpected owner projection result".to_owned())
            }
        }
    }

    pub async fn read_resident_selector(
        &self,
        request: &TursoResidentSelectorQuery,
    ) -> Result<Option<TursoResidentSelectorRead>, String> {
        match self
            .call_operation(WorkspaceDbIpcOperation::ReadResidentSelector {
                request: request.clone(),
            })
            .await?
        {
            WorkspaceDbIpcResult::ResidentSelector { read } => Ok(read),
            _ => Err(
                "workspace owner IPC returned an unexpected resident selector result".to_owned(),
            ),
        }
    }

    pub async fn write_provider_treesitter_owner_result(
        &self,
        query: &ProviderTreeSitterQueryIdentity,
        result: &ProviderTreeSitterOwnerResult,
    ) -> Result<ProviderTreeSitterOwnerWriteReceipt, String> {
        match self
            .call_operation(
                WorkspaceDbIpcOperation::WriteProviderTreeSitterOwnerResult {
                    query: query.clone(),
                    result: result.clone(),
                },
            )
            .await?
        {
            WorkspaceDbIpcResult::ProviderTreeSitterOwner { receipt } => Ok(receipt),
            _ => Err(
                "workspace owner IPC returned an unexpected Tree-sitter write result".to_owned(),
            ),
        }
    }

    pub async fn upsert_provider_owner_inventory(
        &self,
        request: &ProviderOwnerInventoryWrite,
    ) -> Result<ProviderOwnerInventoryWriteReceipt, String> {
        match self
            .call_operation(WorkspaceDbIpcOperation::UpsertProviderInventory {
                request: request.clone(),
            })
            .await?
        {
            WorkspaceDbIpcResult::ProviderInventory { receipt } => Ok(receipt),
            _ => Err("workspace owner IPC returned an unexpected inventory result".to_owned()),
        }
    }

    pub async fn finish_writes(
        &self,
        scope: &ProviderIncrementalScoped,
        mode: WorkspaceDbWriteFinishMode,
    ) -> Result<WorkspaceDbWriteFinishReceipt, String> {
        match self
            .call_operation(WorkspaceDbIpcOperation::FinishWrites {
                scope: scope.clone(),
                mode,
            })
            .await?
        {
            WorkspaceDbIpcResult::WriteFinish { receipt } => Ok(receipt),
            _ => Err("workspace owner IPC returned an unexpected write finish result".to_owned()),
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
