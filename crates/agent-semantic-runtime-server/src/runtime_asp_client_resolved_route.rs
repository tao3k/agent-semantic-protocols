// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Resolved graph, Search, and Query dispatch after lifecycle-owned special routes.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationAdmission;
use agent_semantic_client_db::runtime_server_workspace::RuntimeServerWorkspaceRegistry;
use agent_semantic_client_db::runtime_telemetry_bus::RuntimeTelemetryBusSender;
use agent_semantic_client_server::{AspClientDispatchError, AspClientDispatchRequest};

use super::query_generation_support::{RequestDispatchBudget, await_runtime_query_generation};
use crate::RuntimeQueryGenerationState;
use crate::runtime_query_generation_key::RuntimeProjectWorkspaceKey;

use super::service::{ClientRequestKey, ClientWorkspaceKey, InitializedWorkspace};
use super::workspace_search_playbook::execute_progressive_search_clauses;
use super::workspace_search_playbook::synthesize_progressive_search_projection;
use super::{
    AspClientExactQueryRequest, AspClientOperationError, AspClientWorkspaceQueryPlaybookRequest,
    AspClientWorkspaceSearchPlaybookRequest, AspClientWorkspaceSyntaxPlanContextRequest,
    AspClientWorkspaceSyntaxQueryRequest, QueryNotReadyContext, ResidentGraphEvaluationRequestV1,
    ServerClientRoute, dispatch_exact_query, dispatch_source_index_lookup,
    dispatch_workspace_syntax_plan_context, dispatch_workspace_syntax_query, elapsed_micros,
    query_generation_not_ready_error, record_runtime_route_performance,
    request_runtime_query_generation_ready,
};

