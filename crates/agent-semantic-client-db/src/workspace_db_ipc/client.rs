use super::{
    ClientDbSourceIndexLookupResult, WorkspaceDbIpcSession, WorkspaceDbSourceIndexLookupRequest,
};
use std::path::{Path, PathBuf};

pub async fn connect_runtime_server_workspace_session(
    project_root: &Path,
) -> Result<WorkspaceDbIpcSession, String> {
    let workspace_identity =
        agent_semantic_client_core::state_core::ResolvedState::resolve(project_root)?
            .workspace
            .workspace_id
            .to_string();
    let state_home =
        if let Some(path) = std::env::var_os("ASP_STATE_HOME").filter(|value| !value.is_empty()) {
            PathBuf::from(path)
        } else {
            let home = std::env::var_os("HOME")
                .filter(|value| !value.is_empty())
                .ok_or_else(|| "ASP_STATE_HOME and HOME are both unset".to_owned())?;
            PathBuf::from(home).join(".agent-semantic-protocols")
        };
    let endpoint_path = crate::runtime_server_endpoint_path(&state_home);
    let endpoint_bytes = tokio::fs::read(&endpoint_path).await.map_err(|error| {
        format!(
            "Runtime Server endpoint is unavailable at {}: {error}",
            endpoint_path.display()
        )
    })?;
    let endpoint: crate::RuntimeServerEndpoint = serde_json::from_slice(&endpoint_bytes)
        .map_err(|error| format!("failed to decode Runtime Server endpoint: {error}"))?;
    endpoint.validate()?;
    Ok(WorkspaceDbIpcSession::for_runtime_server(
        &endpoint,
        workspace_identity,
        tokio::fs::canonicalize(project_root)
            .await
            .map_err(|error| {
                format!(
                    "failed to canonicalize Runtime Server project root {}: {error}",
                    project_root.display()
                )
            })?,
    ))
}

pub fn read_source_index_via_runtime_server(
    request: WorkspaceDbSourceIndexLookupRequest,
) -> Result<ClientDbSourceIndexLookupResult, String> {
    crate::engine::facade::block_on_db_engine_async(async move {
        let session = connect_runtime_server_workspace_session(&request.project_root).await?;
        session.read_source_index(&request).await
    })
}

/// Execute one cache-control request through the resident Runtime Server.
pub fn cache_control_via_runtime_server(
    request: super::RuntimeCacheControlRequest,
) -> Result<super::RuntimeCacheControlReceipt, String> {
    let project_root = PathBuf::from(request.project_root());
    crate::engine::facade::block_on_db_engine_async(async move {
        let session = connect_runtime_server_workspace_session(&project_root).await?;
        session.cache_control(request).await
    })
}
