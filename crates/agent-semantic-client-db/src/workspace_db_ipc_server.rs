use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use tokio::io::BufStream;
use tokio::net::{UnixListener, UnixStream};

use crate::workspace_db_ipc::{
    WORKSPACE_DB_OWNER_REQUEST_SCHEMA_ID, WORKSPACE_DB_OWNER_RESPONSE_SCHEMA_ID,
    WORKSPACE_DB_OWNER_SCHEMA_VERSION, WorkspaceDbIpcOperation, WorkspaceDbIpcRequest,
    WorkspaceDbIpcResponse, WorkspaceDbIpcResult, WorkspaceDbOwnerEndpoint,
};
use crate::{ProviderSearchWorkspaceSession, WorkspaceDbRegistry};

/// Serve one framed health request, validating workspace, epoch, and binding token.
pub async fn serve_one_workspace_db_ipc_request(
    listener: &UnixListener,
    endpoint: &WorkspaceDbOwnerEndpoint,
) -> Result<(), String> {
    let (mut stream, _) = listener
        .accept()
        .await
        .map_err(|error| format!("failed to accept workspace owner request: {error}"))?;
    let request: WorkspaceDbIpcRequest = read_frame(&mut stream).await?;
    let result = if request.workspace_identity == endpoint.workspace_identity
        && request.transport_contract_digest == endpoint.transport_contract_digest
        && request.owner_epoch == endpoint.owner_epoch
        && request.binding_token == endpoint.binding_token
    {
        WorkspaceDbIpcResult::Healthy
    } else {
        WorkspaceDbIpcResult::Failed {
            code: "workspace-owner-binding-mismatch".to_owned(),
            message: "request workspace, transport contract, epoch, or token does not match owner"
                .to_owned(),
        }
    };
    write_frame(
        &mut stream,
        &WorkspaceDbIpcResponse {
            schema_id: WORKSPACE_DB_OWNER_RESPONSE_SCHEMA_ID.to_owned(),
            schema_version: WORKSPACE_DB_OWNER_SCHEMA_VERSION.to_owned(),
            workspace_identity: endpoint.workspace_identity.clone(),
            transport_contract_digest: endpoint.transport_contract_digest.clone(),
            owner_epoch: endpoint.owner_epoch,
            request_id: request.request_id,
            result,
        },
    )
    .await
}

/// Serve one real workspace-session operation through the unique owner registry.
pub async fn serve_one_workspace_db_session_request(
    listener: &UnixListener,
    endpoint: &WorkspaceDbOwnerEndpoint,
    registry: &WorkspaceDbRegistry,
) -> Result<(), String> {
    let (mut stream, _) = listener
        .accept()
        .await
        .map_err(|error| format!("failed to accept workspace owner session request: {error}"))?;
    serve_workspace_db_session_stream(&mut stream, endpoint, registry)
        .await
        .map(|_| ())
}

async fn serve_workspace_db_session_stream(
    stream: &mut UnixStream,
    endpoint: &WorkspaceDbOwnerEndpoint,
    registry: &WorkspaceDbRegistry,
) -> Result<bool, String> {
    while let Some(request) = read_optional_frame::<WorkspaceDbIpcRequest>(&mut *stream).await? {
        let shutdown_requested = matches!(request.operation, WorkspaceDbIpcOperation::Shutdown);
        let result = if request.schema_id != WORKSPACE_DB_OWNER_REQUEST_SCHEMA_ID {
            WorkspaceDbIpcResult::Failed {
                code: "workspace-owner-request-schema-id-mismatch".to_owned(),
                message: format!(
                    "request schema_id must be {:?}, got {:?}",
                    WORKSPACE_DB_OWNER_REQUEST_SCHEMA_ID, request.schema_id
                ),
            }
        } else if request.schema_version != WORKSPACE_DB_OWNER_SCHEMA_VERSION {
            WorkspaceDbIpcResult::Failed {
                code: "workspace-owner-request-schema-version-mismatch".to_owned(),
                message: format!(
                    "request schema_version must be {:?}, got {:?}",
                    WORKSPACE_DB_OWNER_SCHEMA_VERSION, request.schema_version
                ),
            }
        } else if request.workspace_identity != endpoint.workspace_identity
            || request.transport_contract_digest != endpoint.transport_contract_digest
            || request.owner_epoch != endpoint.owner_epoch
            || request.binding_token != endpoint.binding_token
        {
            WorkspaceDbIpcResult::Failed {
                code: "workspace-owner-binding-mismatch".to_owned(),
                message:
                    "request workspace, transport contract, epoch, or token does not match owner"
                        .to_owned(),
            }
        } else if shutdown_requested {
            WorkspaceDbIpcResult::ShutdownAccepted
        } else {
            dispatch_workspace_db_session_operation(
                registry,
                &request.workspace_identity,
                request.operation,
            )
            .await
        };
        let shutdown_accepted =
            shutdown_requested && matches!(&result, WorkspaceDbIpcResult::ShutdownAccepted);
        write_frame(
            &mut *stream,
            &WorkspaceDbIpcResponse {
                schema_id: WORKSPACE_DB_OWNER_RESPONSE_SCHEMA_ID.to_owned(),
                schema_version: WORKSPACE_DB_OWNER_SCHEMA_VERSION.to_owned(),
                workspace_identity: endpoint.workspace_identity.clone(),
                transport_contract_digest: endpoint.transport_contract_digest.clone(),
                owner_epoch: endpoint.owner_epoch,
                request_id: request.request_id,
                result,
            },
        )
        .await?;
        if shutdown_accepted {
            return Ok(true);
        }
    }
    Ok(false)
}