pub(super) struct ResolvedRouteContext {
    pub(super) dispatch_budget: RequestDispatchBudget,
    pub(super) request: AspClientDispatchRequest,
    pub(super) project_workspace_key: RuntimeProjectWorkspaceKey,
    pub(super) initialized_workspaces:
        Arc<Mutex<HashMap<ClientWorkspaceKey, InitializedWorkspace>>>,
    pub(super) generation_admission: Arc<WorkspaceGenerationAdmission>,
    pub(super) workspace_registry: Arc<RuntimeServerWorkspaceRegistry>,
    pub(super) runtime_search_service:
        agent_semantic_client_db::runtime_search_service::RuntimeSearchServiceHandle,
    pub(super) owner_materializer: super::owner_materialization::RuntimeOwnerMaterializer,
    pub(super) workspace_store_root: std::path::PathBuf,
    pub(super) active_provider_targets: Arc<[(String, String)]>,
    pub(super) workspace_search_providers: Arc<[agent_semantic_search::WorkspaceSearchProvider]>,
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
        .chain(
            documents
                .into_iter()
                .flat_map(|expression| expression.split('|'))
                .map(|producer| {
                    (
                        producer.trim(),
                        agent_semantic_search::WorkspaceSearchProducerAxis::Document,
                    )
                }),
        )
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

use super::query_playbook::dispatch_workspace_query_playbook;

pub(super) async fn dispatch_resolved_route(
    context: ResolvedRouteContext,
) -> Result<serde_json::Value, AspClientOperationError> {
    let ResolvedRouteContext {
        dispatch_budget,
        request,
        project_workspace_key,
        initialized_workspaces,
        generation_admission,
        workspace_registry,
        runtime_search_service,
        owner_materializer,
        workspace_store_root,
        active_provider_targets,
        workspace_search_providers,
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
        if validated_params.as_value()["queryTerms"]
            .as_array()
            .is_some_and(|terms| !terms.is_empty())
        {
            generation
                .await_lexical_attachment()
                .await
                .map_err(AspClientOperationError::Message)?;
        }
        let map_graph_failure = |error: crate::runtime_search_graph::RuntimeSearchGraphFailure| {
            AspClientOperationError::Terminal(AspClientDispatchError {
                reason_kind: error.reason_kind.to_owned(),
                message: error.message,
                details: error.details,
            })
        };
        let owner_scope = crate::runtime_search_graph::resident_graph_candidate_owner_scope(
            language_id,
            provider_id,
            generation.resident(),
            validated_params.as_value(),
        )
        .map_err(map_graph_failure)?;
        let mut resident = owner_materializer
            .ensure_candidates(
                request.request_id.as_str(),
                request.workspace_id.as_str(),
                &project_root,
                &workspace_store_root,
                generation.generation_digest(),
                &owner_scope,
                workspace_search_providers.as_ref(),
                &runtime_search_service,
                &workspace_registry,
            )
            .await?;
        let graph_node_limit = usize::try_from(
            validated_params.as_value()["budget"]["maxNodes"]
                .as_u64()
                .expect("validated maxNodes"),
        )
        .map_err(|_| AspClientOperationError::Message("maxNodes exceeds usize".to_owned()))?;
        let topology_scope = super::workspace_search_playbook::relation_neighbor_scope(
            &resident,
            &owner_scope,
            graph_node_limit.max(owner_scope.len()),
        )?;
        if topology_scope != owner_scope {
            resident = owner_materializer
                .ensure_candidates(
                    request.request_id.as_str(),
                    request.workspace_id.as_str(),
                    &project_root,
                    &workspace_store_root,
                    generation.generation_digest(),
                    &topology_scope,
                    workspace_search_providers.as_ref(),
                    &runtime_search_service,
                    &workspace_registry,
                )
                .await?;
        }
        let graph_generation = resident
            .build_graph_generation_for_owner_scope(&topology_scope)
            .map_err(AspClientOperationError::Message)?;
        let result = crate::runtime_search_graph::evaluate_resident_search_graph(
            request.request_id.as_str(),
            request.workspace_id.as_str(),
            language_id,
            provider_id,
            generation.generation_digest(),
            &resident,
            &graph_generation,
            validated_params.as_value(),
        )
        .map_err(map_graph_failure)?;
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
        return dispatch_workspace_query_playbook(
            &dispatch_budget,
            &request,
            params,
            &project_workspace_key,
            &initialized_workspaces,
            generation_admission.as_ref(),
            &workspace_registry,
            &runtime_search_service,
            &owner_materializer,
            &workspace_store_root,
            workspace_search_providers.as_ref(),
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
        agent_semantic_client_protocol::ResolvedServerClientMethod::Server(
            ServerClientRoute::WorkspaceSyntaxPlanContext,
        ) => (
            "workspace".to_owned(),
            "workspace".to_owned(),
            ServerClientRoute::WorkspaceSyntaxPlanContext,
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
    let workspace_syntax_plan_context_params =
        if matches!(&route, ServerClientRoute::WorkspaceSyntaxPlanContext) {
            let params: AspClientWorkspaceSyntaxPlanContextRequest =
                serde_json::from_value(request.params.clone())
                    .map_err(|error| error.to_string())?;
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
            selected_playbook_provider_targets(
                params.language.as_deref(),
                params.documents.as_deref(),
                workspace_search_providers.as_ref(),
            )?
        }
        ServerClientRoute::WorkspaceSyntaxQuery => selected_provider_targets(
            workspace_syntax_query_params
                .as_ref()
                .and_then(|params| params.languages.as_deref()),
            active_provider_targets.as_ref(),
        )?,
        ServerClientRoute::WorkspaceSyntaxPlanContext => selected_provider_targets(
            workspace_syntax_plan_context_params
                .as_ref()
                .map(|params| params.producer.as_str()),
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
    let generation_result = match generation_state {
        Some(RuntimeQueryGenerationState::Ready(generation)) => Ok(generation),
        Some(RuntimeQueryGenerationState::Failed { reason, .. }) => Err(reason.to_string()),
        None => {
            dispatch_budget.observe_miss();
            let wait_started = tokio::time::Instant::now();
            let result = match request_runtime_query_generation_ready(
                generation_admission.as_ref(),
                request.workspace_id.as_str().to_owned(),
                project_root.clone(),
                generation_provider_targets,
            ) {
                Ok(_) => {
                    await_runtime_query_generation(&query_generation, &project_workspace_key).await
                }
                Err(error) => Err(error),
            };
            eprintln!(
                "[runtime-generation-wait] requestId={} elapsedMicros={}",
                request.request_id.as_str(),
                wait_started.elapsed().as_micros()
            );
            result
        }
    };
    let generation = match generation_result {
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
                    generation_state: "failed",
                    publication_error: Some(&error),
                    elapsed_micros: elapsed_micros(dispatch_started),
                })?,
            ));
        }
    };
    // Only the daemon publishes authority; the request owns a cancellable wait.
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
            let materialization_key =
                workspace_search_materialization_key(&params, generation.generation_digest())?;
            match generation.search_materialization(&materialization_key)? {
                Some(
                    crate::runtime_query_generation::RuntimeSearchMaterializationState::Ready(
                        result,
                    ),
                ) => {
                    dispatch_budget.observe_resident_hit();
                    Ok(result.as_ref().clone())
                }
                Some(
                    crate::runtime_query_generation::RuntimeSearchMaterializationState::Failed(
                        error,
                    ),
                ) => {
                    dispatch_budget.observe_resident_hit();
                    Err(AspClientOperationError::Terminal(error.as_ref().clone()))
                }
                Some(
                    crate::runtime_query_generation::RuntimeSearchMaterializationState::Building(_),
                ) => {
                    dispatch_budget.observe_miss();
                    settled_search_materialization(&generation, &materialization_key).await
                }
                None => {
                    dispatch_budget.observe_miss();
                    if generation.begin_search_materialization(materialization_key.clone())? {
                        let materialization_generation = Arc::clone(&generation);
                        let materialization_providers = Arc::clone(&workspace_search_providers);
                        let materialization_project_root = project_root.clone();
                        let materialization_parser_artifact_root = workspace_store_root.clone();
                        let materialization_workspace_identity =
                            request.workspace_id.as_str().to_owned();
                        let materialization_request_id = request.request_id.as_str().to_owned();
                        let materialization_runtime_search_service = runtime_search_service.clone();
                        let materialization_owner_materializer = owner_materializer.clone();
                        let materialization_workspace_registry = Arc::clone(&workspace_registry);
                        let materialization_key_for_task = materialization_key.clone();
                        let materialization_binding =
                            agent_semantic_search::WorkspaceSearchPlanBinding {
                                project_id: request.project_id.as_str().to_owned(),
                                workspace_id: request.workspace_id.as_str().to_owned(),
                                content_generation_digest: generation
                                    .content_generation_digest()
                                    .to_owned(),
                            };
                        tokio::spawn(async move {
                            let compute_started = std::time::Instant::now();
                            let result = match build_workspace_search_materialization_plan(
                                params,
                                materialization_binding,
                                materialization_providers.iter().cloned(),
                            ) {
                                Ok(plan) => {
                                    materialize_workspace_search(
                                        &materialization_key_for_task,
                                        &materialization_request_id,
                                        &materialization_workspace_identity,
                                        plan,
                                        materialization_project_root,
                                        materialization_parser_artifact_root,
                                        Arc::clone(&materialization_generation),
                                        materialization_providers,
                                        materialization_runtime_search_service,
                                        materialization_owner_materializer,
                                        materialization_workspace_registry,
                                    )
                                    .await
                                }
                                Err(error) => Err(error),
                            }
                            .map_err(search_materialization_dispatch_error);
                            eprintln!(
                                "[runtime-search-materialization-wall] key={} requestWallMicros={} includesProviderLifecycle=true state={}",
                                materialization_key_for_task,
                                compute_started.elapsed().as_micros(),
                                if result.is_ok() { "ready" } else { "failed" }
                            );
                            let _ = materialization_generation.publish_search_materialization(
                                materialization_key_for_task,
                                result,
                            );
                        });
                    }
                    settled_search_materialization(&generation, &materialization_key).await
                }
            }
        }
        agent_semantic_client_protocol::ServerClientRoute::WorkspaceQueryPlaybook => {
            unreachable!("workspace Query Playbook is fail-closed before generation admission")
        }
        agent_semantic_client_protocol::ServerClientRoute::WorkspaceSyntaxQuery => {
            let syntax_params = workspace_syntax_query_params
                .as_ref()
                .expect("workspace syntax Query decoded its request");
            let selected_languages = syntax_params
                .languages
                .iter()
                .flat_map(|languages| languages.split('|'))
                .chain(
                    syntax_params
                        .documents
                        .iter()
                        .flat_map(|documents| documents.split('|')),
                )
                .collect::<std::collections::BTreeSet<_>>();
            let selected_extensions = workspace_search_providers
                .iter()
                .filter(|provider| {
                    selected_languages.is_empty()
                        || selected_languages.contains(provider.language_id.as_str())
                })
                .flat_map(|provider| provider.source_extensions.iter().map(String::as_str))
                .collect::<std::collections::BTreeSet<_>>();
            let owner_scope = generation
                .resident()
                .indexed_owner_paths()
                .into_iter()
                .filter(|owner| {
                    std::path::Path::new(owner)
                        .extension()
                        .and_then(|extension| extension.to_str())
                        .is_some_and(|extension| selected_extensions.contains(extension))
                })
                .collect::<std::collections::BTreeSet<_>>();
            let resident = owner_materializer
                .ensure_candidates(
                    request.request_id.as_str(),
                    request.workspace_id.as_str(),
                    &project_root,
                    &workspace_store_root,
                    generation.generation_digest(),
                    &owner_scope,
                    workspace_search_providers.as_ref(),
                    &runtime_search_service,
                    &workspace_registry,
                )
                .await?;
            dispatch_workspace_syntax_query(
                workspace_syntax_query_params.expect("workspace syntax Query decoded its request"),
                generation.generation_digest(),
                &resident,
                workspace_search_providers.as_ref(),
            )
            .await
        }
        agent_semantic_client_protocol::ServerClientRoute::WorkspaceSyntaxPlanContext => {
            dispatch_workspace_syntax_plan_context(
                workspace_syntax_plan_context_params
                    .expect("workspace syntax plan context decoded its request"),
                generation.as_ref(),
                workspace_search_providers.as_ref(),
            )
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
            let selector = exact_query_params
                .as_ref()
                .expect("exact Query decoded its request")
                .selector
                .as_str();
            let owner_path = agent_semantic_content_identity::CanonicalItemSelector::parse_root_or_exact_descendant(selector)
                .and_then(|selector| selector.owner_path())
                .map_err(AspClientOperationError::Message)?;
            let owner_scope = std::collections::BTreeSet::from([owner_path]);
            let resident = owner_materializer
                .ensure_candidates(
                    request.request_id.as_str(),
                    request.workspace_id.as_str(),
                    &project_root,
                    &workspace_store_root,
                    generation.generation_digest(),
                    &owner_scope,
                    workspace_search_providers.as_ref(),
                    &runtime_search_service,
                    &workspace_registry,
                )
                .await?;
            dispatch_exact_query(
                request.project_id.as_str(),
                request.workspace_id.as_str(),
                request.request_id.as_str(),
                params,
                &language_id,
                &provider_id,
                generation.as_ref(),
                &resident,
                &telemetry_sender,
            )
        }
    }
}

