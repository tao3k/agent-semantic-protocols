use std::path::Path;

pub(super) async fn read_runtime_owner(
    memory_registry: &crate::runtime_server_workspace::RuntimeServerWorkspaceRegistry,
    workspace_identity: &str,
    request_id: String,
    project_root: String,
    owner_path: String,
    telemetry_sender: Option<&crate::runtime_telemetry_bus::RuntimeTelemetryBusSender>,
) -> Result<crate::workspace_db_ipc::WorkspaceDbIpcResult, String> {
    let started = tokio::time::Instant::now();
    let counters_before = memory_registry.data_plane_counters();
    match memory_registry
        .read_projection_owner(workspace_identity, Path::new(&project_root), &owner_path)
        .await
    {
        Ok(read) => {
            let counters = memory_registry
                .data_plane_counters()
                .delta_since(&counters_before);
            let (generation_digest, root_digest, read_state) = match &read {
                crate::runtime_server_workspace::WorkspaceRuntimeOwnerRead::Owner { generation_digest, root_digest, .. } => (generation_digest.clone(), root_digest.clone(), crate::workspace_db_ipc::RuntimeResidentReadState::Owner),
                crate::runtime_server_workspace::WorkspaceRuntimeOwnerRead::SparseProviderOwner { cache_digest, root_digest, .. } => (cache_digest.clone(), root_digest.clone(), crate::workspace_db_ipc::RuntimeResidentReadState::SparseOwner),
                crate::runtime_server_workspace::WorkspaceRuntimeOwnerRead::OwnerMissing { generation_digest, root_digest } => (generation_digest.clone(), root_digest.clone(), crate::workspace_db_ipc::RuntimeResidentReadState::OwnerMissing),
                crate::runtime_server_workspace::WorkspaceRuntimeOwnerRead::GenerationMissing => (String::new(), String::new(), crate::workspace_db_ipc::RuntimeResidentReadState::GenerationMissing),
            };
            let evidence = super::resident_read::evidence(
                request_id,
                workspace_identity.to_owned(),
                generation_digest,
                root_digest,
                read_state,
                started,
                super::resident_read::counters(counters),
            );
            super::resident_read::record_terminal(
                telemetry_sender,
                &evidence,
                "runtime-resident-owner",
            )
            .map_err(|error| format!("runtime-resident-read-telemetry-failed: {error}"))?;
            Ok(crate::workspace_db_ipc::WorkspaceDbIpcResult::RuntimeOwner { read, evidence })
        }
        Err(message) => Ok(crate::workspace_db_ipc::WorkspaceDbIpcResult::Failed {
            code: "runtime-server-owner-read-failed".to_owned(),
            message,
        }),
    }
}
