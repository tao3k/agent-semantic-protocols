// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Typed Runtime Server stop receipt owned by the global State Home lifecycle.

use serde::Serialize;
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
    let outcome = agent_semantic_client_db::runtime_server_supervisor::RuntimeServerSupervisor
        .stop_runtime_server(&state_home)
        .await?;
    let receipt = RuntimeServerStopReceipt {
        schema_id: "agent.semantic-protocols.runtime-server-stop-receipt.v1",
        schema_version: "1",
        request_id: super::request_identity("control-stop").await?,
        state: "stopped",
        lifecycle_authority: "state-home-lifecycle",
        operator_stop_recorded: outcome.operator_stop_recorded,
        endpoint_path: outcome.endpoint_path,
        endpoint_removed: outcome.endpoint_removed,
        control_socket_removed: outcome.control_socket_removed,
        data_plane_socket_removed: outcome.data_plane_socket_removed,
        status_memory_removed: outcome.status_memory_removed,
    };
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
