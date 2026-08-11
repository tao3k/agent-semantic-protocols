use super::{
    NEXT_WORKSPACE_DB_IPC_CLIENT_ID, WorkspaceDbSessionBinding, WorkspaceDbSessionProfile,
    types::{
        WORKSPACE_DB_OWNER_REQUEST_SCHEMA_ID, WORKSPACE_DB_OWNER_RESPONSE_SCHEMA_ID,
        WORKSPACE_DB_OWNER_SCHEMA_VERSION, WorkspaceDbIpcOperation, WorkspaceDbIpcRequest,
        WorkspaceDbIpcResponse, WorkspaceDbIpcResult, WorkspaceDbSourceIndexLookupRequest,
    },
};
use crate::workspace_db_endpoint::WorkspaceDbOwnerEndpoint;
use crate::workspace_db_ipc::{
    MutationWorkspaceLane, RuntimeSearchAuthorityCache, read_frame, resident_state,
    runtime_server_data_connect_error, workspace_db_ipc_read_lane_capacity, write_frame,
};
use crate::{
    ClientDbSourceIndexLookupResult, ProviderIncrementalScoped, ProviderOwnerInventoryWrite,
    ProviderOwnerInventoryWriteReceipt, ProviderTreeSitterContinuation,
    ProviderTreeSitterOwnerResult, ProviderTreeSitterOwnerWriteReceipt,
    ProviderTreeSitterQueryIdentity, ProviderTreeSitterQueryRead, TursoResidentSelectorQuery,
    TursoResidentSelectorRead, WorkspaceDbWriteFinishMode, WorkspaceDbWriteFinishReceipt,
};
use std::path::{Path, PathBuf};
use tokio::{io::BufStream, net::UnixStream};

/// Per-operation deadline for immutable search/query reads.
///
/// This is deliberately local to IPC. It is not a process-startup or
/// generation-admission budget, and a timeout never authorizes repair.
const SEARCH_DATA_PLANE_IO_BUDGET: std::time::Duration = std::time::Duration::from_millis(500);
const RUNTIME_HEALTH_IO_BUDGET: std::time::Duration = std::time::Duration::from_millis(500);

#[derive(Debug)]
pub struct WorkspaceDbIpcSession {
    pub(in crate::workspace_db_ipc) endpoint: WorkspaceDbSessionBinding,
    pub(in crate::workspace_db_ipc) shared: std::sync::Arc<WorkspaceDbIpcSessionState>,
}

#[derive(Debug)]
pub(in crate::workspace_db_ipc) struct WorkspaceDbIpcSessionState {
    client_id: u64,
    next_request_id: std::sync::atomic::AtomicU64,
    lanes: Vec<tokio::sync::Mutex<Option<BufStream<UnixStream>>>>,
    pub(in crate::workspace_db_ipc) runtime_generation_mutations:
        std::sync::OnceLock<std::sync::Arc<MutationWorkspaceLane>>,
    pub(in crate::workspace_db_ipc) runtime_search_generation_authority:
        tokio::sync::OnceCell<std::sync::Arc<RuntimeSearchAuthorityCache>>,
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
    fn new_state(profile: WorkspaceDbSessionProfile) -> std::sync::Arc<WorkspaceDbIpcSessionState> {
        let read_lane_capacity = match profile {
            WorkspaceDbSessionProfile::Full => workspace_db_ipc_read_lane_capacity(),
            WorkspaceDbSessionProfile::HookReadOnly => 1,
        };
        std::sync::Arc::new(WorkspaceDbIpcSessionState {
            client_id: NEXT_WORKSPACE_DB_IPC_CLIENT_ID
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            next_request_id: std::sync::atomic::AtomicU64::new(1),
            lanes: (0..read_lane_capacity)
                .map(|_| tokio::sync::Mutex::new(None))
                .collect(),
            runtime_generation_mutations: std::sync::OnceLock::new(),
            runtime_search_generation_authority: tokio::sync::OnceCell::new(),
        })
    }

