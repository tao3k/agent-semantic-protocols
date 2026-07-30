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
use std::path::PathBuf;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;

pub use crate::workspace_db_endpoint::{
    WorkspaceDbOwnerEndpoint, bind_workspace_db_owner, prepare_workspace_db_owner_endpoint,
    workspace_db_owner_runtime_base, workspace_db_owner_transport_contract_digest,
};
pub use crate::workspace_db_owner_election::{
    WorkspaceDbOwnerRetirement, remove_stale_workspace_db_owner_socket,
    try_acquire_workspace_db_owner_election, try_retire_workspace_db_owner_endpoint,
};

const MAX_FRAME_BYTES: usize = 8 * 1024 * 1024;

fn workspace_db_ipc_read_lane_capacity() -> usize {
    std::thread::available_parallelism()
        .map(std::num::NonZeroUsize::get)
        .unwrap_or(1)
}

pub async fn connect_workspace_db_owner_session(
    project_root: &std::path::Path,
) -> Result<WorkspaceDbIpcSession, String> {
    let resolved = agent_semantic_client_core::state_core::ResolvedState::resolve(project_root)?;
    let endpoint_path = resolved
        .paths
        .hooks_dir
        .join("state")
        .join("workspace-db-resident-service-endpoint.v1.json");
    let bytes = tokio::fs::read(&endpoint_path).await.map_err(|error| {
        format!(
            "workspace resident endpoint is unavailable for {} at {}: {error}",
            resolved.workspace.workspace_id,
            endpoint_path.display()
        )
    })?;
    let endpoint: WorkspaceDbOwnerEndpoint = serde_json::from_slice(&bytes).map_err(|error| {
        format!(
            "invalid workspace resident endpoint {}: {error}",
            endpoint_path.display()
        )
    })?;
    endpoint.validate_for_workspace(resolved.workspace.workspace_id.as_str())?;
    Ok(WorkspaceDbIpcSession::new(endpoint))
}

