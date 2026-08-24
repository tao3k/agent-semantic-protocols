use agent_semantic_context_product::codex_multi_agent_v2_control_plane::CodexMultiAgentV2ControlPlaneProjection;

use super::{WorkspaceDbIpcOperation, WorkspaceDbIpcResult, WorkspaceDbIpcSession};

impl WorkspaceDbIpcSession {
    /// Ask the Runtime Server to materialize the current downstream control
    /// plane from its durable AgentSession Registry owner.
    pub async fn refresh_codex_multi_agent_control_plane(
        &self,
        project_id: impl Into<crate::agent_session_registry::AgentSessionProjectId>,
        root_session_id: impl Into<crate::agent_session_registry::AgentSessionRootSessionId>,
    ) -> Result<
        crate::codex_multi_agent_control_plane_owner::CodexControlPlanePublicationReceipt,
        String,
    > {
        match self
            .call_operation(
                WorkspaceDbIpcOperation::RefreshCodexMultiAgentControlPlane {
                    project_id: project_id.into(),
                    root_session_id: root_session_id.into(),
                },
            )
            .await?
        {
            WorkspaceDbIpcResult::CodexMultiAgentControlPlanePublication { receipt } => Ok(receipt),
            other => Err(format!(
                "workspace owner returned an unexpected Codex control-plane refresh result: {other:?}"
            )),
        }
    }

    pub async fn read_codex_multi_agent_control_plane(
        &self,
        root_session_id: impl Into<crate::agent_session_registry::AgentSessionRootSessionId>,
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
