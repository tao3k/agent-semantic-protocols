use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

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
            accepted = listener.accept() => {
                let (mut stream, _) = accepted
                    .map_err(|error| format!("failed to accept workspace owner session request: {error}"))?;
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|duration| duration.as_secs())
                    .unwrap_or(0);
                last_activity_epoch_seconds.store(now, Ordering::Relaxed);
                let endpoint = endpoint.clone();
                let registry = Arc::clone(&registry);
                sessions.spawn(async move {
                    serve_workspace_db_session_stream(&mut stream, &endpoint, &registry).await
                });
            }
            completed = sessions.join_next(), if !sessions.is_empty() => {
                match completed {
                    Some(Ok(Ok(true))) => {
                        sessions.abort_all();
                        return Ok(());
                    }
                    Some(Ok(Ok(false))) => {}
                    Some(Ok(Err(client_error))) => {
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
        WorkspaceDbIpcOperation::ReadRuntimeSelector { .. } => Err(
            "resident runtime selector reads are only accepted by the Runtime Server data plane"
                .to_owned(),
        ),
        WorkspaceDbIpcOperation::EnsureRuntimeOwner { .. } => Err(
            "resident runtime owner freshness is only accepted by the Runtime Server data plane"
                .to_owned(),
        ),
        WorkspaceDbIpcOperation::PublishRuntimeSelectorOverlay { .. } => Err(
            "resident runtime selector writes are only accepted by the Runtime Server data plane"
                .to_owned(),
        ),
        WorkspaceDbIpcOperation::AdmitRuntimeGeneration { .. }
        | WorkspaceDbIpcOperation::EnsureRuntimeGeneration { .. }
        | WorkspaceDbIpcOperation::RepairRuntimeGenerationLocator { .. }
        | WorkspaceDbIpcOperation::ReadRuntimeGenerationDurability { .. }
        | WorkspaceDbIpcOperation::EvaluateHook { .. }
        | WorkspaceDbIpcOperation::EvaluateGraphTurbo { .. } => Err(
            "canonical generation admission is only accepted by the Runtime Server data plane"
                .to_owned(),
        ),
        WorkspaceDbIpcOperation::PublishCodexMultiAgentControlPlane { .. }
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

/// Serve workspace-scoped data-plane requests through the single Runtime Server.
///
/// Unlike the removed per-workspace owner transport, the endpoint authenticates
/// the daemon while each request carries the workspace identity used to select
/// the server-resident registry entry.
pub async fn serve_runtime_server_workspace_stream(
    stream: &mut UnixStream,
    endpoint: &crate::runtime_server_control::RuntimeServerEndpoint,
    registry: &WorkspaceDbRegistry,
    memory_registry: &crate::runtime_server_workspace::RuntimeServerWorkspaceRegistry,
    generation_admission: Option<&crate::runtime_server_admission::WorkspaceGenerationAdmission>,
    owner_projection_builder: Option<
        &crate::runtime_server_workspace::WorkspaceOwnerProjectionBuilder,
    >,
    hook_evaluation_builder: Option<&crate::runtime_server::HookEvaluationBuilder>,
    graph_turbo_evaluation_builder: Option<&crate::runtime_server::GraphTurboEvaluationBuilder>,
    agent_session_registry_owner: Option<&std::sync::Arc<crate::AgentSessionRegistry>>,
    codex_multi_agent_control_plane_owner: &std::sync::Arc<
        crate::codex_multi_agent_control_plane_owner::CodexMultiAgentControlPlaneOwner,
    >,
    mut drain: tokio::sync::watch::Receiver<bool>,
) -> Result<(), String> {
    loop {
        let request = tokio::select! {
            request = read_optional_frame::<WorkspaceDbIpcRequest>(&mut *stream) => request?,
            changed = drain.changed() => {
                let _ = changed;
                return Ok(());
            }
        };
        let Some(request) = request else {
            return Ok(());
        };
        let workspace_request = memory_registry.begin_request(&request.workspace_identity);
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
        } else {
            match request.operation {
                WorkspaceDbIpcOperation::ReadRuntimeSelector {
                    project_root,
                    projection_kind,
                    structural_selector,
                } => match memory_registry.read_runtime_selector(
                    &request.workspace_identity,
                    Path::new(&project_root),
                    &projection_kind,
                    &structural_selector,
                ) {
                    Ok(read) => WorkspaceDbIpcResult::RuntimeSelector { read },
                    Err(message) => WorkspaceDbIpcResult::Failed {
                        code: "runtime-server-selector-read-failed".to_owned(),
                        message,
                    },
                },
                WorkspaceDbIpcOperation::EnsureRuntimeOwner {
                    project_root,
                    language_id,
                    owner_path,
                } => match ensure_runtime_owner_freshness(
                    memory_registry,
                    &request.request_id,
                    &request.workspace_identity,
                    Path::new(&project_root),
                    &language_id,
                    &owner_path,
                    owner_projection_builder,
                )
                .await
                {
                    Ok(receipt) => WorkspaceDbIpcResult::RuntimeOwnerFreshness { receipt },
                    Err(message) => WorkspaceDbIpcResult::Failed {
                        code: "runtime-server-owner-freshness-failed".to_owned(),
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
                WorkspaceDbIpcOperation::AdmitRuntimeGeneration {
                    mutation_id,
                    project_root,
                    changed_paths,
                    candidate,
                } => {
                    generation::admit_mutation(
                        memory_registry,
                        generation_admission,
                        &request.workspace_identity,
                        mutation_id,
                        project_root,
                        changed_paths,
                        candidate,
                    )
                    .await
                }
                WorkspaceDbIpcOperation::EnsureRuntimeGeneration {
                    project_root,
                    candidate,
                } => {
                    match generation_admission {
                        Some(admission) => {
                            let project_root = std::path::PathBuf::from(project_root);
                            // Admission is the foreground contract.  A Building
                            // receipt proves the WorkspaceResident accepted the
                            // work; Merkle materialization remains server-owned
                            // background execution and must not hold this IPC
                            // request until terminal publication.
                            let ensured = admission
                                .ensure(&request.workspace_identity, &project_root, candidate)
                                .await;
                            match ensured {
                                Ok(receipt) => {
                                    WorkspaceDbIpcResult::RuntimeGenerationAdmission { receipt }
                                }
                                Err(message) => WorkspaceDbIpcResult::Failed {
                                    code: "runtime-server-generation-ensure-failed".to_owned(),
                                    message,
                                },
                            }
                        }
                        None => WorkspaceDbIpcResult::Failed {
                            code: "runtime-server-generation-admission-unavailable".to_owned(),
                            message: "Runtime Server has no canonical generation builder"
                                .to_owned(),
                        },
                    }
                }
                WorkspaceDbIpcOperation::RepairRuntimeGenerationLocator { project_root } => {
                    match generation_admission {
                        Some(admission) => {
                            let project_root = std::path::PathBuf::from(project_root);
                            let repair = async {
                                match memory_registry
                                    .published_generation_state(
                                        &request.workspace_identity,
                                        &project_root,
                                    )
                                    .await?
                                {
                                crate::runtime_server_workspace::PublishedWorkspaceGenerationState::Ready => {
                                    let (commit, reconciled) = match memory_registry
                                        .lease(&request.workspace_identity, &project_root)
                                    {
                                        Ok(generation) => (
                                            crate::runtime_server_admission::WorkspaceGenerationCommitReceipt {
                                                active_epoch: generation.epoch(),
                                                generation_digest: generation.runtime_generation_digest(),
                                                source_root_digest: generation
                                                    .generation()
                                                    .source_snapshot
                                                    .root_digest
                                                    .clone(),
                                            },
                                            false,
                                        ),
                                        Err(_) => {
                                            let restored = memory_registry
                                                .restore_published_generation(
                                                    format!(
                                                        "runtime-generation-locator-repair-{}",
                                                        request.request_id
                                                    ),
                                                    request.workspace_identity.clone(),
                                                    &project_root,
                                                )
                                                .await?;
                                            (
                                                crate::runtime_server_admission::WorkspaceGenerationCommitReceipt::from_recovery(
                                                    &restored,
                                                )?,
                                                true,
                                            )
                                        }
                                    };
                                    crate::runtime_server_admission::WorkspaceGenerationReadinessReceipt::new(
                                        request.workspace_identity.clone(),
                                        commit,
                                        reconciled,
                                    )
                                }
                                crate::runtime_server_workspace::PublishedWorkspaceGenerationState::Missing
                                | crate::runtime_server_workspace::PublishedWorkspaceGenerationState::RecoveryRequired { .. } => {
                                    let candidate = crate::runtime_server_admission::discover_workspace_generation_candidate(
                                        &project_root,
                                    )
                                    .await?;
                                    let admitted = admission
                                        .admit(
                                            request.workspace_identity.clone(),
                                            project_root.clone(),
                                            candidate,
                                        )
                                        .await?;
                                    let terminal = if admitted.state
                                        == crate::runtime_server_admission::WorkspaceGenerationAdmissionState::Building
                                    {
                                        admission
                                            .wait_terminal(
                                                &request.workspace_identity,
                                                &project_root,
                                            )
                                            .await
                                    } else {
                                        Ok(admitted)
                                    }?;
                                    let commit = terminal.commit.ok_or_else(|| {
                                        terminal.error.unwrap_or_else(|| {
                                            "workspace generation reconciliation completed without a commit"
                                                .to_owned()
                                        })
                                    })?;
                                    crate::runtime_server_admission::WorkspaceGenerationReadinessReceipt::new(
                                        request.workspace_identity.clone(),
                                        commit,
                                        true,
                                    )
                                }
                            }
                            }
                            .await;
                            match repair {
                                Ok(receipt) => {
                                    WorkspaceDbIpcResult::RuntimeGenerationReadiness { receipt }
                                }
                                Err(message) => WorkspaceDbIpcResult::Failed {
                                    code: "runtime-server-generation-locator-repair-failed"
                                        .to_owned(),
                                    message,
                                },
                            }
                        }
                        None => WorkspaceDbIpcResult::Failed {
                            code: "runtime-server-generation-admission-unavailable".to_owned(),
                            message: "Runtime Server has no canonical generation admission owner"
                                .to_owned(),
                        },
                    }
                }
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
                    agent_session::evaluate(agent_session_registry_owner, project_root, operation)
                        .await
                }
                WorkspaceDbIpcOperation::PublishCodexMultiAgentControlPlane { projection } => {
                    if projection.workspace_server.workspace_identity != request.workspace_identity
                    {
                        WorkspaceDbIpcResult::Failed {
                            code: "runtime-server-codex-control-plane-workspace-mismatch"
                                .to_owned(),
                            message: format!(
                                "Codex control-plane workspace identity must match the Runtime Server request: request={} projection={}",
                                request.workspace_identity,
                                projection.workspace_server.workspace_identity
                            ),
                        }
                    } else {
                        match codex_multi_agent_control_plane_owner
                            .publish(projection)
                            .await
                        {
                            Ok(receipt) => {
                                WorkspaceDbIpcResult::CodexMultiAgentControlPlanePublication {
                                    receipt,
                                }
                            }
                            Err(message) => WorkspaceDbIpcResult::Failed {
                                code: "runtime-server-codex-control-plane-publication-failed"
                                    .to_owned(),
                                message,
                            },
                        }
                    }
                }
                WorkspaceDbIpcOperation::ReadCodexMultiAgentControlPlane { root_session_id } => {
                    let projection = codex_multi_agent_control_plane_owner
                        .read(&request.workspace_identity, &root_session_id)
                        .await
                        .map(|projection| projection.as_ref().clone());
                    WorkspaceDbIpcResult::CodexMultiAgentControlPlane { projection }
                }
                WorkspaceDbIpcOperation::EvaluateHook {
                    project_root,
                    arguments,
                    input,
                } => match hook_evaluation_builder {
                    Some(builder) => match builder(
                        request.workspace_identity.clone(),
                        std::path::PathBuf::from(&project_root),
                        arguments,
                        input,
                    )
                    .await
                    {
                        Ok(output) => WorkspaceDbIpcResult::HookEvaluation {
                            workspace_identity: request.workspace_identity.clone(),
                            project_root,
                            output,
                        },
                        Err(message) => WorkspaceDbIpcResult::Failed {
                            code: "runtime-server-hook-evaluation-failed".to_owned(),
                            message,
                        },
                    },
                    None => WorkspaceDbIpcResult::Failed {
                        code: "runtime-server-hook-evaluation-unavailable".to_owned(),
                        message: "Runtime Server has no resident hook evaluator".to_owned(),
                    },
                },
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
                    let lookup = async {
                        let project_root = Path::new(&lookup_request.project_root);
                        let lease = memory_registry
                            .lease(&request.workspace_identity, project_root)
                            .map_err(|error| {
                                format!(
                                    "active workspace generation lease is required before source-index read: workspaceIdentity={} error={error}",
                                    request.workspace_identity
                                )
                            })?;
                        lease.read_source_index(
                            &lookup_request.query,
                            lookup_request.language_id.as_ref(),
                            lookup_request.limit,
                        )
                    }
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
        write_frame(
            &mut *stream,
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

async fn ensure_runtime_owner_freshness(
    memory_registry: &crate::runtime_server_workspace::RuntimeServerWorkspaceRegistry,
    request_id: &str,
    workspace_identity: &str,
    project_root: &Path,
    language_id: &str,
    owner_path: &str,
    projection_builder: Option<&crate::runtime_server_workspace::WorkspaceOwnerProjectionBuilder>,
) -> Result<crate::runtime_server_workspace::WorkspaceRuntimeOwnerFreshnessReceipt, String> {
    memory_registry
        .ensure_runtime_owner_freshness(
            request_id,
            workspace_identity,
            project_root,
            language_id,
            owner_path,
            projection_builder,
        )
        .await
}

#[cfg(test)]
#[cfg(test)]
#[tokio::test]
async fn bounded_runtime_generation_wait_rejects_expired_budget() {
    let error = bounded_runtime_generation_wait(
        std::time::Duration::ZERO,
        std::future::pending::<Result<(), String>>(),
        "runtime generation foreground wait expired".to_owned(),
    )
    .await
    .expect_err("an expired foreground budget must fail closed");

    assert_eq!(error, "runtime generation foreground wait expired");
}

#[cfg(test)]
async fn bounded_runtime_generation_wait<T>(
    timeout: std::time::Duration,
    wait: impl std::future::Future<Output = Result<T, String>>,
    timeout_error: String,
) -> Result<T, String> {
    match tokio::time::timeout(timeout, wait).await {
        Ok(result) => result,
        Err(_) => Err(timeout_error),
    }
}

use crate::workspace_db_ipc::transport::{read_frame, read_optional_frame, write_frame};
