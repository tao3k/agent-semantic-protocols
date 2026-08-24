//! Runtime Server workspace data-plane router.
//!
//! The parent owns framing, request admission, drain, and terminal response.
//! Child modules own disjoint operation families: exact projection, generation,
//! resident owner/read, graph evaluation, cache control, agent sessions, and
//! Codex control-plane projection. Children return typed results and cannot
//! create listeners, Runtime generations, or alternative lifecycle owners.

use std::path::Path;

use tokio::io::BufStream;
use tokio::net::UnixStream;

use crate::workspace_db_ipc::{
    WORKSPACE_DB_OWNER_REQUEST_SCHEMA_ID, WORKSPACE_DB_OWNER_SCHEMA_VERSION,
    WorkspaceDbIpcOperation, WorkspaceDbIpcRequest, WorkspaceDbIpcResult,
};
use crate::{ProviderSearchWorkspaceSession, WorkspaceDbRegistry};

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
#[path = "workspace_db_ipc_server_dispatch.rs"]
mod dispatch;
#[path = "workspace_db_ipc_server_exact_projection.rs"]
mod exact_projection;
#[path = "workspace_db_ipc_server_generation.rs"]
pub(crate) mod generation;
#[cfg(test)]
pub(crate) use generation::require_or_submit_terminal_generation_for_read;
#[cfg(test)]
pub(crate) fn resident_read_query_targets_for_test(
    operation: &crate::workspace_db_ipc::WorkspaceDbIpcOperation,
    project_root: &std::path::Path,
) -> Result<Vec<std::path::PathBuf>, String> {
    generation::resident_read_query_targets(operation, project_root)
}

#[cfg(test)]
pub(crate) fn resident_read_query_provider_target_for_test(
    operation: &crate::workspace_db_ipc::WorkspaceDbIpcOperation,
) -> Option<crate::runtime_server_admission::WorkspaceGenerationProviderTarget> {
    generation::resident_read_query_provider_target(operation)
}
#[path = "workspace_db_ipc_server_graph_turbo.rs"]
mod graph_turbo;

#[path = "workspace_db_ipc_server_cache_control.rs"]
mod cache_control;
#[path = "workspace_db_ipc_server_codex_control_plane.rs"]
mod codex_control_plane;
pub(crate) struct RuntimeServerWorkspaceStreamContext<'a> {
    pub(crate) endpoint: &'a crate::runtime_server_control::RuntimeServerEndpoint,
    pub(crate) registry: &'a WorkspaceDbRegistry,
    pub(crate) memory_registry:
        &'a std::sync::Arc<crate::runtime_server_workspace::RuntimeServerWorkspaceRegistry>,
    pub(crate) generation_admission:
        Option<&'a std::sync::Arc<crate::runtime_server_admission::WorkspaceGenerationAdmission>>,
    pub(crate) graph_turbo_evaluation_builder:
        Option<&'a crate::runtime_server::GraphTurboEvaluationBuilder>,
    pub(crate) runtime_search_service:
        Option<&'a crate::runtime_search_service::RuntimeSearchServiceHandle>,
    pub(crate) agent_session_registry_owner:
        Option<&'a std::sync::Arc<crate::AgentSessionRegistry>>,
    pub(crate) session_control_plane_runtime_registry:
        &'a std::sync::Arc<crate::SessionControlPlaneRuntimeRegistry>,
    pub(crate) agent_session_status:
        Option<&'a crate::runtime_server_agent_session_status::AgentSessionStatusHandle>,
    pub(crate) codex_multi_agent_control_plane_owner: &'a std::sync::Arc<
        crate::codex_multi_agent_control_plane_owner::CodexMultiAgentControlPlaneOwner,
    >,
    pub(crate) telemetry_sender:
        Option<&'a crate::runtime_telemetry_bus::RuntimeTelemetryBusSender>,
    pub(crate) drain: tokio::sync::watch::Receiver<bool>,
}

