// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

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
use agent_semantic_client_protocol::AspClientWorkspaceQueryPlaybookRequest;
use agent_semantic_client_protocol::AspClientWorkspaceSearchPlaybookRequest;
use agent_semantic_client_protocol::AspClientWorkspaceSyntaxQueryRequest;
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

#[path = "runtime_asp_client_syntax_query.rs"]
mod syntax_query_route;

#[path = "runtime_workspace_search_playbook.rs"]
mod workspace_search_playbook;

#[path = "runtime_asp_client_projection.rs"]
mod projection_routes;

#[path = "runtime_asp_client_telemetry.rs"]
mod telemetry;

#[path = "runtime_asp_client_query_playbook.rs"]
mod query_playbook;

#[path = "runtime_asp_client_resolved_route.rs"]
mod resolved_route;

use projection_routes::dispatch_exact_query;
use projection_routes::dispatch_source_index_lookup;
use query_generation_support::AspClientOperationError;
use query_generation_support::QueryNotReadyContext;
use query_generation_support::RUNTIME_CLIENT_DISPATCH_BUDGET;
use query_generation_support::classify_exact_query_failure;
use query_generation_support::query_generation_not_ready_error;
use query_generation_support::request_runtime_query_generation_ready;
use query_generation_support::{dispatch_budget_for_method, enforce_completed_dispatch_budget};
use resolved_route::{ResolvedRouteContext, dispatch_resolved_route};
use syntax_query_route::dispatch_workspace_syntax_query;
use telemetry::elapsed_micros;
use telemetry::record_runtime_route_performance;

#[path = "runtime_asp_client_service.rs"]
mod service;

pub use service::HostWorkspaceInitializationBindingResolver;
pub use service::RuntimeAspClientDispatcher;
pub use service::build_frame_service;
pub use service::workspace_search_providers_from_provider_register;

fn response_telemetry_key(
    response: &agent_semantic_client_server::AspClientResponseTelemetry,
) -> service::ClientRequestKey {
    (
        response.project_id.clone(),
        response.workspace_id.clone(),
        response.session_id.clone(),
        response.request_id.clone(),
    )
}

fn record_runtime_response_serialized(
    active_trace_count: &std::sync::atomic::AtomicUsize,
    traces: &std::sync::Mutex<
        std::collections::HashMap<
            service::ClientRequestKey,
            agent_semantic_client_db::runtime_server_opentelemetry::RuntimeSearchTelemetryTrace,
        >,
    >,
    telemetry_sender: &agent_semantic_client_db::runtime_telemetry_bus::RuntimeTelemetryBusSender,
    response: &agent_semantic_client_server::AspClientResponseTelemetry,
    elapsed_micros: u64,
) {
    if active_trace_count.load(std::sync::atomic::Ordering::Acquire) == 0 {
        return;
    }
    let trace = traces
        .lock()
        .ok()
        .and_then(|traces| traces.get(&response_telemetry_key(response)).cloned());
    if let Some(trace) = trace
        && let Ok(observation) = trace.record_response_serialized(elapsed_micros, 1_000)
    {
        let _ = telemetry_sender.try_record_performance(observation);
    }
}

fn record_runtime_terminal_egressed(
    active_trace_count: &std::sync::atomic::AtomicUsize,
    traces: &std::sync::Mutex<
        std::collections::HashMap<
            service::ClientRequestKey,
            agent_semantic_client_db::runtime_server_opentelemetry::RuntimeSearchTelemetryTrace,
        >,
    >,
    telemetry_sender: &agent_semantic_client_db::runtime_telemetry_bus::RuntimeTelemetryBusSender,
    response: &agent_semantic_client_server::AspClientResponseTelemetry,
    elapsed_micros: u64,
    delivered: bool,
) {
    if active_trace_count.load(std::sync::atomic::Ordering::Acquire) == 0 {
        return;
    }
    let trace = traces
        .lock()
        .ok()
        .and_then(|mut traces| traces.remove(&response_telemetry_key(response)));
    if let Some(trace) = trace {
        active_trace_count.fetch_sub(1, std::sync::atomic::Ordering::AcqRel);
        let terminal_state = if delivered {
            match response.outcome {
                agent_semantic_client_protocol::ClientOutcome::Ready => "ready",
                agent_semantic_client_protocol::ClientOutcome::Cancelled => "cancelled",
                agent_semantic_client_protocol::ClientOutcome::StaleGeneration => {
                    "stale-generation"
                }
                agent_semantic_client_protocol::ClientOutcome::Error => "error",
            }
        } else {
            "transport-closed"
        };
        if let Ok(observation) =
            trace.record_transport_terminal_egress(terminal_state, elapsed_micros, 1_000)
        {
            let _ = telemetry_sender.try_record_performance(observation);
        }
    }
}

fn resident_request_operation(
    method: &str,
) -> Option<agent_semantic_client_protocol::RuntimeResidentRequestOperation> {
    if agent_semantic_client_protocol::classify_client_dispatch(method)
        != agent_semantic_client_protocol::ClientDispatchClass::ResidentGenerationRead
    {
        return None;
    }
    Some(
        if method == agent_semantic_client_protocol::WORKSPACE_SEARCH_PLAYBOOK_METHOD
            || method == agent_semantic_client_protocol::GRAPH_EVALUATE_METHOD
            || method.ends_with(".search")
        {
            agent_semantic_client_protocol::RuntimeResidentRequestOperation::Search
        } else {
            agent_semantic_client_protocol::RuntimeResidentRequestOperation::Query
        },
    )
}

