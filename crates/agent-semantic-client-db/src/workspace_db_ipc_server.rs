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
        WorkspaceDbIpcOperation::ReadSourceIndex { request } => {
            let session = admitted_or_bootstrap_workspace(
                registry,
                workspace_identity,
                Path::new(&request.project_root),
            )
            .await;
            match session {
                Ok(session) => session
                    .read_source_index(
                        &request.indexed_project_root,
                        &request.source_snapshot,
                        &request.query,
                        request.language_id.as_ref(),
                        request.limit,
                    )
                    .await
                    .map(|lookup| WorkspaceDbIpcResult::SourceIndex { lookup }),
                Err(error) => Err(error),
            }
        }
        WorkspaceDbIpcOperation::CommitSourceIndexGeneration { .. } => Err(
            "canonical source-index generation publication is only accepted by the Runtime Server data plane"
                .to_owned(),
        ),
        WorkspaceDbIpcOperation::EnsureRuntimeGeneration { .. } => Err(
            "canonical generation restore is only accepted by the Runtime Server data plane"
                .to_owned(),
        ),
        WorkspaceDbIpcOperation::AdmitRuntimeGeneration { .. } => Err(
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
        WorkspaceDbIpcOperation::ReadProviderOwnerProjections { scope, owner_path } => {
            let session = registry.acquire(&scope.project_root, &scope).await;
            match session {
                Ok(session) => session
                    .read_provider_owner_projections(&scope, &owner_path)
                    .await
                    .map(
                        |projections| WorkspaceDbIpcResult::ProviderOwnerProjections {
                            projections,
                        },
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
        } else {
            match request.operation {
                WorkspaceDbIpcOperation::EnsureRuntimeGeneration { project_root } => {
                    let restoration = async {
                        let session = admitted_or_bootstrap_workspace(
                            registry,
                            &request.workspace_identity,
                            Path::new(&project_root),
                        )
                        .await?;
                        let materialization = session
                            .load_active_workspace_generation_materialization()
                            .await?;
                        if let Some(materialization) = materialization {
                            materialization.validate_persisted(&request.workspace_identity)?;
                            return memory_registry
                                .ensure_canonical_generation(
                                    request.request_id.clone(),
                                    &request.workspace_identity,
                                    materialization,
                                )
                                .await
                                .map(|receipt| WorkspaceDbIpcResult::RuntimeGeneration {
                                    receipt,
                                });
                        }
                        let admission = generation_admission.ok_or_else(|| {
                            format!(
                                "active workspace generation materialization is unavailable: workspaceIdentity={}",
                                request.workspace_identity
                            )
                        })?;
                        let receipt = admission
                            .status(&request.workspace_identity)
                            .await
                            .ok_or_else(|| {
                                format!(
                                    "active workspace generation is not admitted: workspaceIdentity={}; hook or supervisor admission is required",
                                    request.workspace_identity
                                )
                            })?;
                        Ok(WorkspaceDbIpcResult::RuntimeGenerationAdmission { receipt })
                    }
                    .await;
                    match restoration {
                        Ok(result) => result,
                        Err(message) => WorkspaceDbIpcResult::Failed {
                            code: "runtime-server-canonical-generation-restore-failed".to_owned(),
                            message,
                        },
                    }
                }
                WorkspaceDbIpcOperation::AdmitRuntimeGeneration { project_root } => {
                    match generation_admission {
                        Some(admission) => match admission
                            .admit(
                                request.workspace_identity.clone(),
                                std::path::PathBuf::from(project_root),
                            )
                            .await
                        {
                            Ok(receipt) => {
                                WorkspaceDbIpcResult::RuntimeGenerationAdmission { receipt }
                            }
                            Err(message) => WorkspaceDbIpcResult::Failed {
                                code: "runtime-server-generation-admission-failed".to_owned(),
                                message,
                            },
                        },
                        None => WorkspaceDbIpcResult::Failed {
                            code: "runtime-server-generation-admission-unavailable".to_owned(),
                            message: "Runtime Server has no canonical generation builder"
                                .to_owned(),
                        },
                    }
                }
                WorkspaceDbIpcOperation::CommitSourceIndexGeneration {
                    request: refresh,
                    materialization,
                } => {
                    let publication = async {
                        materialization
                            .validate_refresh_request(&request.workspace_identity, &refresh)?;
                        let project_root = refresh.import.project_root.clone();
                        let session = admitted_or_bootstrap_workspace(
                            registry,
                            &request.workspace_identity,
                            Path::new(&project_root),
                        )
                        .await?;
                        let durable = session
                            .commit_source_index_generation(refresh, materialization)
                            .await?;
                        let materialization = session
                            .load_active_workspace_generation_materialization()
                            .await?
                            .ok_or_else(|| {
                                "durable source-index commit published no canonical materialization"
                                    .to_owned()
                            })?;
                        materialization.validate_persisted(&request.workspace_identity)?;
                        if !durable
                            .source_snapshot
                            .has_same_content_identity(&materialization.source_snapshot)
                        {
                            return Err(format!(
                                "Turso generation evidence differs from canonical materialization: durable={:?} materialized={:?}",
                                durable.source_snapshot, materialization.source_snapshot
                            ));
                        }
                        memory_registry
                            .ensure_canonical_generation(
                                request.request_id.clone(),
                                &request.workspace_identity,
                                materialization,
                            )
                            .await?;
                        Ok::<_, String>(durable)
                    }
                    .await;
                    match publication {
                        Ok(receipt) => WorkspaceDbIpcResult::SourceIndexGeneration { receipt },
                        Err(message) => WorkspaceDbIpcResult::Failed {
                            code: "runtime-server-canonical-generation-publish-failed".to_owned(),
                            message,
                        },
                    }
                }
                WorkspaceDbIpcOperation::ReadSourceIndex {
                    request: lookup_request,
                } => {
                    let lookup = async {
                        let lease = match memory_registry.lease(&request.workspace_identity) {
                            Ok(lease) => lease,
                            Err(_) => {
                                let session = admitted_or_bootstrap_workspace(
                                    registry,
                                    &request.workspace_identity,
                                    Path::new(&lookup_request.project_root),
                                )
                                .await?;
                                let materialization = session
                                    .load_active_workspace_generation_materialization()
                                    .await?
                                    .ok_or_else(|| {
                                        format!(
                                            "active workspace generation materialization is unavailable: workspaceIdentity={}",
                                            request.workspace_identity
                                        )
                                    })?;
                                memory_registry
                                    .ensure_canonical_generation(
                                        request.request_id.clone(),
                                        &request.workspace_identity,
                                        materialization,
                                    )
                                    .await?;
                                memory_registry.lease(&request.workspace_identity)?
                            }
                        };
                        lease.read_source_index(
                            &lookup_request.source_snapshot,
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
                WorkspaceDbIpcOperation::PublishRuntimeOwner { owner } => {
                    let publication = memory_registry
                        .publish_owner_overlay(
                            request.request_id.clone(),
                            request.workspace_identity.clone(),
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
                WorkspaceDbIpcOperation::TombstoneRuntimeOwner { owner_path } => {
                    let publication = memory_registry
                        .tombstone_owner_overlay(
                            request.request_id.clone(),
                            request.workspace_identity.clone(),
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
                    previous_owner_path,
                    owner,
                } => {
                    let publication = memory_registry
                        .relocate_owner_overlay(
                            request.request_id.clone(),
                            request.workspace_identity.clone(),
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
use crate::workspace_db_ipc::transport::{read_frame, read_optional_frame, write_frame};
