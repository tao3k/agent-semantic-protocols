use std::path::Path;

use agent_semantic_client_db::{
    AgentSessionRegistry, workspace_db_ipc::connect_runtime_server_workspace_session,
};

use super::{SessionArgs, current_root_session_id};

fn requested_root_session_id(args: &SessionArgs) -> Result<String, String> {
    args.root_session_id
        .clone()
        .or_else(current_root_session_id)
        .ok_or_else(|| {
            "Codex control-plane observation requires --root-session-id outside an active agent session"
                .to_owned()
        })
}

/// Publishes the current registry-derived downstream control-plane projection.
pub(super) fn refresh_control_plane(args: &SessionArgs, project_root: &Path) -> Result<(), String> {
    let project_id = AgentSessionRegistry::workspace_id(project_root)?; // Registry-derived authority.
    let root_session_id = requested_root_session_id(args)?;
    let project_root = project_root.to_path_buf();
    let receipt = crate::server::runtime_server::block_on_runtime_server_client(async move {
        let session = connect_runtime_server_workspace_session(&project_root).await?;
        session
            .refresh_codex_multi_agent_control_plane(project_id, root_session_id)
            .await
    })??;
    println!(
        "{}",
        serde_json::to_string(&receipt).map_err(|error| {
            format!("failed to encode Codex control-plane publication receipt: {error}")
        })?
    );
    Ok(())
}

pub(super) fn show_control_plane(args: &SessionArgs, project_root: &Path) -> Result<(), String> {
    let root_session_id = requested_root_session_id(args)?;
    let requested_root_session_id = root_session_id.clone();
    let project_root = project_root.to_path_buf();
    let projection = crate::server::runtime_server::block_on_runtime_server_client(async move {
        let session = connect_runtime_server_workspace_session(&project_root).await?;
        session
            .read_codex_multi_agent_control_plane(root_session_id)
            .await
    })??
    .ok_or_else(|| {
        format!(
            "Codex control-plane projection is not materialized for rootSessionId={requested_root_session_id}; run `asp agent session control-plane refresh --root-session-id {requested_root_session_id}`"
        )
    })?;
    println!(
        "{}",
        serde_json::to_string(&projection)
            .map_err(|error| format!("failed to encode Codex control-plane projection: {error}"))?
    );
    Ok(())
}