fn build_workspace_search_materialization_plan(
    params: AspClientWorkspaceSearchPlaybookRequest,
    binding: agent_semantic_search::WorkspaceSearchPlanBinding,
    providers: impl IntoIterator<Item = agent_semantic_search::WorkspaceSearchProvider>,
) -> Result<agent_semantic_search::WorkspaceSearchPlaybookPlan, AspClientOperationError> {
    let request = agent_semantic_search::NormalizedWorkspaceSearchPlaybookRequest {
        language: params.language,
        documents: params.documents,
        workspace: params.workspace,
        rg: params.rg.unwrap_or_default(),
        tantivy: params.tantivy.unwrap_or_default(),
        syntax: params.syntax.unwrap_or_default(),
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
    agent_semantic_search::build_workspace_search_playbook_plan(&request, binding, providers)
        .map_err(AspClientOperationError::Message)
}

pub(super) fn workspace_search_materialization_key(
    request: &AspClientWorkspaceSearchPlaybookRequest,
    generation_digest: &str,
) -> Result<String, AspClientOperationError> {
    let request_value = serde_json::to_value(request).map_err(|error| {
        AspClientOperationError::Message(format!(
            "encode normalized Search materialization identity: {error}"
        ))
    })?;
    let request_bytes = agent_semantic_client_protocol::canonical_json_bytes(&request_value)
        .map_err(|error| {
            AspClientOperationError::Message(format!(
                "canonicalize normalized Search materialization identity: {error}"
            ))
        })?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"agent.semantic-protocols.workspace-search-materialization.v1\0");
    hasher.update(generation_digest.as_bytes());
    hasher.update(b"\0");
    hasher.update(&request_bytes);
    Ok(format!("blake3-256:{}", hasher.finalize().to_hex()))
}