pub async fn serve_workspace_db_session_until_shutdown(
    listener: &UnixListener,
    endpoint: &WorkspaceDbOwnerEndpoint,
    registry: Arc<WorkspaceDbRegistry>,
    last_activity_epoch_seconds: Arc<AtomicU64>,
    mut shutdown: tokio::sync::watch::Receiver<bool>,
) -> Result<(), String> {
    let mut sessions = tokio::task::JoinSet::new();
    let connection_supervisor =
        crate::runtime_server_runtime::RuntimeServerConnectionSupervisor::for_current_runtime(
            "workspace-db-ipc",
        );
    loop {
        if *shutdown.borrow() {
            sessions.abort_all();
            return Ok(());
        }
        tokio::select! {
            changed = shutdown.changed() => {
                match changed {
                    Ok(()) if *shutdown.borrow() => {
                        sessions.abort_all();
                        return Ok(());
                    }
                    Ok(()) => {}
                    Err(_) => {
                        sessions.abort_all();
                        return Ok(());
                    }
                }
            }
            accepted = listener.accept(), if connection_supervisor.has_capacity() => {
                let (mut stream, _) = accepted
                    .map_err(|error| format!("failed to accept workspace owner session request: {error}"))?;
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|duration| duration.as_secs())
                    .unwrap_or(0);
                last_activity_epoch_seconds.store(now, Ordering::Relaxed);
                let endpoint = endpoint.clone();
                let registry = Arc::clone(&registry);
                let connection_lease = connection_supervisor
                    .try_admit()
                    .expect("capacity guard must admit one workspace IPC connection");
                sessions.spawn(async move {
                    let result = serve_workspace_db_session_stream(&mut stream, &endpoint, &registry).await;
                    (connection_lease, result)
                });
            }
            completed = sessions.join_next(), if !sessions.is_empty() => {
                match completed {
                    Some(Ok((_connection_lease, Ok(true)))) => {
                        sessions.abort_all();
                        return Ok(());
                    }
                    Some(Ok((_connection_lease, Ok(false)))) => {}
                    Some(Ok((_connection_lease, Err(client_error)))) => {
                        eprintln!(
                            "[runtime-server-workspace-client] status=failed workspaceIdentity={} error={client_error}",
                            endpoint.workspace_identity
                        );
                    }
                    Some(Err(error)) => {
                        return Err(format!("workspace owner session task failed: {error}"));
                    }
                    None => {}
                }
            }
        }
    }
}