pub fn commit_source_index_generation_via_resident(
    request: crate::ClientDbSourceIndexRefreshRequest,
) -> Result<crate::ClientDbSourceIndexRefreshReport, String> {
    crate::engine::facade::block_on_db_engine_async(async move {
        let project_root = request.import.project_root.clone();
        let session = connect_workspace_db_owner_session(&project_root).await?;
        session.commit_source_index_generation(&request).await
    })
}
/// Typed operation accepted by the first owner transport slice.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum WorkspaceDbIpcOperation {
    Health,
    Shutdown,
    ReadSourceIndex {
        request: WorkspaceDbSourceIndexLookupRequest,
    },
    CommitSourceIndexGeneration {
        request: crate::ClientDbSourceIndexRefreshRequest,
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

/// Typed response result for the first owner transport slice.
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
    endpoint: WorkspaceDbOwnerEndpoint,
    shared: std::sync::Arc<WorkspaceDbIpcSessionState>,
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
            endpoint,
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

    async fn call_operation(
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

    /// Process that owns the resident endpoint.
    pub fn owner_pid(&self) -> u32 {
        self.endpoint.owner_pid
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
        match self.call_operation(WorkspaceDbIpcOperation::Shutdown).await? {
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

    pub async fn commit_source_index_generation(
        &self,
        request: &crate::ClientDbSourceIndexRefreshRequest,
    ) -> Result<crate::ClientDbSourceIndexRefreshReport, String> {
        match self
            .call_operation(WorkspaceDbIpcOperation::CommitSourceIndexGeneration {
                request: request.clone(),
            })
            .await?
        {
            WorkspaceDbIpcResult::SourceIndexGeneration { receipt } => Ok(receipt),
            _ => Err(
                "workspace owner IPC returned an unexpected source-index generation result"
                    .to_owned(),
            ),
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

/// Connect, send one typed frame, and verify the response binding.
pub async fn call_workspace_db_owner(
    endpoint: &WorkspaceDbOwnerEndpoint,
    request: &WorkspaceDbIpcRequest,
) -> Result<WorkspaceDbIpcResponse, String> {
    let mut stream = UnixStream::connect(&endpoint.socket_path)
        .await
        .map_err(|error| format!("failed to connect workspace owner endpoint: {error}"))?;
    write_frame(&mut stream, request).await?;
    let response: WorkspaceDbIpcResponse = read_frame(&mut stream).await?;
    if response.schema_id != WORKSPACE_DB_OWNER_RESPONSE_SCHEMA_ID {
        return Err(format!(
            "workspace owner response schema_id drift: expected {:?}, got {:?}",
            WORKSPACE_DB_OWNER_RESPONSE_SCHEMA_ID, response.schema_id
        ));
    }
    if response.schema_version != WORKSPACE_DB_OWNER_SCHEMA_VERSION {
        return Err(format!(
            "workspace owner response schema_version drift: expected {:?}, got {:?}",
            WORKSPACE_DB_OWNER_SCHEMA_VERSION, response.schema_version
        ));
    }
    if response.workspace_identity != endpoint.workspace_identity {
        return Err(format!(
            "workspace owner response workspace_identity drift: expected {:?}, got {:?}",
            endpoint.workspace_identity, response.workspace_identity
        ));
    }
    if response.owner_epoch != endpoint.owner_epoch {
        return Err(format!(
            "workspace owner response owner_epoch drift: expected {}, got {}",
            endpoint.owner_epoch, response.owner_epoch
        ));
    }
    if response.request_id != request.request_id {
        return Err(format!(
            "workspace owner response request_id drift: expected {:?}, got {:?}",
            request.request_id, response.request_id
        ));
    }
    Ok(response)
}

pub(crate) async fn write_frame<T: Serialize>(
    stream: &mut UnixStream,
    value: &T,
) -> Result<(), String> {
    let body = serde_json::to_vec(value)
        .map_err(|error| format!("failed to encode workspace owner frame: {error}"))?;
    if body.len() > MAX_FRAME_BYTES {
        return Err("workspace owner frame exceeds size limit".to_owned());
    }
    stream
        .write_u32(body.len().try_into().map_err(|_| "frame length overflow")?)
        .await
        .map_err(|error| format!("failed to write workspace owner frame length: {error}"))?;
    stream
        .write_all(&body)
        .await
        .map_err(|error| format!("failed to write workspace owner frame: {error}"))
}

pub(crate) async fn read_frame<T: for<'de> Deserialize<'de>>(
    stream: &mut UnixStream,
) -> Result<T, String> {
    read_optional_frame(stream)
        .await?
        .ok_or_else(|| "workspace owner endpoint closed before a frame was received".to_owned())
}

pub(crate) async fn read_optional_frame<T: for<'de> Deserialize<'de>>(
    stream: &mut UnixStream,
) -> Result<Option<T>, String> {
    let length = match stream.read_u32().await {
        Ok(length) => length as usize,
        Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(error) => {
            return Err(format!(
                "failed to read workspace owner frame length: {error}"
            ));
        }
    };
    if length > MAX_FRAME_BYTES {
        return Err("workspace owner frame exceeds size limit".to_owned());
    }
    let mut body = vec![0; length];
    stream
        .read_exact(&mut body)
        .await
        .map_err(|error| format!("failed to read workspace owner frame: {error}"))?;
    serde_json::from_slice(&body)
        .map(Some)
        .map_err(|error| format!("failed to decode workspace owner frame: {error}"))
}

#[cfg(test)]
#[path = "../tests/unit/workspace_db_ipc.rs"]
mod tests;
pub const WORKSPACE_DB_OWNER_ENDPOINT_SCHEMA_ID: &str =
    "agent.semantic-protocols.workspace-db-owner-endpoint.v1";
pub const WORKSPACE_DB_OWNER_REQUEST_SCHEMA_ID: &str =
    "agent.semantic-protocols.workspace-db-owner-request.v1";
pub const WORKSPACE_DB_OWNER_RESPONSE_SCHEMA_ID: &str =
    "agent.semantic-protocols.workspace-db-owner-response.v1";
pub const WORKSPACE_DB_OWNER_SCHEMA_VERSION: &str = "1";
