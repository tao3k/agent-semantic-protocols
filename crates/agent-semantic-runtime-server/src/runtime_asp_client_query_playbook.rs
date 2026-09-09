// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Runtime-bound Query Playbook materialization and telemetry settlement.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use agent_semantic_client_db::runtime_server_workspace::RuntimeServerWorkspaceRegistry;
use agent_semantic_client_db::runtime_telemetry_bus::RuntimeTelemetryBusSender;
use agent_semantic_client_server::{AspClientDispatchError, AspClientDispatchRequest};

use crate::RuntimeQueryGenerationState;
use crate::runtime_query_generation_key::RuntimeProjectWorkspaceKey;

use super::service::{ClientRequestKey, ClientWorkspaceKey, InitializedWorkspace};
use super::{
    AspClientOperationError, AspClientWorkspaceQueryPlaybookRequest,
    RUNTIME_CLIENT_DISPATCH_BUDGET, elapsed_micros, request_runtime_query_generation_ready,
};

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

fn query_playbook_generation_provider_targets(
    selectors: &[String],
    active_provider_targets: &[(String, String)],
) -> Vec<agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationProviderTarget> {
    let languages = selectors
        .iter()
        .filter_map(|selector| selector.split_once("://").map(|(language, _)| language))
        .collect::<std::collections::BTreeSet<_>>();
    active_provider_targets
        .iter()
        .filter(|(language, _)| languages.contains(language.as_str()))
        .map(|(language_id, provider_id)| {
            agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationProviderTarget {
                language_id: language_id.clone(),
                provider_id: Some(provider_id.clone()),
            }
        })
        .collect()
}

pub(super) fn record_settled_client_timing_observations(
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

pub(super) fn emit_runtime_search_trace_observation(
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

pub(super) fn runtime_search_trace_budget_micros() -> u64 {
    RUNTIME_CLIENT_DISPATCH_BUDGET
        .as_micros()
        .min(u128::from(u64::MAX)) as u64
}

pub(super) fn insert_runtime_search_trace(
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
    let mut admitted_selectors = Vec::with_capacity(params.selectors.len());
    for selector in &params.selectors {
        let language_id = selector.split_once("://").map(|(language, _)| language);
        let provider_id = language_id.and_then(|language_id| {
            active_provider_targets
                .iter()
                .find_map(|(language, provider)| {
                    (language == language_id).then_some(provider.as_str())
                })
        });
        let owner_path = selector_owner_path(selector);
        match (language_id, provider_id, owner_path) {
            (Some(language_id), Some(provider_id), Some(owner_path)) => {
                admitted_selectors.push((selector, language_id, provider_id, owner_path));
            }
            _ => {
                failure_reason = Some("query-playbook-selector-not-materialized");
                break;
            }
        }
    }
    if failure_reason.is_none() {
        for (selector, language_id, provider_id, owner_path) in admitted_selectors {
            let read = match read_selector(projection, selector) {
                Ok(read) => read,
                Err(_) => {
                    failure_reason = Some("query-playbook-selector-read-failed");
                    break;
                }
            };
            match read {
                agent_semantic_client_db::runtime_server_workspace::WorkspaceRuntimeSelectorRead::Projection {
                    resolved_selector,
                    bytes,
                    ..
                } if resolved_selector == *selector => {
                    materializations.push(serde_json::json!({
                        "selector": selector,
                        "languageId": language_id,
                        "providerId": provider_id,
                        "ownerPath": owner_path,
                        "projection": params.projection,
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

pub(super) async fn dispatch_workspace_query_playbook(
    request: &AspClientDispatchRequest,
    params: AspClientWorkspaceQueryPlaybookRequest,
    project_workspace_key: &RuntimeProjectWorkspaceKey,
    initialized_workspaces: &Arc<Mutex<HashMap<ClientWorkspaceKey, InitializedWorkspace>>>,
    generation_admission: &agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationAdmission,
    workspace_registry: &Arc<RuntimeServerWorkspaceRegistry>,
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
    let generation = match query_generation
        .borrow()
        .get(project_workspace_key)
        .cloned()
    {
        Some(RuntimeQueryGenerationState::Ready(generation)) => generation,
        state => {
            let provider_targets = query_playbook_generation_provider_targets(
                &params.selectors,
                active_provider_targets,
            );
            let submission = request_runtime_query_generation_ready(
                generation_admission,
                request.workspace_id.as_str().to_owned(),
                initialized.project_root.clone(),
                provider_targets,
            );
            let state_name = if submission.is_err() {
                "submission-failed"
            } else {
                "building"
            };
            let prior_failure = match state {
                Some(RuntimeQueryGenerationState::Failed { reason, .. }) => {
                    Some(reason.to_string())
                }
                _ => None,
            };
            let cause = submission
                .err()
                .or(prior_failure)
                .map_or_else(String::new, |error| format!(" cause={error}"));
            return Err(query_playbook_terminal(
                "query-playbook-runtime-binding-unavailable",
                format!(
                    "Query Playbook requires an already resident source/Runtime execution product: state={state_name}{cause}"
                ),
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
        |projection, selector| generation.read_runtime_selector(projection, selector),
    )
}

#[cfg(test)]
#[path = "../tests/unit/runtime_asp_client_query_playbook.rs"]
mod tests;
