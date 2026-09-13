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

fn query_playbook_generation_provider_targets(
    selectors: &[String],
    active_provider_targets: &[(String, String)],
) -> Result<
    Vec<agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationProviderTarget>,
    AspClientOperationError,
> {
    let languages = selectors
        .iter()
        .filter_map(|selector| selector.split_once("://").map(|(language, _)| language))
        .collect::<std::collections::BTreeSet<_>>();
    let targets = active_provider_targets
        .iter()
        .filter(|(language, _)| languages.contains(language.as_str()))
        .map(|(language_id, provider_id)| {
            agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationProviderTarget {
                language_id: language_id.clone(),
                provider_id: Some(provider_id.clone()),
            }
        })
        .collect::<Vec<_>>();
    if targets.len() != languages.len() {
        let installed = targets
            .iter()
            .map(|target| target.language_id.as_str())
            .collect::<std::collections::BTreeSet<_>>();
        let missing = languages
            .into_iter()
            .filter(|language| !installed.contains(language))
            .collect::<Vec<_>>()
            .join("|");
        return Err(query_playbook_terminal(
            "query-playbook-provider-not-installed",
            format!("Query Playbook provider is not installed: languageId={missing}"),
            selectors.len(),
        ));
    }
    Ok(targets)
}

async fn admit_cold_query_owner_paths(
    project_root: &std::path::Path,
    selectors: &[String],
) -> Result<(), AspClientOperationError> {
    let canonical_root = tokio::fs::canonicalize(project_root)
        .await
        .map_err(|error| {
            query_playbook_terminal(
                "query-playbook-workspace-root-unavailable",
                format!(
                    "resolve Query Playbook workspace root {}: {error}",
                    project_root.display()
                ),
                selectors.len(),
            )
        })?;
    for selector in selectors {
        let Some(owner_path) = selector_owner_path(selector) else {
            return Err(query_playbook_terminal(
                "query-playbook-selector-invalid",
                format!("Query Playbook selector has no exact owner path: {selector}"),
                selectors.len(),
            ));
        };
        let relative = std::path::Path::new(owner_path);
        if relative.is_absolute()
            || relative.components().any(|component| {
                matches!(
                    component,
                    std::path::Component::ParentDir
                        | std::path::Component::RootDir
                        | std::path::Component::Prefix(_)
                )
            })
        {
            return Err(query_playbook_terminal(
                "query-playbook-owner-path-invalid",
                format!("Query Playbook owner path escapes its workspace: {owner_path}"),
                selectors.len(),
            ));
        }
        let path = project_root.join(relative);
        let canonical_path = match tokio::fs::canonicalize(&path).await {
            Ok(path) if path.starts_with(&canonical_root) => path,
            Ok(_) => {
                return Err(query_playbook_terminal(
                    "query-playbook-owner-path-invalid",
                    format!("Query Playbook owner path escapes its workspace: {owner_path}"),
                    selectors.len(),
                ));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Err(query_playbook_terminal(
                    "query-playbook-owner-missing",
                    format!("Query Playbook owner path does not exist: {owner_path}"),
                    selectors.len(),
                ));
            }
            Err(error) => {
                return Err(query_playbook_terminal(
                    "query-playbook-owner-metadata-failed",
                    format!("resolve Query Playbook owner path {owner_path}: {error}"),
                    selectors.len(),
                ));
            }
        };
        match tokio::fs::metadata(&canonical_path).await {
            Ok(metadata) if metadata.is_file() => {}
            Ok(_) => {
                return Err(query_playbook_terminal(
                    "query-playbook-owner-not-file",
                    format!("Query Playbook owner path is not a file: {owner_path}"),
                    selectors.len(),
                ));
            }
            Err(error) => {
                return Err(query_playbook_terminal(
                    "query-playbook-owner-metadata-failed",
                    format!("inspect Query Playbook owner path {owner_path}: {error}"),
                    selectors.len(),
                ));
            }
        }
    }
    Ok(())
}

#[expect(
    clippy::too_many_arguments,
    reason = "settled timing records preserve each V1 phase measurement explicitly"
)]
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