async fn dispatch_workspace_db_session_operation(
    registry: &WorkspaceDbRegistry,
    workspace_identity: &str,
    operation: WorkspaceDbIpcOperation,
) -> WorkspaceDbIpcResult {
    let dispatched = match operation {
        WorkspaceDbIpcOperation::ProjectTreeSitterQuery { .. } => Err(
            "Tree-sitter queries are available only through the Runtime Server".to_owned(),
        ),
        WorkspaceDbIpcOperation::Health => return WorkspaceDbIpcResult::Healthy,
        WorkspaceDbIpcOperation::Shutdown => return WorkspaceDbIpcResult::ShutdownAccepted,
        WorkspaceDbIpcOperation::CacheControl { .. } => Err(
            "cache-control operations are only accepted by the Runtime Server data plane"
                .to_owned(),
        ),
        WorkspaceDbIpcOperation::AgentSessionRegistry { .. } => Err(
            "agent-session registry operations are not admitted until the staged registry dispatcher is published"
                .to_owned(),
        ),
        WorkspaceDbIpcOperation::ReadSourceIndex { .. } => {
            Err("source-index reads are only accepted by the Runtime Server data plane".to_owned())
        }
        WorkspaceDbIpcOperation::ReadRuntimeSelector { .. }
        | WorkspaceDbIpcOperation::ReadRuntimeOwner { .. }
        | WorkspaceDbIpcOperation::ProjectProviderOwner { .. }
        | WorkspaceDbIpcOperation::ReadRuntimeSearchGenerationAuthority { .. } => Err(
            "resident runtime selector reads are only accepted by the Runtime Server data plane"
                .to_owned(),
        ),
        WorkspaceDbIpcOperation::PublishRuntimeSelectorOverlay { .. } => Err(
            "resident runtime selector writes are only accepted by the Runtime Server data plane"
                .to_owned(),
        ),
        WorkspaceDbIpcOperation::EnsureRuntimeGenerationReady { .. }
        | WorkspaceDbIpcOperation::AdmitRuntimeGeneration { .. }
        | WorkspaceDbIpcOperation::SubmitRuntimeGenerationMutation { .. }
        | WorkspaceDbIpcOperation::ReadRuntimeGenerationDurability { .. }
        | WorkspaceDbIpcOperation::ReadRuntimeGraphFacts { .. }
        | WorkspaceDbIpcOperation::EvaluateGraphTurbo { .. } => Err(
            "canonical generation admission is only accepted by the Runtime Server data plane"
                .to_owned(),
        ),
        WorkspaceDbIpcOperation::RefreshCodexMultiAgentControlPlane { .. }
        | WorkspaceDbIpcOperation::ReadCodexMultiAgentControlPlane { .. } => Err(
            "Codex multi-agent control-plane operations are only accepted by the Runtime Server data plane"
                .to_owned(),
        ),
        WorkspaceDbIpcOperation::WriteProviderIncrementalOwner { request } => {
            let session = registry
                .acquire(&request.scope.project_root, &request.scope)
                .await;
            match session {
                Ok(session) => session
                    .write_provider_incremental_owner(&request)
                    .await
                    .map(|receipt| WorkspaceDbIpcResult::ProviderIncrementalOwner { receipt }),
                Err(error) => Err(error),
            }
        }
        WorkspaceDbIpcOperation::ReadProviderTreeSitterQuery {
            query,
            incremental_budget,
            continuation,
        } => {
            let session = registry
                .acquire(&query.scope.project_root, &query.scope)
                .await;
            match session {
                Ok(session) => session
                    .read_provider_treesitter_query(
                        &query,
                        incremental_budget,
                        continuation.as_ref(),
                    )
                    .await
                    .map(|read| WorkspaceDbIpcResult::ProviderTreeSitterQuery { read }),
                Err(error) => Err(error),
            }
        }
        WorkspaceDbIpcOperation::ReadProviderOwnerSnapshot { scope, owner_path } => {
            let session = registry.acquire(&scope.project_root, &scope).await;
            match session {
                Ok(session) => session
                    .read_provider_owner_snapshot(&scope, &owner_path)
                    .await
                    .map(|snapshot| WorkspaceDbIpcResult::ProviderOwnerSnapshot { snapshot }),
                Err(error) => Err(error),
            }
        }
        WorkspaceDbIpcOperation::ReadProviderOwnerWarm { scope, owner } => {
            let session = admitted_or_bootstrap_workspace(
                registry,
                workspace_identity,
                Path::new(&scope.project_root),
            )
            .await;
            match session {
                Ok(session) => session.read_provider_owner_warm(&scope, &owner).await.map(
                    |(probe, snapshot)| WorkspaceDbIpcResult::ProviderOwnerWarm { probe, snapshot },
                ),
                Err(error) => Err(error),
            }
        }
        WorkspaceDbIpcOperation::ReadResidentSelector { request } => {
            let session = admitted_or_bootstrap_workspace(
                registry,
                workspace_identity,
                Path::new(&request.project_root),
            )
            .await;
            match session {
                Ok(session) => session
                    .read_resident_selector(&request)
                    .await
                    .map(|read| WorkspaceDbIpcResult::ResidentSelector { read }),
                Err(error) => Err(error),
            }
        }
        WorkspaceDbIpcOperation::WriteProviderTreeSitterOwnerResult { query, result } => {
            let session = registry
                .acquire(&query.scope.project_root, &query.scope)
                .await;
            match session {
                Ok(session) => session
                    .write_provider_treesitter_owner_result(&query, &result)
                    .await
                    .map(|receipt| WorkspaceDbIpcResult::ProviderTreeSitterOwner { receipt }),
                Err(error) => Err(error),
            }
        }
        WorkspaceDbIpcOperation::ProbeProviderOwners { scope, owners } => {
            let session = registry.acquire(&scope.project_root, &scope).await;
            match session {
                Ok(session) => session
                    .probe_provider_owners(&scope, &owners)
                    .await
                    .map(|receipt| WorkspaceDbIpcResult::ProviderOwners { receipt }),
                Err(error) => Err(error),
            }
        }
        WorkspaceDbIpcOperation::UpsertProviderInventory { request } => {
            let session = registry
                .acquire(&request.scope.project_root, &request.scope)
                .await;
            match session {
                Ok(session) => session
                    .upsert_provider_owner_inventory(&request)
                    .await
                    .map(|receipt| WorkspaceDbIpcResult::ProviderInventory { receipt }),
                Err(error) => Err(error),
            }
        }
        WorkspaceDbIpcOperation::FinishWrites { scope, mode } => {
            let session = registry.acquire(&scope.project_root, &scope).await;
            match session {
                Ok(session) => session
                    .finish_writes(mode)
                    .await
                    .map(|receipt| WorkspaceDbIpcResult::WriteFinish { receipt }),
                Err(error) => Err(error),
            }
        }
        WorkspaceDbIpcOperation::PublishRuntimeOwner { .. }
        | WorkspaceDbIpcOperation::TombstoneRuntimeOwner { .. }
        | WorkspaceDbIpcOperation::RelocateRuntimeOwner { .. } => Err(
            "Runtime owner publication is only accepted by the Runtime Server data plane"
                .to_owned(),
        ),
    };
    dispatched.unwrap_or_else(|message| WorkspaceDbIpcResult::Failed {
        code: "workspace-owner-operation-failed".to_owned(),
        message,
    })
}