    pub(super) fn from_binding(endpoint: WorkspaceDbSessionBinding) -> Self {
        Self::from_binding_with_profile(endpoint, WorkspaceDbSessionProfile::Full)
    }

    pub(in crate::workspace_db_ipc) fn from_binding_with_profile(
        endpoint: WorkspaceDbSessionBinding,
        profile: WorkspaceDbSessionProfile,
    ) -> Self {
        Self {
            endpoint,
            shared: Self::new_state(profile),
        }
    }

    pub fn new(endpoint: WorkspaceDbOwnerEndpoint) -> Self {
        Self::from_binding(WorkspaceDbSessionBinding {
            workspace_identity: endpoint.workspace_identity,
            project_root: None,
            transport_contract_digest: endpoint.transport_contract_digest,
            owner_epoch: endpoint.owner_epoch,
            runtime_binary_path: endpoint.runtime_binary_path,
            runtime_binary_digest: endpoint.runtime_binary_digest,
            binding_token: endpoint.binding_token,
            socket_path: endpoint.socket_path,
            generation_pointer_path: None,
        })
    }

    pub fn for_runtime_server(
        endpoint: &crate::runtime_server_control::RuntimeServerEndpoint,
        workspace_identity: impl Into<String>,
        project_root: PathBuf,
    ) -> Self {
        let workspace_identity = workspace_identity.into();
        let generation_pointer_path =
            crate::runtime_server_workspace::workspace_generation_pointer_path(
                Path::new(&endpoint.workspace_store_path),
                &workspace_identity,
                &project_root,
            )
            .ok();
        let endpoint = WorkspaceDbSessionBinding {
            workspace_identity,
            project_root: Some(project_root),
            transport_contract_digest: endpoint.transport_contract_digest.clone(),
            owner_epoch: endpoint.owner_epoch,
            runtime_binary_path: endpoint.runtime_artifact_path.clone(),
            runtime_binary_digest: endpoint.runtime_artifact_digest.clone(),
            binding_token: endpoint.binding_token.clone(),
            socket_path: endpoint.data_plane_socket_path.clone(),
            generation_pointer_path,
        };
        let shared = resident_state(&endpoint, || {
            Self::new_state(WorkspaceDbSessionProfile::Full)
        });
        Self { endpoint, shared }
    }

    /// Bind a query-only client to the global Runtime Server data plane.
    ///
    /// The client deliberately receives no generation-pointer path: mmap and
    /// generation-open authority stay inside the daemon, and every read is
    /// routed by the workspace identity carried by the typed IPC request.
    pub fn for_runtime_server_read_only(
        endpoint: &crate::runtime_server_control::RuntimeServerEndpoint,
        workspace_identity: impl Into<String>,
        project_root: PathBuf,
    ) -> Self {
        let endpoint = WorkspaceDbSessionBinding {
            workspace_identity: workspace_identity.into(),
            project_root: Some(project_root),
            transport_contract_digest: endpoint.transport_contract_digest.clone(),
            owner_epoch: endpoint.owner_epoch,
            runtime_binary_path: endpoint.runtime_artifact_path.clone(),
            runtime_binary_digest: endpoint.runtime_artifact_digest.clone(),
            binding_token: endpoint.binding_token.clone(),
            socket_path: endpoint.data_plane_socket_path.clone(),
            generation_pointer_path: None,
        };
        let shared = Self::new_state(WorkspaceDbSessionProfile::HookReadOnly);
        Self { endpoint, shared }
    }

    pub(in crate::workspace_db_ipc) fn runtime_project_root(&self) -> Result<&Path, String> {
        self.endpoint.project_root.as_deref().ok_or_else(|| {
            "Runtime Server operation requires a session-bound project root".to_owned()
        })
    }

    pub fn runtime_generation_pointer_path(&self) -> Option<&Path> {
        self.endpoint.generation_pointer_path.as_deref()
    }

