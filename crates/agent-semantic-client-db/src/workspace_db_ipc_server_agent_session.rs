//! Serialized agent-session registry dispatch on the Runtime Server data plane.

use std::sync::{Arc, LazyLock};

use crate::workspace_db_ipc::{
    AgentSessionRegistryIpcOperation, AgentSessionRegistryIpcResult, WorkspaceDbIpcResult,
};
use crate::{AgentSessionRegistry, SessionControlPlaneRuntimeRegistry};

static OWNER_LANE: LazyLock<tokio::sync::Mutex<()>> = LazyLock::new(|| tokio::sync::Mutex::new(()));

pub(super) async fn evaluate(
    owner: Option<&Arc<AgentSessionRegistry>>,
    control_plane_registry: &Arc<SessionControlPlaneRuntimeRegistry>,
    project_root: String,
    operation: AgentSessionRegistryIpcOperation,
) -> WorkspaceDbIpcResult {
    match operation {
        AgentSessionRegistryIpcOperation::RegisterControlPlaneAgent { registration } => {
            let result = async {
                let runtime = control_plane_runtime(control_plane_registry, &project_root).await?;
                runtime.register_agent(&registration).await?;
                Ok(AgentSessionRegistryIpcResult::ControlPlaneAgentRegistered)
            }
            .await;
            return control_plane_result(result);
        }
        AgentSessionRegistryIpcOperation::AdmitControlPlaneDelegation { proposal } => {
            let result = async {
                let runtime = control_plane_runtime(control_plane_registry, &project_root).await?;
                let receipt = runtime.admit_delegation(&proposal).await?;
                Ok(AgentSessionRegistryIpcResult::ControlPlaneDelegationAdmitted { receipt })
            }
            .await;
            return control_plane_result(result);
        }
        AgentSessionRegistryIpcOperation::ReadControlPlaneSnapshot {
            project_id,
            root_session_id,
        } => {
            let result = async {
                let runtime = control_plane_runtime(control_plane_registry, &project_root).await?;
                let snapshot = runtime.snapshot(&project_id, &root_session_id).await?;
                Ok(AgentSessionRegistryIpcResult::ControlPlaneSnapshot { snapshot })
            }
            .await;
            return control_plane_result(result);
        }
        operation => evaluate_legacy(owner, project_root, operation).await,
    }
}

async fn evaluate_legacy(
    owner: Option<&Arc<AgentSessionRegistry>>,
    project_root: String,
    operation: AgentSessionRegistryIpcOperation,
) -> WorkspaceDbIpcResult {
    let _owner_guard = OWNER_LANE.lock().await;
    match owner {
        Some(owner) => match super::run_agent_session_registry_operation(
            Arc::clone(owner),
            project_root.into(),
            operation,
        )
        .await
        {
            Ok(result) => WorkspaceDbIpcResult::AgentSessionRegistry { result },
            Err(message) => WorkspaceDbIpcResult::Failed {
                code: "runtime-server-agent-session-registry-failed".to_owned(),
                message,
            },
        },
        None => WorkspaceDbIpcResult::Failed {
            code: "runtime-server-agent-session-registry-unavailable".to_owned(),
            message: "Runtime Server has no resident agent-session registry owner".to_owned(),
        },
    }
}

async fn control_plane_runtime(
    registry: &SessionControlPlaneRuntimeRegistry,
    project_root: &str,
) -> Result<Arc<crate::SessionControlPlaneRuntime>, String> {
    let state_root = AgentSessionRegistry::state_root_for_project(project_root)?;
    let db_path = AgentSessionRegistry::db_path_for_state_root(&state_root);
    let client_dir = db_path.parent().ok_or_else(|| {
        format!(
            "agent-session state root has no client directory: {}",
            db_path.display()
        )
    })?;
    registry.runtime_for_client_dir(client_dir).await
}

fn control_plane_result(
    result: Result<AgentSessionRegistryIpcResult, String>,
) -> WorkspaceDbIpcResult {
    match result {
        Ok(result) => WorkspaceDbIpcResult::AgentSessionRegistry { result },
        Err(message) => WorkspaceDbIpcResult::Failed {
            code: "runtime-server-session-control-plane-failed".to_owned(),
            message,
        },
    }
}
