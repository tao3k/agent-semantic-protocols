//! Public ASP Client Protocol binding for the resident Runtime Server.

use std::collections::hash_map::Entry;
use std::sync::Arc;

use crate::RuntimeQueryGenerationState;
use crate::runtime_query_generation_key::RuntimeProjectWorkspaceKey;
use agent_semantic_client_protocol::AGENT_SESSION_REGISTER_METHOD;
use agent_semantic_client_protocol::AGENT_SESSION_REGISTER_RESPONSE_SCHEMA_ID;
use agent_semantic_client_protocol::AgentSessionPlatform;
use agent_semantic_client_protocol::AgentSessionRegisterReceipt;
use agent_semantic_client_protocol::AgentSessionRegisterRequest;
use agent_semantic_client_protocol::AgentSessionRegisterState;
use agent_semantic_client_protocol::AgentSessionRegistryOwner;
use agent_semantic_client_protocol::AgentSessionTransport;
use agent_semantic_client_protocol::AspClientExactQueryRequest;
use agent_semantic_client_protocol::AspClientSearchRequest;
use agent_semantic_client_protocol::AspClientWorkspaceSearchPlaybookRequest;
use agent_semantic_client_protocol::ClientProjectId;
use agent_semantic_client_protocol::ClientRequestId;
use agent_semantic_client_protocol::ClientSchemaId;
use agent_semantic_client_protocol::ClientSessionId;
use agent_semantic_client_protocol::ClientWorkspaceIdentity;
use agent_semantic_client_protocol::GRAPH_TIMELINE_METHOD;
use agent_semantic_client_protocol::LIVE_CORPUS_CACHE_STATE_METHOD;
use agent_semantic_client_protocol::LiveCorpusCacheStateReceipt;
use agent_semantic_client_protocol::LiveCorpusCacheStateRequest;
use agent_semantic_client_protocol::SCHEMA_BUNDLE_METHOD;
use agent_semantic_client_protocol::SchemaBundleRequest;
use agent_semantic_client_protocol::ServerClientRoute;
use agent_semantic_client_server::AspClientDispatchError;
use agent_semantic_client_server::AspClientDispatchFuture;
use agent_semantic_client_server::AspClientDispatchRequest;
use agent_semantic_client_server::AspClientDispatcher;
use agent_semantic_search_projection::ResidentGraphEvaluationRequestV1;

#[path = "runtime_asp_client_query_generation.rs"]
mod query_generation_support;

#[path = "runtime_asp_client_search.rs"]
mod search_route;

#[path = "runtime_asp_client_projection.rs"]
mod projection_routes;

#[path = "runtime_asp_client_telemetry.rs"]
mod telemetry;

use projection_routes::dispatch_exact_query;
use projection_routes::dispatch_source_index_lookup;
use query_generation_support::AspClientOperationError;
use query_generation_support::QueryNotReadyContext;
use query_generation_support::RUNTIME_CLIENT_DISPATCH_BUDGET;
use query_generation_support::classify_exact_query_failure;
use query_generation_support::dispatch_budget_for_method;
use query_generation_support::install_runtime_query_generation_terminal;
use query_generation_support::query_generation_not_ready_error;
use query_generation_support::request_runtime_query_generation_ready;
use query_generation_support::wait_for_runtime_query_generation;
use query_generation_support::wait_for_runtime_query_generation_change;
use search_route::dispatch_search_route;
use telemetry::elapsed_micros;
use telemetry::record_runtime_route_performance;

#[cfg(test)]
#[path = "../tests/unit/runtime_asp_client_recovery.rs"]
mod runtime_asp_client_recovery_tests;

#[path = "runtime_asp_client_service.rs"]
mod service;

pub use service::RuntimeAspClientDispatcher;
pub use service::build_frame_service;
pub use service::workspace_search_providers_from_provider_register;

