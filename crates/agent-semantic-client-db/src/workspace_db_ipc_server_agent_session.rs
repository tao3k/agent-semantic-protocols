//! Serialized agent-session registry dispatch on the Runtime Server data plane.

use std::sync::{Arc, LazyLock};

use crate::AgentSessionRegistry;
use crate::workspace_db_ipc::{AgentSessionRegistryIpcOperation, WorkspaceDbIpcResult};

static OWNER_LANE: LazyLock<tokio::sync::Mutex<()>> = LazyLock::new(|| tokio::sync::Mutex::new(()));

pub(super) async fn evaluate(
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