/// Serve one workspace-scoped data-plane stream through the single Runtime Server.
pub(crate) async fn serve_runtime_server_workspace_stream(
    stream: UnixStream,
    context: RuntimeServerWorkspaceStreamContext<'_>,
) -> Result<(), String> {
    let RuntimeServerWorkspaceStreamContext {
        endpoint,
        registry,
        memory_registry,
        generation_admission,
        graph_turbo_evaluation_builder,
        runtime_search_service,
        agent_session_registry_owner,
        session_control_plane_runtime_registry,
        agent_session_status,
        codex_multi_agent_control_plane_owner,
        telemetry_sender,
        mut drain,
    } = context;
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
            && let Err(message) = generation::require_or_submit_terminal_generation_for_operation(
                memory_registry,
                generation_admission,
                &request.workspace_identity,
                project_root,
                &request.operation,
            )
            .await
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
                } => {
                    let evidence_started = tokio::time::Instant::now();
                    let counters_before = memory_registry.data_plane_counters();
                    match memory_registry
                        .read_projection_selector(
                            &request.workspace_identity,
                            Path::new(&project_root),
                            projection_kind,
                            &structural_selector,
                        )
                        .await
                    {
                        Ok(read) => {
                            let counters = memory_registry
                                .data_plane_counters()
                                .delta_since(&counters_before);
                            let (generation_digest, root_digest, read_state) =
                                resident_read::selector_identity(&read);
                            let evidence = resident_read::evidence(
                                request.request_id.as_str().to_owned(),
                                request.workspace_identity.clone(),
                                generation_digest,
                                root_digest,
                                read_state,
                                evidence_started,
                                resident_read::counters(counters),
                            );
                            if let Err(error) = resident_read::record_terminal(
                                telemetry_sender,
                                &evidence,
                                "runtime-resident-runtime-selector",
                            ) {
                                return Err(format!(
                                    "runtime-resident-read-telemetry-failed: {error}"
                                ));
                            }
                            WorkspaceDbIpcResult::RuntimeSelector { read, evidence }
                        }
                        Err(message) => WorkspaceDbIpcResult::Failed {
                            code: "runtime-server-selector-read-failed".to_owned(),
                            message,
                        },
                    }
                }
                WorkspaceDbIpcOperation::ProviderSearch {
                    operation_id,
                    project_root,
                    language_id,
                    args,
                } => {
                    let provider_search_started = tokio::time::Instant::now();
                    let query = args
                        .iter()
                        .enumerate()
                        .filter_map(|(index, arg)| {
                            (arg == "--query").then(|| args.get(index + 1)).flatten()
                        })
                        .cloned()
                        .collect::<Vec<_>>()
                        .join(" ");
                    match memory_registry
                        .read_projection_source_index(
                            &request.workspace_identity,
                            Path::new(&project_root),
                            &query,
                            Some(&language_id),
                            200,
                        )
                        .await
                    {
                        Ok(lookup) => {
                            let owner_paths = lookup
                                .hits
                                .iter()
                                .map(|hit| hit.owner_path.clone())
                                .collect::<Vec<_>>();
                            match memory_registry
                                .read_projection_parser_owned_callable_selector_pairs(
                                    &request.workspace_identity,
                                    Path::new(&project_root),
                                    &owner_paths,
                                ) {
                                Ok(parser_owned_selector_pairs) => {
                                    let resident_read_elapsed_micros = provider_search_started
                                        .elapsed()
                                        .as_micros()
                                        .min(u128::from(u64::MAX))
                                        as u64;
                                    match agent_semantic_search::build_runtime_provider_search_receipt(
                                        operation_id.clone(),
                                        language_id.clone(),
                                        vec![agent_semantic_search::RuntimeSearchSource::once(
                                            "resident",
                                            lookup,
                                        )],
                                        resident_read_elapsed_micros,
                                        parser_owned_selector_pairs,
                                    )
                                    .await
                                    {
                                        Ok(receipt) => {
                                            WorkspaceDbIpcResult::ProviderSearch { receipt }
                                        }
                                        Err(message) => {
                                            tracing::error!(
                                                event = "runtime_provider_search_terminal",
                                                operation_id,
                                                workspace_identity = %request.workspace_identity,
                                                language_id = %language_id,
                                                reason_kind = "runtime-server-provider-search-failed",
                                                elapsed_micros = provider_search_started.elapsed().as_micros().min(u128::from(u64::MAX)) as u64,
                                                error = %message
                                            );
                                            WorkspaceDbIpcResult::Failed {
                                                code: "runtime-server-provider-search-failed"
                                                    .to_owned(),
                                                message,
                                            }
                                        }
                                    }
                                }
                                Err(message) => WorkspaceDbIpcResult::Failed {
                                    code: "runtime-server-provider-search-selector-read-failed"
                                        .to_owned(),
                                    message,
                                },
                            }
                        }
                        Err(message) => {
                            tracing::error!(
                                event = "runtime_provider_search_terminal",
                                operation_id,
                                workspace_identity = %request.workspace_identity,
                                language_id = %language_id,
                                reason_kind = "runtime-server-provider-search-index-read-failed",
                                elapsed_micros = provider_search_started.elapsed().as_micros().min(u128::from(u64::MAX)) as u64,
                                error = %message
                            );
                            WorkspaceDbIpcResult::Failed {
                                code: "runtime-server-provider-search-index-read-failed".to_owned(),
                                message,
                            }
                        }
                    }
                }
                WorkspaceDbIpcOperation::ReadRuntimeExactProjection {
                    project_root,
                    language_id: _,
                    projection_kind,
                    structural_selector,
                } => {
                    let evidence_started = tokio::time::Instant::now();
                    let counters_before = memory_registry.data_plane_counters();
                    match memory_registry
                        .read_projection_selector(
                            &request.workspace_identity,
                            Path::new(&project_root),
                            projection_kind,
                            &structural_selector,
                        )
                        .await
                    {
                        Ok(read) => {
                            let counters = memory_registry
                                .data_plane_counters()
                                .delta_since(&counters_before);
                            let (generation_digest, root_digest, read_state) =
                                resident_read::selector_identity(&read);
                            let evidence = resident_read::evidence(
                                request.request_id.as_str().to_owned(),
                                request.workspace_identity.clone(),
                                generation_digest,
                                root_digest,
                                read_state,
                                evidence_started,
                                resident_read::counters(counters),
                            );
                            if let Err(error) = resident_read::record_terminal(
                                telemetry_sender,
                                &evidence,
                                "runtime-resident-exact-projection",
                            ) {
                                return Err(format!(
                                    "runtime-resident-read-telemetry-failed: {error}"
                                ));
                            }
                            WorkspaceDbIpcResult::RuntimeSelector { read, evidence }
                        }
                        Err(message) => WorkspaceDbIpcResult::Failed {
                            code: "runtime-server-exact-projection-read-failed".to_owned(),
                            message,
                        },
                    }
                }
                WorkspaceDbIpcOperation::ReadRuntimeOwner {
                    project_root,
                    owner_path,
                } => {
                    match resident_owner::read_runtime_owner(
                        memory_registry,
                        &request.workspace_identity,
                        request.request_id.as_str().to_owned(),
                        project_root,
                        owner_path,
                        telemetry_sender,
                    )
                    .await
                    {
                        Ok(result) => result,
                        Err(error) => return Err(error),
                    }
                }
                WorkspaceDbIpcOperation::ReadRuntimeMerkleOwner {
                    request: merkle_request,
                } => {
                    resident_read::read_merkle_owner(
                        memory_registry,
                        &request.workspace_identity,
                        request.request_id.as_str(),
                        merkle_request,
                        telemetry_sender,
                    )
                    .await
                }
                WorkspaceDbIpcOperation::ProjectProviderOwner {
                    project_root,
                    language_id,
                    owner_path,
                } => {
                    exact_projection::provider_owner(
                        runtime_search_service,
                        memory_registry.as_ref(),
                        generation_admission,
                        request.request_id.as_str().to_owned(),
                        &request.workspace_identity,
                        project_root,
                        language_id,
                        owner_path,
                    )
                    .await
                }
                WorkspaceDbIpcOperation::ResolveProviderRuntime {
                    project_root,
                    language_id,
                } => match runtime_search_service {
                    Some(service) => match service
                        .provider_runtime(
                            Path::new(&project_root).to_path_buf(),
                            language_id.to_string(),
                        )
                        .await
                    {
                        Ok(runtime) => WorkspaceDbIpcResult::ProviderRuntime { runtime },
                        Err(message) => WorkspaceDbIpcResult::Failed {
                            code: "runtime-server-provider-runtime-resolution-failed".to_owned(),
                            message,
                        },
                    },
                    None => WorkspaceDbIpcResult::Failed {
                        code: "runtime-server-provider-runtime-resolution-unavailable".to_owned(),
                        message: "Runtime Server provider runtime service is unavailable"
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
                WorkspaceDbIpcOperation::RebindRuntimeSelectorOverlay {
                    project_root,
                    rebind,
                } => {
                    match memory_registry
                        .rebind_selector_overlay(
                            &request.workspace_identity,
                            Path::new(&project_root),
                            rebind,
                        )
                        .await
                    {
                        Ok(receipt) => WorkspaceDbIpcResult::RuntimeSelectorOverlay { receipt },
                        Err(message) => WorkspaceDbIpcResult::Failed {
                            code: "runtime-server-selector-rebind-failed".to_owned(),
                            message,
                        },
                    }
                }
                WorkspaceDbIpcOperation::RequireRuntimeGeneration { project_root } => {
                    generation::require_lifecycle_generation(
                        memory_registry,
                        generation_admission,
                        &request.workspace_identity,
                        project_root,
                    )
                    .await
                }
                WorkspaceDbIpcOperation::RestoreRuntimeGenerationFromPointer { project_root } => {
                    generation::restore_lifecycle_generation_from_pointer(
                        memory_registry,
                        &request.workspace_identity,
                        project_root,
                    )
                    .await
                }
                WorkspaceDbIpcOperation::AdmitRuntimeGenerationForRead {
                    project_root,
                    language_id,
                    provider_id,
                } => {
                    generation::admit_generation_for_read(
                        memory_registry,
                        generation_admission,
                        &request.workspace_identity,
                        project_root,
                        language_id,
                        provider_id,
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
                }
                | WorkspaceDbIpcOperation::ReadRuntimeSourceIndex {
                    request: lookup_request,
                }
                | WorkspaceDbIpcOperation::ReadTreeSitterInventory {
                    request: lookup_request,
                } => {
                    let evidence_started = tokio::time::Instant::now();
                    let counters_before = memory_registry.data_plane_counters();
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
                        Ok(lookup) => {
                            let counters = memory_registry
                                .data_plane_counters()
                                .delta_since(&counters_before);
                            let authority = memory_registry
                                .projection_search_generation_authority(
                                    &request.workspace_identity,
                                    Path::new(&lookup_request.project_root),
                                )
                                .await;
                            let (generation_digest, root_digest) =
                                resident_read::source_index_identity(&lookup, authority);
                            let evidence = resident_read::evidence(
                                request.request_id.as_str().to_owned(),
                                request.workspace_identity.clone(),
                                generation_digest,
                                root_digest,
                                crate::workspace_db_ipc::RuntimeResidentReadState::SourceIndex,
                                evidence_started,
                                resident_read::counters(counters),
                            );
                            if let Err(error) = resident_read::record_terminal(
                                telemetry_sender,
                                &evidence,
                                "runtime-resident-source-index",
                            ) {
                                return Err(format!(
                                    "runtime-resident-read-telemetry-failed: {error}"
                                ));
                            }
                            WorkspaceDbIpcResult::SourceIndex { lookup, evidence }
                        }
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
                            request.request_id.as_str().to_owned(),
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
                            request.request_id.as_str().to_owned(),
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
                            request.request_id.as_str().to_owned(),
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
                    dispatch::dispatch_workspace_db_session_operation(
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
        resident_read::write_response(
            &mut stream,
            endpoint,
            request.workspace_identity,
            request.request_id.as_str().to_owned(),
            result,
        )
        .await?;
    }
}

use crate::workspace_db_ipc::read_optional_frame;

#[path = "workspace_db_ipc_server_resident_owner.rs"]
mod resident_owner;
#[path = "workspace_db_ipc_server_resident_read.rs"]
mod resident_read;
