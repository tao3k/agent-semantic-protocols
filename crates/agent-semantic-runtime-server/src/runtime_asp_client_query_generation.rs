use std::collections::HashMap;
use std::sync::Arc;

use agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationAdmission;
use agent_semantic_client_protocol::AspClientExactQueryFailure;
use agent_semantic_client_protocol::AspClientExactQueryRequest;
use agent_semantic_client_protocol::AspClientRuntimeWorkCounters;
use agent_semantic_client_server::AspClientDispatchError;

use crate::RuntimeQueryGenerationAuthority;
use crate::RuntimeQueryGenerationState;
use crate::query_generation::RuntimeProjectWorkspaceKey;

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
    std::time::Duration::from_secs(5);

pub(super) fn dispatch_budget_for_method(method: &str) -> Option<std::time::Duration> {
    // Search and exact Query establish their immutable CompleteGeneration
    // barrier before beginning the bounded resident read. Charging cold
    // construction to the interactive read deadline would expose Building as
    // a client result instead of awaiting the single-flight terminal.
    (agent_semantic_client_protocol::classify_client_dispatch(method)
        == agent_semantic_client_protocol::ClientDispatchClass::InteractiveRead)
        .then_some(RUNTIME_CLIENT_DISPATCH_BUDGET)
}

pub(super) struct ExactQueryFailure {
    pub(super) reason_kind: &'static str,
    pub(super) resolved_selector: Option<String>,
    pub(super) recommended_next: serde_json::Value,
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
            recommended_next: serde_json::json!({"action": "admit-runtime-generation"}),
        }),
        WorkspaceRuntimeSelectorRead::ProjectionMissing {
            resolved_selector, ..
        } => Some(ExactQueryFailure {
            reason_kind: "projection-missing",
            resolved_selector: Some(resolved_selector.clone()),
            recommended_next: serde_json::json!({
                "action": "query-owner-or-admitted-scope",
                "selector": resolved_selector,
            }),
        }),
        WorkspaceRuntimeSelectorRead::ProjectionScopeOmitted {
            resolved_selector,
            projection_scope,
            ..
        } => Some(ExactQueryFailure {
            reason_kind: "projection-scope-omitted",
            resolved_selector: Some(resolved_selector.clone()),
            recommended_next: serde_json::json!({
                "action": "select-admitted-projection-scope",
                "projectionScope": projection_scope,
            }),
        }),
        WorkspaceRuntimeSelectorRead::OwnerForRepair { .. } => Some(ExactQueryFailure {
            reason_kind: "owner-repair-required",
            resolved_selector: None,
            recommended_next: serde_json::json!({"action": "repair-resident-owner-projection"}),
        }),
        WorkspaceRuntimeSelectorRead::OwnerMissing { .. } => Some(ExactQueryFailure {
            reason_kind: "owner-missing",
            resolved_selector: None,
            recommended_next: serde_json::json!({"action": "reconcile-runtime-owner"}),
        }),
        WorkspaceRuntimeSelectorRead::RelocationAmbiguous { candidates, .. } => {
            Some(ExactQueryFailure {
                reason_kind: "relocation-ambiguous",
                resolved_selector: None,
                recommended_next: serde_json::json!({
                    "action": "choose-relocation-candidate",
                    "candidates": candidates,
                }),
            })
        }
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
    let recommended_next = serde_json::json!({
        "action": "publish-complete-workspace-generation",
        "projectId": project_id,
        "workspaceId": workspace_id,
    });
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
            recommended_next,
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
            message: "no immutable CompleteGeneration is published for this workspace".to_owned(),
            details: Some(serde_json::to_value(failure).map_err(|error| error.to_string())?),
        });
    }
    Ok(AspClientDispatchError {
        reason_kind: reason_kind.to_owned(),
        message: "no immutable CompleteGeneration is published for this workspace".to_owned(),
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
            "recommendedNext": recommended_next,
            "elapsedMicros": elapsed_micros,
            "workCounters": AspClientRuntimeWorkCounters::default(),
        })),
    })
}

pub(super) struct RuntimeQueryGenerationInstallError {
    pub(super) message: String,
    already_published: bool,
}

impl RuntimeQueryGenerationInstallError {
    fn pending(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            already_published: false,
        }
    }

    fn published(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            already_published: true,
        }
    }
}