    pub(in crate::workspace_db_ipc) async fn call_runtime_generation_admission(
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

    pub(in crate::workspace_db_ipc) async fn call_operation(
        &self,
        operation: WorkspaceDbIpcOperation,
    ) -> Result<WorkspaceDbIpcResult, String> {
        if is_search_data_plane_read(&operation) {
            return match tokio::time::timeout(
                SEARCH_DATA_PLANE_IO_BUDGET,
                self.call_operation_inner(operation),
            )
            .await
            {
                Ok(result) => result,
                Err(_) => {
                    self.discard_idle_connection_lanes();
                    Err(format!(
                        "runtime-search-io-timeout: immutable data-plane read exceeded {}ms",
                        SEARCH_DATA_PLANE_IO_BUDGET.as_millis()
                    ))
                }
            };
        }
        self.call_operation_inner(operation).await
    }

    async fn call_operation_inner(
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
        let mut connected_lane = None;
        let mut empty_lane = None;
        for offset in 0..self.shared.lanes.len() {
            let lane_index = (first_lane + offset) % self.shared.lanes.len();
            if let Ok(lane) = self.shared.lanes[lane_index].try_lock() {
                if lane.is_some() {
                    connected_lane = Some(lane);
                    break;
                }
                if empty_lane.is_none() {
                    empty_lane = Some(lane);
                }
            }
        }
        let mut lane = match connected_lane.or(empty_lane) {
            Some(lane) => lane,
            None => self.shared.lanes[first_lane].lock().await,
        };
        if lane.is_none() {
            *lane = Some(BufStream::new(
                UnixStream::connect(&self.endpoint.socket_path)
                    .await
                    .map_err(runtime_server_data_connect_error)?,
            ));
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

    fn discard_idle_connection_lanes(&self) {
        for lane in &self.shared.lanes {
            if let Ok(mut lane) = lane.try_lock() {
                *lane = None;
            }
        }
    }
}

#[cfg(test)]
#[path = "../../tests/unit/workspace_db_ipc_search_deadline.rs"]
mod search_deadline_tests;

fn is_search_data_plane_read(operation: &WorkspaceDbIpcOperation) -> bool {
    matches!(
        operation,
        WorkspaceDbIpcOperation::ReadRuntimeSelector { .. }
            | WorkspaceDbIpcOperation::ReadSourceIndex { .. }
            | WorkspaceDbIpcOperation::ReadRuntimeSearchGenerationAuthority { .. }
            | WorkspaceDbIpcOperation::ReadRuntimeOwner { .. }
            | WorkspaceDbIpcOperation::ReadRuntimeGraphFacts { .. }
    )
}

impl WorkspaceDbIpcSession {
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
            RUNTIME_HEALTH_IO_BUDGET,
            self.call_operation(WorkspaceDbIpcOperation::Health),
        )
        .await
        .map_err(|_| {
            format!(
                "workspace resident DB service health exceeded {}ms",
                RUNTIME_HEALTH_IO_BUDGET.as_millis()
            )
        })??;
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
        let mut request = request.clone();
        if let Some(project_root) = self.endpoint.project_root.as_ref() {
            request.project_root = project_root.clone();
        }
        match self
            .call_operation(WorkspaceDbIpcOperation::ReadSourceIndex { request })
            .await?
        {
            WorkspaceDbIpcResult::SourceIndex { lookup } => Ok(lookup),
            _ => Err("workspace owner IPC returned an unexpected source-index result".to_owned()),
        }
    }

    pub async fn project_tree_sitter_query(
        &self,
        project_root: &std::path::Path,
        language_id: &str,
        args: &[String],
    ) -> Result<Option<String>, String> {
        match self
            .call_operation(WorkspaceDbIpcOperation::ProjectTreeSitterQuery {
                project_root: project_root.to_string_lossy().into_owned(),
                language_id: language_id.into(),
                args: args.to_vec(),
            })
            .await?
        {
            WorkspaceDbIpcResult::TreeSitterQuery { rendered } => Ok(rendered),
            _ => {
                Err("Runtime Server IPC returned an unexpected Tree-sitter query result".to_owned())
            }
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