pub(crate) async fn admitted_or_bootstrap_workspace(
    registry: &WorkspaceDbRegistry,
    workspace_identity: &str,
    project_root: &Path,
) -> Result<ProviderSearchWorkspaceSession, String> {
    if workspace_identity.trim().is_empty() {
        return Err("workspace admission requires a non-empty workspace identity".to_owned());
    }
    if let Some(session) = registry.loaded_session(workspace_identity) {
        return Ok(session);
    }
    let session = registry.bootstrap_workspace(project_root).await?;
    if session.workspace_identity() != workspace_identity {
        return Err(format!(
            "workspace admission identity mismatch: requested={workspace_identity} resolved={}",
            session.workspace_identity()
        ));
    }
    Ok(session)
}

async fn run_agent_session_registry_operation(
    registry: std::sync::Arc<crate::AgentSessionRegistry>,
    project_root: std::path::PathBuf,
    operation: crate::workspace_db_ipc::AgentSessionRegistryIpcOperation,
) -> Result<crate::workspace_db_ipc::AgentSessionRegistryIpcResult, String> {
    agent_session_registry_dispatch::run_operation(registry, project_root, operation).await
}
#[path = "workspace_db_ipc_server_agent_session.rs"]
mod agent_session;
#[path = "workspace_db_ipc_server_agent_session_registry.rs"]
mod agent_session_registry_dispatch;
#[path = "workspace_db_ipc_server_generation.rs"]
mod generation;
#[path = "workspace_db_ipc_server_graph_turbo.rs"]
mod graph_turbo;