#[expect(
    clippy::too_many_arguments,
    reason = "Search materialization keeps route, generation, deadline, and telemetry authorities explicit"
)]
async fn materialize_workspace_search(
    materialization_key: &str,
    request_id: &str,
    workspace_identity: &str,
    plan: agent_semantic_search::WorkspaceSearchPlaybookPlan,
    project_root: std::path::PathBuf,
    parser_artifact_root: std::path::PathBuf,
    generation: Arc<crate::RuntimeQueryGeneration>,
    providers: Arc<[agent_semantic_search::WorkspaceSearchProvider]>,
    runtime_search_service: agent_semantic_client_db::runtime_search_service::RuntimeSearchServiceHandle,
    owner_materializer: super::owner_materialization::RuntimeOwnerMaterializer,
    workspace_registry: Arc<RuntimeServerWorkspaceRegistry>,
) -> Result<serde_json::Value, AspClientOperationError> {
    let evidence = execute_progressive_search_clauses(
        &plan,
        &project_root,
        Arc::clone(&generation),
        providers.as_ref(),
        request_id,
        workspace_identity,
        &parser_artifact_root,
        &runtime_search_service,
        &owner_materializer,
        &workspace_registry,
    )
    .await?;
    let projection = synthesize_progressive_search_projection(
        materialization_key,
        "workspace",
        evidence,
        generation.as_ref(),
        &project_root,
    )
    .await?;
    Ok(projection.result)
}

