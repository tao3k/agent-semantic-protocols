// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationAdmission;
use agent_semantic_client_protocol::AspClientExactQueryFailure;
use agent_semantic_client_protocol::AspClientExactQueryRequest;
use agent_semantic_client_protocol::AspClientRuntimeWorkCounters;
use agent_semantic_client_server::AspClientDispatchError;

pub(super) enum AspClientOperationError {
    Message(String),
    Terminal(AspClientDispatchError),
}

impl From<String> for AspClientOperationError {
    fn from(message: String) -> Self {
        Self::Message(message)
    }
}

pub(super) const RUNTIME_CLIENT_DISPATCH_BUDGET: std::time::Duration =
    std::time::Duration::from_micros(1_000);

pub(super) fn dispatch_budget_for_method(method: &str) -> Option<std::time::Duration> {
    if method == agent_semantic_client_protocol::WORKSPACE_SEARCH_PLAYBOOK_METHOD {
        // A composite playbook may scan the complete immutable rg corpus and
        // evaluate the generation-derived Graph frontier. Its finite work and
        // cancellation are owned by the playbook budget, not the exact-read
        // latency gate retained by Query.
        return None;
    }
    (agent_semantic_client_protocol::classify_client_dispatch(method)
        == agent_semantic_client_protocol::ClientDispatchClass::ResidentGenerationRead)
        .then_some(RUNTIME_CLIENT_DISPATCH_BUDGET)
}

pub(super) fn enforce_completed_dispatch_budget(
    result: Result<serde_json::Value, AspClientDispatchError>,
    budget: Option<std::time::Duration>,
    elapsed: std::time::Duration,
) -> Result<serde_json::Value, AspClientDispatchError> {
    let Some(budget) = budget else {
        return result;
    };
    if elapsed < budget {
        return result;
    }
    Err(AspClientDispatchError {
        reason_kind: "client-request-deadline-exceeded".to_owned(),
        message: format!(
            "Runtime ClientFrame dispatch completed after its strict {}us interactive deadline",
            budget.as_micros(),
        ),
        details: Some(serde_json::json!({
            "schemaId": "agent.semantic-protocols.asp-client-dispatch-failure",
            "schemaVersion": "1",
            "state": "failed",
            "phase": "runtime-client-dispatch-completion",
            "reasonKind": "client-request-deadline-exceeded",
            "budgetMicros": budget.as_micros(),
            "elapsedMicros": elapsed.as_micros(),
        })),
    })
}

pub(super) struct ExactQueryFailure {
    pub(super) reason_kind: &'static str,
    pub(super) resolved_selector: Option<String>,
}

pub(super) fn classify_exact_query_failure(
    projection: &agent_semantic_client_db::runtime_server_workspace::WorkspaceRuntimeSelectorRead,
) -> Option<ExactQueryFailure> {
    use agent_semantic_client_db::runtime_server_workspace::WorkspaceRuntimeSelectorRead;

    match projection {
        WorkspaceRuntimeSelectorRead::Projection { .. }
        | WorkspaceRuntimeSelectorRead::ProviderProjection { .. } => None,
        WorkspaceRuntimeSelectorRead::GenerationMissing => Some(ExactQueryFailure {
            reason_kind: "runtime-generation-missing",
            resolved_selector: None,
        }),
        WorkspaceRuntimeSelectorRead::ProjectionMissing {
            resolved_selector, ..
        } => Some(ExactQueryFailure {
            reason_kind: "projection-missing",
            resolved_selector: Some(resolved_selector.clone()),
        }),
        WorkspaceRuntimeSelectorRead::ProjectionScopeOmitted {
            resolved_selector, ..
        } => Some(ExactQueryFailure {
            reason_kind: "projection-scope-omitted",
            resolved_selector: Some(resolved_selector.clone()),
        }),
        WorkspaceRuntimeSelectorRead::OwnerForRepair { .. } => Some(ExactQueryFailure {
            reason_kind: "owner-repair-required",
            resolved_selector: None,
        }),
        WorkspaceRuntimeSelectorRead::OwnerMissing { .. } => Some(ExactQueryFailure {
            reason_kind: "owner-missing",
            resolved_selector: None,
        }),
        WorkspaceRuntimeSelectorRead::RelocationAmbiguous { .. } => Some(ExactQueryFailure {
            reason_kind: "relocation-ambiguous",
            resolved_selector: None,
        }),
    }
}