fn schedule_settled_query_client_timing_observations(
    request: &AspClientDispatchRequest,
    params: &AspClientWorkspaceQueryPlaybookRequest,
    generation: Arc<crate::RuntimeQueryGeneration>,
    active_provider_targets: &[(String, String)],
    telemetry_sender: &RuntimeTelemetryBusSender,
) {
    let Some(witness) = request.client_timing_witness.clone() else {
        return;
    };
    let session_id = request.session_id.as_str().to_owned();
    let request_id = request.request_id.as_str().to_owned();
    let selectors = params.selectors.clone();
    let projection = params.projection.clone();
    let provider_targets = active_provider_targets.to_vec();
    let telemetry_sender = telemetry_sender.clone();
    tokio::spawn(async move {
        let mut languages = std::collections::BTreeSet::new();
        let mut providers = std::collections::BTreeSet::new();
        for selector in selectors {
            let Some((language, _)) = selector.split_once("://") else {
                return;
            };
            let Some(provider) = provider_targets
                .iter()
                .find_map(|(candidate, provider)| (candidate == language).then_some(provider))
            else {
                return;
            };
            languages.insert(language.to_owned());
            providers.insert(provider.clone());
        }
        let Some(publication) = generation.execution_publication() else {
            return;
        };
        let _ = record_settled_client_timing_observations(
            publication,
            &witness,
            &session_id,
            &request_id,
            languages.into_iter().collect(),
            providers.into_iter().collect(),
            Some(&projection),
            &telemetry_sender,
        );
    });
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

#[expect(
    clippy::too_many_arguments,
    reason = "the V1 Query receipt binds each identity and measured phase explicitly"
)]
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

fn workspace_query_materialization_key(
    params: &AspClientWorkspaceQueryPlaybookRequest,
    generation_digest: &str,
) -> Result<String, AspClientOperationError> {
    let value = serde_json::to_value(params).map_err(|error| {
        AspClientOperationError::Message(format!(
            "encode normalized Query materialization identity: {error}"
        ))
    })?;
    let canonical =
        agent_semantic_client_protocol::canonical_json_bytes(&value).map_err(|error| {
            AspClientOperationError::Message(format!(
                "canonicalize normalized Query materialization identity: {error}"
            ))
        })?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"agent.semantic-protocols.workspace-query-materialization.v1\0");
    hasher.update(generation_digest.as_bytes());
    hasher.update(b"\0");
    hasher.update(&canonical);
    Ok(format!("blake3-256:{}", hasher.finalize().to_hex()))
}

fn query_materialization_dispatch_error(error: AspClientOperationError) -> AspClientDispatchError {
    match error {
        AspClientOperationError::Terminal(error) => error,
        AspClientOperationError::Message(message) => AspClientDispatchError {
            reason_kind: "query-materialization-failed".to_owned(),
            message,
            details: Some(serde_json::json!({
                "schemaId": "agent.semantic-protocols.asp-client-dispatch-failure",
                "schemaVersion": "1",
                "state": "failed",
                "phase": "runtime-query-materialization",
                "reasonKind": "query-materialization-failed"
            })),
        },
    }
}

fn bind_query_materialization_to_request(
    template: &serde_json::Value,
    request_id: &str,
) -> Result<serde_json::Value, AspClientOperationError> {
    let mut result = template.clone();
    result
        .as_object_mut()
        .ok_or_else(|| {
            AspClientOperationError::Message(
                "resident Query materialization is not an object".to_owned(),
            )
        })?
        .insert(
            "requestId".to_owned(),
            serde_json::Value::String(request_id.to_owned()),
        );
    Ok(result)
}

fn materialize_generation_bound_query_playbook(
    materialization_key: &str,
    workspace_id: &str,
    params: &AspClientWorkspaceQueryPlaybookRequest,
    initialized: &InitializedWorkspace,
    workspace_registry: &RuntimeServerWorkspaceRegistry,
    active_provider_targets: &[(String, String)],
    generation: &crate::RuntimeQueryGeneration,
) -> Result<serde_json::Value, AspClientOperationError> {
    let (resident_root, resident_generation_digest) = workspace_registry
        .unique_resident_scope(workspace_id)
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
    let resident = workspace_registry
        .resident_read_client(workspace_id, &initialized.project_root)
        .map_err(|error| {
            query_playbook_terminal(
                "query-playbook-runtime-binding-unavailable",
                error,
                params.selectors.len(),
            )
        })?;
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
    materialize_query_playbook_receipt(
        materialization_key,
        params,
        runtime_binding,
        execution_publication.publication_digest.as_str(),
        execution_publication.runtime_bundle_digest.as_str(),
        initialized.host_workspace.project_workspace(),
        active_provider_targets,
        None,
        |projection, selector| resident.read_runtime_selector(projection, selector),
    )
}