fn search_materialization_dispatch_error(error: AspClientOperationError) -> AspClientDispatchError {
    match error {
        AspClientOperationError::Terminal(error) => error,
        AspClientOperationError::Message(message) => AspClientDispatchError {
            reason_kind: "search-materialization-failed".to_owned(),
            message,
            details: Some(serde_json::json!({
                "schemaId": "agent.semantic-protocols.asp-client-dispatch-failure",
                "schemaVersion": "1",
                "state": "failed",
                "phase": "runtime-search-materialization",
                "reasonKind": "search-materialization-failed"
            })),
        },
    }
}

async fn settled_search_materialization(
    generation: &crate::runtime_query_generation::RuntimeQueryGeneration,
    key: &str,
) -> Result<serde_json::Value, AspClientOperationError> {
    use crate::runtime_query_generation::RuntimeSearchMaterializationState;
    match generation.await_search_materialization(key).await? {
        RuntimeSearchMaterializationState::Ready(result) => Ok(result.as_ref().clone()),
        RuntimeSearchMaterializationState::Failed(error) => {
            Err(AspClientOperationError::Terminal(error.as_ref().clone()))
        }
        RuntimeSearchMaterializationState::Building(_) => Err(AspClientOperationError::Message(
            "Search completion preceded terminal publication".to_owned(),
        )),
    }
}

#[cfg(test)]
#[path = "../tests/unit/runtime_asp_client_resolved_route.rs"]
mod tests;
