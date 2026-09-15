// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Content-bound construction and admission of Query Playbook V1 receipts.

use agent_semantic_client_db::runtime_telemetry_bus::RuntimeTelemetryBusSender;

use super::{
    AspClientOperationError, AspClientWorkspaceQueryPlaybookRequest, elapsed_micros,
    emit_runtime_search_trace_observation, query_playbook_terminal,
    runtime_search_trace_budget_micros, selector_owner_path,
};

#[expect(
    clippy::too_many_arguments,
    reason = "the V1 Query receipt binds each identity and measured phase explicitly"
)]
pub(super) fn materialize_query_playbook_receipt(
    request_id: &str,
    params: &AspClientWorkspaceQueryPlaybookRequest,
    runtime_binding: &agent_semantic_content_identity::runtime_execution::RuntimeExecutionBinding,
    execution_publication_digest: &str,
    runtime_bundle_digest: &str,
    source_generation_digest: &str,
    source_root_digest: &str,
    manifest_project_workspace: &agent_semantic_content_identity::ProjectWorkspaceBinding,
    active_provider_targets: &[(String, String)],
    telemetry: Option<(
        &agent_semantic_runtime_observability::RuntimeSearchTelemetryTrace,
        &RuntimeTelemetryBusSender,
    )>,
    mut read_selector: impl FnMut(
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
    let mut exact_read_identity = None::<(String, String)>;
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
                    generation_digest,
                    root_digest,
                    resolved_selector,
                    bytes,
                } if resolved_selector == *selector
                    && root_digest == source_root_digest
                    && exact_read_identity.as_ref().is_none_or(|identity| {
                        identity.0 == generation_digest && identity.1 == root_digest
                    }) => {
                    exact_read_identity.get_or_insert((generation_digest, root_digest));
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
                agent_semantic_client_db::runtime_server_workspace::WorkspaceRuntimeSelectorRead::Projection {
                    resolved_selector,
                    ..
                } if resolved_selector == *selector => {
                    failure_reason = Some("query-playbook-content-identity-mismatch");
                    break;
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
    let (receipt_generation_digest, receipt_root_digest) = exact_read_identity
        .as_ref()
        .map_or((source_generation_digest, source_root_digest), |identity| {
            (identity.0.as_str(), identity.1.as_str())
        });
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
        "sourceGenerationDigest": receipt_generation_digest,
        "sourceRootDigest": receipt_root_digest,
        "requestProfile": "materialized",
        "requestPlaneElapsedMicros": 0,
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