#[expect(
    clippy::too_many_arguments,
    reason = "Query dispatch keeps route, generation, deadline, and telemetry authorities explicit"
)]
pub(super) async fn dispatch_workspace_query_playbook(
    dispatch_budget: &super::query_generation_support::RequestDispatchBudget,
    request: &AspClientDispatchRequest,
    params: AspClientWorkspaceQueryPlaybookRequest,
    project_workspace_key: &RuntimeProjectWorkspaceKey,
    initialized_workspaces: &Arc<Mutex<HashMap<ClientWorkspaceKey, InitializedWorkspace>>>,
    generation_admission: &agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationAdmission,
    workspace_registry: &Arc<RuntimeServerWorkspaceRegistry>,
    runtime_search_service: &agent_semantic_client_db::runtime_search_service::RuntimeSearchServiceHandle,
    owner_materializer: &super::owner_materialization::RuntimeOwnerMaterializer,
    parser_artifact_root: &std::path::Path,
    workspace_search_providers: &[agent_semantic_search::WorkspaceSearchProvider],
    active_provider_targets: &[(String, String)],
    telemetry_sender: &RuntimeTelemetryBusSender,
    _telemetry_traces: &Arc<
        Mutex<
            HashMap<
                ClientRequestKey,
                agent_semantic_client_db::runtime_server_opentelemetry::RuntimeSearchTelemetryTrace,
            >,
        >,
    >,
    _active_telemetry_trace_count: &std::sync::atomic::AtomicUsize,
    query_generation: &tokio::sync::watch::Receiver<
        Arc<HashMap<RuntimeProjectWorkspaceKey, RuntimeQueryGenerationState>>,
    >,
) -> Result<serde_json::Value, AspClientOperationError> {
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
    let generation_state = query_generation
        .borrow()
        .get(project_workspace_key)
        .cloned();
    let generation = match generation_state {
        Some(RuntimeQueryGenerationState::Ready(generation)) => generation,
        Some(RuntimeQueryGenerationState::Failed { reason, .. }) => {
            return Err(query_playbook_terminal(
                "query-not-ready",
                reason.to_string(),
                params.selectors.len(),
            ));
        }
        None => {
            dispatch_budget.observe_miss();
            let wait_started = tokio::time::Instant::now();
            let provider_targets = query_playbook_generation_provider_targets(
                &params.selectors,
                active_provider_targets,
            )?;
            // An exact Query has a directly addressable owner.  Reject an
            // impossible cold request before admitting any repository-wide
            // generation work; a ready resident generation remains the sole
            // authority for retained content after source changes.
            admit_cold_query_owner_paths(&initialized.project_root, &params.selectors).await?;
            request_runtime_query_generation_ready(
                generation_admission,
                request.workspace_id.as_str().to_owned(),
                initialized.project_root.clone(),
                provider_targets,
            )
            .map_err(|error| {
                query_playbook_terminal("query-not-ready", error, params.selectors.len())
            })?;
            let generation = super::query_generation_support::await_runtime_query_generation(
                query_generation,
                project_workspace_key,
            )
            .await
            .map_err(|error| {
                query_playbook_terminal("query-not-ready", error, params.selectors.len())
            })?;
            eprintln!(
                "[runtime-generation-wait] requestId={} elapsedMicros={}",
                request.request_id.as_str(),
                wait_started.elapsed().as_micros()
            );
            generation
        }
    };
    let materialization_key =
        workspace_query_materialization_key(&params, generation.generation_digest())?;
    let result = match generation.query_materialization(&materialization_key)? {
        Some(crate::runtime_query_generation::RuntimeQueryMaterializationState::Ready(
            template,
        )) => {
            dispatch_budget.observe_resident_hit();
            let result = bind_query_materialization_to_request(
                template.as_ref(),
                request.request_id.as_str(),
            )?;
            Ok(result)
        }
        Some(crate::runtime_query_generation::RuntimeQueryMaterializationState::Failed(error)) => {
            dispatch_budget.observe_resident_hit();
            Err(AspClientOperationError::Terminal(error.as_ref().clone()))
        }
        Some(crate::runtime_query_generation::RuntimeQueryMaterializationState::Building(_)) => {
            dispatch_budget.observe_miss();
            settled_query_materialization(
                &generation,
                &materialization_key,
                request.request_id.as_str(),
            )
            .await
        }
        None => {
            dispatch_budget.observe_miss();
            if generation.begin_query_materialization(materialization_key.clone())? {
                let materialization_generation = Arc::clone(&generation);
                let materialization_registry = Arc::clone(workspace_registry);
                let materialization_targets = active_provider_targets.to_vec();
                let materialization_workspace_id = request.workspace_id.as_str().to_owned();
                let materialization_key_for_task = materialization_key.clone();
                let materialization_initialized = initialized;
                let materialization_params = params.clone();
                let materialization_request_id = request.request_id.as_str().to_owned();
                let materialization_runtime_search_service = runtime_search_service.clone();
                let materialization_owner_materializer = owner_materializer.clone();
                let materialization_parser_artifact_root = parser_artifact_root.to_path_buf();
                let materialization_providers = workspace_search_providers.to_vec();
                tokio::spawn(async move {
                    let compute_started = std::time::Instant::now();
                    let owner_paths = materialization_params
                        .selectors
                        .iter()
                        .filter_map(|selector| selector_owner_path(selector).map(str::to_owned))
                        .collect::<std::collections::BTreeSet<_>>();
                    let result = match materialization_owner_materializer
                        .ensure_candidates(
                            &materialization_request_id,
                            &materialization_workspace_id,
                            &materialization_initialized.project_root,
                            &materialization_parser_artifact_root,
                            materialization_generation.generation_digest(),
                            &owner_paths,
                            &materialization_providers,
                            &materialization_runtime_search_service,
                            &materialization_registry,
                        )
                        .await
                    {
                        Ok(_) => tokio::task::spawn_blocking({
                            let materialization_generation =
                                Arc::clone(&materialization_generation);
                            let materialization_registry = Arc::clone(&materialization_registry);
                            let materialization_key_for_compute =
                                materialization_key_for_task.clone();
                            move || {
                                materialize_generation_bound_query_playbook(
                                    &materialization_key_for_compute,
                                    &materialization_workspace_id,
                                    &materialization_params,
                                    &materialization_initialized,
                                    materialization_registry.as_ref(),
                                    &materialization_targets,
                                    materialization_generation.as_ref(),
                                )
                            }
                        })
                        .await
                        .map_err(|error| {
                            AspClientOperationError::Message(format!(
                                "resident Query materialization lane failed: {error}"
                            ))
                        })
                        .and_then(|result| result),
                        Err(error) => Err(error),
                    }
                    .map_err(query_materialization_dispatch_error);
                    eprintln!(
                        "[runtime-query-materialization-wall] key={} requestWallMicros={} includesProviderLifecycle=true state={}",
                        materialization_key_for_task,
                        compute_started.elapsed().as_micros(),
                        if result.is_ok() { "ready" } else { "failed" }
                    );
                    let _ = materialization_generation
                        .publish_query_materialization(materialization_key_for_task, result);
                });
            }
            settled_query_materialization(
                &generation,
                &materialization_key,
                request.request_id.as_str(),
            )
            .await
        }
    };
    if result.is_ok() {
        schedule_settled_query_client_timing_observations(
            request,
            &params,
            Arc::clone(&generation),
            active_provider_targets,
            telemetry_sender,
        );
    }
    result
}

async fn settled_query_materialization(
    generation: &crate::runtime_query_generation::RuntimeQueryGeneration,
    key: &str,
    request_id: &str,
) -> Result<serde_json::Value, AspClientOperationError> {
    use crate::runtime_query_generation::RuntimeQueryMaterializationState;
    match generation.await_query_materialization(key).await? {
        RuntimeQueryMaterializationState::Ready(template) => {
            bind_query_materialization_to_request(template.as_ref(), request_id)
        }
        RuntimeQueryMaterializationState::Failed(error) => {
            Err(AspClientOperationError::Terminal(error.as_ref().clone()))
        }
        RuntimeQueryMaterializationState::Building(_) => Err(AspClientOperationError::Message(
            "Query completion preceded terminal publication".to_owned(),
        )),
    }
}

#[cfg(test)]
#[path = "../tests/unit/runtime_asp_client_query_playbook.rs"]
mod tests;
