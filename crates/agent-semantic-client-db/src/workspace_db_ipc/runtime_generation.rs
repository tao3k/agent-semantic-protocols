use super::protocol::{
    RuntimeGenerationEnsure, WorkspaceDbIpcOperation, WorkspaceDbIpcResult, WorkspaceDbIpcSession,
};

impl WorkspaceDbIpcSession {
    pub async fn commit_source_index_generation(
        &self,
        request: &crate::ClientDbSourceIndexRefreshRequest,
        materialization: crate::runtime_server_workspace::WorkspaceCanonicalMaterialization,
    ) -> Result<crate::ClientDbSourceIndexRefreshReport, String> {
        materialization.validate_refresh_request(self.workspace_identity(), request)?;
        match self
            .call_operation(WorkspaceDbIpcOperation::CommitSourceIndexGeneration {
                request: request.clone(),
                materialization,
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

    pub async fn ensure_runtime_generation(
        &self,
        project_root: &std::path::Path,
    ) -> Result<RuntimeGenerationEnsure, String> {
        match self
            .call_operation(WorkspaceDbIpcOperation::EnsureRuntimeGeneration {
                project_root: project_root.display().to_string(),
            })
            .await?
        {
            WorkspaceDbIpcResult::RuntimeGeneration { receipt } => {
                Ok(RuntimeGenerationEnsure::Ready(receipt))
            }
            WorkspaceDbIpcResult::RuntimeGenerationAdmission { receipt } => {
                Ok(RuntimeGenerationEnsure::Admission(receipt))
            }
            _ => Err("Runtime Server returned an unexpected generation restore result".to_owned()),
        }
    }

    pub async fn admit_runtime_generation(
        &self,
        project_root: &std::path::Path,
    ) -> Result<crate::runtime_server_admission::WorkspaceGenerationAdmissionReceipt, String> {
        match self
            .call_operation(WorkspaceDbIpcOperation::AdmitRuntimeGeneration {
                project_root: project_root.display().to_string(),
            })
            .await?
        {
            WorkspaceDbIpcResult::RuntimeGenerationAdmission { receipt } => Ok(receipt),
            _ => {
                Err("Runtime Server returned an unexpected generation admission result".to_owned())
            }
        }
    }

    pub async fn publish_runtime_owner(
        &self,
        owner: crate::runtime_server_workspace::WorkspaceOwnerSnapshot,
    ) -> Result<crate::runtime_server_workspace::WorkspaceRecoveryReceipt, String> {
        match self
            .call_operation(WorkspaceDbIpcOperation::PublishRuntimeOwner { owner })
            .await?
        {
            WorkspaceDbIpcResult::RuntimeGeneration { receipt } => Ok(receipt),
            _ => {
                Err("Runtime Server returned an unexpected workspace generation result".to_owned())
            }
        }
    }

    pub async fn tombstone_runtime_owner(
        &self,
        owner_path: impl Into<String>,
    ) -> Result<crate::runtime_server_workspace::WorkspaceRecoveryReceipt, String> {
        match self
            .call_operation(WorkspaceDbIpcOperation::TombstoneRuntimeOwner {
                owner_path: owner_path.into(),
            })
            .await?
        {
            WorkspaceDbIpcResult::RuntimeGeneration { receipt } => Ok(receipt),
            _ => {
                Err("Runtime Server returned an unexpected workspace generation result".to_owned())
            }
        }
    }

    pub async fn relocate_runtime_owner(
        &self,
        previous_owner_path: impl Into<String>,
        owner: crate::runtime_server_workspace::WorkspaceOwnerSnapshot,
    ) -> Result<crate::runtime_server_workspace::WorkspaceRecoveryReceipt, String> {
        match self
            .call_operation(WorkspaceDbIpcOperation::RelocateRuntimeOwner {
                previous_owner_path: previous_owner_path.into(),
                owner,
            })
            .await?
        {
            WorkspaceDbIpcResult::RuntimeGeneration { receipt } => Ok(receipt),
            _ => {
                Err("Runtime Server returned an unexpected workspace generation result".to_owned())
            }
        }
    }
}
