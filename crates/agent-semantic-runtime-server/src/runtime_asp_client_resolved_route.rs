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

use super::service::{ClientWorkspaceKey, InitializedWorkspace};
use super::workspace_search_playbook::execute_progressive_search_clauses;
use super::{
    AspClientExactQueryRequest, AspClientOperationError, AspClientSearchRequest,
    AspClientWorkspaceSearchPlaybookRequest, AspClientWorkspaceSyntaxQueryRequest,
    QueryNotReadyContext, ResidentGraphEvaluationRequestV1, ServerClientRoute,
    dispatch_exact_query, dispatch_search_route, dispatch_source_index_lookup,
    dispatch_workspace_syntax_query, elapsed_micros, query_generation_not_ready_error,
    record_runtime_route_performance, request_runtime_query_generation_ready,
    revalidate_runtime_query_generation, wait_for_runtime_query_generation,
    wait_for_runtime_query_generation_change, workspace_search_providers_from_provider_register,
};

pub(super) struct ResolvedRouteContext {
    pub(super) request: AspClientDispatchRequest,
    pub(super) project_workspace_key: RuntimeProjectWorkspaceKey,
    pub(super) initialized_workspaces:
        Arc<Mutex<HashMap<ClientWorkspaceKey, InitializedWorkspace>>>,
    pub(super) runtime_search_service: RuntimeSearchServiceHandle,
    pub(super) generation_admission: Arc<WorkspaceGenerationAdmission>,
    pub(super) workspace_registry: Arc<RuntimeServerWorkspaceRegistry>,
    pub(super) installed_provider_targets: Arc<[(String, String)]>,
    pub(super) provider_register: Arc<RuntimeProviderRegister>,
    pub(super) query_generation_authority: RuntimeQueryGenerationAuthority,
    pub(super) query_generation: tokio::sync::watch::Receiver<
        Arc<HashMap<RuntimeProjectWorkspaceKey, RuntimeQueryGenerationState>>,
    >,
    pub(super) telemetry_sender: RuntimeTelemetryBusSender,
}

fn selected_provider_targets(
    languages: Option<&str>,
    installed_provider_targets: &[(String, String)],
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
            let provider_id = installed_provider_targets
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
        installed_provider_targets,
        provider_register,
        query_generation_authority,
        mut query_generation,
        telemetry_sender,
    } = context;

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
        let validated_params = ResidentGraphEvaluationRequestV1::from_value(request.params)
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
    if let Some(params) = workspace_search_playbook_params
        .as_ref()
        .filter(|params| params.clause_order.is_none())
    {
        let receipt = agent_semantic_search::resolve_provider_search_playbook_contract(
            workspace_search_providers.iter(),
            params.languages.as_deref(),
            params.documents.as_deref(),
        );
        return serde_json::to_value(receipt)
            .map_err(|error| AspClientOperationError::Message(error.to_string()));
    }
    let generation_provider_targets = match &route {
        ServerClientRoute::WorkspaceSearchPlaybook => selected_provider_targets(
            workspace_search_playbook_params
                .as_ref()
                .and_then(|params| params.languages.as_deref()),
            installed_provider_targets.as_ref(),
        )?,
        ServerClientRoute::WorkspaceSyntaxQuery => selected_provider_targets(
            workspace_syntax_query_params
                .as_ref()
                .and_then(|params| params.languages.as_deref()),
            installed_provider_targets.as_ref(),
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
            let plan_request = agent_semantic_search::ProgressiveSearchPlaybookRequest::Execute {
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
                    .expect("validated Search Playbook clause order")
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
                            agent_semantic_client_protocol::AspClientSearchPlaybookClauseAxis::Graph => {
                                agent_semantic_search::SearchPlaybookClauseAxis::Graph
                            }
                        },
                        block_index: clause.block_index,
                    })
                    .collect(),
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
            let evidence = execute_progressive_search_clauses(
                &plan,
                &project_root,
                generation.as_ref(),
                workspace_search_providers.as_ref(),
                &runtime_search_service,
            )
            .await?;
            let graph_fan_in = if evidence.graph_query_clauses.is_empty() {
                None
            } else {
                const GRAPH_CANDIDATE_FRONTIER_LIMIT: usize = 4096;
                let mut seen = std::collections::BTreeSet::new();
                let mut candidate_owners = Vec::new();
                let mut candidate_frontier_truncated = false;
                'clauses: for receipt in &evidence.clause_receipts {
                    for owner in &receipt.candidate_owners {
                        if seen.insert(owner.as_str()) {
                            if candidate_owners.len() == GRAPH_CANDIDATE_FRONTIER_LIMIT {
                                candidate_frontier_truncated = true;
                                break 'clauses;
                            }
                            candidate_owners.push(owner.clone());
                        }
                    }
                }
                let graph = crate::runtime_search_graph::evaluate_python_workspace_playbook_graph(
                    request.request_id.as_str(),
                    &language_id,
                    &evidence.graph_query_clauses,
                    &candidate_owners,
                    30,
                    generation.resident(),
                    &runtime_search_service,
                )
                .await
                .map_err(|error| {
                    AspClientOperationError::Terminal(AspClientDispatchError {
                        reason_kind: error.reason_kind.to_owned(),
                        message: error.message,
                        details: error.details,
                    })
                })?;
                let truncated =
                    candidate_frontier_truncated || graph.candidate_owner_ids.len() == 30;
                Some(agent_semantic_search::WorkspaceSearchGraphFanIn {
                    ranked_candidate_owners: graph.candidate_owner_ids,
                    applied_clause_count: evidence.graph_query_clauses.len(),
                    complete: true,
                    truncated,
                })
            };
            let result = agent_semantic_search::synthesize_workspace_search_playbook_result(
                evidence.clause_receipts,
                evidence.syntax_candidates,
                graph_fan_in,
            )?;
            serde_json::to_value(result)
                .map_err(|error| AspClientOperationError::Message(error.to_string()))
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
