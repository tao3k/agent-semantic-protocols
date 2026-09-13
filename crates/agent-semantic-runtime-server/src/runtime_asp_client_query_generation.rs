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
    (agent_semantic_client_protocol::classify_client_dispatch(method)
        == agent_semantic_client_protocol::ClientDispatchClass::ResidentGenerationRead)
        .then_some(RUNTIME_CLIENT_DISPATCH_BUDGET)
}

/// Temporary measurement boundary, not a first-result performance SLO.
use agent_semantic_client_protocol::FIRST_COMPUTATION_OBSERVATION_BUDGET;

#[derive(Clone)]
pub(super) struct RequestDispatchBudget {
    resident: Option<std::time::Duration>,
    phase: std::sync::Arc<RequestDispatchPhase>,
}

const DISPATCH_PHASE_CLASSIFYING: u8 = 0;
const DISPATCH_PHASE_RESIDENT: u8 = 1;
const DISPATCH_PHASE_FIRST_COMPUTATION: u8 = 2;

#[derive(Default)]
struct RequestDispatchPhase {
    state: std::sync::atomic::AtomicU8,
    changed: tokio::sync::Notify,
}

impl RequestDispatchBudget {
    pub(super) fn for_method(method: &str) -> Self {
        Self {
            resident: dispatch_budget_for_method(method),
            phase: Default::default(),
        }
    }

    pub(super) fn observe_miss(&self) {
        self.phase.state.store(
            DISPATCH_PHASE_FIRST_COMPUTATION,
            std::sync::atomic::Ordering::Release,
        );
        self.phase.changed.notify_waiters();
    }

    /// Set only after all generation/materialization lookups prove that this
    /// request is a ready resident read.  Until then the request is
    /// classifying: a late miss must still be allowed to enter the separate
    /// first-computation observation window.
    pub(super) fn observe_resident_hit(&self) {
        if self
            .phase
            .state
            .compare_exchange(
                DISPATCH_PHASE_CLASSIFYING,
                DISPATCH_PHASE_RESIDENT,
                std::sync::atomic::Ordering::AcqRel,
                std::sync::atomic::Ordering::Acquire,
            )
            .is_ok()
        {
            self.phase.changed.notify_waiters();
        }
    }

    pub(super) fn is_first_computation(&self) -> bool {
        self.phase.state.load(std::sync::atomic::Ordering::Acquire)
            == DISPATCH_PHASE_FIRST_COMPUTATION
    }

    pub(super) fn limit(&self) -> Option<std::time::Duration> {
        self.resident.map(|resident| {
            if self.is_first_computation() {
                FIRST_COMPUTATION_OBSERVATION_BUDGET
            } else {
                resident
            }
        })
    }

    pub(super) async fn expired(&self, started: tokio::time::Instant) {
        let Some(resident) = self.resident else {
            return std::future::pending().await;
        };
        loop {
            let state = self.phase.state.load(std::sync::atomic::Ordering::Acquire);
            let deadline = match state {
                DISPATCH_PHASE_RESIDENT => started + resident,
                DISPATCH_PHASE_CLASSIFYING | DISPATCH_PHASE_FIRST_COMPUTATION => {
                    started + FIRST_COMPUTATION_OBSERVATION_BUDGET
                }
                _ => unreachable!("request dispatch phase is closed over three states"),
            };
            let changed = self.phase.changed.notified();
            tokio::pin!(changed);
            changed.as_mut().enable();
            if self.phase.state.load(std::sync::atomic::Ordering::Acquire) != state {
                continue;
            }
            tokio::select! {
                _ = tokio::time::sleep_until(deadline) => return,
                _ = changed.as_mut() => {}
            }
        }
    }
}

/// Retained publication state is inspected before every event wait. This is
/// not a build retry loop: failed/closed publication terminates this request.
pub(super) async fn await_runtime_query_generation(
    states: &tokio::sync::watch::Receiver<
        std::sync::Arc<
            std::collections::HashMap<
                crate::runtime_query_generation_key::RuntimeProjectWorkspaceKey,
                crate::RuntimeQueryGenerationState,
            >,
        >,
    >,
    key: &crate::runtime_query_generation_key::RuntimeProjectWorkspaceKey,
) -> Result<std::sync::Arc<crate::runtime_query_generation::RuntimeQueryGeneration>, String> {
    let mut states = states.clone();
    loop {
        let state = states.borrow_and_update().get(key).cloned();
        match state {
            Some(crate::RuntimeQueryGenerationState::Ready(generation)) => return Ok(generation),
            Some(crate::RuntimeQueryGenerationState::Failed { reason, .. }) => {
                return Err(reason.to_string());
            }
            None => states.changed().await.map_err(|_| {
                "Runtime generation publication channel closed before readiness".to_owned()
            })?,
        }
    }
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
