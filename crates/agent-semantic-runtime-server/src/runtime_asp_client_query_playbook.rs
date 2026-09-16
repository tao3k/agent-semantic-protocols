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
    RUNTIME_CLIENT_DISPATCH_BUDGET, elapsed_micros,
};

#[path = "runtime_asp_client_query_playbook_materialization.rs"]
mod query_playbook_materialization;
use query_playbook_materialization::materialize_query_playbook_receipt;

#[path = "runtime_asp_client_query_playbook_exact_replay.rs"]
mod query_playbook_exact_replay;
use query_playbook_exact_replay::try_process_cold_exact_owner_replay;
#[cfg(test)]
pub(super) use query_playbook_exact_replay::{
    durable_exact_execution_matches_current, durable_projection_is_direct,
    process_cold_owner_content_digest,
};

#[path = "runtime_asp_client_query_playbook_telemetry.rs"]
mod query_playbook_telemetry;
use query_playbook_telemetry::record_query_client_timing_observations;
#[cfg(test)]
pub(super) use query_playbook_telemetry::record_settled_client_timing_observations;
pub(super) use query_playbook_telemetry::{
    emit_runtime_search_trace_observation, runtime_search_trace_budget_micros,
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

type ResidentQueryProjections = std::collections::VecDeque<
    agent_semantic_client_db::runtime_server_workspace::WorkspaceRuntimeSelectorRead,
>;

fn resident_exact_query_projections(
    params: &AspClientWorkspaceQueryPlaybookRequest,
    mut read_selector: impl FnMut(
        agent_semantic_client_db::runtime_server_workspace::ExactProjectionKind,
        &str,
    ) -> Result<
        agent_semantic_client_db::runtime_server_workspace::WorkspaceRuntimeSelectorRead,
        String,
    >,
) -> Result<Option<ResidentQueryProjections>, AspClientOperationError> {
    use agent_semantic_client_db::runtime_server_workspace::WorkspaceRuntimeSelectorRead;

    let projection =
        agent_semantic_client_db::runtime_server_workspace::ExactProjectionKind::try_from(
            params.projection.as_str(),
        )?;
    let mut resident = std::collections::VecDeque::with_capacity(params.selectors.len());
    for selector in &params.selectors {
        let read = read_selector(projection, selector)?;
        match &read {
            WorkspaceRuntimeSelectorRead::Projection {
                resolved_selector, ..
            } if resolved_selector == selector => resident.push_back(read),
            _ => return Ok(None),
        }
    }
    Ok(Some(resident))
}

fn read_query_projection_handoff(
    resident_projections: &mut Option<ResidentQueryProjections>,
    projection: agent_semantic_client_db::runtime_server_workspace::ExactProjectionKind,
    selector: &str,
    fallback: impl FnOnce(
        agent_semantic_client_db::runtime_server_workspace::ExactProjectionKind,
        &str,
    ) -> Result<
        agent_semantic_client_db::runtime_server_workspace::WorkspaceRuntimeSelectorRead,
        String,
    >,
) -> Result<agent_semantic_client_db::runtime_server_workspace::WorkspaceRuntimeSelectorRead, String>
{
    if let Some(projections) = resident_projections.as_mut() {
        return projections.pop_front().ok_or_else(|| {
            "resident Query projection handoff ended before the requested selector".to_owned()
        });
    }
    fallback(projection, selector)
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
    Ok(format!(
        "{}\0blake3-256:{}",
        params.projection,
        hasher.finalize().to_hex()
    ))
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

struct QueryMaterializationPublicationGuard {
    generation: Arc<crate::RuntimeQueryGeneration>,
    key: Option<String>,
}

impl QueryMaterializationPublicationGuard {
    fn new(generation: Arc<crate::RuntimeQueryGeneration>, key: String) -> Self {
        Self {
            generation,
            key: Some(key),
        }
    }

    fn publish(
        mut self,
        result: Result<serde_json::Value, AspClientDispatchError>,
    ) -> Result<(), String> {
        let key = self
            .key
            .take()
            .expect("Query materialization publication guard must own its key");
        self.generation.publish_query_materialization(key, result)
    }
}

impl Drop for QueryMaterializationPublicationGuard {
    fn drop(&mut self) {
        let Some(key) = self.key.take() else {
            return;
        };
        let terminal = AspClientDispatchError {
            reason_kind: "query-materialization-cancelled".to_owned(),
            message: "Query materialization task ended before terminal publication".to_owned(),
            details: Some(serde_json::json!({
                "schemaId": "agent.semantic-protocols.asp-client-dispatch-failure",
                "schemaVersion": "1",
                "state": "cancelled",
                "phase": "runtime-query-materialization",
                "reasonKind": "query-materialization-cancelled"
            })),
        };
        let _ = self
            .generation
            .publish_query_materialization(key, Err(terminal));
    }
}

fn bind_query_materialization_to_request(
    template: Arc<serde_json::Value>,
    request_id: &str,
    request_profile: &str,
    started: tokio::time::Instant,
) -> Result<agent_semantic_client_protocol::ClientResponsePayload, AspClientOperationError> {
    if !template.is_object() {
        return Err(AspClientOperationError::Message(
            "resident Query materialization is not an object".to_owned(),
        ));
    }
    agent_semantic_client_protocol::ClientResponsePayload::from_shared_object_overlay(
        template,
        std::collections::BTreeMap::from([
            (
                "requestId".to_owned(),
                serde_json::Value::String(request_id.to_owned()),
            ),
            (
                "requestProfile".to_owned(),
                serde_json::Value::String(request_profile.to_owned()),
            ),
            (
                "requestPlaneElapsedMicros".to_owned(),
                serde_json::Value::from(elapsed_micros(started)),
            ),
        ]),
    )
    .map_err(|message| AspClientOperationError::Message(message.to_owned()))
}

struct ResidentQueryProjectionHandoff<'a> {
    generation: &'a crate::RuntimeQueryGeneration,
    projections: Option<ResidentQueryProjections>,
}

fn materialize_generation_bound_query_playbook(
    template_request_id: &str,
    workspace_id: &str,
    params: &AspClientWorkspaceQueryPlaybookRequest,
    initialized: &InitializedWorkspace,
    workspace_registry: &RuntimeServerWorkspaceRegistry,
    active_provider_targets: &[(String, String)],
    mut projection_handoff: ResidentQueryProjectionHandoff<'_>,
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
        || resident_generation_digest != projection_handoff.generation.generation_digest()
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
    let execution_publication = projection_handoff
        .generation
        .execution_publication()
        .ok_or_else(|| {
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
        template_request_id,
        params,
        runtime_binding,
        execution_publication.publication_digest.as_str(),
        execution_publication.runtime_bundle_digest.as_str(),
        projection_handoff.generation.generation_digest(),
        resident.source_root_digest().as_str(),
        initialized.host_workspace.project_workspace(),
        active_provider_targets,
        None,
        |projection, selector| {
            read_query_projection_handoff(
                &mut projection_handoff.projections,
                projection,
                selector,
                |projection, selector| resident.read_runtime_selector(projection, selector),
            )
        },
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
    current_runtime_bundle_digest: &str,
    resource_supervisor: &agent_semantic_workspace_scheduler::RuntimeServerResourceSupervisor,
    task_scope: &agent_semantic_workspace_scheduler::RuntimeServerTaskScope,
    workspace_search_providers: &[agent_semantic_search::WorkspaceSearchProvider],
    active_provider_targets: &[(String, String)],
    telemetry_sender: &RuntimeTelemetryBusSender,
    _telemetry_traces: &Arc<
        Mutex<
            HashMap<
                ClientRequestKey,
                agent_semantic_runtime_observability::RuntimeSearchTelemetryTrace,
            >,
        >,
    >,
    _active_telemetry_trace_count: &std::sync::atomic::AtomicUsize,
    query_generation: &tokio::sync::watch::Receiver<
        Arc<HashMap<RuntimeProjectWorkspaceKey, RuntimeQueryGenerationState>>,
    >,
) -> Result<agent_semantic_client_protocol::ClientResponsePayload, AspClientOperationError> {
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
            admit_cold_query_owner_paths(&initialized.project_root, &params.selectors).await?;
            if let Some(result) = try_process_cold_exact_owner_replay(
                request.request_id.as_str(),
                request.workspace_id.as_str(),
                &params,
                &initialized,
                parser_artifact_root,
                current_runtime_bundle_digest,
                resource_supervisor,
                task_scope,
                active_provider_targets,
                wait_started,
            )
            .await?
            {
                eprintln!(
                    "[runtime-query-cold-exact-owner-replay] requestId={} elapsedMicros={} selectorCount={} filesystemOwnerReadCount={} state=ready",
                    request.request_id.as_str(),
                    wait_started.elapsed().as_micros(),
                    params.selectors.len(),
                    params
                        .selectors
                        .iter()
                        .filter_map(|selector| selector_owner_path(selector))
                        .collect::<std::collections::BTreeSet<_>>()
                        .len(),
                );
                return Ok(result);
            }
            // A durable exact-owner miss cannot authorize relocation or
            // repair. Escalate to the sole complete-generation authority.
            let generation =
                super::query_generation_support::request_and_await_runtime_query_generation_ready(
                    generation_admission,
                    request.workspace_id.as_str().to_owned(),
                    initialized.project_root.clone(),
                    provider_targets,
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
    let resident_lookup_started = tokio::time::Instant::now();
    let result = match generation.query_materialization(&materialization_key)? {
        Some(crate::runtime_query_generation::RuntimeQueryMaterializationState::Ready(
            template,
        )) => {
            dispatch_budget.observe_resident_hit();
            let result = bind_query_materialization_to_request(
                Arc::clone(&template),
                request.request_id.as_str(),
                "resident-hit",
                resident_lookup_started,
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
                resident_lookup_started,
            )
            .await
        }
        None => {
            dispatch_budget.observe_miss();
            if generation.begin_query_materialization(materialization_key.clone())? {
                let resident_projections =
                    resident_exact_query_projections(&params, |projection, selector| {
                        generation.read_runtime_selector(projection, selector)
                    })?;
                let projections_are_resident = resident_projections.is_some();
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
                let task_admission = generation.spawn_materialization(
                    "query-materialization",
                    async move {
                    let publication_guard = QueryMaterializationPublicationGuard::new(
                        Arc::clone(&materialization_generation),
                        materialization_key_for_task.clone(),
                    );
                    let compute_started = std::time::Instant::now();
                    let owner_paths = materialization_params
                        .selectors
                        .iter()
                        .filter_map(|selector| selector_owner_path(selector).map(str::to_owned))
                        .collect::<std::collections::BTreeSet<_>>();
                    let owner_materialization = if projections_are_resident {
                        Ok(())
                    } else {
                        materialization_owner_materializer
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
                            .map(|_| ())
                    };
                    let result = match owner_materialization {
                        Ok(()) => {
                            let blocking_task = materialization_generation.spawn_search_blocking(
                                "query-materialization-receipt",
                                {
                                    let materialization_generation =
                                        Arc::clone(&materialization_generation);
                                    let materialization_registry =
                                        Arc::clone(&materialization_registry);
                                    move || {
                                        materialize_generation_bound_query_playbook(
                                            &materialization_request_id,
                                            &materialization_workspace_id,
                                            &materialization_params,
                                            &materialization_initialized,
                                            materialization_registry.as_ref(),
                                            &materialization_targets,
                                            ResidentQueryProjectionHandoff {
                                                generation: materialization_generation.as_ref(),
                                                projections: resident_projections,
                                            },
                                        )
                                    }
                                },
                            );
                            match blocking_task {
                                Ok(task) => task
                                    .join()
                                    .await
                                    .map_err(AspClientOperationError::Message)
                                    .and_then(|result| result),
                                Err(error) => Err(AspClientOperationError::Message(error)),
                            }
                        }
                        Err(error) => Err(error),
                    }
                    .map_err(query_materialization_dispatch_error);
                    eprintln!(
                        "[runtime-query-materialization-wall] key={} requestWallMicros={} ownerMaterialization={} includesProviderLifecycle={} state={}",
                        materialization_key_for_task,
                        compute_started.elapsed().as_micros(),
                        if projections_are_resident {
                            "skipped-resident-exact"
                        } else {
                            "candidate-scoped"
                        },
                        !projections_are_resident,
                        if result.is_ok() { "ready" } else { "failed" }
                    );
                    let _ = publication_guard.publish(result);
                    },
                );
                if let Err(error) = task_admission {
                    let terminal = query_materialization_dispatch_error(
                        AspClientOperationError::Message(error),
                    );
                    generation.publish_query_materialization(
                        materialization_key.clone(),
                        Err(terminal.clone()),
                    )?;
                    return Err(AspClientOperationError::Terminal(terminal));
                }
            }
            settled_query_materialization(
                &generation,
                &materialization_key,
                request.request_id.as_str(),
                resident_lookup_started,
            )
            .await
        }
    };
    if result.is_ok() {
        record_query_client_timing_observations(
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
    started: tokio::time::Instant,
) -> Result<agent_semantic_client_protocol::ClientResponsePayload, AspClientOperationError> {
    use crate::runtime_query_generation::RuntimeQueryMaterializationState;
    match generation.await_query_materialization(key).await? {
        RuntimeQueryMaterializationState::Ready(template) => {
            bind_query_materialization_to_request(template, request_id, "materialized", started)
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
