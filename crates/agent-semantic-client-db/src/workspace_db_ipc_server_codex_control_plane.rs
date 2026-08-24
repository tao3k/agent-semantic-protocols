use std::sync::Arc;

use crate::workspace_db_ipc::WorkspaceDbIpcResult;

pub(super) async fn refresh(
    workspace_identity: &str,
    agent_session_registry_owner: Option<&Arc<crate::AgentSessionRegistry>>,
    owner: &Arc<crate::codex_multi_agent_control_plane_owner::CodexMultiAgentControlPlaneOwner>,
    project_id: crate::agent_session_registry::AgentSessionProjectId,
    root_session_id: crate::agent_session_registry::AgentSessionRootSessionId,
) -> WorkspaceDbIpcResult {
    let Some(agent_session_registry_owner) = agent_session_registry_owner else {
        return WorkspaceDbIpcResult::Failed {
            code: "runtime-server-agent-session-registry-unavailable".to_owned(),
            message: "Runtime Server has no AgentSession Registry owner".to_owned(),
        };
    };
    let publication = async {
        let records = agent_session_registry_owner
            .query_sessions_local(
                project_id.clone(),
                Some(root_session_id.clone().into()),
                None,
            )
            .await?;
        let current = owner.read(workspace_identity, root_session_id.as_str()).await;
        let projection = crate::codex_multi_agent_control_plane_materializer::materialize_codex_multi_agent_control_plane(
            workspace_identity,
            project_id.as_str(),
            root_session_id.as_str(),
            records,
            current.as_deref(),
        )?;
        owner.publish(projection).await
    }
    .await;
    match publication {
        Ok(receipt) => WorkspaceDbIpcResult::CodexMultiAgentControlPlanePublication { receipt },
        Err(message) => WorkspaceDbIpcResult::Failed {
            code: "runtime-server-codex-control-plane-refresh-failed".to_owned(),
            message,
        },
    }
}

pub(super) async fn read(
    workspace_identity: &str,
    owner: &Arc<crate::codex_multi_agent_control_plane_owner::CodexMultiAgentControlPlaneOwner>,
    root_session_id: crate::agent_session_registry::AgentSessionRootSessionId,
) -> WorkspaceDbIpcResult {
    let projection = owner
        .read(workspace_identity, root_session_id.as_str())
        .await
        .map(|projection| projection.as_ref().clone());
    WorkspaceDbIpcResult::CodexMultiAgentControlPlane { projection }
}
