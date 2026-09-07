// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Resolved graph, Search, and Query dispatch after lifecycle-owned special routes.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use agent_semantic_client_db::runtime_provider_register::RuntimeProviderRegister;
use agent_semantic_client_db::runtime_search_service::RuntimeSearchServiceHandle;
use agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationAdmission;
use agent_semantic_client_db::runtime_server_workspace::RuntimeServerWorkspaceRegistry;
use agent_semantic_client_db::runtime_telemetry_bus::RuntimeTelemetryBusSender;
use agent_semantic_client_server::{AspClientDispatchError, AspClientDispatchRequest};

use crate::runtime_query_generation_key::RuntimeProjectWorkspaceKey;
use crate::{RuntimeQueryGenerationAuthority, RuntimeQueryGenerationState};

use super::service::{ClientRequestKey, ClientWorkspaceKey, InitializedWorkspace};
use super::workspace_search_playbook::execute_progressive_search_clauses;
use super::workspace_search_playbook::synthesize_progressive_search_projection;
use super::{
    AspClientExactQueryRequest, AspClientOperationError, AspClientSearchRequest,
    AspClientWorkspaceQueryPlaybookRequest, AspClientWorkspaceSearchPlaybookRequest,
    AspClientWorkspaceSyntaxQueryRequest, QueryNotReadyContext, RUNTIME_CLIENT_DISPATCH_BUDGET,
    ResidentGraphEvaluationRequestV1, ServerClientRoute, dispatch_exact_query,
    dispatch_search_route, dispatch_source_index_lookup, dispatch_workspace_syntax_query,
    elapsed_micros, query_generation_not_ready_error, record_runtime_route_performance,
    request_runtime_query_generation_ready, revalidate_runtime_query_generation,
    wait_for_runtime_query_generation, wait_for_runtime_query_generation_change,
    workspace_search_providers_from_provider_register,
};

pub(super) struct ResolvedRouteContext {
    pub(super) request: AspClientDispatchRequest,
    pub(super) project_workspace_key: RuntimeProjectWorkspaceKey,
    pub(super) initialized_workspaces:
        Arc<Mutex<HashMap<ClientWorkspaceKey, InitializedWorkspace>>>,
    pub(super) runtime_search_service: RuntimeSearchServiceHandle,
    pub(super) generation_admission: Arc<WorkspaceGenerationAdmission>,
    pub(super) workspace_registry: Arc<RuntimeServerWorkspaceRegistry>,
    pub(super) active_provider_targets: Arc<[(String, String)]>,
    pub(super) provider_register: Arc<RuntimeProviderRegister>,
    pub(super) query_generation_authority: RuntimeQueryGenerationAuthority,
    pub(super) query_generation: tokio::sync::watch::Receiver<
        Arc<HashMap<RuntimeProjectWorkspaceKey, RuntimeQueryGenerationState>>,
    >,
    pub(super) telemetry_sender: RuntimeTelemetryBusSender,
    pub(super) telemetry_traces: Arc<
        Mutex<
            HashMap<
                ClientRequestKey,
                agent_semantic_client_db::runtime_server_opentelemetry::RuntimeSearchTelemetryTrace,
            >,
        >,
    >,
    pub(super) active_telemetry_trace_count: Arc<std::sync::atomic::AtomicUsize>,
}

fn selected_provider_targets(
    languages: Option<&str>,
    active_provider_targets: &[(String, String)],
) -> Result<
    Vec<agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationProviderTarget>,
    AspClientOperationError,
> {
    let mut seen = std::collections::BTreeSet::new();
    languages
        .into_iter()
        .flat_map(|languages| languages.split('|'))
        .map(str::trim)
        .filter(|language_id| !language_id.is_empty())
        .filter(|language_id| seen.insert((*language_id).to_owned()))
        .map(|language_id| {
            let provider_id = active_provider_targets
                .iter()
                .find_map(|(installed_language_id, provider_id)| {
                    (installed_language_id == language_id).then(|| provider_id.clone())
                })
                .ok_or_else(|| {
                    AspClientOperationError::Message(format!(
                        "installed provider target missing for languageId={language_id}"
                    ))
                })?;
            Ok(agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationProviderTarget {
                language_id: language_id.to_owned(),
                provider_id: Some(provider_id),
            })
        })
        .collect()
}

fn query_playbook_terminal(
    reason_kind: impl Into<String>,
    message: impl Into<String>,
    selector_count: usize,
) -> AspClientOperationError {
    AspClientOperationError::Terminal(AspClientDispatchError {
        reason_kind: reason_kind.into(),
        message: message.into(),
        details: Some(serde_json::json!({
            "selectorCount": selector_count,
            "failureStage": "runtime-execution-binding-admission"
        })),
    })
}

fn selector_owner_path(selector: &str) -> Option<&str> {
    selector
        .split_once("://")
        .and_then(|(_, suffix)| suffix.split_once("#item/"))
        .map(|(owner_path, _)| owner_path)
        .filter(|owner_path| !owner_path.is_empty())
}

fn query_playbook_provider_identity_sets(
    selectors: &[String],
    active_provider_targets: &[(String, String)],
) -> Option<(Vec<String>, Vec<String>)> {
    let mut languages = std::collections::BTreeSet::new();
    let mut providers = std::collections::BTreeSet::new();
    for selector in selectors {
        let language = selector.split_once("://")?.0;
        let provider =
            active_provider_targets
                .iter()
                .find_map(|(candidate_language, provider)| {
                    (candidate_language == language).then_some(provider)
                })?;
        languages.insert(language.to_owned());
        providers.insert(provider.clone());
    }
    (!languages.is_empty() && !providers.is_empty()).then(|| {
        (
            languages.into_iter().collect(),
            providers.into_iter().collect(),
        )
    })
}

fn record_settled_client_timing_observations(
    publication: &agent_semantic_content_identity::runtime_workspace_execution_publication::RuntimeWorkspaceExecutionPublication,
    witness: &agent_semantic_client_protocol::RuntimeSearchClientTimingWitness,
    session_id: &str,
    request_id: &str,
    language_ids: Vec<String>,
    provider_ids: Vec<String>,
    projection: Option<&str>,
    telemetry_sender: &RuntimeTelemetryBusSender,
) -> Result<
    agent_semantic_client_db::runtime_server_opentelemetry::RuntimeSearchTelemetryTrace,
    String,
