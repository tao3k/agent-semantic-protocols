use super::protocol::{WorkspaceDbIpcOperation, WorkspaceDbIpcResult, WorkspaceDbIpcSession};

impl WorkspaceDbIpcSession {
    pub async fn ensure_runtime_owner(
        &self,
        language_id: impl Into<String>,
        owner_path: impl Into<String>,
    ) -> Result<crate::runtime_server_workspace::WorkspaceRuntimeOwnerFreshnessReceipt, String>
    {
        match self
            .call_operation(WorkspaceDbIpcOperation::EnsureRuntimeOwner {
                project_root: self.runtime_project_root()?.display().to_string(),
                language_id: language_id.into(),
                owner_path: owner_path.into(),
            })
            .await?
        {
            WorkspaceDbIpcResult::RuntimeOwnerFreshness { receipt } => Ok(receipt),
            _ => Err(
                "Runtime Server returned an unexpected runtime owner freshness result".to_owned(),
            ),
        }
    }

    pub async fn read_runtime_selector(
        &self,
        projection_kind: impl Into<String>,
        structural_selector: impl Into<String>,
    ) -> Result<crate::runtime_server_workspace::WorkspaceRuntimeSelectorRead, String> {
        match self
            .call_operation(WorkspaceDbIpcOperation::ReadRuntimeSelector {
                project_root: self.runtime_project_root()?.display().to_string(),
                projection_kind: projection_kind.into(),
                structural_selector: structural_selector.into(),
            })
            .await?
        {
            WorkspaceDbIpcResult::RuntimeSelector { read } => Ok(read),
            _ => Err("Runtime Server returned an unexpected runtime selector result".to_owned()),
        }
    }

    pub async fn publish_runtime_selector_overlay(
        &self,
        overlay: crate::runtime_server_workspace::WorkspaceRuntimeSelectorOverlay,
    ) -> Result<crate::runtime_server_workspace::WorkspaceRuntimeSelectorOverlayReceipt, String>
    {
        match self
            .call_operation(WorkspaceDbIpcOperation::PublishRuntimeSelectorOverlay {
                project_root: self.runtime_project_root()?.display().to_string(),
                overlay,
            })
            .await?
        {
            WorkspaceDbIpcResult::RuntimeSelectorOverlay { receipt } => Ok(receipt),
            _ => Err(
                "Runtime Server returned an unexpected runtime selector overlay result".to_owned(),
            ),
        }
    }

    pub async fn admit_runtime_generation(
        &self,
    ) -> Result<crate::runtime_server_admission::WorkspaceGenerationAdmissionReceipt, String> {
        self.call_runtime_generation_admission(self.runtime_project_root()?.display().to_string())
            .await
    }

    pub async fn ensure_runtime_generation(
        &self,
    ) -> Result<crate::runtime_server_admission::WorkspaceGenerationAdmissionReceipt, String> {
        match self
            .call_operation(WorkspaceDbIpcOperation::EnsureRuntimeGeneration {
                project_root: self.runtime_project_root()?.display().to_string(),
            })
            .await?
        {
            WorkspaceDbIpcResult::RuntimeGenerationAdmission { receipt } => Ok(receipt),
            _ => Err("Runtime Server returned an unexpected ensured generation result".to_owned()),
        }
    }

    pub async fn repair_runtime_generation_locator(
        &self,
    ) -> Result<crate::runtime_server_admission::WorkspaceGenerationAdmissionReceipt, String> {
        match self
            .call_operation(WorkspaceDbIpcOperation::RepairRuntimeGenerationLocator {
                project_root: self.runtime_project_root()?.display().to_string(),
            })
            .await?
        {
            WorkspaceDbIpcResult::RuntimeGenerationAdmission { receipt } => Ok(receipt),
            _ => Err(
                "Runtime Server returned an unexpected generation locator repair result".to_owned(),
            ),
        }
    }

    pub async fn evaluate_hook(
        &self,
        arguments: Vec<String>,
        input: String,
    ) -> Result<String, String> {
        let expected_workspace_identity = self.workspace_identity().to_owned();
        let expected_project_root = self.runtime_project_root()?.display().to_string();
        let result = self
            .call_operation(WorkspaceDbIpcOperation::EvaluateHook {
                project_root: expected_project_root.clone(),
                arguments,
                input,
            })
            .await?;
        admit_hook_evaluation_result(&expected_workspace_identity, &expected_project_root, result)
    }

    pub async fn publish_runtime_owner(
        &self,
        owner: crate::runtime_server_workspace::WorkspaceOwnerSnapshot,
    ) -> Result<crate::runtime_server_workspace::WorkspaceRecoveryReceipt, String> {
        match self
            .call_operation(WorkspaceDbIpcOperation::PublishRuntimeOwner {
                project_root: self.runtime_project_root()?.display().to_string(),
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

    pub async fn tombstone_runtime_owner(
        &self,
        owner_path: impl Into<String>,
    ) -> Result<crate::runtime_server_workspace::WorkspaceRecoveryReceipt, String> {
        match self
            .call_operation(WorkspaceDbIpcOperation::TombstoneRuntimeOwner {
                project_root: self.runtime_project_root()?.display().to_string(),
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
                project_root: self.runtime_project_root()?.display().to_string(),
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

fn admit_hook_evaluation_result(
    expected_workspace_identity: &str,
    expected_project_root: &str,
    result: WorkspaceDbIpcResult,
) -> Result<String, String> {
    match result {
        WorkspaceDbIpcResult::HookEvaluation {
            workspace_identity,
            project_root,
            output,
        } if workspace_identity == expected_workspace_identity
            && project_root == expected_project_root =>
        {
            Ok(output)
        }
        WorkspaceDbIpcResult::HookEvaluation {
            workspace_identity,
            project_root,
            ..
        } => Err(format!(
            "Runtime Server hook evaluation identity mismatch: expectedWorkspaceIdentity={expected_workspace_identity} actualWorkspaceIdentity={workspace_identity} expectedProjectRoot={expected_project_root} actualProjectRoot={project_root}"
        )),
        _ => Err("Runtime Server returned an unexpected hook evaluation result".to_owned()),
    }
}

#[cfg(test)]
#[path = "../../tests/unit/workspace_db_ipc_hook_evaluation.rs"]
mod hook_evaluation_tests;