pub(super) async fn install_runtime_query_generation_terminal(
    terminal: &agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationAdmissionReceipt,
    query_generation_authority: &RuntimeQueryGenerationAuthority,
    workspace_registry: &agent_semantic_client_db::runtime_server_workspace::RuntimeServerWorkspaceRegistry,
    project_workspace_key: &RuntimeProjectWorkspaceKey,
    workspace_identity: &str,
    project_root: &std::path::Path,
) -> Result<(), RuntimeQueryGenerationInstallError> {
    let commit = terminal.commit.as_ref().ok_or_else(|| {
        RuntimeQueryGenerationInstallError::pending(
            "workspace generation readiness terminal is missing its commit",
        )
    })?;
    let resident_read = workspace_registry
        .resident_read_client(workspace_identity, project_root)
        .map_err(RuntimeQueryGenerationInstallError::published)?;
    let resident = query_generation_authority
        .ensure_ready_resident(
            project_workspace_key,
            project_root,
            resident_read,
            &commit.generation_digest,
        )
        .await
        .map_err(RuntimeQueryGenerationInstallError::published)?;
    let actual_root_digest = resident.resident().source_root_digest();
    if actual_root_digest == commit.source_root_digest {
        return Ok(());
    }
    let error = format!(
        "workspace generation resident root mismatch: expected={} actual={actual_root_digest}",
        commit.source_root_digest,
    );
    let publication_token = resident.generation_token();
    let error = match query_generation_authority.evict_ready_exact(
        project_workspace_key,
        &commit.generation_digest,
        &actual_root_digest,
    ) {
        Ok(_) => error,
        Err(eviction_error) => format!("{error}; resident eviction failed: {eviction_error}"),
    };
    query_generation_authority.publish_failed(
        project_workspace_key.clone(),
        publication_token.saturating_add(1),
        commit.generation_digest.clone(),
        error.clone(),
    );
    Err(RuntimeQueryGenerationInstallError::published(error))
}

async fn publish_runtime_query_generation_terminal(
    terminal: Result<
        agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationAdmissionReceipt,
        String,
    >,
    query_generation_authority: RuntimeQueryGenerationAuthority,
    workspace_registry: Arc<
        agent_semantic_client_db::runtime_server_workspace::RuntimeServerWorkspaceRegistry,
    >,
    project_workspace_key: RuntimeProjectWorkspaceKey,
    workspace_identity: String,
    project_root: std::path::PathBuf,
) {
    let terminal = match terminal {
        Ok(terminal) => terminal,
        Err(error) => {
            query_generation_authority.publish_failed(
                project_workspace_key,
                0,
                "unpublished",
                error,
            );
            return;
        }
    };
    let expected_generation_digest = terminal.commit.as_ref().map_or_else(
        || "unpublished".to_owned(),
        |commit| commit.generation_digest.clone(),
    );
    if let Err(error) = install_runtime_query_generation_terminal(
        &terminal,
        &query_generation_authority,
        workspace_registry.as_ref(),
        &project_workspace_key,
        &workspace_identity,
        &project_root,
    )
    .await
    {
        if !error.already_published {
            query_generation_authority.publish_failed(
                project_workspace_key,
                0,
                expected_generation_digest,
                error.message,
            );
        }
    }
}

pub(super) fn request_runtime_query_generation_ready(
    generation_admission: &WorkspaceGenerationAdmission,
    query_generation_authority: &RuntimeQueryGenerationAuthority,
    workspace_registry: &Arc<
        agent_semantic_client_db::runtime_server_workspace::RuntimeServerWorkspaceRegistry,
    >,
    project_workspace_key: &RuntimeProjectWorkspaceKey,
    workspace_identity: String,
    project_root: std::path::PathBuf,
) -> Result<
    agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationReadinessRequestState,
    String,
> {
    let terminal_authority = query_generation_authority.clone();
    let terminal_registry = Arc::clone(workspace_registry);
    let terminal_key = project_workspace_key.clone();
    let terminal_workspace_identity = workspace_identity.clone();
    let terminal_project_root = project_root.clone();
    generation_admission.request_runtime_generation_ready_with_terminal(
        workspace_identity,
        project_root,
        move |terminal| {
            publish_runtime_query_generation_terminal(
                terminal,
                terminal_authority,
                terminal_registry,
                terminal_key,
                terminal_workspace_identity,
                terminal_project_root,
            )
        },
    )
}

pub(super) async fn wait_for_runtime_query_generation(
    receiver: &mut tokio::sync::watch::Receiver<
        Arc<HashMap<RuntimeProjectWorkspaceKey, RuntimeQueryGenerationState>>,
    >,
    key: &RuntimeProjectWorkspaceKey,
) -> Option<RuntimeQueryGenerationState> {
    if let Some(state) = receiver.borrow().get(key).cloned() {
        return Some(state);
    }
    loop {
        match receiver.changed().await {
            Ok(()) => {
                if let Some(state) = receiver.borrow_and_update().get(key).cloned() {
                    return Some(state);
                }
            }
            Err(_) => return None,
        }
    }
}

pub(super) async fn wait_for_runtime_query_generation_change(
    receiver: &mut tokio::sync::watch::Receiver<
        Arc<HashMap<RuntimeProjectWorkspaceKey, RuntimeQueryGenerationState>>,
    >,
    key: &RuntimeProjectWorkspaceKey,
) -> Option<RuntimeQueryGenerationState> {
    receiver.borrow_and_update();
    loop {
        match receiver.changed().await {
            Ok(()) => {
                if let Some(state) = receiver.borrow_and_update().get(key).cloned() {
                    return Some(state);
                }
            }
            Err(_) => return None,
        }
    }
}