> {
    let identity = agent_semantic_client_db::runtime_server_opentelemetry::RuntimeSearchTelemetryIdentity::settle_client_timing(
        publication,
        witness,
        session_id,
        request_id,
        language_ids,
        provider_ids,
    )
    .map_err(|error| error.reason_kind().to_owned())?;
    let trace =
        agent_semantic_client_db::runtime_server_opentelemetry::RuntimeSearchTelemetryTrace::new(
            identity,
        );
    let budget_micros = RUNTIME_CLIENT_DISPATCH_BUDGET
        .as_micros()
        .min(u128::from(u64::MAX)) as u64;
    for phase in &witness.phases {
        let mut observation = trace
            .record_client_phase(&phase.name, phase.elapsed_micros, budget_micros)
            .map_err(|error| error.reason_kind().to_owned())?;
        observation.requested_projection = projection.map(str::to_owned);
        let _ = telemetry_sender.try_record_performance(observation);
    }
    Ok(trace)
}

fn emit_runtime_search_trace_observation(
    telemetry_sender: &RuntimeTelemetryBusSender,
    observation: Result<
        agent_semantic_client_db::runtime_server_opentelemetry::RuntimePerformanceObservation,
        agent_semantic_client_db::runtime_server_opentelemetry::RuntimeSearchTelemetryError,
    >,
) {
    if let Ok(observation) = observation {
        let _ = telemetry_sender.try_record_performance(observation);
    }
}

fn runtime_search_trace_budget_micros() -> u64 {
    RUNTIME_CLIENT_DISPATCH_BUDGET
        .as_micros()
        .min(u128::from(u64::MAX)) as u64
}

fn insert_runtime_search_trace(
    traces: &Arc<
        Mutex<
            HashMap<
                ClientRequestKey,
                agent_semantic_client_db::runtime_server_opentelemetry::RuntimeSearchTelemetryTrace,
            >,
        >,
    >,
    active_count: &std::sync::atomic::AtomicUsize,
    key: ClientRequestKey,
    trace: agent_semantic_client_db::runtime_server_opentelemetry::RuntimeSearchTelemetryTrace,
) {
    if let Ok(mut traces) = traces.lock()
        && traces.insert(key, trace).is_none()
    {
        active_count.fetch_add(1, std::sync::atomic::Ordering::Release);
    }
}

fn materialize_query_playbook_receipt(
    request_id: &str,
    params: &AspClientWorkspaceQueryPlaybookRequest,
    runtime_binding: &agent_semantic_content_identity::runtime_execution::RuntimeExecutionBinding,
    execution_publication_digest: &str,
    runtime_bundle_digest: &str,
    manifest_project_workspace: &agent_semantic_content_identity::ProjectWorkspaceBinding,
    active_provider_targets: &[(String, String)],
    telemetry: Option<(
        &agent_semantic_client_db::runtime_server_opentelemetry::RuntimeSearchTelemetryTrace,
        &RuntimeTelemetryBusSender,
    )>,
    resolve_gql_relationships: impl Fn(
        &str,
    ) -> Result<
        Vec<agent_semantic_search_projection::QueryPlaybookGqlRelationship>,
        String,
    >,
    read_selector: impl Fn(
        agent_semantic_client_db::runtime_server_workspace::ExactProjectionKind,
        &str,
    ) -> Result<
        agent_semantic_client_db::runtime_server_workspace::WorkspaceRuntimeSelectorRead,
        String,
    >,
) -> Result<serde_json::Value, AspClientOperationError> {
    let provider_dispatch_started = tokio::time::Instant::now();
    let internal_request = serde_json::json!({
        "schemaId": "agent.semantic-protocols.query-playbook-materialization-request",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.query-playbook",
        "protocolVersion": "1",
        "requestId": request_id,
        "projectWorkspaceIdentity": runtime_binding.project_workspace.project_workspace_identity(),
        "worktreeInstanceId": runtime_binding.worktree_instance_id,
        "runtimeExecutionBinding": runtime_binding,
        "runtimeWorkspaceExecutionPublicationDigest": execution_publication_digest,
        "runtimeBundleDigest": runtime_bundle_digest,
        "selectors": params.selectors,
        "projection": params.projection,
    });
    let admitted_request =
        agent_semantic_search_projection::QueryPlaybookMaterializationRequest::admit_for_runtime(
            internal_request,
            runtime_binding,
            execution_publication_digest,
            runtime_bundle_digest,
            manifest_project_workspace,
        )
        .map_err(|error| {
            query_playbook_terminal(
                error.reason_kind(),
                error.to_string(),
                params.selectors.len(),
            )
        })?;
    let projection =
        agent_semantic_client_db::runtime_server_workspace::ExactProjectionKind::try_from(
            params.projection.as_str(),
        )?;
    if let Some((trace, sender)) = telemetry {
        emit_runtime_search_trace_observation(
            sender,
            trace.record_provider_dispatch(
                elapsed_micros(provider_dispatch_started),
                runtime_search_trace_budget_micros(),
            ),
        );
    }
    let parse_index_query_started = tokio::time::Instant::now();
    let mut materializations = Vec::with_capacity(params.selectors.len());
    let mut failure_reason = None;
    for selector in &params.selectors {
        let gql_relationships = match resolve_gql_relationships(selector) {
            Ok(relationships) if !relationships.is_empty() => relationships,
            _ => {
                failure_reason = Some("query-playbook-gql-relationship-unavailable");
                break;
            }
        };
        let language_id = selector.split_once("://").map(|(language, _)| language);
        let provider_id = language_id.and_then(|language_id| {
            active_provider_targets
                .iter()
                .find_map(|(language, provider)| {
                    (language == language_id).then_some(provider.as_str())
                })
        });
        let owner_path = selector_owner_path(selector);
        let read = read_selector(projection, selector)?;
        match (language_id, provider_id, owner_path, read) {
            (
                Some(language_id),
                Some(provider_id),
                Some(owner_path),
                agent_semantic_client_db::runtime_server_workspace::WorkspaceRuntimeSelectorRead::Projection {
                    resolved_selector,
                    bytes,
                    ..
                },
            ) if resolved_selector == *selector => {
                materializations.push(serde_json::json!({
                    "selector": selector,
                    "languageId": language_id,
                    "providerId": provider_id,
                    "ownerPath": owner_path,
                    "projection": params.projection,
                    "gqlRelationships": gql_relationships,
                    "sourceContentDigest": blake3::hash(&bytes).to_hex().to_string(),
                    "bytes": bytes,
                }));
            }
            _ => {
                failure_reason = Some("query-playbook-selector-not-materialized");
                break;
            }
        }
    }
    if failure_reason.is_some() {
        materializations.clear();
    }
    if let Some((trace, sender)) = telemetry {
        emit_runtime_search_trace_observation(
            sender,
            trace.record_search_execution(
                elapsed_micros(parse_index_query_started),
                runtime_search_trace_budget_micros(),
            ),
        );
    }
    let projection_rank_started = tokio::time::Instant::now();
    let terminal = match failure_reason {
        Some(reason_kind) => serde_json::json!({
            "state": "failed",
            "terminalCount": 1,
            "reasonKind": reason_kind,
        }),
        None => serde_json::json!({"state": "ready", "terminalCount": 1}),
    };
    let receipt = serde_json::json!({
        "schemaId": "agent.semantic-protocols.query-playbook-materialization-receipt",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.query-playbook",
        "protocolVersion": "1",
        "requestId": request_id,
        "projectWorkspaceIdentity": runtime_binding.project_workspace.project_workspace_identity(),
        "worktreeInstanceId": runtime_binding.worktree_instance_id,
        "runtimeExecutionBinding": runtime_binding,
        "runtimeWorkspaceExecutionPublicationDigest": execution_publication_digest,
        "runtimeBundleDigest": runtime_bundle_digest,
        "projection": params.projection,
        "requestedSelectors": params.selectors,
        "materializations": materializations,
        "terminal": terminal,
    });
    let receipt =
        agent_semantic_search_projection::QueryPlaybookMaterializationReceipt::admit_for_runtime(
            receipt,
            &admitted_request,
            runtime_binding,
            execution_publication_digest,
            runtime_bundle_digest,
            manifest_project_workspace,
        )
        .map_err(|error| {
            query_playbook_terminal(
                error.reason_kind(),
                error.to_string(),
                params.selectors.len(),
            )
        })?;
    if let Some((trace, sender)) = telemetry {
        emit_runtime_search_trace_observation(
            sender,
            trace.record_search_projection(
                elapsed_micros(projection_rank_started),
                runtime_search_trace_budget_micros(),
            ),
        );
    }
    Ok(receipt.as_json().clone())
}

