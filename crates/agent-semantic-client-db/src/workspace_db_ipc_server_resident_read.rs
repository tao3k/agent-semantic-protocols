use tokio::time::Instant;

use crate::workspace_db_ipc::{
    RuntimeResidentReadEvidence, RuntimeResidentReadState, RuntimeResidentReadTerminalState,
    WorkspaceIpcResidentReadWorkCounters,
};

pub(super) fn record_terminal(
    telemetry_sender: Option<&crate::runtime_telemetry_bus::RuntimeTelemetryBusSender>,
    evidence: &RuntimeResidentReadEvidence,
    surface: &str,
) -> Result<String, String> {
    let Some(sender) = telemetry_sender else {
        return Ok(format!("sha256:resident-read:{}", evidence.operation_id));
    };
    let digest = sender.try_record_resident_read_terminal(
        crate::runtime_telemetry_bus::ResidentReadTerminalContext {
            operation_id: evidence.operation_id.clone(),
            surface: surface.to_owned(),
            workspace_identity: evidence.workspace_identity.clone(),
            generation_digest: evidence.generation_digest.clone(),
            root_digest: evidence.root_digest.clone(),
            read_state: evidence.read_state.clone(),
            elapsed_micros: evidence.elapsed_micros,
            work_counters: evidence.work_counters.clone(),
        },
        crate::runtime_telemetry_bus::ResidentReadTerminalOutcome {
            terminal_state: evidence.terminal_state.clone(),
        },
    )?;
    if digest != evidence.telemetry_digest {
        return Err("runtime resident telemetry digest mismatch".to_owned());
    }
    Ok(digest)
}

pub(super) async fn read_merkle_owner(
    memory_registry: &std::sync::Arc<
        crate::runtime_server_workspace::RuntimeServerWorkspaceRegistry,
    >,
    workspace_identity: &str,
    request_id: &str,
    merkle_request: crate::workspace_db_ipc::RuntimeMerkleOwnerReadRequest,
    telemetry_sender: Option<&crate::runtime_telemetry_bus::RuntimeTelemetryBusSender>,
) -> crate::workspace_db_ipc::WorkspaceDbIpcResult {
    if let Err(message) = merkle_request.validate() {
        return crate::workspace_db_ipc::WorkspaceDbIpcResult::Failed {
            code: "runtime-server-merkle-owner-request-invalid".to_owned(),
            message,
        };
    }
    let started = Instant::now();
    let counters_before = memory_registry.data_plane_counters();
    match memory_registry
        .read_projection_merkle_owner(
            workspace_identity,
            std::path::Path::new(&merkle_request.project_root),
            &merkle_request.owner_path,
        )
        .await
    {
        Ok(read) => {
            let counter_delta = memory_registry
                .data_plane_counters()
                .delta_since(&counters_before);
            let (generation_digest, root_digest, read_state) = match &read {
                crate::runtime_server_workspace::WorkspaceRuntimeMerkleOwnerRead::Owner {
                    generation_digest,
                    root_digest,
                    ..
                } => (generation_digest.clone(), root_digest.clone(), RuntimeResidentReadState::Owner),
                crate::runtime_server_workspace::WorkspaceRuntimeMerkleOwnerRead::OwnerMissing {
                    generation_digest,
                    root_digest,
                    ..
                } => (generation_digest.clone(), root_digest.clone(), RuntimeResidentReadState::OwnerMissing),
            };
            let evidence = evidence(
                request_id.to_owned(),
                workspace_identity.to_owned(),
                generation_digest,
                root_digest,
                read_state,
                started,
                counters(counter_delta),
            );
            if let Err(error) =
                record_terminal(telemetry_sender, &evidence, "runtime-resident-merkle-owner")
            {
                return crate::workspace_db_ipc::WorkspaceDbIpcResult::Failed {
                    code: "runtime-resident-read-telemetry-failed".to_owned(),
                    message: error,
                };
            }
            crate::workspace_db_ipc::WorkspaceDbIpcResult::RuntimeMerkleOwner { read, evidence }
        }
        Err(message) => crate::workspace_db_ipc::WorkspaceDbIpcResult::Failed {
            code: "runtime-server-merkle-owner-read-failed".to_owned(),
            message,
        },
    }
}

