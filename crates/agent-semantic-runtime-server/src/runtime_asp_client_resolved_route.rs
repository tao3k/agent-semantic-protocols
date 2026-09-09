// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Resolved graph, Search, and Query dispatch after lifecycle-owned special routes.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use agent_semantic_client_db::runtime_provider_register::RuntimeProviderRegister;
use agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationAdmission;
use agent_semantic_client_db::runtime_server_workspace::RuntimeServerWorkspaceRegistry;
use agent_semantic_client_db::runtime_telemetry_bus::RuntimeTelemetryBusSender;
use agent_semantic_client_server::{AspClientDispatchError, AspClientDispatchRequest};

use crate::RuntimeQueryGenerationState;
use crate::runtime_query_generation_key::RuntimeProjectWorkspaceKey;

use super::service::{ClientRequestKey, ClientWorkspaceKey, InitializedWorkspace};
use super::workspace_search_playbook::execute_progressive_search_clauses;
use super::workspace_search_playbook::synthesize_progressive_search_projection;
use super::{
    AspClientExactQueryRequest, AspClientOperationError, AspClientWorkspaceQueryPlaybookRequest,
    AspClientWorkspaceSearchPlaybookRequest, AspClientWorkspaceSyntaxQueryRequest,
    QueryNotReadyContext, ResidentGraphEvaluationRequestV1, ServerClientRoute,
    dispatch_exact_query, dispatch_source_index_lookup, dispatch_workspace_syntax_query,
    elapsed_micros, query_generation_not_ready_error, record_runtime_route_performance,
    request_runtime_query_generation_ready, workspace_search_providers_from_provider_register,
};

pub(super) struct ResolvedRouteContext {
    pub(super) request: AspClientDispatchRequest,
    pub(super) schema_bundles: crate::schema_bundle::RuntimeSchemaBundleCatalog,
    pub(super) project_workspace_key: RuntimeProjectWorkspaceKey,
    pub(super) initialized_workspaces:
        Arc<Mutex<HashMap<ClientWorkspaceKey, InitializedWorkspace>>>,
    pub(super) generation_admission: Arc<WorkspaceGenerationAdmission>,
    pub(super) workspace_registry: Arc<RuntimeServerWorkspaceRegistry>,
    pub(super) active_provider_targets: Arc<[(String, String)]>,
    pub(super) provider_register: Arc<RuntimeProviderRegister>,
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

fn selected_playbook_provider_targets(
    language: Option<&str>,
    documents: Option<&str>,
    providers: &[agent_semantic_search::WorkspaceSearchProvider],
) -> Result<
    Vec<agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationProviderTarget>,
    AspClientOperationError,
> {
    let mut requested = language
        .into_iter()
        .flat_map(|expression| expression.split('|'))
        .map(|producer| {
            (
                producer.trim(),
                agent_semantic_search::WorkspaceSearchProducerAxis::Language,
            )
        })
        .chain(documents.into_iter().flat_map(|expression| expression.split('|')).map(
            |producer| {
                (
                    producer.trim(),
                    agent_semantic_search::WorkspaceSearchProducerAxis::Document,
                )
            },
        ))
        .filter(|(producer, _)| !producer.is_empty())
        .collect::<Vec<_>>();
    requested.sort_unstable();
    requested.dedup();
    let mut admitted = std::collections::BTreeSet::new();
    let mut targets = Vec::new();
    for (producer, producer_axis) in requested {
        let provider = providers
            .iter()
            .find(|provider| provider.language_id == producer)
            .ok_or_else(|| {
                AspClientOperationError::Message(format!(
                    "Search Playbook producer is not installed: producer={producer}"
                ))
            })?;
        if !provider.producer_axes.contains(&producer_axis) {
            return Err(AspClientOperationError::Message(format!(
                "Playbook producer is declared on the wrong axis: producer={producer} requestedAxis={producer_axis:?} admittedAxes={:?}",
                provider.producer_axes
            )));
        }
        if admitted.insert((producer.to_owned(), provider.provider_id.clone())) {
            targets.push(
                agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationProviderTarget {
                    language_id: producer.to_owned(),
                    provider_id: Some(provider.provider_id.clone()),
                },
            );
        }
    }
    Ok(targets)
}

use super::query_playbook::{
    dispatch_workspace_query_playbook, emit_runtime_search_trace_observation,
    insert_runtime_search_trace, record_settled_client_timing_observations,
    runtime_search_trace_budget_micros,
};

pub(super) async fn dispatch_resolved_route(
    context: ResolvedRouteContext,
) -> Result<serde_json::Value, AspClientOperationError> {
    let ResolvedRouteContext {
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
            Some(RuntimeQueryGenerationState::Ready(generation)) => generation,
            Some(RuntimeQueryGenerationState::Failed { reason, .. }) => {
                let readiness_submission = request_runtime_query_generation_ready(
                    generation_admission.as_ref(),
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
                let readiness_submission = request_runtime_query_generation_ready(
                    generation_admission.as_ref(),
                    request.workspace_id.as_str().to_owned(),
                    project_root.clone(),
                    generation_provider_targets,
                );
                let submission_error = readiness_submission.err();
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
                        publication_error: submission_error.as_deref(),
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
        params.validate_schema_identity()?;
        let query_providers = workspace_search_providers_from_provider_register(
            provider_register.as_ref(),
            &schema_bundles,
        )
        .map_err(AspClientOperationError::Message)?;
        selected_playbook_provider_targets(
            params.language.as_deref(),
            params.documents.as_deref(),
            query_providers.as_ref(),
        )?;
        let query_provider_targets = query_providers
        .iter()
        .map(|provider| (provider.language_id.clone(), provider.provider_id.clone()))
        .collect::<Vec<_>>();
        return dispatch_workspace_query_playbook(
            &request,
            params,
            &project_workspace_key,
            &initialized_workspaces,
            generation_admission.as_ref(),
            &workspace_registry,
            &query_provider_targets,
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
    let generation_provider_targets = match &route {
        ServerClientRoute::WorkspaceSearchPlaybook => {
            let params = workspace_search_playbook_params
                .as_ref()
                .expect("workspace Search Playbook request was decoded");
            let providers = workspace_search_providers_from_provider_register(
                provider_register.as_ref(),
                &schema_bundles,
            )
            .map_err(AspClientOperationError::Message)?;
            selected_playbook_provider_targets(
                params.language.as_deref(),
                params.documents.as_deref(),
                providers.as_ref(),
            )?
        }
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
    let generation = {
        match generation_state {
            Some(RuntimeQueryGenerationState::Ready(generation)) => generation,
            Some(RuntimeQueryGenerationState::Failed { reason, .. }) => {
                let readiness_submission = request_runtime_query_generation_ready(
                    generation_admission.as_ref(),
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
    // CompleteGeneration admission is detached lifecycle work.  The request
    // path above only clones an already published immutable resident handle.
    let workspace_search_providers = workspace_search_providers_from_provider_register(
        provider_register.as_ref(),
        &schema_bundles,
    )
    .map_err(AspClientOperationError::Message)?;
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
                language: params.language,
                documents: params.documents,
                workspace: params.workspace,
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
                generation.as_ref(),
                workspace_search_providers.as_ref(),
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
#[path = "../tests/unit/runtime_asp_client_resolved_route.rs"]
mod tests;