async fn dispatch_workspace_query_playbook(
    request: &AspClientDispatchRequest,
    params: AspClientWorkspaceQueryPlaybookRequest,
    project_workspace_key: &RuntimeProjectWorkspaceKey,
    initialized_workspaces: &Arc<Mutex<HashMap<ClientWorkspaceKey, InitializedWorkspace>>>,
    workspace_registry: &RuntimeServerWorkspaceRegistry,
    active_provider_targets: &[(String, String)],
    telemetry_sender: &RuntimeTelemetryBusSender,
    telemetry_traces: &Arc<
        Mutex<
            HashMap<
                ClientRequestKey,
                agent_semantic_client_db::runtime_server_opentelemetry::RuntimeSearchTelemetryTrace,
            >,
        >,
    >,
    active_telemetry_trace_count: &std::sync::atomic::AtomicUsize,
    query_generation: &tokio::sync::watch::Receiver<
        Arc<HashMap<RuntimeProjectWorkspaceKey, RuntimeQueryGenerationState>>,
    >,
) -> Result<serde_json::Value, AspClientOperationError> {
    let server_admission_started = tokio::time::Instant::now();
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
            query_playbook_terminal(
                "query-playbook-runtime-binding-unavailable",
                "Query Playbook requires an initialized Host workspace binding",
                params.selectors.len(),
            )
        })?;
    let generation = match query_generation.borrow().get(project_workspace_key) {
        Some(RuntimeQueryGenerationState::Ready(generation)) => Arc::clone(generation),
        _ => {
            return Err(query_playbook_terminal(
                "query-playbook-runtime-binding-unavailable",
                "Query Playbook requires a resident source/Runtime execution product",
                params.selectors.len(),
            ));
        }
    };
    let server_admission_elapsed = elapsed_micros(server_admission_started);
    let snapshot_resolve_started = tokio::time::Instant::now();
    let (resident_root, resident_generation_digest) = workspace_registry
        .unique_resident_scope(request.workspace_id.as_str())
        .map_err(|error| {
            query_playbook_terminal(
                "query-playbook-runtime-binding-unavailable",
                error,
                params.selectors.len(),
            )
        })?;
    if resident_root != initialized.project_root
        || resident_generation_digest != generation.generation_digest()
    {
        return Err(query_playbook_terminal(
            "query-playbook-runtime-binding-mismatch",
            "Query Playbook resident generation changed after execution publication",
            params.selectors.len(),
        ));
    }
    let execution_publication = generation.execution_publication().ok_or_else(|| {
        query_playbook_terminal(
            "query-playbook-runtime-binding-unavailable",
            "Query Playbook generation has no admitted RuntimeExecutionBinding V2",
            params.selectors.len(),
        )
    })?;
    execution_publication.validate().map_err(|error| {
        query_playbook_terminal(
            "query-playbook-runtime-binding-mismatch",
            format!("invalid Runtime execution publication: {error:?}"),
            params.selectors.len(),
        )
    })?;
    let runtime_binding = &execution_publication.runtime_execution_binding;
    if runtime_binding.project_workspace != *initialized.host_workspace.project_workspace()
        || runtime_binding.worktree_instance_id != initialized.host_workspace.worktree_instance_id()
    {
        return Err(query_playbook_terminal(
            "query-playbook-runtime-context-mismatch",
            "Query Playbook Host workspace differs from the resident Runtime binding",
            params.selectors.len(),
        ));
    }
    let snapshot_resolve_elapsed = elapsed_micros(snapshot_resolve_started);
    let mut telemetry_trace = None;
    if let Some(witness) = request.client_timing_witness.as_ref() {
        if let Some((language_ids, provider_ids)) =
            query_playbook_provider_identity_sets(&params.selectors, active_provider_targets)
        {
            if let Ok(trace) = record_settled_client_timing_observations(
                execution_publication,
                witness,
                request.session_id.as_str(),
                request.request_id.as_str(),
                language_ids,
                provider_ids,
                Some(&params.projection),
                telemetry_sender,
            ) {
                emit_runtime_search_trace_observation(
                    telemetry_sender,
                    trace.record_server_admission_queue(
                        server_admission_elapsed,
                        runtime_search_trace_budget_micros(),
                    ),
                );
                emit_runtime_search_trace_observation(
                    telemetry_sender,
                    trace.record_snapshot_resolve(
                        snapshot_resolve_elapsed,
                        runtime_search_trace_budget_micros(),
                    ),
                );
                insert_runtime_search_trace(
                    telemetry_traces,
                    active_telemetry_trace_count,
                    (
                        request.project_id.clone(),
                        request.workspace_id.clone(),
                        request.session_id.clone(),
                        request.request_id.clone(),
                    ),
                    trace.clone(),
                );
                telemetry_trace = Some(trace);
            }
        }
    }
    materialize_query_playbook_receipt(
        request.request_id.as_str(),
        &params,
        runtime_binding,
        execution_publication.publication_digest.as_str(),
        execution_publication.runtime_bundle_digest.as_str(),
        initialized.host_workspace.project_workspace(),
        active_provider_targets,
        telemetry_trace
            .as_ref()
            .map(|trace| (trace, telemetry_sender)),
        |selector| {
            agent_semantic_search_projection::QueryPlaybookGqlRelationship::new(
                "Query",
                "materializes",
                selector,
            )
            .map(|relationship| vec![relationship])
            .map_err(|error| error.to_string())
        },
        |projection, selector| generation.read_runtime_selector(projection, selector),
    )
}

