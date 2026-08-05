//! Owns Runtime Server endpoint file and socket lifecycle operations.

use agent_semantic_client_db::{RuntimeServerEndpoint, runtime_server_endpoint_path};
use std::path::{Path, PathBuf};

pub(super) async fn read_endpoint(path: &Path) -> Result<RuntimeServerEndpoint, String> {
    let endpoint = read_supervisor_endpoint(path).await?;
    endpoint.validate()?;
    Ok(endpoint)
}

pub(super) async fn read_supervisor_endpoint(path: &Path) -> Result<RuntimeServerEndpoint, String> {
    let bytes = tokio::fs::read(path).await.map_err(|error| {
        format!(
            "Runtime Server endpoint is unavailable at {}: {error}",
            path.display()
        )
    })?;
    let endpoint: RuntimeServerEndpoint = serde_json::from_slice(&bytes)
        .map_err(|error| format!("failed to decode Runtime Server endpoint: {error}"))?;
    endpoint.validate_supervisor_control()?;
    Ok(endpoint)
}

pub(super) async fn remove_stale_socket(path: &Path) -> Result<(), String> {
    match tokio::fs::remove_file(path).await {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!(
            "failed to remove stale Runtime Server socket {}: {error}",
            path.display()
        )),
    }
}

pub(super) async fn cleanup_endpoint(state_home: &Path, endpoint: &RuntimeServerEndpoint) {
    let endpoint_path = runtime_server_endpoint_path(state_home);
    let socket_path = PathBuf::from(&endpoint.socket_path);
    let data_plane_socket_path = PathBuf::from(&endpoint.data_plane_socket_path);
    let status_memory_path = PathBuf::from(&endpoint.status_memory_path);
    let owned = read_endpoint(&endpoint_path).await.is_ok_and(|actual| {
        actual.owner_epoch == endpoint.owner_epoch && actual.binding_token == endpoint.binding_token
    });
    if owned {
        let _ = tokio::fs::remove_file(endpoint_path).await;
        let _ = tokio::fs::remove_file(socket_path).await;
        let _ = tokio::fs::remove_file(data_plane_socket_path).await;
        let _ = tokio::fs::remove_file(status_memory_path).await;
    }
}