#[path = "workspace_db_ipc_server_cache_control.rs"]
mod cache_control;
#[path = "workspace_db_ipc_server_codex_control_plane.rs"]
mod codex_control_plane;
/// Serve workspace-scoped data-plane requests through the single Runtime Server.
///
/// Unlike the removed per-workspace owner transport, the endpoint authenticates
/// the daemon while each request carries the workspace identity used to select
/// the server-resident registry entry.
pub async fn serve_runtime_server_workspace_stream(
    stream: UnixStream,
    endpoint: &crate::runtime_server_control::RuntimeServerEndpoint,
    registry: &WorkspaceDbRegistry,
    memory_registry: &std::sync::Arc<
        crate::runtime_server_workspace::RuntimeServerWorkspaceRegistry,
    >,
    generation_admission: Option<
        &std::sync::Arc<crate::runtime_server_admission::WorkspaceGenerationAdmission>,
    >,
    graph_turbo_evaluation_builder: Option<&crate::runtime_server::GraphTurboEvaluationBuilder>,
    runtime_search_service: Option<&crate::runtime_search_service::RuntimeSearchServiceHandle>,
    agent_session_registry_owner: Option<&std::sync::Arc<crate::AgentSessionRegistry>>,
    session_control_plane_runtime_registry: &std::sync::Arc<
        crate::SessionControlPlaneRuntimeRegistry,
    >,
    agent_session_status: Option<
        &crate::runtime_server_agent_session_status::AgentSessionStatusHandle,
    >,
    codex_multi_agent_control_plane_owner: &std::sync::Arc<
        crate::codex_multi_agent_control_plane_owner::CodexMultiAgentControlPlaneOwner,
    >,
    telemetry_sender: Option<&crate::runtime_telemetry_bus::RuntimeTelemetryBusSender>,
    mut drain: tokio::sync::watch::Receiver<bool>,
) -> Result<(), String> {
    let mut stream = BufStream::new(stream);
    loop {
        let request = tokio::select! {
            request = read_optional_frame::<WorkspaceDbIpcRequest>(&mut stream) => request?,
            changed = drain.changed() => {
                let _ = changed;
                return Ok(());
            }
        };
        let Some(request) = request else {
            return Ok(());
        };
        let operation_started = std::time::Instant::now();
        let mut incident_context = crate::search_incident::workspace_ipc_terminal_context(&request);
        let workspace_request = memory_registry.begin_request(&request.workspace_identity);
        if let Some(context) = memory_registry.workspace_context(&request.workspace_identity) {
            registry.register_workspace_context(context);
        }
        let result = if request.schema_id != WORKSPACE_DB_OWNER_REQUEST_SCHEMA_ID {
            WorkspaceDbIpcResult::Failed {
                code: "runtime-server-data-request-schema-id-mismatch".to_owned(),
                message: format!(
                    "request schema_id must be {:?}, got {:?}",
                    WORKSPACE_DB_OWNER_REQUEST_SCHEMA_ID, request.schema_id
                ),
            }
        } else if request.schema_version != WORKSPACE_DB_OWNER_SCHEMA_VERSION {
            WorkspaceDbIpcResult::Failed {
                code: "runtime-server-data-request-schema-version-mismatch".to_owned(),
                message: format!(
                    "request schema_version must be {:?}, got {:?}",
                    WORKSPACE_DB_OWNER_SCHEMA_VERSION, request.schema_version
                ),
            }
        } else if request.workspace_identity.trim().is_empty() {
            WorkspaceDbIpcResult::Failed {
                code: "runtime-server-workspace-identity-missing".to_owned(),
                message: "request workspace identity must be non-empty text".to_owned(),
            }
        } else if request.transport_contract_digest != endpoint.transport_contract_digest
            || request.owner_epoch != endpoint.owner_epoch
            || request.binding_token != endpoint.binding_token
        {
            WorkspaceDbIpcResult::Failed {
                code: "runtime-server-data-binding-mismatch".to_owned(),
                message:
                    "request transport contract, epoch, or token does not match Runtime Server"
                        .to_owned(),
            }
        } else if matches!(request.operation, WorkspaceDbIpcOperation::Shutdown) {
            WorkspaceDbIpcResult::Failed {
                code: "runtime-server-shutdown-control-required".to_owned(),
                message: "workspace requests cannot stop the shared Runtime Server".to_owned(),
            }
        } else if let Err(message) = &workspace_request {
            WorkspaceDbIpcResult::Failed {
                code: "runtime-server-workspace-retiring".to_owned(),
                message: message.clone(),
            }
        } else if let Some(project_root) =
            generation::resident_read_project_root(&request.operation)
            && let Err(message) = generation::require_terminal_generation_for_read(
                generation_admission.map(std::sync::Arc::as_ref),
                &request.workspace_identity,
                project_root,
            )
        {
            WorkspaceDbIpcResult::Failed {
                code: "active-workspace-generation-required".to_owned(),
                message,
            }
        } else {
            match request.operation {
                WorkspaceDbIpcOperation::CacheControl {
                    request: cache_request,
                } => {
                    cache_control::evaluate(
                        memory_registry,
                        generation_admission.map(std::sync::Arc::as_ref),
                        &request.workspace_identity,
                        cache_request,
                    )
                    .await
                }
                WorkspaceDbIpcOperation::ReadRuntimeSelector {
                    project_root,
                    language_id: _,
                    projection_kind,
                    structural_selector,
                } => match memory_registry
                    .read_projection_selector(
                        &request.workspace_identity,
                        Path::new(&project_root),
                        projection_kind,
                        &structural_selector,
                    )
                    .await
                {
                    Ok(read) => WorkspaceDbIpcResult::RuntimeSelector { read },
                    Err(message) => WorkspaceDbIpcResult::Failed {
                        code: "runtime-server-selector-read-failed".to_owned(),
                        message,
                    },
                },
                WorkspaceDbIpcOperation::ReadRuntimeOwner {
                    project_root,
                    owner_path,
                } => match memory_registry
                    .read_projection_owner(
                        &request.workspace_identity,
                        Path::new(&project_root),
                        &owner_path,
                    )
                    .await
                {
                    Ok(read) => WorkspaceDbIpcResult::RuntimeOwner { read },
                    Err(message) => WorkspaceDbIpcResult::Failed {
                        code: "runtime-server-owner-read-failed".to_owned(),
                        message,
                    },
                },
                WorkspaceDbIpcOperation::ProjectTreeSitterQuery {
                    project_root,
                    language_id,
                    args,
                } => match runtime_search_service {
                    Some(service) => match service
                        .tree_sitter_query(
                            request.workspace_identity.clone(),
                            Path::new(&project_root).to_path_buf(),
                            language_id.to_string(),
                            args,
                        )
                        .await
                    {
                        Ok(rendered) => WorkspaceDbIpcResult::TreeSitterQuery { rendered },
                        Err(message) => WorkspaceDbIpcResult::Failed {
                            code: "runtime-server-tree-sitter-query-failed".to_owned(),
                            message,
                        },
                    },
                    None => WorkspaceDbIpcResult::Failed {
                        code: "runtime-server-search-service-unavailable".to_owned(),
                        message: "Runtime search service is not configured".to_owned(),
                    },
                },
                WorkspaceDbIpcOperation::ProjectProviderOwner {
                    project_root,
                    language_id,
                    owner_path,
                } => match runtime_search_service {
                    Some(service) => match service
                        .provider_owner(
                            request.workspace_identity.clone(),
                            Path::new(&project_root).to_path_buf(),
                            language_id.to_string(),
                            owner_path,
                        )
                        .await
                    {
                        Ok(owner) => WorkspaceDbIpcResult::ProviderOwnerProjection { owner },
                        Err(message) => WorkspaceDbIpcResult::Failed {
                            code: "runtime-server-provider-owner-query-failed".to_owned(),
                            message,
                        },
                    },
                    None => WorkspaceDbIpcResult::Failed {
                        code: "runtime-server-provider-owner-query-unavailable".to_owned(),
                        message: "Runtime Server provider owner query service is unavailable"
                            .to_owned(),
                    },
                },
                WorkspaceDbIpcOperation::ReadRuntimeSearchGenerationAuthority { project_root } => {
                    match memory_registry
                        .projection_search_generation_authority(
                            &request.workspace_identity,
                            Path::new(&project_root),
                        )
                        .await
                    {
                        Ok(authority) => WorkspaceDbIpcResult::RuntimeSearchGenerationAuthority {
                            authority: Some(authority),
                        },
                        Err(message) => WorkspaceDbIpcResult::Failed {
                            code: "runtime-server-search-generation-authority-read-failed"
                                .to_owned(),
                            message,
                        },
                    }
                }
                WorkspaceDbIpcOperation::ReadRuntimeGraphFacts {
                    project_root,
                    sources,
                } => match memory_registry
                    .read_projection_graph_facts(
                        &request.workspace_identity,
                        Path::new(&project_root),
                        &sources,
                    )
                    .await
                {
                    Ok(read) => WorkspaceDbIpcResult::RuntimeGraphFacts { read },
                    Err(message) => WorkspaceDbIpcResult::Failed {
                        code: "runtime-server-graph-facts-read-failed".to_owned(),
                        message,
                    },
                },
                WorkspaceDbIpcOperation::PublishRuntimeSelectorOverlay {
                    project_root,
                    overlay,
                } => {
                    match memory_registry
                        .publish_selector_overlay(
                            &request.workspace_identity,
                            Path::new(&project_root),
                            overlay,
                        )
                        .await
                    {
                        Ok(receipt) => WorkspaceDbIpcResult::RuntimeSelectorOverlay { receipt },
                        Err(message) => WorkspaceDbIpcResult::Failed {
                            code: "runtime-server-selector-overlay-failed".to_owned(),
                            message,
                        },
                    }
                }
                WorkspaceDbIpcOperation::EnsureRuntimeGenerationReady {
                    request_id: _,
                    project_root,
                } => {
                    generation::ensure_runtime_generation_ready(
                        memory_registry,
                        generation_admission.cloned(),
                        &request.workspace_identity,
                        project_root,
                    )
                    .await
                }
                WorkspaceDbIpcOperation::AdmitRuntimeGeneration {
                    mutation_id,
                    project_root,
                    changed_paths,
                    candidate,
                } => {
                    generation::admit_mutation(
                        memory_registry,
                        generation_admission.map(std::sync::Arc::as_ref),
                        &request.workspace_identity,
                        mutation_id,
                        project_root,
                        changed_paths,
                        candidate,
                    )
                    .await
                }
                WorkspaceDbIpcOperation::SubmitRuntimeGenerationMutation {
                    mutation_id,
                    project_root,
                    changed_paths,
                } => generation::submit_mutation(
                    std::sync::Arc::clone(memory_registry),
                    generation_admission.cloned(),
                    request.workspace_identity.clone(),
                    mutation_id,
                    project_root,
                    changed_paths,
                ),
                WorkspaceDbIpcOperation::ReadRuntimeGenerationDurability { project_root } => {
                    match memory_registry.generation_durability(
                        &request.workspace_identity,
                        std::path::Path::new(&project_root),
                    ) {
                        Ok(receipt) => {
                            WorkspaceDbIpcResult::RuntimeGenerationDurability { receipt }
                        }
                        Err(message) => WorkspaceDbIpcResult::Failed {
                            code: "runtime-server-generation-durability-read-failed".to_owned(),
                            message,
                        },
                    }
                }
                WorkspaceDbIpcOperation::AgentSessionRegistry {
                    project_root,
                    operation,
                } => {
                    let changes_registry = !matches!(
                        &operation,
                        crate::workspace_db_ipc::AgentSessionRegistryIpcOperation::Query { .. }
                            | crate::workspace_db_ipc::AgentSessionRegistryIpcOperation::SessionById { .. }
                            | crate::workspace_db_ipc::AgentSessionRegistryIpcOperation::SessionByName { .. }
                            | crate::workspace_db_ipc::AgentSessionRegistryIpcOperation::SessionIsRetired { .. }
                            | crate::workspace_db_ipc::AgentSessionRegistryIpcOperation::SessionByIdAnyProject { .. }
                            | crate::workspace_db_ipc::AgentSessionRegistryIpcOperation::ProjectIdForRootSessionId { .. }
                            | crate::workspace_db_ipc::AgentSessionRegistryIpcOperation::DispatchLease { .. }
                    );
                    let result = agent_session::evaluate(
                        agent_session_registry_owner,
                        session_control_plane_runtime_registry,
                        project_root,
                        operation,
                    )
                    .await;
                    if changes_registry
                        && let (Some(status), Some(owner)) =
                            (agent_session_status, agent_session_registry_owner)
                        && let Err(message) = status.refresh_and_wait_published(owner).await
                    {
                        WorkspaceDbIpcResult::Failed {
                            code: "runtime-server-agent-session-status-publication-failed"
                                .to_owned(),
                            message,
                        }
                    } else {
                        result
                    }
                }
                WorkspaceDbIpcOperation::RefreshCodexMultiAgentControlPlane {
                    project_id,
                    root_session_id,
                } => {
                    codex_control_plane::refresh(
                        &request.workspace_identity,
                        agent_session_registry_owner,
                        codex_multi_agent_control_plane_owner,
                        project_id,
                        root_session_id,
                    )
                    .await
                }
                WorkspaceDbIpcOperation::ReadCodexMultiAgentControlPlane { root_session_id } => {
                    codex_control_plane::read(
                        &request.workspace_identity,
                        codex_multi_agent_control_plane_owner,
                        root_session_id,
                    )
                    .await
                }
                WorkspaceDbIpcOperation::EvaluateGraphTurbo {
                    project_root,
                    message,
                } => {
                    graph_turbo::evaluate(
                        memory_registry,
                        graph_turbo_evaluation_builder,
                        &request.workspace_identity,
                        project_root,
                        message,
                    )
                    .await
                }
                WorkspaceDbIpcOperation::ReadSourceIndex {
                    request: lookup_request,
                } => {
                    let lookup = memory_registry
                        .read_projection_source_index(
                            &request.workspace_identity,
                            Path::new(&lookup_request.project_root),
                            &lookup_request.query,
                            lookup_request.language_id.as_ref(),
                            lookup_request.limit,
                        )
                        .await;
                    match lookup {
                        Ok(lookup) => WorkspaceDbIpcResult::SourceIndex { lookup },
                        Err(message) => WorkspaceDbIpcResult::Failed {
                            code: "runtime-server-source-index-read-failed".to_owned(),
                            message,
                        },
                    }
                }
                WorkspaceDbIpcOperation::PublishRuntimeOwner {
                    project_root,
                    owner,
                } => {
                    let publication = memory_registry
                        .publish_owner_overlay(
                            request.request_id.clone(),
                            request.workspace_identity.clone(),
                            Path::new(&project_root),
                            owner,
                        )
                        .await;
                    match publication {
                        Ok(receipt) => WorkspaceDbIpcResult::RuntimeGeneration { receipt },
                        Err(message) => WorkspaceDbIpcResult::Failed {
                            code: "runtime-server-generation-publish-failed".to_owned(),
                            message,
                        },
                    }
                }
                WorkspaceDbIpcOperation::TombstoneRuntimeOwner {
                    project_root,
                    owner_path,
                } => {
                    let publication = memory_registry
                        .tombstone_owner_overlay(
                            request.request_id.clone(),
                            request.workspace_identity.clone(),
                            Path::new(&project_root),
                            owner_path,
                        )
                        .await;
                    match publication {
                        Ok(receipt) => WorkspaceDbIpcResult::RuntimeGeneration { receipt },
                        Err(message) => WorkspaceDbIpcResult::Failed {
                            code: "runtime-server-owner-tombstone-failed".to_owned(),
                            message,
                        },
                    }
                }
                WorkspaceDbIpcOperation::RelocateRuntimeOwner {
                    project_root,
                    previous_owner_path,
                    owner,
                } => {
                    let publication = memory_registry
                        .relocate_owner_overlay(
                            request.request_id.clone(),
                            request.workspace_identity.clone(),
                            Path::new(&project_root),
                            previous_owner_path,
                            owner,
                        )
                        .await;
                    match publication {
                        Ok(receipt) => WorkspaceDbIpcResult::RuntimeGeneration { receipt },
                        Err(message) => WorkspaceDbIpcResult::Failed {
                            code: "runtime-server-owner-relocation-failed".to_owned(),
                            message,
                        },
                    }
                }
                operation => {
                    dispatch_workspace_db_session_operation(
                        registry,
                        &request.workspace_identity,
                        operation,
                    )
                    .await
                }
            }
        };
        let _ = crate::search_incident::record_workspace_ipc_terminal(
            telemetry_sender,
            incident_context.take(),
            &result,
            operation_started.elapsed(),
        );
        write_frame(
            &mut stream,
            &WorkspaceDbIpcResponse {
                schema_id: WORKSPACE_DB_OWNER_RESPONSE_SCHEMA_ID.to_owned(),
                schema_version: WORKSPACE_DB_OWNER_SCHEMA_VERSION.to_owned(),
                workspace_identity: request.workspace_identity,
                transport_contract_digest: endpoint.transport_contract_digest.clone(),
                owner_epoch: endpoint.owner_epoch,
                request_id: request.request_id,
                result,
            },
        )
        .await?;
    }
}

use crate::workspace_db_ipc::{read_frame, read_optional_frame, write_frame};
