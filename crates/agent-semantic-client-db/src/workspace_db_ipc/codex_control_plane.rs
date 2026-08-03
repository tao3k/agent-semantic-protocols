use agent_semantic_context_product::codex_multi_agent_v2_control_plane::CodexMultiAgentV2ControlPlaneProjection;

use super::{WorkspaceDbIpcOperation, WorkspaceDbIpcResult, WorkspaceDbIpcSession};

impl WorkspaceDbIpcSession {
    pub async fn publish_codex_multi_agent_control_plane(
        &self,
        projection: CodexMultiAgentV2ControlPlaneProjection,
    ) -> Result<
        crate::codex_multi_agent_control_plane_owner::CodexControlPlanePublicationReceipt,
        String,
    > {
        match self
            .call_operation(
                WorkspaceDbIpcOperation::PublishCodexMultiAgentControlPlane { projection },
            )
            .await?
        {
            WorkspaceDbIpcResult::CodexMultiAgentControlPlanePublication { receipt } => Ok(receipt),
            other => Err(format!(
                "workspace owner returned an unexpected Codex control-plane publication result: {other:?}"
            )),
        }
    }

    pub async fn read_codex_multi_agent_control_plane(
        &self,
        root_session_id: impl Into<String>,
    ) -> Result<Option<CodexMultiAgentV2ControlPlaneProjection>, String> {
        match self
            .call_operation(WorkspaceDbIpcOperation::ReadCodexMultiAgentControlPlane {
                root_session_id: root_session_id.into(),
            })
            .await?
        {
            WorkspaceDbIpcResult::CodexMultiAgentControlPlane { projection } => Ok(projection),
            other => Err(format!(
                "workspace owner returned an unexpected Codex control-plane read result: {other:?}"
            )),
        }
    }
}