pub(super) fn source_index_identity(
    lookup: &agent_semantic_search_projection::ResidentSearchReadyResult,
    authority: Result<crate::runtime_server_workspace::WorkspaceSearchGenerationAuthority, String>,
) -> (String, String) {
    authority
        .map(|authority| {
            (
                authority.generation_digest,
                authority.owner_merkle_root_digest,
            )
        })
        .unwrap_or_else(|_| (lookup.generation_digest.clone(), lookup.root_digest.clone()))
}

pub(super) fn selector_identity(
    read: &crate::runtime_server_workspace::WorkspaceRuntimeSelectorRead,
) -> (String, String, RuntimeResidentReadState) {
    match read {
        crate::runtime_server_workspace::WorkspaceRuntimeSelectorRead::Projection {
            generation_digest,
            root_digest,
            ..
        } => (
            generation_digest.clone(),
            root_digest.clone(),
            RuntimeResidentReadState::Projection,
        ),
        crate::runtime_server_workspace::WorkspaceRuntimeSelectorRead::ProjectionMissing {
            generation_digest,
            root_digest,
            ..
        } => (
            generation_digest.clone(),
            root_digest.clone(),
            RuntimeResidentReadState::ProjectionMissing,
        ),
        crate::runtime_server_workspace::WorkspaceRuntimeSelectorRead::ProjectionScopeOmitted {
            generation_digest,
            root_digest,
            ..
        } => (
            generation_digest.clone(),
            root_digest.clone(),
            RuntimeResidentReadState::ProjectionScopeOmitted,
        ),
        _ => (
            String::new(),
            String::new(),
            RuntimeResidentReadState::Other,
        ),
    }
}

pub(super) fn counters(
    counters: crate::runtime_server_workspace::RuntimeDataPlaneCounters,
) -> WorkspaceIpcResidentReadWorkCounters {
    WorkspaceIpcResidentReadWorkCounters {
        database_opens: counters.database_opens,
        filesystem_reads: counters.filesystem_reads,
        provider_spawns: counters.provider_spawns,
        control_socket_roundtrips: counters.control_socket_roundtrips,
    }
}

pub(super) fn evidence(
    operation_id: String,
    workspace_identity: String,
    generation_digest: String,
    root_digest: String,
    read_state: RuntimeResidentReadState,
    started: Instant,
    counters: WorkspaceIpcResidentReadWorkCounters,
) -> RuntimeResidentReadEvidence {
    let mut result = RuntimeResidentReadEvidence {
        schema_id: "agent.semantic-protocols.runtime-resident-read-evidence".to_owned(),
        schema_version: "1".to_owned(),
        operation_id,
        workspace_identity,
        generation_digest,
        root_digest,
        read_state,
        elapsed_micros: started.elapsed().as_micros().min(u128::from(u64::MAX)) as u64,
        work_counters: counters,
        terminal_state: RuntimeResidentReadTerminalState::Success,
        telemetry_digest: String::new(),
    };
    result.telemetry_digest = telemetry_digest(&result);
    result
}

fn telemetry_digest(evidence: &RuntimeResidentReadEvidence) -> String {
    crate::workspace_db_ipc::resident_read_terminal_digest(
        &crate::workspace_db_ipc::RuntimeResidentReadTerminalDigestInput {
            operation_id: &evidence.operation_id,
            surface: evidence.read_state.telemetry_surface(),
            workspace_identity: &evidence.workspace_identity,
            generation_digest: &evidence.generation_digest,
            root_digest: &evidence.root_digest,
            read_state: evidence.read_state,
            elapsed_micros: evidence.elapsed_micros,
            terminal_state: evidence.terminal_state,
        },
    )
}

pub(super) async fn write_response(
    stream: &mut tokio::io::BufStream<tokio::net::UnixStream>,
    endpoint: &crate::runtime_server_control::RuntimeServerEndpoint,
    workspace_identity: String,
    request_id: String,
    result: crate::workspace_db_ipc::WorkspaceDbIpcResult,
) -> Result<(), String> {
    crate::workspace_db_ipc::write_frame(
        stream,
        &crate::workspace_db_ipc::WorkspaceDbIpcResponse {
            schema_id: crate::workspace_db_ipc::WORKSPACE_DB_OWNER_RESPONSE_SCHEMA_ID.into(),
            schema_version: crate::workspace_db_ipc::WORKSPACE_DB_OWNER_SCHEMA_VERSION.to_owned(),
            workspace_identity,
            transport_contract_digest: endpoint.transport_contract_digest.clone(),
            owner_epoch: endpoint.owner_epoch,
            request_id: request_id.into(),
            result,
        },
    )
    .await
}
