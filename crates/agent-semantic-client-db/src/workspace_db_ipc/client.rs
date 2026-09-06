// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

//! Typed northbound helpers that connect only to the published Runtime Server endpoint.

use super::WorkspaceDbIpcSession;
use std::path::{Path, PathBuf};

/// Connect a typed client session to the published Runtime Server workspace endpoint.
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
    let endpoint_path = crate::runtime_server_endpoint_path(&state_home)?;
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

/// Read one owner-bound Merkle proof from the immutable Runtime search segment.
pub async fn read_runtime_merkle_owner_via_runtime_server(
    project_root: &Path,
    owner_path: impl Into<String>,
) -> Result<crate::runtime_server_workspace::WorkspaceRuntimeMerkleOwnerRead, String> {
    let session = connect_runtime_server_workspace_session(project_root).await?;
    match session
        .call_operation(super::WorkspaceDbIpcOperation::ReadRuntimeMerkleOwner {
            request: super::RuntimeMerkleOwnerReadRequest::new(
                project_root.to_string_lossy().into_owned(),
                owner_path,
            ),
        })
        .await?
    {
        super::WorkspaceDbIpcResult::RuntimeMerkleOwner { read, .. } => Ok(read),
        super::WorkspaceDbIpcResult::Failed { code, message } => Err(format!("{code}: {message}")),
        _ => Err("Runtime Server returned an unexpected Merkle owner response".to_owned()),
    }
}