pub(super) async fn dispatch_resolved_route(
    context: ResolvedRouteContext,
) -> Result<serde_json::Value, AspClientOperationError> {
    let ResolvedRouteContext {
        request,
        project_workspace_key,
        initialized_workspaces,
        runtime_search_service,
        generation_admission,
        workspace_registry,
        active_provider_targets,
        provider_register,
        query_generation_authority,
        mut query_generation,
        telemetry_sender,
        telemetry_traces,
        active_telemetry_trace_count,
    } = context;

    let resolved = agent_semantic_client_protocol::resolve_server_client_method_owner(
        &request.method,
        active_provider_targets
            .iter()
            .map(|(language_id, _)| language_id.clone()),
    )?;
    if let agent_semantic_client_protocol::ResolvedServerClientMethod::Server(
        ServerClientRoute::GraphEvaluate,
    ) = resolved
    {
        let validated_params = ResidentGraphEvaluationRequestV1::from_value(request.params)
            .map_err(|error| {
                AspClientOperationError::Message(format!(
                    "invalid resident graph evaluation request: {error}"
                ))
            })?;
        let language_id = validated_params.as_value()["languageId"]
            .as_str()
            .expect("validated languageId");
        let provider_id = active_provider_targets
            .iter()
            .find_map(|(installed_language_id, provider_id)| {
                (installed_language_id == language_id).then_some(provider_id.as_str())
            })
            .ok_or_else(|| {
                format!("installed provider target missing for languageId={language_id}")
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
                "ASP client graph request requires an initialized workspace root".to_owned()
            })?
            .project_root
            .clone();
        let dispatch_started = tokio::time::Instant::now();
        let generation_provider_targets = vec![
            agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationProviderTarget {
                language_id: language_id.to_owned(),
                provider_id: Some(provider_id.to_owned()),
            },
        ];
        let generation_state = query_generation
            .borrow()
            .get(&project_workspace_key)
            .cloned();
        let generation = match generation_state {
            Some(RuntimeQueryGenerationState::Ready(_)) => revalidate_runtime_query_generation(
                generation_admission.as_ref(),
                &query_generation_authority,
                &workspace_registry,
                &project_workspace_key,
                request.workspace_id.as_str().to_owned(),
                project_root.clone(),
                &generation_provider_targets,
            )
            .await
            .map_err(|error| {
                AspClientOperationError::Terminal(
                    query_generation_not_ready_error(QueryNotReadyContext {
                        operation_id: request.request_id.as_str(),
                        project_id: request.project_id.as_str(),
                        workspace_id: request.workspace_id.as_str(),
                        language_id,
                        provider_id,
                        exact_query: None,
                        generation_state: "binding-refresh-failed",
                        publication_error: Some(&error),
                        elapsed_micros: elapsed_micros(dispatch_started),
                    })
                    .expect("query readiness failure is serializable"),
                )
            })?,
            Some(RuntimeQueryGenerationState::Failed { reason, .. }) => {
                let readiness_submission = request_runtime_query_generation_ready(
                    generation_admission.as_ref(),
                    &query_generation_authority,
                    &workspace_registry,
                    &project_workspace_key,
                    request.workspace_id.as_str().to_owned(),
                    project_root.clone(),
                    generation_provider_targets.clone(),
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
    if matches!(
        resolved,
        agent_semantic_client_protocol::ResolvedServerClientMethod::Server(
            ServerClientRoute::WorkspaceQueryPlaybook
        )
    ) {
        let params: AspClientWorkspaceQueryPlaybookRequest =
            serde_json::from_value(request.params.clone()).map_err(|error| error.to_string())?;
        return dispatch_workspace_query_playbook(
            &request,
            params,
            &project_workspace_key,
            &initialized_workspaces,
            &workspace_registry,
            active_provider_targets.as_ref(),
            &telemetry_sender,
            &telemetry_traces,
            active_telemetry_trace_count.as_ref(),
            &query_generation,
        )
        .await;
    }
    let (language_id, provider_id, route) = match resolved {
        agent_semantic_client_protocol::ResolvedServerClientMethod::Language {
            language_id,
            route,
        } => {
            let provider_id = active_provider_targets
                .iter()
                .find_map(|(installed_language_id, provider_id)| {
                    (installed_language_id == &language_id).then(|| provider_id.clone())
                })
                .ok_or_else(|| {
                    format!("installed provider target missing for languageId={language_id}")
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
        agent_semantic_client_protocol::ResolvedServerClientMethod::Server(
            ServerClientRoute::WorkspaceSyntaxQuery,
        ) => (
            "workspace".to_owned(),
            "workspace".to_owned(),
            ServerClientRoute::WorkspaceSyntaxQuery,
        ),
        agent_semantic_client_protocol::ResolvedServerClientMethod::Server(_) => {
            return Err(AspClientOperationError::Message(
                "unsupported server-owned client method".to_owned(),
            ));
        }
    };
    let exact_query_params = if matches!(&route, ServerClientRoute::ExactQuery) {
        let params: AspClientExactQueryRequest =
            serde_json::from_value(request.params.clone()).map_err(|error| error.to_string())?;
        params.validate_schema_identity()?;
        Some(params)
    } else {
        None
    };
    let search_params = if matches!(&route, ServerClientRoute::Search) {
        let params: AspClientSearchRequest =
            serde_json::from_value(request.params.clone()).map_err(|error| error.to_string())?;
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
    let workspace_syntax_query_params = if matches!(&route, ServerClientRoute::WorkspaceSyntaxQuery)
    {
        let params: AspClientWorkspaceSyntaxQueryRequest =
            serde_json::from_value(request.params.clone()).map_err(|error| error.to_string())?;
        params.validate_schema_identity()?;
        Some(params)
    } else {
        None
    };
    let workspace_search_providers =
        workspace_search_providers_from_provider_register(provider_register.as_ref())
            .map_err(AspClientOperationError::Message)?;
    let generation_provider_targets = match &route {
        ServerClientRoute::WorkspaceSearchPlaybook => selected_provider_targets(
            workspace_search_playbook_params
                .as_ref()
                .and_then(|params| params.languages.as_deref()),
            active_provider_targets.as_ref(),
        )?,
        ServerClientRoute::WorkspaceSyntaxQuery => selected_provider_targets(
            workspace_syntax_query_params
                .as_ref()
                .and_then(|params| params.languages.as_deref()),
            active_provider_targets.as_ref(),
        )?,
        _ => vec![
            agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationProviderTarget {
                language_id: language_id.clone(),
                provider_id: Some(provider_id.clone()),
            },
        ],
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
        .ok_or_else(|| "ASP client request requires an initialized workspace root".to_owned())?
        .project_root
        .clone();
    let params = request.params;
    let generation_state = query_generation
        .borrow()
        .get(&project_workspace_key)
        .cloned();
    let generation = 'generation_resolution: {
        match generation_state {
            Some(RuntimeQueryGenerationState::Ready(_)) => {
                match revalidate_runtime_query_generation(
                    generation_admission.as_ref(),
                    &query_generation_authority,
                    &workspace_registry,
                    &project_workspace_key,
                    request.workspace_id.as_str().to_owned(),
                    project_root.clone(),
                    &generation_provider_targets,
                )
                .await
                {
                    Ok(generation) => generation,
                    Err(error) => {
                        return Err(AspClientOperationError::Terminal(
                            query_generation_not_ready_error(QueryNotReadyContext {
                                operation_id: request.request_id.as_str(),
                                project_id: request.project_id.as_str(),
                                workspace_id: request.workspace_id.as_str(),
                                language_id: &language_id,
                                provider_id: &provider_id,
                                exact_query: exact_query_params.as_ref(),
                                generation_state: "binding-refresh-failed",
                                publication_error: Some(&error),
                                elapsed_micros: elapsed_micros(dispatch_started),
                            })?,
                        ));
                    }
                }
            }
            Some(RuntimeQueryGenerationState::Failed { reason, .. }) => {
                let readiness_submission = request_runtime_query_generation_ready(
                    generation_admission.as_ref(),
                    &query_generation_authority,
                    &workspace_registry,
                    &project_workspace_key,
                    request.workspace_id.as_str().to_owned(),
                    project_root.clone(),
                    generation_provider_targets.clone(),
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
                                    query_generation_not_ready_error(QueryNotReadyContext {
                                        operation_id: request.request_id.as_str(),
                                        project_id: request.project_id.as_str(),
                                        workspace_id: request.workspace_id.as_str(),
                                        language_id: &language_id,
                                        provider_id: &provider_id,
                                        exact_query: exact_query_params.as_ref(),
                                        generation_state: "failed",
                                        publication_error: Some(reason.as_ref()),
                                        elapsed_micros: elapsed_micros(dispatch_started),
                                    })?,
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
                    generation_provider_targets.clone(),
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
                                    query_generation_not_ready_error(QueryNotReadyContext {
                                        operation_id: request.request_id.as_str(),
                                        project_id: request.project_id.as_str(),
                                        workspace_id: request.workspace_id.as_str(),
                                        language_id: &language_id,
                                        provider_id: &provider_id,
                                        exact_query: exact_query_params.as_ref(),
                                        generation_state: "failed",
                                        publication_error: Some(reason.as_ref()),
                                        elapsed_micros: elapsed_micros(dispatch_started),
                                    })?,
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
            unreachable!("server-owned AgentSession route is handled before language dispatch")
        }
        agent_semantic_client_protocol::ServerClientRoute::MultiAgentHostEvent => {
            unreachable!("server-owned Host event route requires its typed dispatcher")
        }
        agent_semantic_client_protocol::ServerClientRoute::MultiAgentChildren => {
            unreachable!("server-owned child projection route requires its typed dispatcher")
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
            unreachable!("workspace generation ensure-ready is handled before language dispatch")
        }
        agent_semantic_client_protocol::ServerClientRoute::WorkspaceSearchPlaybook => {
            let params = workspace_search_playbook_params
                .expect("workspace Search playbook route decoded its request");
            let plan_request = agent_semantic_search::ProgressiveSearchPlaybookRequest {
                languages: params.languages,
                documents: params.documents,
                workspace: params.workspace,
                fd: params.fd.unwrap_or_default(),
                rg: params.rg.unwrap_or_default(),
                tantivy: params.tantivy.unwrap_or_default(),
                syntax: params
                    .syntax
                    .unwrap_or_default()
                    .into_iter()
                    .map(|block| agent_semantic_search::ProducerNativeBlock {
                        producer: block.producer,
                        argv: block.argv,
                    })
                    .collect(),
                native_syntax: params.native_syntax.unwrap_or_default(),
                graph: params
                    .graph
                    .unwrap_or_default()
                    .into_iter()
                    .map(|block| agent_semantic_search::GraphNativeBlock {
                        language: block.language,
                        argv: block.argv,
                    })
                    .collect(),
                clause_order: params
                    .clause_order
                    .into_iter()
                    .map(|clause| agent_semantic_search::SearchPlaybookClauseRef {
                        axis: match clause.axis {
                            agent_semantic_client_protocol::AspClientSearchPlaybookClauseAxis::Fd => {
                                agent_semantic_search::SearchPlaybookClauseAxis::Fd
                            }
                            agent_semantic_client_protocol::AspClientSearchPlaybookClauseAxis::Rg => {
                                agent_semantic_search::SearchPlaybookClauseAxis::Rg
                            }
                            agent_semantic_client_protocol::AspClientSearchPlaybookClauseAxis::Tantivy => {
                                agent_semantic_search::SearchPlaybookClauseAxis::Tantivy
                            }
                            agent_semantic_client_protocol::AspClientSearchPlaybookClauseAxis::Syntax => {
                                agent_semantic_search::SearchPlaybookClauseAxis::Syntax
                            }
                            agent_semantic_client_protocol::AspClientSearchPlaybookClauseAxis::NativeSyntax => {
                                agent_semantic_search::SearchPlaybookClauseAxis::NativeSyntax
                            }
                            agent_semantic_client_protocol::AspClientSearchPlaybookClauseAxis::Graph => {
                                agent_semantic_search::SearchPlaybookClauseAxis::Graph
                            }
                        },
                        block_index: clause.block_index,
                    })
                    .collect(),
            };
            let provider_dispatch_started = tokio::time::Instant::now();
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
            let mut telemetry_trace = None;
            let snapshot_resolve_started = tokio::time::Instant::now();
            if let (Some(witness), Some(execution_publication)) = (
                request.client_timing_witness.as_ref(),
                generation.execution_publication(),
            ) {
                let language_ids = plan
                    .routes
                    .iter()
                    .map(|route| route.language_id.clone())
                    .collect::<std::collections::BTreeSet<_>>()
                    .into_iter()
                    .collect();
                let provider_ids = plan
                    .routes
                    .iter()
                    .map(|route| route.provider_id.clone())
                    .collect::<std::collections::BTreeSet<_>>()
                    .into_iter()
                    .collect();
                if let Ok(trace) = record_settled_client_timing_observations(
                    execution_publication,
                    witness,
                    request.session_id.as_str(),
                    request.request_id.as_str(),
                    language_ids,
                    provider_ids,
                    None,
                    &telemetry_sender,
                ) {
                    emit_runtime_search_trace_observation(
                        &telemetry_sender,
                        trace.record_server_admission_queue(
                            elapsed_micros(dispatch_started),
                            runtime_search_trace_budget_micros(),
                        ),
                    );
                    emit_runtime_search_trace_observation(
                        &telemetry_sender,
                        trace.record_snapshot_resolve(
                            elapsed_micros(snapshot_resolve_started),
                            runtime_search_trace_budget_micros(),
                        ),
                    );
                    emit_runtime_search_trace_observation(
                        &telemetry_sender,
                        trace.record_provider_dispatch(
                            elapsed_micros(provider_dispatch_started),
                            runtime_search_trace_budget_micros(),
                        ),
                    );
                    insert_runtime_search_trace(
                        &telemetry_traces,
                        active_telemetry_trace_count.as_ref(),
                        (
                            request.project_id.clone(),
                            request.workspace_id.clone(),
                            request.session_id.clone(),
                            request.request_id.clone(),
                        ),
                        trace.clone(),
                    );
                    telemetry_trace = Some(trace);
                }
            }
            let evidence = execute_progressive_search_clauses(
                &plan,
                &project_root,
                generation.as_ref(),
                workspace_search_providers.as_ref(),
                &runtime_search_service,
            )
            .await?;
            if let Some(trace) = telemetry_trace.as_ref() {
                emit_runtime_search_trace_observation(
                    &telemetry_sender,
                    trace.record_search_execution(
                        evidence.search_execution_elapsed_micros,
                        runtime_search_trace_budget_micros(),
                    ),
                );
            }
            let projection = synthesize_progressive_search_projection(
                request.request_id.as_str(),
                &language_id,
                evidence,
                generation.as_ref(),
                &runtime_search_service,
            )
            .await?;
            let result = Ok(projection.result);
            if let Some(trace) = telemetry_trace.as_ref() {
                emit_runtime_search_trace_observation(
                    &telemetry_sender,
                    trace.record_search_projection(
                        projection.elapsed_micros,
                        runtime_search_trace_budget_micros(),
                    ),
                );
            }
            result
        }
        agent_semantic_client_protocol::ServerClientRoute::WorkspaceQueryPlaybook => {
            unreachable!("workspace Query Playbook is fail-closed before generation admission")
        }
        agent_semantic_client_protocol::ServerClientRoute::WorkspaceSyntaxQuery => {
            dispatch_workspace_syntax_query(
                workspace_syntax_query_params.expect("workspace syntax Query decoded its request"),
                &project_root,
                generation.as_ref(),
                workspace_search_providers.as_ref(),
                &runtime_search_service,
            )
            .await
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
        agent_semantic_client_protocol::ServerClientRoute::ExactQuery => dispatch_exact_query(
            request.project_id.as_str(),
            request.workspace_id.as_str(),
            request.request_id.as_str(),
            params,
            &language_id,
            &provider_id,
            generation.as_ref(),
            &telemetry_sender,
        ),
    }
}

#[cfg(test)]
mod query_playbook_materialization_tests {
    use super::*;
    use agent_semantic_content_identity::content_binding::{
        AuthorityStamp, ContentBinding, ContentIdentity, ContentPublicationCommit,
    };
    use agent_semantic_content_identity::runtime_execution::{
        RuntimeExecutionBinding, RuntimeExecutionBindingInput,
    };
    use agent_semantic_content_identity::runtime_workspace_execution_publication::{
        RuntimeWorkspaceExecutionPublication, RuntimeWorkspaceExecutionPublicationInput,
    };

    fn digest(byte: char) -> String {
        format!("blake3-256:{}", byte.to_string().repeat(64))
    }

    fn runtime_binding() -> RuntimeExecutionBinding {
        let project_workspace = agent_semantic_content_identity::ProjectWorkspaceBinding::new(
            "git+https://github.com/tao3k/agent-semantic-protocols.git#workspace/main",
            ".",
            "cross-machine",
            Vec::new(),
        )
        .expect("Project Workspace");
        let identity = ContentIdentity {
            runtime_artifact_digest: digest('1'),
            workspace_snapshot_digest: digest('2'),
            source_generation_digest: digest('3'),
            source_index_digest: digest('4'),
            schema_digest: digest('5'),
            provider_catalog_digest: digest('6'),
        };
        let content_binding = ContentBinding::new(
            identity.clone(),
            AuthorityStamp {
                key_id: "runtime-server-complete-generation".into(),
                canonical_digest: identity.digest(),
                signature: digest('7'),
            },
        )
        .expect("content binding");
        RuntimeExecutionBinding::new(RuntimeExecutionBindingInput {
            project_workspace,
            worktree_instance_id: "worktree-main".into(),
            publication_nonce: "publication-1".into(),
            content_binding,
            runtime_artifact_digest: digest('1').into(),
            evaluator_policy_digest: digest('8').into(),
            active_artifact_receipt_digest: digest('9').into(),
            evaluator_abi_digest: digest('a').into(),
        })
        .expect("Runtime execution binding")
    }

    fn params() -> AspClientWorkspaceQueryPlaybookRequest {
        AspClientWorkspaceQueryPlaybookRequest {
            schema_id: "agent.semantic-protocols.asp-client-workspace-query-playbook-request"
                .into(),
            schema_version: "1".into(),
            selectors: vec![
                "org://docs/publication.org#item/heading/Publication".into(),
                "rust://src/registry.rs#item/function/refresh_registry".into(),
            ],
            projection: "source".into(),
        }
    }

    fn gql_relationships(
        selector: &str,
    ) -> Result<Vec<agent_semantic_search_projection::QueryPlaybookGqlRelationship>, String> {
        agent_semantic_search_projection::QueryPlaybookGqlRelationship::new(
            "Query",
            "materializes",
            selector,
        )
        .map(|relationship| vec![relationship])
        .map_err(|error| error.to_string())
    }

    fn execution_publication() -> RuntimeWorkspaceExecutionPublication {
        let runtime_execution_binding = runtime_binding();
        let content_publication_commit = ContentPublicationCommit::linearize(
            runtime_execution_binding.content_binding.identity.clone(),
            runtime_execution_binding
                .content_binding
                .authority_stamp
                .clone(),
        )
        .expect("content publication commit");
        RuntimeWorkspaceExecutionPublication::new(RuntimeWorkspaceExecutionPublicationInput {
            workspace_identity: "workspace-23cc5ba784c605ae".into(),
            generation_digest: digest('3').into(),
            source_root_digest: digest('2').into(),
            content_publication_commit,
            runtime_execution_binding,
            runtime_bundle_digest: digest('f').into(),
        })
        .expect("workspace execution publication")
    }

    #[test]
    fn client_timing_is_settled_once_against_the_multilanguage_execution_publication() {
        let publication = execution_publication();
        let witness = agent_semantic_client_protocol::RuntimeSearchClientTimingWitness::new(
            "session-1",
            "request-1",
            [11, 22, 33],
        )
        .expect("client timing witness");
        let mut bus = agent_semantic_client_db::runtime_telemetry_bus::RuntimeTelemetryBus::new();
        let selector_params = params();
        let trace = record_settled_client_timing_observations(
            &publication,
            &witness,
            "session-1",
            "request-1",
            vec!["org".into(), "rust".into()],
            vec!["asp-org".into(), "asp-rust".into()],
            Some(&selector_params.projection),
            &bus.sender,
        )
        .expect("settled client timing observations");

        assert_eq!(trace.observations().len(), 3);
        let observations = std::array::from_fn::<_, 3, _>(|_| {
            bus.receiver
                .try_recv()
                .expect("one client timing event")
                .into_observation()
        });
        assert_eq!(
            observations
                .each_ref()
                .map(|observation| observation.stage.as_str()),
            ["launcher", "client-frame-encode", "ipc-connect"]
        );
        let generation_digest = digest('3');
        let runtime_artifact_digest = digest('1');
        for observation in observations {
            assert_eq!(
                observation.workspace_identity.as_deref(),
                Some("workspace-23cc5ba784c605ae")
            );
            assert_eq!(observation.operation_id.as_deref(), Some("request-1"));
            assert_eq!(
                observation.language_ids,
                Some(vec!["org".into(), "rust".into()])
            );
            assert_eq!(
                observation.provider_ids,
                Some(vec!["asp-org".into(), "asp-rust".into()])
            );
            assert_eq!(
                observation.generation_digest.as_deref(),
                Some(generation_digest.as_str())
            );
            assert_eq!(
                observation.runtime_artifact_digest.as_deref(),
                Some(runtime_artifact_digest.as_str())
            );
            assert_eq!(observation.requested_projection.as_deref(), Some("source"));
        }

        let foreign = agent_semantic_client_protocol::RuntimeSearchClientTimingWitness::new(
            "session-1",
            "request-foreign",
            [1, 2, 3],
        )
        .expect("foreign timing witness");
        assert_eq!(
            record_settled_client_timing_observations(
                &publication,
                &foreign,
                "session-1",
                "request-1",
                vec!["org".into(), "rust".into()],
                vec!["asp-org".into(), "asp-rust".into()],
                Some(&selector_params.projection),
                &bus.sender,
            )
            .expect_err("foreign request must fail before telemetry admission"),
            "runtime-search-client-timing-identity-mismatch"
        );
        assert!(bus.receiver.try_recv().is_err());

        emit_runtime_search_trace_observation(
            &bus.sender,
            trace.record_server_admission_queue(5, runtime_search_trace_budget_micros()),
        );
        emit_runtime_search_trace_observation(
            &bus.sender,
            trace.record_snapshot_resolve(8, runtime_search_trace_budget_micros()),
        );
        let binding = &publication.runtime_execution_binding;
        let receipt = materialize_query_playbook_receipt(
            "request-1",
            &selector_params,
            binding,
            publication.publication_digest.as_str(),
            publication.runtime_bundle_digest.as_str(),
            &binding.project_workspace,
            &[
                ("org".into(), "asp-org".into()),
                ("rust".into(), "asp-rust".into()),
            ],
            Some((&trace, &bus.sender)),
            gql_relationships,
            |_projection, selector| {
                Ok(agent_semantic_client_db::runtime_server_workspace::WorkspaceRuntimeSelectorRead::Projection {
                    generation_digest: digest('3'),
                    root_digest: digest('2'),
                    resolved_selector: selector.to_owned(),
                    bytes: format!("materialized:{selector}").into_bytes(),
                })
            },
        )
        .unwrap_or_else(|_| panic!("runtime-bound Query materialization"));
        assert_eq!(receipt["terminal"]["state"], "ready");
        let server_stages = std::array::from_fn::<_, 5, _>(|_| {
            bus.receiver
                .try_recv()
                .expect("one server timing event")
                .into_observation()
                .stage
        });
        assert_eq!(
            server_stages,
            [
                "server-admission-queue",
                "snapshot-resolve",
                "provider-dispatch",
                "parse-index-query",
                "projection-rank",
            ]
        );
        assert_eq!(trace.observations().len(), 8);
    }

    #[test]
    fn query_playbook_materializes_one_runtime_bound_terminal_in_selector_order() {
        let binding = runtime_binding();
        let receipt = materialize_query_playbook_receipt(
            "request-query-playbook",
            &params(),
            &binding,
            &digest('e'),
            &digest('f'),
            &binding.project_workspace,
            &[
                ("org".into(), "asp-org".into()),
                ("rust".into(), "asp-rust".into()),
            ],
            None,
            gql_relationships,
            |_projection, selector| {
                Ok(agent_semantic_client_db::runtime_server_workspace::WorkspaceRuntimeSelectorRead::Projection {
                    generation_digest: digest('3'),
                    root_digest: digest('2'),
                    resolved_selector: selector.to_owned(),
                    bytes: format!("materialized:{selector}").into_bytes(),
                })
            },
        )
        .unwrap_or_else(|_| panic!("one complete materialization terminal"));
        assert_eq!(receipt["terminal"]["state"], "ready");
        assert_eq!(receipt["terminal"]["terminalCount"], 1);
        assert_eq!(receipt["materializations"].as_array().unwrap().len(), 2);
        assert_eq!(
            receipt["materializations"][0]["selector"],
            params().selectors[0]
        );
        assert_eq!(
            receipt["runtimeExecutionBinding"],
            serde_json::to_value(binding).unwrap()
        );
    }

    #[test]
    fn query_playbook_selector_failure_exposes_no_partial_materialization() {
        let binding = runtime_binding();
        let receipt = materialize_query_playbook_receipt(
            "request-query-playbook-failed",
            &params(),
            &binding,
            &digest('e'),
            &digest('f'),
            &binding.project_workspace,
            &[
                ("org".into(), "asp-org".into()),
                ("rust".into(), "asp-rust".into()),
            ],
            None,
            gql_relationships,
            |_projection, selector| {
                if selector.starts_with("org://") {
                    Ok(agent_semantic_client_db::runtime_server_workspace::WorkspaceRuntimeSelectorRead::Projection {
                        generation_digest: digest('3'),
                        root_digest: digest('2'),
                        resolved_selector: selector.to_owned(),
                        bytes: b"publication".to_vec(),
                    })
                } else {
                    Ok(agent_semantic_client_db::runtime_server_workspace::WorkspaceRuntimeSelectorRead::OwnerMissing {
                        generation_digest: digest('3'),
                        root_digest: digest('2'),
                    })
                }
            },
        )
        .unwrap_or_else(|_| panic!("typed failed materialization terminal"));
        assert_eq!(receipt["terminal"]["state"], "failed");
        assert_eq!(receipt["terminal"]["terminalCount"], 1);
        assert_eq!(
            receipt["terminal"]["reasonKind"],
            "query-playbook-selector-not-materialized"
        );
        assert_eq!(receipt["materializations"], serde_json::json!([]));
    }
}

#[cfg(test)]
mod selected_provider_target_tests {
    use super::selected_provider_targets;

    #[test]
    fn pipe_order_is_preserved_and_duplicates_are_removed() {
        let installed = vec![
            ("python".to_owned(), "asp-python".to_owned()),
            ("rust".to_owned(), "asp-rust".to_owned()),
        ];
        let targets = selected_provider_targets(Some("rust|python|rust"), &installed)
            .unwrap_or_else(|_| panic!("selected targets"));

        assert_eq!(
            targets
                .iter()
                .map(|target| target.language_id.as_str())
                .collect::<Vec<_>>(),
            vec!["rust", "python"]
        );
    }

    #[test]
    fn unrelated_uninstalled_provider_is_not_part_of_selected_closure() {
        let installed = vec![("rust".to_owned(), "asp-rust".to_owned())];
        let targets = selected_provider_targets(Some("rust"), &installed)
            .unwrap_or_else(|_| panic!("selected target"));

        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].provider_id.as_deref(), Some("asp-rust"));
    }
}