#[cfg(test)]
#[path = "../tests/unit/runtime_asp_client_query_generation.rs"]
mod exact_query_terminal_tests;

pub(super) struct QueryNotReadyContext<'a> {
    pub(super) operation_id: &'a str,
    pub(super) project_id: &'a str,
    pub(super) workspace_id: &'a str,
    pub(super) language_id: &'a str,
    pub(super) provider_id: &'a str,
    pub(super) exact_query: Option<&'a AspClientExactQueryRequest>,
    pub(super) generation_state: &'a str,
    pub(super) publication_error: Option<&'a str>,
    pub(super) elapsed_micros: u64,
}

pub(super) fn query_generation_not_ready_error(
    context: QueryNotReadyContext<'_>,
) -> Result<AspClientDispatchError, String> {
    let QueryNotReadyContext {
        operation_id,
        project_id,
        workspace_id,
        language_id,
        provider_id,
        exact_query,
        generation_state,
        publication_error,
        elapsed_micros,
    } = context;
    let reason_kind = "query-not-ready";
    let message = publication_error.map_or_else(
        || {
            format!(
                "no immutable CompleteGeneration is published for this workspace: state={generation_state}"
            )
        },
        |error| {
            format!(
                "no immutable CompleteGeneration is published for this workspace: state={generation_state} cause={error}"
            )
        },
    );
    if let Some(exact_query) = exact_query {
        let failure = AspClientExactQueryFailure {
            schema_id: "agent.semantic-protocols.asp-client-exact-query-failure".to_owned(),
            schema_version: "1".to_owned(),
            state: "failed".to_owned(),
            operation_id: operation_id.to_owned(),
            project_id: project_id.to_owned(),
            workspace_id: workspace_id.to_owned(),
            language_id: language_id.to_owned(),
            provider_id: provider_id.to_owned(),
            requested_selector: Some(exact_query.selector.clone()),
            resolved_selector: None,
            projection_kind: Some(exact_query.projection.clone()),
            phase: "runtime-generation-authority".to_owned(),
            reason_kind: reason_kind.to_owned(),
            generation_digest: None,
            root_digest: None,
            resident_read_elapsed_micros: 0,
            service_elapsed_micros: elapsed_micros,
            elapsed_micros,
            work_counters: AspClientRuntimeWorkCounters::default(),
            details: serde_json::json!({
                "generationState": generation_state,
                "publicationError": publication_error,
            }),
        };
        failure.validate()?;
        return Ok(AspClientDispatchError {
            reason_kind: reason_kind.to_owned(),
            message,
            details: Some(serde_json::to_value(failure).map_err(|error| error.to_string())?),
        });
    }
    Ok(AspClientDispatchError {
        reason_kind: reason_kind.to_owned(),
        message,
        details: Some(serde_json::json!({
            "schemaId": "agent.semantic-protocols.asp-client-query-readiness-failure",
            "schemaVersion": "1",
            "state": "failed",
            "phase": "runtime-generation-authority",
            "reasonKind": reason_kind,
            "projectId": project_id,
            "workspaceId": workspace_id,
            "languageId": language_id,
            "providerId": provider_id,
            "generationState": generation_state,
            "publicationError": publication_error,
            "elapsedMicros": elapsed_micros,
            "workCounters": AspClientRuntimeWorkCounters::default(),
        })),
    })
}

/// Submit detached source-generation work only. Resident publication is owned
/// exclusively by the daemon execution-publication observer.
pub(super) fn request_runtime_query_generation_ready(
    generation_admission: &WorkspaceGenerationAdmission,
    workspace_identity: String,
    project_root: std::path::PathBuf,
    provider_targets: Vec<
        agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationProviderTarget,
    >,
) -> Result<
    agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationReadinessRequestState,
    String,
> {
    generation_admission.request_runtime_generations_ready_for_providers(
        workspace_identity,
        project_root,
        provider_targets,
    )
}
