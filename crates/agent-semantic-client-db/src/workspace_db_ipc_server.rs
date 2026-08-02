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
        | WorkspaceDbIpcOperation::EvaluateHook { .. } => Err(
            "canonical generation admission is only accepted by the Runtime Server data plane"
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
    let state_root = crate::AgentSessionRegistry::state_root_for_project(&project_root)?;
    let requested_db_path = crate::AgentSessionRegistry::db_path_for_state_root(state_root);
    if requested_db_path != registry.db_path() {
        return Err(format!(
            "agent-session registry owner state-root mismatch: owner={} requested={}",
            registry.db_path().display(),
            requested_db_path.display()
        ));
    }
    tokio::task::spawn_blocking(move || {
        use crate::workspace_db_ipc::{
            AgentSessionRegistryIpcOperation as Operation,
            AgentSessionRegistryIpcResult as IpcResult,
        };

        match operation {
            Operation::Register { request } => {
                let model_source = request
                    .model_observation
                    .as_ref()
                    .map(|observation| match observation.source.as_str() {
                        "codex.subagent-start" => Ok(
                            crate::agent_session_registry::AgentSessionModelObservationSource::CodexSubagentStart,
                        ),
                        "codex.rollout" => Ok(
                            crate::agent_session_registry::AgentSessionModelObservationSource::CodexRollout,
                        ),
                        source => Err(format!(
                            "unsupported agent-session model observation source: {source}"
                        )),
                    })
                    .transpose()?;
                let model_observation = match (request.model_observation.as_ref(), model_source) {
                    (Some(observation), Some(source)) => Some(
                        crate::agent_session_registry::AgentSessionModelObservationRef {
                            model: observation.model.as_str(),
                            source,
                            observed_at: observation.observed_at,
                            evidence_ref: observation.evidence_ref.as_deref(),
                        },
                    ),
                    (None, None) => None,
                    _ => {
                        return Err(
                            "agent-session model observation/source presence mismatch".to_owned()
                        );
                    }
                };
                let session = registry.register_session(crate::AgentSessionRegisterRequest {
                    project_id: request.project_id.into(),
                    root_session_id: request.root_session_id.into(),
                    session_id: request.session_id.into(),
                    message_target_id: request.message_target_id.map(Into::into),
                    parent_session_id: request.parent_session_id.map(Into::into),
                    name: request.name.into(),
                    role: request.role.into(),
                    model_observation,
                    status: request.status.into(),
                    expires_at: request.expires_at,
                    metadata_json: request.metadata_json.into(),
                    now: request.now,
                })?;
                Ok(IpcResult::Registered { session })
            }
            Operation::Query {
                project_id,
                root_session_id,
                name,
            } => Ok(IpcResult::Sessions {
                sessions: registry.query_sessions(
                    project_id,
                    root_session_id.map(Into::into),
                    name.map(Into::into),
                )?,
            }),
            Operation::SessionById {
                project_id,
                session_id,
            } => Ok(IpcResult::Session {
                session: registry.session_by_id(project_id, session_id)?,
            }),
            Operation::SessionByName {
                project_id,
                root_session_id,
                name,
            } => Ok(IpcResult::Session {
                session: registry.session_by_name(project_id, root_session_id, name)?,
            }),
            Operation::UpdateStatus {
                project_id,
                session_id,
                status,
                now,
            } => Ok(IpcResult::Changed {
                changed: registry.update_session_status(project_id, session_id, status, now)?,
            }),
            Operation::SessionIsRetired {
                project_id,
                session_id,
            } => Ok(IpcResult::Changed {
                changed: registry.session_is_retired(project_id, session_id)?,
            }),
            Operation::RefreshExpired => {
                registry.refresh_expired_sessions()?;
                Ok(IpcResult::Refreshed)
            }
            Operation::SessionByIdAnyProject { session_id } => Ok(IpcResult::Session {
                session: registry.session_by_id_any_project(session_id)?,
            }),
            Operation::ProjectIdForRootSessionId { root_session_id } => {
                Ok(IpcResult::ProjectId {
                    project_id: registry.project_id_for_root_session_id(root_session_id)?,
                })
            }
        }
    })
    .await
    .map_err(|error| format!("agent-session registry owner task failed: {error}"))?
}
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
    agent_session_registry_owner: Option<&std::sync::Arc<crate::AgentSessionRegistry>>,
    mut drain: tokio::sync::watch::Receiver<bool>,
) -> Result<(), String> {
    static AGENT_SESSION_REGISTRY_OWNER_LANE: std::sync::LazyLock<tokio::sync::Mutex<()>> =
        std::sync::LazyLock::new(|| tokio::sync::Mutex::new(()));
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
                } => match generation_admission {
                    Some(admission) => match admission
                        .admit_changed_paths(
                            mutation_id,
                            request.workspace_identity.clone(),
                            std::path::PathBuf::from(project_root),
                            changed_paths
                                .into_iter()
                                .map(std::path::PathBuf::from)
                                .collect(),
                        )
                        .await
                    {
                        Ok(receipt) => {
                            WorkspaceDbIpcResult::RuntimeGenerationMutationAdmission { receipt }
                        }
                        Err(message) => WorkspaceDbIpcResult::Failed {
                            code: "runtime-server-generation-admission-failed".to_owned(),
                            message,
                        },
                    },
                    None => WorkspaceDbIpcResult::Failed {
                        code: "runtime-server-generation-admission-unavailable".to_owned(),
                        message: "Runtime Server has no canonical generation builder".to_owned(),
                    },
                },
                WorkspaceDbIpcOperation::EnsureRuntimeGeneration { project_root } => {
                    match generation_admission {
                        Some(admission) => {
                            let project_root = std::path::PathBuf::from(project_root);
                            let ensured = match admission
                        .ensure(&request.workspace_identity, &project_root)
                        .await
                    {
                        Ok(receipt)
                            if receipt.state
                                == crate::runtime_server_admission::WorkspaceGenerationAdmissionState::Building =>
                        {
                            bounded_runtime_generation_wait(
                                std::time::Duration::from_secs(30),
                                admission.wait_terminal(
                                    &request.workspace_identity,
                                    &project_root,
                                ),
                                format!(
                                    "workspace generation admission timed out: workspaceIdentity={} projectRoot={}",
                                    request.workspace_identity,
                                    project_root.display()
                                ),
                            )
                            .await
                        }
                        other => other,
                    };
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
                                if !memory_registry
                                    .published_generation_is_ready(
                                        &request.workspace_identity,
                                        &project_root,
                                    )
                                    .await?
                                {
                                    return Err(format!(
                                        "resident workspace generation is unavailable: workspaceIdentity={} projectRoot={}",
                                        request.workspace_identity,
                                        project_root.display()
                                    ));
                                }
                                admission
                                    .publish_resident_generation_locator(
                                        &request.workspace_identity,
                                        &project_root,
                                    )
                                    .await
                            }
                            .await;
                            match repair {
                                Ok(receipt) => {
                                    WorkspaceDbIpcResult::RuntimeGenerationAdmission { receipt }
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
                WorkspaceDbIpcOperation::AgentSessionRegistry {
                    project_root,
                    operation,
                } => {
                    let _owner_guard = AGENT_SESSION_REGISTRY_OWNER_LANE.lock().await;
                    match agent_session_registry_owner {
                        Some(owner) => match run_agent_session_registry_operation(
                            std::sync::Arc::clone(owner),
                            std::path::PathBuf::from(project_root),
                            operation,
                        )
                        .await
                        {
                            Ok(result) => WorkspaceDbIpcResult::AgentSessionRegistry { result },
                            Err(message) => WorkspaceDbIpcResult::Failed {
                                code: "runtime-server-agent-session-registry-failed".to_owned(),
                                message,
                            },
                        },
                        None => WorkspaceDbIpcResult::Failed {
                            code: "runtime-server-agent-session-registry-unavailable".to_owned(),
                            message: "Runtime Server has no resident agent-session registry owner"
                                .to_owned(),
                        },
                    }
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

#[cfg(test)]
#[path = "../tests/unit/workspace_db_ipc_server_bounded_wait.rs"]
mod bounded_runtime_generation_wait_tests;

use crate::workspace_db_ipc::transport::{read_frame, read_optional_frame, write_frame};