impl AspClientDispatcher for RuntimeAspClientDispatcher {
    fn dispatch(&self, request: AspClientDispatchRequest) -> AspClientDispatchFuture {
        let key = (
            request.project_id.clone(),
            request.workspace_id.clone(),
            request.session_id.clone(),
            request.request_id.clone(),
        );
        let (cancel, mut cancelled) = tokio::sync::watch::channel(false);
        let admitted = {
            let mut cancellations = self
                .cancellations
                .lock()
                .expect("ASP Client Protocol cancellation registry poisoned");
            match cancellations.entry(key.clone()) {
                Entry::Vacant(entry) => {
                    entry.insert(cancel);
                    true
                }
                Entry::Occupied(_) => false,
            }
        };
        if !admitted {
            return Box::pin(async {
                Err(AspClientDispatchError {
                    reason_kind: "client-request-id-conflict".to_owned(),
                    message: "requestId is already in flight for this client session".to_owned(),
                    details: None,
                })
            });
        }
        self.cancellation_admitted.notify_waiters();

        let schema_bundles = self.schema_bundles.clone();
        let agent_session_registry = Arc::clone(&self.agent_session_registry);
        let initialized_workspaces = Arc::clone(&self.initialized_workspaces);
        let runtime_search_service = self.runtime_search_service.clone();
        let generation_admission = Arc::clone(&self.generation_admission);
        let workspace_registry = Arc::clone(&self.workspace_registry);
        let installed_provider_targets = Arc::clone(&self.installed_provider_targets);
        let workspace_search_providers = Arc::clone(&self.workspace_search_providers);
        let workspace_store_root = self.workspace_store_root.clone();
        let query_generation_authority = self.query_generation_authority.clone();
        let mut query_generation = query_generation_authority.subscribe();
        let telemetry_sender = self.telemetry_sender.clone();
        let cancellations = Arc::clone(&self.cancellations);
        let dispatch_budget = dispatch_budget_for_method(&request.method);
        Box::pin(async move {
            let project_workspace_key = RuntimeProjectWorkspaceKey::new(
                request.project_id.clone(),
                request.workspace_id.clone(),
            );
            let operation = async {
                if request.method == agent_semantic_client_protocol::CANCELLATION_PROBE_METHOD {
                    return std::future::pending::<
                        Result<serde_json::Value, AspClientOperationError>,
                    >()
                    .await;
                }
                if request.method == AGENT_SESSION_REGISTER_METHOD {
                    let params: AgentSessionRegisterRequest =
                        serde_json::from_value(request.params).map_err(|error| {
                            format!("decode child registration request: {error}")
                        })?;
                    params.validate()?;
                    let initialized = initialized_workspaces
                        .lock()
                        .map_err(|_| "ASP client workspace-root registry poisoned".to_owned())?
                        .get(&(
                            request.project_id.as_str().to_owned(),
                            request.workspace_id.as_str().to_owned(),
                            request.session_id.as_str().to_owned(),
                        ))
                        .cloned()
                        .ok_or_else(|| {
                            "ASP client request requires an initialized workspace root".to_owned()
                        })?;
                    let state = agent_semantic_client_core::state_core::ResolvedState::resolve(
                        &initialized.project_root,
                    )?;
                    if state.repo.repo_id.as_str() != request.project_id.as_str()
                        || state.workspace.workspace_id.as_str() != request.workspace_id.as_str()
                    {
                        return Err(AspClientOperationError::Message(
                            "child registration project/workspace identity mismatch".to_owned(),
                        ));
                    }
                    let now = agent_semantic_client_db::agent_session_unix_timestamp()?;
                    let metadata = serde_json::json!({
                        "event": "child-registration-command",
                        "schemaId": AGENT_SESSION_REGISTER_RESPONSE_SCHEMA_ID,
                        "schemaVersion": 1,
                        "platform": "codex",
                        "nativeEnvironment": true,
                        "rootSessionId": params.root_session_id,
                        "parentThreadId": params.parent_thread_id,
                        "childThreadId": params.child_thread_id,
                        "agentType": params.agent_name,
                        "configuredRoute": params.route_key,
                        "messageTargetBinding": {
                            "source": "codex-child-registration-command",
                            "boundRootSessionId": params.root_session_id,
                            "childThreadId": params.child_thread_id,
                            "messageTargetId": params.agent_path
                        }
                    });
                    let registered = agent_session_registry
                        .register_session_from_runtime_owner(
                            agent_semantic_client_db::agent_session_registry::AgentSessionRegisterRequest {
                                project_id: request.project_id.as_str().into(),
                                root_session_id: params.root_session_id.as_str().into(),
                                session_id: params.child_thread_id.as_str().into(),
                                message_target_id: Some(params.agent_path.as_str().into()),
                                parent_session_id: Some(params.parent_thread_id.as_str().into()),
                                name: params.route_key.as_str().into(),
                                role: params.route_key.as_str().into(),
                                model_observation: None,
                                status: "active".into(),
                                expires_at: None,
                                metadata_json: metadata.to_string().into(),
                                now,
                            },
                        )
                        .await?;
                    let physical_generation =
                        registered.physical_generation.try_into().map_err(|_| {
                            "child registration physical generation is invalid".to_owned()
                        })?;
                    let receipt = AgentSessionRegisterReceipt {
                        schema_id: ClientSchemaId::new(AGENT_SESSION_REGISTER_RESPONSE_SCHEMA_ID)?,
                        schema_version: 1,
                        state: AgentSessionRegisterState::Registered,
                        platform: AgentSessionPlatform::Codex,
                        project_id: request.project_id.clone(),
                        root_session_id: params.root_session_id,
                        parent_thread_id: params.parent_thread_id,
                        child_thread_id: params.child_thread_id,
                        agent_name: params.agent_name,
                        agent_path: params.agent_path,
                        route_key: params.route_key,
                        physical_generation,
                        registry_owner:
                            AgentSessionRegistryOwner::RuntimeServerAgentSessionRegistry,
                        transport: AgentSessionTransport::GrpcClientFrame,
                    };
                    receipt.validate()?;
                    return serde_json::to_value(receipt)
                        .map_err(|error| error.to_string())
                        .map_err(AspClientOperationError::Message);
                }
                if request.method == SCHEMA_BUNDLE_METHOD {
                    let params: SchemaBundleRequest = serde_json::from_value(request.params)
                        .map_err(|error| format!("decode schema bundle request: {error}"))?;
                    params.validate()?;
                    let response = schema_bundles.project(&params);
                    response.validate()?;
                    return serde_json::to_value(response.as_ref())
                        .map_err(|error| error.to_string())
                        .map_err(AspClientOperationError::Message);
                }
                if request.method == LIVE_CORPUS_CACHE_STATE_METHOD {
                    let started = tokio::time::Instant::now();
                    let params: LiveCorpusCacheStateRequest =
                        serde_json::from_value(request.params).map_err(|error| {
                            format!("decode Live Corpus cache-state request: {error}")
                        })?;
                    params.validate()?;
                    let installed_provider_matches =
                        installed_provider_targets
                            .iter()
                            .any(|(language_id, provider_id)| {
                                language_id == &params.language_id
                                    && provider_id == &params.provider_id
                            });
                    if !installed_provider_matches {
                        return Err(AspClientOperationError::Message(format!(
                            "Live Corpus cache-state provider identity is not installed: languageId={} providerId={}",
                            params.language_id, params.provider_id
                        )));
                    }
                    let workspace_identity = request.workspace_id.as_str();
                    let resident_generation_evicted = match params.cache_state.as_str() {
                        "cold-build" => {
                            query_generation_authority
                                .require_workspace_absent(&project_workspace_key)?;
                            false
                        }
                        "cold-load" => {
                            let expected_generation_digest = params
                                .expected_generation_digest
                                .as_deref()
                                .expect("validated cold-load generation digest");
                            let expected_root_digest = params
                                .expected_root_digest
                                .as_deref()
                                .expect("validated cold-load root digest");
                            query_generation_authority.evict_ready_exact(
                                &project_workspace_key,
                                expected_generation_digest,
                                expected_root_digest,
                            )?;
                            let initialized = initialized_workspaces
                                .lock()
                                .map_err(|_| {
                                    "ASP client workspace-root registry poisoned".to_owned()
                                })?
                                .get(&(
                                    request.project_id.as_str().to_owned(),
                                    workspace_identity.to_owned(),
                                    request.session_id.as_str().to_owned(),
                                ))
                                .cloned()
                                .ok_or_else(|| {
                                    "Live Corpus cache state requires an initialized workspace root"
                                        .to_owned()
                                })?;
                            let pointer_path = agent_semantic_client_db::runtime_server_workspace::workspace_generation_pointer_path(
                                &workspace_store_root,
                                workspace_identity,
                                &initialized.project_root,
                            )?;
                            let reopened = query_generation_authority
                                .ensure_ready(
                                    &project_workspace_key,
                                    &pointer_path,
                                    &initialized.project_root,
                                    expected_generation_digest,
                                )
                                .await?;
                            if reopened.resident().source_root_digest() != expected_root_digest {
                                return Err(AspClientOperationError::Message(
                                    "Live Corpus cold-load reopened a different source root"
                                        .to_owned(),
                                ));
                            }
                            true
                        }
                        "warm-read" => {
                            query_generation_authority.verify_ready_exact(
                                &project_workspace_key,
                                params
                                    .expected_generation_digest
                                    .as_deref()
                                    .expect("validated warm-read generation digest"),
                                params
                                    .expected_root_digest
                                    .as_deref()
                                    .expect("validated warm-read root digest"),
                            )?;
                            false
                        }
                        "released" => {
                            query_generation_authority.evict_ready_exact(
                                &project_workspace_key,
                                params
                                    .expected_generation_digest
                                    .as_deref()
                                    .expect("validated release generation digest"),
                                params
                                    .expected_root_digest
                                    .as_deref()
                                    .expect("validated release root digest"),
                            )?;
                            true
                        }
                        _ => unreachable!("validated Live Corpus cache state"),
                    };
                    let receipt = LiveCorpusCacheStateReceipt {
                        schema_id: agent_semantic_client_protocol::LIVE_CORPUS_CACHE_STATE_RECEIPT_SCHEMA_ID
                            .to_owned(),
                        schema_version: "1".to_owned(),
                        operation_id: params.operation_id,
                        state: "ready".to_owned(),
                        cache_state: params.cache_state,
                        project_id: request.project_id.as_str().to_owned(),
                        workspace_id: workspace_identity.to_owned(),
                        generation_digest: params.expected_generation_digest,
                        root_digest: params.expected_root_digest,
                        resident_generation_evicted,
                        client_session_evicted: false,
                        source_workspace_mutation_count: 0,
                        filesystem_delete_count: 0,
                        elapsed_micros: elapsed_micros(started),
                    };
                    receipt.validate()?;
                    return serde_json::to_value(receipt)
                        .map_err(|error| error.to_string())
                        .map_err(AspClientOperationError::Message);
                }
                if request.method == GRAPH_TIMELINE_METHOD {
                    let params: agent_semantic_client_protocol::AspClientGraphsTimelineRequest =
                        serde_json::from_value(request.params).map_err(|error| {
                            AspClientOperationError::Message(format!(
                                "decode graph timeline request: {error}"
                            ))
                        })?;
                    params.validate_schema_identity()?;
                    let initialized = initialized_workspaces
                        .lock()
                        .map_err(|_| "ASP client workspace-root registry poisoned".to_owned())?
                        .get(&(
                            request.project_id.as_str().to_owned(),
                            request.workspace_id.as_str().to_owned(),
                            request.session_id.as_str().to_owned(),
                        ))
                        .cloned()
                        .ok_or_else(|| {
                            "ASP client request requires an initialized workspace root".to_owned()
                        })?;
                    let cancellation =
                        agent_semantic_client_db::runtime_generation_cancellation::GenerationCancellation::new();
                    let timeline_payload = serde_json::json!({
                        "schemaId": params.schema_id,
                        "schemaVersion": params.schema_version,
                        "eventPacket": params.event_packet,
                        "arguments": params.arguments,
                    });
                    return runtime_search_service
                        .graphs_timeline(
                            initialized.project_root,
                            request.request_id.as_str().to_owned(),
                            timeline_payload,
                            cancellation,
                        )
                        .await
                        .map_err(AspClientOperationError::Message);
                }
                if request.method
                    == agent_semantic_client_protocol::WORKSPACE_GENERATION_ENSURE_READY_METHOD
                {
                    #[derive(serde::Deserialize)]
                    #[serde(rename_all = "camelCase", deny_unknown_fields)]
                    struct EnsureReadyParams {
                        language_id: Option<String>,
                    }
                    let params: EnsureReadyParams = serde_json::from_value(request.params)
                        .map_err(|error| {
                            AspClientOperationError::Message(format!(
                                "invalid workspace generation ensure-ready parameters: {error}"
                            ))
                        })?;
                    let provider_target = match params.language_id {
                        None => None,
                        Some(language_id) => {
                            let provider_id = installed_provider_targets
                                .iter()
                                .find_map(|(installed_language_id, provider_id)| {
                                    (installed_language_id == &language_id)
                                        .then(|| provider_id.clone())
                                })
                                .ok_or_else(|| {
                                    AspClientOperationError::Message(format!(
                                        "installed provider target missing for languageId={language_id}"
                                    ))
                                })?;
                            Some(agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationProviderTarget {
                                language_id,
                                provider_id: Some(provider_id),
                            })
                        }
                    };
                    let initialized = initialized_workspaces
                        .lock()
                        .map_err(|_| "ASP client workspace-root registry poisoned".to_owned())?
                        .get(&(
                            request.project_id.as_str().to_owned(),
                            request.workspace_id.as_str().to_owned(),
                            request.session_id.as_str().to_owned(),
                        ))
                        .cloned()
                        .ok_or_else(|| {
                            "workspace generation ensure-ready requires an initialized workspace root"
                                .to_owned()
                        })?;
                    let terminal = generation_admission
                        .ensure_runtime_generation_ready_for_provider(
                            request.workspace_id.as_str().to_owned(),
                            initialized.project_root.clone(),
                            provider_target,
                        )
                        .await?;
                    install_runtime_query_generation_terminal(
                        &terminal,
                        &query_generation_authority,
                        workspace_registry.as_ref(),
                        &project_workspace_key,
                        request.workspace_id.as_str(),
                        &initialized.project_root,
                    )
                    .await
                    .map_err(|error| error.message)?;
                    return serde_json::to_value(terminal)
                        .map_err(|error| error.to_string())
                        .map_err(AspClientOperationError::Message);
                }
                let resolved = agent_semantic_client_protocol::resolve_server_client_method_owner(
                    &request.method,
                    installed_provider_targets
                        .iter()
                        .map(|(language_id, _)| language_id.clone()),
                )?;
                if let agent_semantic_client_protocol::ResolvedServerClientMethod::Server(
                    ServerClientRoute::GraphEvaluate,
                ) = resolved
                {
                    let validated_params = ResidentGraphEvaluationRequestV1::from_value(
                        request.params,
                    )
                    .map_err(|error| {
                        AspClientOperationError::Message(format!(
                            "invalid resident graph evaluation request: {error}"
                        ))
                    })?;
                    let language_id = validated_params.as_value()["languageId"]
                        .as_str()
                        .expect("validated languageId");
                    let provider_id = installed_provider_targets
                        .iter()
                        .find_map(|(installed_language_id, provider_id)| {
                            (installed_language_id == language_id).then_some(provider_id.as_str())
                        })
                        .ok_or_else(|| {
                            format!(
                                "installed provider target missing for languageId={language_id}"
                            )
                        })?;
                    let project_root = initialized_workspaces
                        .lock()
                        .map_err(|_| "ASP client workspace-root registry poisoned".to_owned())?
                        .get(&(
                            request.project_id.as_str().to_owned(),
                            request.workspace_id.as_str().to_owned(),
                            request.session_id.as_str().to_owned(),
                        ))
                        .ok_or_else(|| {
                            "ASP client graph request requires an initialized workspace root"
                                .to_owned()
                        })?
                        .project_root
                        .clone();
                    let dispatch_started = tokio::time::Instant::now();
                    let generation_state = query_generation
                        .borrow()
                        .get(&project_workspace_key)
                        .cloned();
                    let generation = match generation_state {
                        Some(RuntimeQueryGenerationState::Ready(generation)) => generation,
                        Some(RuntimeQueryGenerationState::Failed { reason, .. }) => {
                            let readiness_submission = request_runtime_query_generation_ready(
                                generation_admission.as_ref(),
                                &query_generation_authority,
                                &workspace_registry,
                                &project_workspace_key,
                                request.workspace_id.as_str().to_owned(),
                                project_root.clone(),
                            );
                            let submission_error = readiness_submission.err();
                            let publication_error = submission_error.as_ref().map_or_else(
                                || reason.to_string(),
                                |error| format!("{reason}; recovery submission failed: {error}"),
                            );
                            return Err(AspClientOperationError::Terminal(
                                query_generation_not_ready_error(QueryNotReadyContext {
                                    operation_id: request.request_id.as_str(),
                                    project_id: request.project_id.as_str(),
                                    workspace_id: request.workspace_id.as_str(),
                                    language_id,
                                    provider_id,
                                    exact_query: None,
                                    generation_state: if submission_error.is_some() {
                                        "submission-failed"
                                    } else {
                                        "building"
                                    },
                                    publication_error: Some(&publication_error),
                                    elapsed_micros: elapsed_micros(dispatch_started),
                                })?,
                            ));
                        }
                        None => {
                            return Err(AspClientOperationError::Terminal(
                                query_generation_not_ready_error(QueryNotReadyContext {
                                    operation_id: request.request_id.as_str(),
                                    project_id: request.project_id.as_str(),
                                    workspace_id: request.workspace_id.as_str(),
                                    language_id,
                                    provider_id,
                                    exact_query: None,
                                    generation_state: "unpublished",
                                    publication_error: None,
                                    elapsed_micros: elapsed_micros(dispatch_started),
                                })?,
                            ));
                        }
                    };
                    let result = crate::runtime_search_graph::evaluate_resident_search_graph(
                        request.request_id.as_str(),
                        request.workspace_id.as_str(),
                        language_id,
                        provider_id,
                        generation.generation_digest(),
                        generation.resident(),
                        validated_params.as_value(),
                    )
                    .map_err(|error| {
                        AspClientOperationError::Terminal(AspClientDispatchError {
                            reason_kind: error.reason_kind.to_owned(),
                            message: error.message,
                            details: error.details,
                        })
                    })?;
                    record_runtime_route_performance(
                        &telemetry_sender,
                        request.workspace_id.as_str(),
                        language_id,
                        generation.generation_digest(),
                        request.request_id.as_str(),
                        "graph.evaluate",
                        "runtime-resident-graph-evaluate",
                        validated_params.as_value()["profile"]
                            .as_str()
                            .expect("validated profile"),
                        elapsed_micros(dispatch_started),
                    )?;
                    return Ok(result);
                }
                let (language_id, provider_id, route) = match resolved {
                    agent_semantic_client_protocol::ResolvedServerClientMethod::Language {
                        language_id,
                        route,
                    } => {
                        let provider_id = installed_provider_targets
                            .iter()
                            .find_map(|(installed_language_id, provider_id)| {
                                (installed_language_id == &language_id).then(|| provider_id.clone())
                            })
                            .ok_or_else(|| {
                                format!(
                                    "installed provider target missing for languageId={language_id}"
                                )
                            })?;
                        (language_id, provider_id, route)
                    }
                    agent_semantic_client_protocol::ResolvedServerClientMethod::Server(
                        ServerClientRoute::WorkspaceSearchPlaybook,
                    ) => (
                        "workspace".to_owned(),
                        "workspace".to_owned(),
                        ServerClientRoute::WorkspaceSearchPlaybook,
                    ),
                    agent_semantic_client_protocol::ResolvedServerClientMethod::Server(_) => {
                        return Err(AspClientOperationError::Message(
                            "unsupported server-owned client method".to_owned(),
                        ));
                    }
                };
                let exact_query_params = if matches!(&route, ServerClientRoute::ExactQuery) {
                    let params: AspClientExactQueryRequest =
                        serde_json::from_value(request.params.clone())
                            .map_err(|error| error.to_string())?;
                    params.validate_schema_identity()?;
                    Some(params)
                } else {
                    None
                };
                let search_params = if matches!(&route, ServerClientRoute::Search) {
                    let params: AspClientSearchRequest =
                        serde_json::from_value(request.params.clone())
                            .map_err(|error| error.to_string())?;
                    params.validate_schema_identity()?;
                    Some(params)
                } else {
                    None
                };
                let workspace_search_playbook_params =
                    if matches!(&route, ServerClientRoute::WorkspaceSearchPlaybook) {
                        let params: AspClientWorkspaceSearchPlaybookRequest =
                            serde_json::from_value(request.params.clone())
                                .map_err(|error| error.to_string())?;
                        params.validate_schema_identity()?;
                        Some(params)
                    } else {
                        None
                    };
                let dispatch_started = tokio::time::Instant::now();
                let project_root = initialized_workspaces
                    .lock()
                    .map_err(|_| "ASP client workspace-root registry poisoned".to_owned())?
                    .get(&(
                        request.project_id.as_str().to_owned(),
                        request.workspace_id.as_str().to_owned(),
                        request.session_id.as_str().to_owned(),
                    ))
                    .ok_or_else(|| {
                        "ASP client request requires an initialized workspace root".to_owned()
                    })?
                    .project_root
                    .clone();
                let params = request.params;
                let generation_state = query_generation
                    .borrow()
                    .get(&project_workspace_key)
                    .cloned();
                let generation = 'generation_resolution: {
                    match generation_state {
                        Some(RuntimeQueryGenerationState::Ready(generation)) => generation,
                        Some(RuntimeQueryGenerationState::Failed { reason, .. }) => {
                            let readiness_submission = request_runtime_query_generation_ready(
                                generation_admission.as_ref(),
                                &query_generation_authority,
                                &workspace_registry,
                                &project_workspace_key,
                                request.workspace_id.as_str().to_owned(),
                                project_root.clone(),
                            );
                            let submission_error = readiness_submission.err();
                            if submission_error.is_none() {
                                if let Some(observed) = wait_for_runtime_query_generation_change(
                                    &mut query_generation,
                                    &project_workspace_key,
                                )
                                .await
                                {
                                    match observed {
                                        RuntimeQueryGenerationState::Ready(generation) => {
                                            break 'generation_resolution generation;
                                        }
                                        RuntimeQueryGenerationState::Failed { reason, .. } => {
                                            return Err(AspClientOperationError::Terminal(
                                                query_generation_not_ready_error(
                                                    QueryNotReadyContext {
                                                        operation_id: request.request_id.as_str(),
                                                        project_id: request.project_id.as_str(),
                                                        workspace_id: request.workspace_id.as_str(),
                                                        language_id: &language_id,
                                                        provider_id: &provider_id,
                                                        exact_query: exact_query_params.as_ref(),
                                                        generation_state: "failed",
                                                        publication_error: Some(reason.as_ref()),
                                                        elapsed_micros: elapsed_micros(
                                                            dispatch_started,
                                                        ),
                                                    },
                                                )?,
                                            ));
                                        }
                                    }
                                } else {
                                    return Err(AspClientOperationError::Terminal(
                                        query_generation_not_ready_error(QueryNotReadyContext {
                                            operation_id: request.request_id.as_str(),
                                            project_id: request.project_id.as_str(),
                                            workspace_id: request.workspace_id.as_str(),
                                            language_id: &language_id,
                                            provider_id: &provider_id,
                                            exact_query: exact_query_params.as_ref(),
                                            generation_state: "building",
                                            publication_error: Some(reason.as_ref()),
                                            elapsed_micros: elapsed_micros(dispatch_started),
                                        })?,
                                    ));
                                }
                            }
                            let publication_error = submission_error.as_ref().map_or_else(
                                || reason.to_string(),
                                |error| format!("{reason}; recovery submission failed: {error}"),
                            );
                            return Err(AspClientOperationError::Terminal(
                                query_generation_not_ready_error(QueryNotReadyContext {
                                    operation_id: request.request_id.as_str(),
                                    project_id: request.project_id.as_str(),
                                    workspace_id: request.workspace_id.as_str(),
                                    language_id: &language_id,
                                    provider_id: &provider_id,
                                    exact_query: exact_query_params.as_ref(),
                                    generation_state: if submission_error.is_some() {
                                        "submission-failed"
                                    } else {
                                        "building"
                                    },
                                    publication_error: Some(&publication_error),
                                    elapsed_micros: elapsed_micros(dispatch_started),
                                })?,
                            ));
                        }
                        None => {
                            let readiness_submission = request_runtime_query_generation_ready(
                                generation_admission.as_ref(),
                                &query_generation_authority,
                                &workspace_registry,
                                &project_workspace_key,
                                request.workspace_id.as_str().to_owned(),
                                project_root.clone(),
                            );
                            let (generation_state, readiness_submission_error) = match readiness_submission {
                                Ok(agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationReadinessRequestState::Accepted) => {
                                    ("building", None)
                                }
                                Ok(agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationReadinessRequestState::Coalesced) => {
                                    ("building", None)
                                }
                                Ok(agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationReadinessRequestState::Ready) => {
                                    ("opening-resident", None)
                                }
                                Err(error) => ("submission-failed", Some(error)),
                            };
                            if readiness_submission_error.is_none() {
                                if let Some(observed) = wait_for_runtime_query_generation(
                                    &mut query_generation,
                                    &project_workspace_key,
                                )
                                .await
                                {
                                    match observed {
                                        RuntimeQueryGenerationState::Ready(generation) => {
                                            break 'generation_resolution generation;
                                        }
                                        RuntimeQueryGenerationState::Failed { reason, .. } => {
                                            return Err(AspClientOperationError::Terminal(
                                                query_generation_not_ready_error(
                                                    QueryNotReadyContext {
                                                        operation_id: request.request_id.as_str(),
                                                        project_id: request.project_id.as_str(),
                                                        workspace_id: request.workspace_id.as_str(),
                                                        language_id: &language_id,
                                                        provider_id: &provider_id,
                                                        exact_query: exact_query_params.as_ref(),
                                                        generation_state: "failed",
                                                        publication_error: Some(reason.as_ref()),
                                                        elapsed_micros: elapsed_micros(
                                                            dispatch_started,
                                                        ),
                                                    },
                                                )?,
                                            ));
                                        }
                                    }
                                } else {
                                    return Err(AspClientOperationError::Terminal(
                                        query_generation_not_ready_error(QueryNotReadyContext {
                                            operation_id: request.request_id.as_str(),
                                            project_id: request.project_id.as_str(),
                                            workspace_id: request.workspace_id.as_str(),
                                            language_id: &language_id,
                                            provider_id: &provider_id,
                                            exact_query: exact_query_params.as_ref(),
                                            generation_state,
                                            publication_error: None,
                                            elapsed_micros: elapsed_micros(dispatch_started),
                                        })?,
                                    ));
                                }
                            }
                            return Err(AspClientOperationError::Terminal(
                                query_generation_not_ready_error(QueryNotReadyContext {
                                    operation_id: request.request_id.as_str(),
                                    project_id: request.project_id.as_str(),
                                    workspace_id: request.workspace_id.as_str(),
                                    language_id: &language_id,
                                    provider_id: &provider_id,
                                    exact_query: exact_query_params.as_ref(),
                                    generation_state,
                                    publication_error: readiness_submission_error.as_deref(),
                                    elapsed_micros: elapsed_micros(dispatch_started),
                                })?,
                            ));
                        }
                    }
                };
                // CompleteGeneration admission is a distinct cold barrier.
                // The bounded Search route starts its own latency budget only
                // after the immutable generation is resident and readable.
                let route_dispatch_started = tokio::time::Instant::now();
                match route {
                    agent_semantic_client_protocol::ServerClientRoute::AgentSessionRegister => {
                        unreachable!(
                            "server-owned AgentSession route is handled before language dispatch"
                        )
                    }
                    agent_semantic_client_protocol::ServerClientRoute::MultiAgentHostEvent => {
                        unreachable!("server-owned Host event route requires its typed dispatcher")
                    }
                    agent_semantic_client_protocol::ServerClientRoute::MultiAgentChildren => {
                        unreachable!(
                            "server-owned child projection route requires its typed dispatcher"
                        )
                    }
                    agent_semantic_client_protocol::ServerClientRoute::GraphEvaluate => {
                        unreachable!("server-owned graph route is handled before language dispatch")
                    }
                    agent_semantic_client_protocol::ServerClientRoute::GraphsTimeline => {
                        unreachable!("server-owned graph route is handled before language dispatch")
                    }
                    agent_semantic_client_protocol::ServerClientRoute::CancellationProbe => {
                        unreachable!("cancellation probe is owned by the lifecycle route")
                    }
                    agent_semantic_client_protocol::ServerClientRoute::LiveCorpusCacheState => {
                        unreachable!("Live Corpus cache state is handled before language dispatch")
                    }
                    agent_semantic_client_protocol::ServerClientRoute::WorkspaceGenerationEnsureReady => {
                        unreachable!(
                            "workspace generation ensure-ready is handled before language dispatch"
                        )
                    }
                    agent_semantic_client_protocol::ServerClientRoute::WorkspaceSearchPlaybook => {
                        let params = workspace_search_playbook_params
                            .expect("workspace Search playbook route decoded its request");
                        let plan_request = agent_semantic_search::SearchPlaybookRequest {
                            query: params.query,
                            intent: params.intent,
                            scope: params.scope,
                            coverage: params.coverage,
                            max_owners: params.max_owners,
                            deadline_ms: params.deadline_ms,
                            explain: params.explain,
                            language: params.language,
                            workspace: project_root.display().to_string(),
                        };
                        let plan = agent_semantic_search::build_workspace_search_playbook_plan(
                            &plan_request,
                            agent_semantic_search::WorkspaceSearchPlanBinding {
                                project_id: request.project_id.as_str().to_owned(),
                                workspace_id: request.workspace_id.as_str().to_owned(),
                                content_generation_digest: generation.content_generation_digest().to_owned(),
                            },
                            workspace_search_providers.iter().cloned(),
                        )
                        .map_err(AspClientOperationError::Message)?;
                        serde_json::to_value(plan)
                            .map_err(|error| AspClientOperationError::Message(error.to_string()))
                    }
                    agent_semantic_client_protocol::ServerClientRoute::Search => {
                        dispatch_search_route(
                            request.workspace_id.as_str(),
                            request.request_id.as_str(),
                            search_params.expect("Search route decoded its request"),
                            &language_id,
                            &provider_id,
                            generation.as_ref(),
                            &runtime_search_service,
                            &telemetry_sender,
                            route_dispatch_started,
                        )
                        .await
                    }
                    agent_semantic_client_protocol::ServerClientRoute::SourceIndexLookup => {
                        dispatch_source_index_lookup(
                            request.workspace_id.as_str(),
                            request.request_id.as_str(),
                            params,
                            &project_root,
                            &language_id,
                            &provider_id,
                            generation.as_ref(),
                            &telemetry_sender,
                        )
                    }
                    agent_semantic_client_protocol::ServerClientRoute::ExactQuery => {
                        dispatch_exact_query(
                            request.project_id.as_str(),
                            request.workspace_id.as_str(),
                            request.request_id.as_str(),
                            params,
                            &language_id,
                            &provider_id,
                            generation.as_ref(),
                            &telemetry_sender,
                        )
                    }
                }
            };
            let result = tokio::select! {
                            result = operation => result.map_err(|error| match error {
                                AspClientOperationError::Message(message) => AspClientDispatchError {
                                    reason_kind: "client-method-dispatch-failed".to_owned(),
                                    message,
                                    details: None,
                                },
                                AspClientOperationError::Terminal(error) => error,
                            }),
                            _ = async {
                                match dispatch_budget {
                                    Some(budget) => tokio::time::sleep(budget).await,
                                    None => std::future::pending::<()>().await,
                                }
                            } => {
            let budget = dispatch_budget.expect("deadline branch requires an interactive budget");
            Err(AspClientDispatchError {
                reason_kind: "client-request-deadline-exceeded".to_owned(),
                message: format!(
                    "Runtime ClientFrame dispatch exceeded its {}ms interactive deadline",
                    budget.as_millis(),
                ),
                details: Some(serde_json::json!({
                    "schemaId": "agent.semantic-protocols.asp-client-dispatch-failure",
                    "schemaVersion": "1",
                    "state": "failed",
                    "phase": "runtime-client-dispatch",
                    "reasonKind": "client-request-deadline-exceeded",
                    "budgetMs": budget.as_millis(),
                })),
            })
                            }
                            changed = cancelled.changed() => {
                                let _ = changed;
            Err(AspClientDispatchError {
                reason_kind: "client-request-cancelled".to_owned(),
                message: "client request was cancelled".to_owned(),
                details: None,
            })
                            }
                        };
            cancellations
                .lock()
                .expect("ASP Client Protocol cancellation registry poisoned")
                .remove(&key);
            result
        })
    }

    fn cancel(
        &self,
        project_id: &ClientProjectId,
        workspace_identity: &ClientWorkspaceIdentity,
        session_id: &ClientSessionId,
        request_id: &ClientRequestId,
    ) -> agent_semantic_client_server::AspClientCancelFuture {
        let key = (
            project_id.clone(),
            workspace_identity.clone(),
            session_id.clone(),
            request_id.clone(),
        );
        let cancellations = Arc::clone(&self.cancellations);
        let cancellation_admitted = Arc::clone(&self.cancellation_admitted);
        Box::pin(async move {
            loop {
                let admitted = cancellation_admitted.notified();
                let sender = cancellations
                    .lock()
                    .expect("ASP Client Protocol cancellation registry poisoned")
                    .remove(&key);
                if let Some(sender) = sender {
                    return sender.send(true).is_ok();
                }
                admitted.await;
            }
        })
    }
}