fn resident_generation_digest(
    value: &serde_json::Value,
    generations: &tokio::sync::watch::Receiver<
        Arc<std::collections::HashMap<RuntimeProjectWorkspaceKey, RuntimeQueryGenerationState>>,
    >,
    key: &RuntimeProjectWorkspaceKey,
) -> Option<String> {
    if let Some(RuntimeQueryGenerationState::Ready(generation)) = generations.borrow().get(key) {
        return Some(generation.generation_digest().to_owned());
    }
    value
        .get("generationDigest")
        .and_then(serde_json::Value::as_str)
        .or_else(|| {
            value
                .get("binding")
                .and_then(|binding| binding.get("sourceGenerationDigest"))
                .and_then(serde_json::Value::as_str)
        })
        .map(str::to_owned)
}

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
        let active_provider_targets = Arc::clone(&self.active_provider_targets);
        let provider_register = Arc::clone(&self.provider_register);
        let workspace_store_root = self.workspace_store_root.clone();
        let query_generation_authority = self.query_generation_authority.clone();
        let query_generation = query_generation_authority.subscribe();
        let query_generation_for_receipt = query_generation.clone();
        let telemetry_sender = self.telemetry_sender.clone();
        let telemetry_traces = Arc::clone(&self.telemetry_traces);
        let active_telemetry_trace_count = Arc::clone(&self.active_telemetry_trace_count);
        let cancellations = Arc::clone(&self.cancellations);
        let resident_request_seen = Arc::clone(&self.resident_request_seen);
        let dispatch_budget = dispatch_budget_for_method(&request.method);
        let request_plane_operation = resident_request_operation(&request.method);
        let request_plane_operation_id = request.request_id.as_str().to_owned();
        let request_plane_key = (request.project_id.clone(), request.workspace_id.clone());
        let request_plane_telemetry_sender = telemetry_sender.clone();
        Box::pin(async move {
            let dispatch_started = tokio::time::Instant::now();
            let project_workspace_key = RuntimeProjectWorkspaceKey::new(
                request.project_id.clone(),
                request.workspace_id.clone(),
            );
            let request_plane_generation_key = project_workspace_key.clone();
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
                        active_provider_targets
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
                            let provider_id = active_provider_targets
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
                    return serde_json::to_value(terminal)
                        .map_err(|error| error.to_string())
                        .map_err(AspClientOperationError::Message);
                }
                dispatch_resolved_route(ResolvedRouteContext {
                    request,
                    schema_bundles,
                    project_workspace_key,
                    initialized_workspaces,
                    generation_admission,
                    workspace_registry,
                    active_provider_targets,
                    provider_register,
                    query_generation,
                    telemetry_sender,
                    telemetry_traces,
                    active_telemetry_trace_count,
                })
                .await
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
            let request_elapsed = dispatch_started.elapsed();
            let result =
                enforce_completed_dispatch_budget(result, dispatch_budget, request_elapsed);
            if let Some(operation) = request_plane_operation {
                let elapsed_micros = u64::try_from(request_elapsed.as_micros()).unwrap_or(u64::MAX);
                let receipt_state = match &result {
                    Ok(value) => resident_generation_digest(
                        value,
                        &query_generation_for_receipt,
                        &request_plane_generation_key,
                    )
                    .map(Some),
                    Err(error) if error.reason_kind == "query-not-ready" => Some(None),
                    Err(_) => None,
                };
                if let Some(generation_digest) = receipt_state {
                    let temperature = if resident_request_seen
                        .lock()
                        .expect("resident request temperature registry poisoned")
                        .insert(request_plane_key)
                    {
                        agent_semantic_client_protocol::RuntimeResidentRequestTemperature::Cold
                    } else {
                        agent_semantic_client_protocol::RuntimeResidentRequestTemperature::Warm
                    };
                    let receipt = generation_digest.map_or_else(
                        || {
                            agent_semantic_client_protocol::RuntimeResidentRequestPlaneReceipt::query_not_ready(
                                operation,
                                temperature,
                                elapsed_micros,
                            )
                        },
                        |generation_digest| {
                            agent_semantic_client_protocol::RuntimeResidentRequestPlaneReceipt::ready(
                                operation,
                                temperature,
                                generation_digest,
                                elapsed_micros,
                            )
                        },
                    );
                    let _ = request_plane_telemetry_sender
                        .try_record_resident_request_plane(&request_plane_operation_id, receipt);
                }
            }
            cancellations
                .lock()
                .expect("ASP Client Protocol cancellation registry poisoned")
                .remove(&key);
            result
        })
    }

    fn response_serialized(
        &self,
        response: &agent_semantic_client_server::AspClientResponseTelemetry,
        elapsed_micros: u64,
    ) {
        record_runtime_response_serialized(
            &self.active_telemetry_trace_count,
            &self.telemetry_traces,
            &self.telemetry_sender,
            response,
            elapsed_micros,
        );
    }

    fn terminal_egressed(
        &self,
        response: &agent_semantic_client_server::AspClientResponseTelemetry,
        elapsed_micros: u64,
        delivered: bool,
    ) {
        record_runtime_terminal_egressed(
            &self.active_telemetry_trace_count,
            &self.telemetry_traces,
            &self.telemetry_sender,
            response,
            elapsed_micros,
            delivered,
        );
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
