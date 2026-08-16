//! Typed Runtime Server stop receipt owned by the global State Home lifecycle.

use agent_semantic_client_db::{RuntimeServerEndpoint, runtime_server_endpoint_path};
use serde::Serialize;
use std::path::Path;
use tokio::io::AsyncWriteExt;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeServerStopReceipt {
    schema_id: &'static str,
    schema_version: &'static str,
    request_id: String,
    state: &'static str,
    lifecycle_authority: &'static str,
    operator_stop_recorded: bool,
    endpoint_path: String,
    endpoint_removed: bool,
    control_socket_removed: bool,
    data_plane_socket_removed: bool,
    status_memory_removed: bool,
}

pub(super) async fn run_stop() -> Result<(), String> {
    let state_home = super::state_home()?;
    let endpoint_path = runtime_server_endpoint_path(&state_home)?;
    let endpoint =
        crate::server::runtime_server_endpoint_io::read_supervisor_endpoint(&endpoint_path)
            .await
            .ok();
    crate::server::runtime_server_exit_receipt::remove_stale(&state_home).await?;
    crate::server::runtime_server_supervisor::remove_runtime_server_run_intent(&state_home).await?;
    if let Some(endpoint) = &endpoint {
        crate::server::runtime_server_supervisor::request_runtime_server_drain(&state_home).await?;
        let exit = crate::server::runtime_server_exit_receipt::await_owner_exit(
            &state_home,
            endpoint.owner_epoch,
        )
        .await?;
        if !exit.clean_drain {
            return Err(format!(
                "Runtime Server owner {} exited after a failed service drain: errors={:?}",
                exit.owner_epoch, exit.errors
            ));
        }
    }
    crate::server::runtime_server_supervisor::unload_runtime_server_supervisor(&state_home).await?;
    crate::server::runtime_server_supervisor::mark_runtime_server_operator_stopped(&state_home)
        .await?;
    let receipt = finalize_stopped_runtime_server(
        &state_home,
        endpoint.as_ref(),
        super::request_identity("control-stop").await?,
    )
    .await?;
    let mut bytes = serde_json::to_vec(&receipt)
        .map_err(|error| format!("failed to encode Runtime Server stop receipt: {error}"))?;
    bytes.push(b'\n');
    let mut stdout = tokio::io::stdout();
    stdout
        .write_all(&bytes)
        .await
        .map_err(|error| format!("failed to write Runtime Server stop receipt: {error}"))?;
    stdout
        .flush()
        .await
        .map_err(|error| format!("failed to flush Runtime Server stop receipt: {error}"))?;
    Ok(())
}

async fn finalize_stopped_runtime_server(
    state_home: &Path,
    endpoint: Option<&RuntimeServerEndpoint>,
    request_id: String,
) -> Result<RuntimeServerStopReceipt, String> {
    if let Some(endpoint) = endpoint {
        crate::server::runtime_server_endpoint_io::cleanup_endpoint(state_home, endpoint).await?;
    }
    let endpoint_path = runtime_server_endpoint_path(state_home)?;
    let endpoint_removed = path_is_removed(&endpoint_path).await?;
    let (control_socket_removed, data_plane_socket_removed, status_memory_removed) = match endpoint
    {
        Some(endpoint) => (
            path_is_removed(Path::new(&endpoint.socket_path)).await?,
            path_is_removed(Path::new(&endpoint.data_plane_socket_path)).await?,
            path_is_removed(Path::new(&endpoint.status_memory_path)).await?,
        ),
        None => (true, true, true),
    };
    if !endpoint_removed
        || !control_socket_removed
        || !data_plane_socket_removed
        || !status_memory_removed
    {
        return Err(format!(
            "Runtime Server supervisor stopped but terminal artifacts remain: endpointRemoved={endpoint_removed} controlSocketRemoved={control_socket_removed} dataPlaneSocketRemoved={data_plane_socket_removed} statusMemoryRemoved={status_memory_removed}"
        ));
    }
    Ok(RuntimeServerStopReceipt {
        schema_id: "agent.semantic-protocols.runtime-server-stop-receipt.v1",
        schema_version: "1",
        request_id,
        state: "stopped",
        lifecycle_authority: "state-home-lifecycle",
        operator_stop_recorded: true,
        endpoint_path: endpoint_path.to_string_lossy().into_owned(),
        endpoint_removed,
        control_socket_removed,
        data_plane_socket_removed,
        status_memory_removed,
    })
}

async fn path_is_removed(path: &Path) -> Result<bool, String> {
    tokio::fs::try_exists(path)
        .await
        .map(|exists| !exists)
        .map_err(|error| {
            format!(
                "failed to inspect Runtime Server artifact {}: {error}",
                path.display()
            )
        })
}

#[cfg(test)]
#[path = "../../tests/unit/runtime_server_stop.rs"]
mod tests;
