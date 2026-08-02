use super::{
    ClientDbSourceIndexLookupResult, ProviderIncrementalScoped, ProviderOwnerInventoryWrite,
    ProviderOwnerInventoryWriteReceipt, ProviderTreeSitterContinuation,
    ProviderTreeSitterOwnerResult, ProviderTreeSitterOwnerWriteReceipt,
    ProviderTreeSitterQueryIdentity, ProviderTreeSitterQueryRead, TursoResidentSelectorQuery,
    TursoResidentSelectorRead, WorkspaceDbIpcOperation, WorkspaceDbIpcResult,
    WorkspaceDbIpcSession, WorkspaceDbSourceIndexLookupRequest, WorkspaceDbWriteFinishMode,
    WorkspaceDbWriteFinishReceipt,
};

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
