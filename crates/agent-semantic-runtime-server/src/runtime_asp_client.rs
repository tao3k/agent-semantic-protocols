//! Public ASP Client Protocol binding for the resident Runtime Server.

use std::collections::{HashMap, hash_map::Entry};
use std::sync::{Arc, Mutex};

use crate::query_generation::RuntimeProjectWorkspaceKey;
use crate::{RuntimeQueryGenerationAuthority, RuntimeQueryGenerationState};
use agent_semantic_client_db::runtime_search_service::RuntimeSearchServiceHandle;
use agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationAdmission;
use agent_semantic_client_protocol::{
    AGENT_SESSION_REGISTER_METHOD, AGENT_SESSION_REGISTER_RESPONSE_SCHEMA_ID,
    AgentSessionRegisterReceipt, AgentSessionRegisterRequest, AspClientExactQueryFailure,
    AspClientExactQueryRequest, AspClientExactQueryResponse, AspClientRuntimeWorkCounters,
    AspClientSearchRequest, ClientProjectId, ClientRequestId, ClientSessionId,
    ClientWorkspaceIdentity, GRAPH_TIMELINE_METHOD, LIVE_CORPUS_CACHE_STATE_METHOD,
    LiveCorpusCacheStateReceipt, LiveCorpusCacheStateRequest, SCHEMA_BUNDLE_METHOD,
    SchemaBundleRequest, ServerClientRoute,
};
use agent_semantic_client_server::{
    AspClientDispatchError, AspClientDispatchFuture, AspClientDispatchRequest, AspClientDispatcher,
    AspClientFrameService,
};
use agent_semantic_search_projection::ResidentGraphEvaluationRequestV1;

enum AspClientOperationError {
    Message(String),
    Terminal(AspClientDispatchError),
}

impl From<String> for AspClientOperationError {
    fn from(message: String) -> Self {
        Self::Message(message)
    }
}

const RUNTIME_CLIENT_DISPATCH_BUDGET: std::time::Duration = std::time::Duration::from_secs(5);

fn dispatch_budget_for_method(method: &str) -> Option<std::time::Duration> {
    (agent_semantic_client_protocol::classify_client_dispatch(method)
        == agent_semantic_client_protocol::ClientDispatchClass::InteractiveRead)
        .then_some(RUNTIME_CLIENT_DISPATCH_BUDGET)
}

struct ExactQueryFailure {
    reason_kind: &'static str,
    resolved_selector: Option<String>,
    recommended_next: serde_json::Value,
}

fn classify_exact_query_failure(
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
mod exact_query_terminal_tests {
    use super::*;
    use agent_semantic_client_db::runtime_server_workspace::WorkspaceRuntimeSelectorRead;

    #[test]
    fn projection_missing_is_a_precise_runtime_terminal() {
        let failure =
            classify_exact_query_failure(&WorkspaceRuntimeSelectorRead::ProjectionMissing {
                generation_digest: format!("blake3-256:{}", "a".repeat(64)),
                root_digest: "b".repeat(64),
                resolved_selector: "rust://src/lib.rs#item/function/missing".to_owned(),
            })
            .expect("typed failure");

        assert_eq!(failure.reason_kind, "projection-missing");
        assert_eq!(
            failure.resolved_selector.as_deref(),
            Some("rust://src/lib.rs#item/function/missing")
        );
        assert_eq!(
            failure.recommended_next["action"],
            "query-owner-or-admitted-scope"
        );
    }

    #[test]
    fn payload_bearing_projection_is_not_classified_as_failure() {
        let ready = WorkspaceRuntimeSelectorRead::Projection {
            generation_digest: format!("blake3-256:{}", "a".repeat(64)),
            root_digest: "b".repeat(64),
            resolved_selector: "rust://src/lib.rs#item/function/ready".to_owned(),
            bytes: b"fn ready() {}".to_vec(),
        };
        assert!(classify_exact_query_failure(&ready).is_none());
    }

    #[test]
    fn unpublished_generation_returns_typed_query_not_ready_after_readiness_submission() {
        let request = AspClientExactQueryRequest {
            schema_id: "agent.semantic-protocols.asp-client-exact-query-request".to_owned(),
            schema_version: "1".to_owned(),
            selector: "rust://src/lib.rs#item/function/ready".to_owned(),
            projection: "source".to_owned(),
        };
        let error = query_generation_not_ready_error(QueryNotReadyContext {
            operation_id: "request-1",
            project_id: "repo-1",
            workspace_id: "workspace-1",
            language_id: "rust",
            provider_id: "asp-rust",
            exact_query: Some(&request),
            generation_state: "unpublished",
            publication_error: None,
            elapsed_micros: 7,
        })
        .expect("typed query readiness terminal");

        assert_eq!(error.reason_kind, "query-not-ready");
        let terminal = error.details.expect("typed exact-query terminal");
        assert_eq!(terminal["state"], "failed");
        assert_eq!(terminal["phase"], "runtime-generation-authority");
        assert_eq!(terminal["reasonKind"], "query-not-ready");
        assert_eq!(terminal["workCounters"]["filesystemReadCount"], 0);
        assert_eq!(terminal["workCounters"]["databaseReadCount"], 0);
        assert_eq!(terminal["workCounters"]["providerProcessCount"], 0);
        assert_eq!(
            terminal["recommendedNext"]["action"],
            "publish-complete-workspace-generation"
        );
    }

    #[test]
    fn readiness_submission_failure_remains_one_typed_query_not_ready_terminal() {
        let request = AspClientExactQueryRequest {
            schema_id: "agent.semantic-protocols.asp-client-exact-query-request".to_owned(),
            schema_version: "1".to_owned(),
            selector: "rust://src/lib.rs#item/function/ready".to_owned(),
            projection: "source".to_owned(),
        };
        let error = query_generation_not_ready_error(QueryNotReadyContext {
            operation_id: "request-1",
            project_id: "repo-1",
            workspace_id: "workspace-1",
            language_id: "rust",
            provider_id: "asp-rust",
            exact_query: Some(&request),
            generation_state: "submission-failed",
            publication_error: Some("runtime generation admission dispatcher is closed"),
            elapsed_micros: 7,
        })
        .expect("typed readiness submission failure");

        assert_eq!(error.reason_kind, "query-not-ready");
        let terminal = error.details.expect("typed exact-query terminal");
        assert_eq!(terminal["details"]["generationState"], "submission-failed");
        assert_eq!(
            terminal["details"]["publicationError"],
            "runtime generation admission dispatcher is closed"
        );
        assert_eq!(terminal["workCounters"]["providerProcessCount"], 0);
    }

    #[test]
    fn cold_generation_admission_has_no_interactive_dispatch_deadline() {
        assert_eq!(
            dispatch_budget_for_method(
                agent_semantic_client_protocol::WORKSPACE_GENERATION_ENSURE_READY_METHOD
            ),
            None
        );
        assert_eq!(
            dispatch_budget_for_method("rust.query"),
            Some(RUNTIME_CLIENT_DISPATCH_BUDGET)
        );
    }
}

struct QueryNotReadyContext<'a> {
    operation_id: &'a str,
    project_id: &'a str,
    workspace_id: &'a str,
    language_id: &'a str,
    provider_id: &'a str,
    exact_query: Option<&'a AspClientExactQueryRequest>,
    generation_state: &'a str,
    publication_error: Option<&'a str>,
    elapsed_micros: u64,
}

fn query_generation_not_ready_error(
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

struct RuntimeQueryGenerationInstallError {
    message: String,
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

async fn install_runtime_query_generation_terminal(
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

fn request_runtime_query_generation_ready(
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

async fn wait_for_runtime_query_generation(
    receiver: &mut tokio::sync::watch::Receiver<
        Arc<HashMap<RuntimeProjectWorkspaceKey, RuntimeQueryGenerationState>>,
    >,
    key: &RuntimeProjectWorkspaceKey,
    budget: std::time::Duration,
) -> Option<RuntimeQueryGenerationState> {
    if let Some(state) = receiver.borrow().get(key).cloned() {
        return Some(state);
    }
    let deadline = tokio::time::Instant::now() + budget;
    loop {
        match tokio::time::timeout_at(deadline, receiver.changed()).await {
            Ok(Ok(())) => {
                if let Some(state) = receiver.borrow_and_update().get(key).cloned() {
                    return Some(state);
                }
            }
            Ok(Err(_)) | Err(_) => return None,
        }
    }
}

async fn wait_for_runtime_query_generation_change(
    receiver: &mut tokio::sync::watch::Receiver<
        Arc<HashMap<RuntimeProjectWorkspaceKey, RuntimeQueryGenerationState>>,
    >,
    key: &RuntimeProjectWorkspaceKey,
    budget: std::time::Duration,
) -> Option<RuntimeQueryGenerationState> {
    receiver.borrow_and_update();
    let deadline = tokio::time::Instant::now() + budget;
    loop {
        match tokio::time::timeout_at(deadline, receiver.changed()).await {
            Ok(Ok(())) => {
                if let Some(state) = receiver.borrow_and_update().get(key).cloned() {
                    return Some(state);
                }
            }
            Ok(Err(_)) | Err(_) => return None,
        }
    }
}

#[cfg(test)]
#[path = "../tests/unit/runtime_asp_client_recovery.rs"]
mod runtime_asp_client_recovery_tests;

type ClientRequestKey = (
    ClientProjectId,
    ClientWorkspaceIdentity,
    ClientSessionId,
    ClientRequestId,
);
type ClientWorkspaceKey = (String, String, String);

#[derive(Clone)]
struct InitializedWorkspace {
    project_root: std::path::PathBuf,
}

#[derive(Clone)]
pub struct RuntimeAspClientDispatcher {
    schema_bundles: crate::schema_bundle::RuntimeSchemaBundleCatalog,
    agent_session_registry: Arc<agent_semantic_client_db::AgentSessionRegistry>,
    initialized_workspaces: Arc<Mutex<HashMap<ClientWorkspaceKey, InitializedWorkspace>>>,
    runtime_search_service: RuntimeSearchServiceHandle,
    generation_admission: Arc<WorkspaceGenerationAdmission>,
    workspace_registry:
        Arc<agent_semantic_client_db::runtime_server_workspace::RuntimeServerWorkspaceRegistry>,
    installed_provider_targets: Arc<[(String, String)]>,
    workspace_store_root: std::path::PathBuf,
    query_generation_authority: RuntimeQueryGenerationAuthority,
    telemetry_sender: agent_semantic_client_db::runtime_telemetry_bus::RuntimeTelemetryBusSender,
    cancellations: Arc<Mutex<HashMap<ClientRequestKey, tokio::sync::watch::Sender<bool>>>>,
    cancellation_admitted: Arc<tokio::sync::Notify>,
}

impl RuntimeAspClientDispatcher {
    fn new(
        schema_bundles: crate::schema_bundle::RuntimeSchemaBundleCatalog,
        agent_session_registry: Arc<agent_semantic_client_db::AgentSessionRegistry>,
        runtime_search_service: RuntimeSearchServiceHandle,
        generation_admission: Arc<WorkspaceGenerationAdmission>,
        workspace_registry: Arc<
            agent_semantic_client_db::runtime_server_workspace::RuntimeServerWorkspaceRegistry,
        >,
        initialized_workspaces: Arc<Mutex<HashMap<ClientWorkspaceKey, InitializedWorkspace>>>,
        installed_provider_targets: Arc<[(String, String)]>,
        workspace_store_root: std::path::PathBuf,
        query_generation_authority: RuntimeQueryGenerationAuthority,
        telemetry_sender: agent_semantic_client_db::runtime_telemetry_bus::RuntimeTelemetryBusSender,
    ) -> Self {
        Self {
            schema_bundles,
            agent_session_registry,
            runtime_search_service,
            generation_admission,
            workspace_registry,
            initialized_workspaces,
            installed_provider_targets,
            workspace_store_root,
            query_generation_authority,
            telemetry_sender,
            cancellations: Arc::new(Mutex::new(HashMap::new())),
            cancellation_admitted: Arc::new(tokio::sync::Notify::new()),
        }
    }
}

/// Build the sole Runtime-owned public ClientFrame service. HTTP and loopback
/// TCP gRPC bindings both mount this same admission/dispatch owner.
pub fn build_frame_service(
    schema_bundles: crate::schema_bundle::RuntimeSchemaBundleCatalog,
    agent_session_registry: Arc<agent_semantic_client_db::AgentSessionRegistry>,
    runtime_search_service: RuntimeSearchServiceHandle,
    generation_admission: Arc<WorkspaceGenerationAdmission>,
    workspace_registry: Arc<
        agent_semantic_client_db::runtime_server_workspace::RuntimeServerWorkspaceRegistry,
    >,
    client_catalog_generation: String,
    installed_provider_targets: Arc<[(String, String)]>,
    workspace_store_root: std::path::PathBuf,
    query_generation_authority: RuntimeQueryGenerationAuthority,
    telemetry_sender: agent_semantic_client_db::runtime_telemetry_bus::RuntimeTelemetryBusSender,
) -> Result<Arc<AspClientFrameService<RuntimeAspClientDispatcher>>, String> {
    let initialized_workspaces = Arc::new(Mutex::new(HashMap::new()));
    let catalog_generation = client_catalog_generation;
    let catalog_provider_targets = Arc::clone(&installed_provider_targets);
    let dispatcher = Arc::new(RuntimeAspClientDispatcher::new(
        schema_bundles,
        agent_session_registry,
        runtime_search_service,
        Arc::clone(&generation_admission),
        workspace_registry,
        Arc::clone(&initialized_workspaces),
        installed_provider_targets,
        workspace_store_root,
        query_generation_authority,
        telemetry_sender,
    ));
    Ok(Arc::new(AspClientFrameService::new_async(
        dispatcher,
        move |project_id, workspace_id, session_id| {
            let initialized_workspaces = Arc::clone(&initialized_workspaces);
            let catalog_generation = catalog_generation.clone();
            let provider_targets = Arc::clone(&catalog_provider_targets);
            let generation_admission = Arc::clone(&generation_admission);
            async move {
                let project_root = generation_admission
                    .resolve_project_workspace_root(&project_id, &workspace_id)?;
                // Client initialization binds identity only. Language generation admission is
                // intentionally deferred to language methods so Multi-Agent lifecycle calls do
                // not depend on a provider project entry or Source Index generation.
                let workspace_generation = format!(
                    "blake3-256:{}",
                    blake3::hash(format!("{project_id}\0{workspace_id}").as_bytes()).to_hex()
                );
                let key = (project_id, workspace_id, session_id);
                let initialized = InitializedWorkspace {
                    project_root: project_root.clone(),
                };
                let mut workspaces = initialized_workspaces
                    .lock()
                    .map_err(|_| "ASP client workspace-root registry poisoned".to_owned())?;
                if let Some(current) = workspaces.get(&key)
                    && current.project_root != project_root
                {
                    return Err("client session was rebound to a different projectRoot".to_owned());
                }
                workspaces.insert(key, initialized);
                agent_semantic_client_protocol::server_client_catalog(
                    catalog_generation,
                    workspace_generation,
                    vec![agent_semantic_client_protocol::ClientTransport::RuntimeIpc],
                    provider_targets
                        .iter()
                        .map(|(language_id, _)| language_id.clone()),
                )
            }
        },
    )))
}

impl AspClientDispatcher for RuntimeAspClientDispatcher {
    fn dispatch(&self, request: AspClientDispatchRequest) -> AspClientDispatchFuture {
        let key = (
            request.project_id.clone(),
            request.workspace_id.clone(),
            request.session_id.clone(),
            request.request_id.clone(),
        );
        let (cancel, mut cancelled) = tokio::sync::watch::channel(false);
        let admitted = {
            let mut cancellations = self
                .cancellations
                .lock()
                .expect("ASP Client Protocol cancellation registry poisoned");
            match cancellations.entry(key.clone()) {
                Entry::Vacant(entry) => {
                    entry.insert(cancel);
                    true
                }
                Entry::Occupied(_) => false,
            }
        };
        if !admitted {
            return Box::pin(async {
                Err(AspClientDispatchError {
                    reason_kind: "client-request-id-conflict".to_owned(),
                    message: "requestId is already in flight for this client session".to_owned(),
                    details: None,
                })
            });
        }
        self.cancellation_admitted.notify_waiters();

        let schema_bundles = self.schema_bundles.clone();
        let agent_session_registry = Arc::clone(&self.agent_session_registry);
        let initialized_workspaces = Arc::clone(&self.initialized_workspaces);
        let runtime_search_service = self.runtime_search_service.clone();
        let generation_admission = Arc::clone(&self.generation_admission);
        let workspace_registry = Arc::clone(&self.workspace_registry);
        let installed_provider_targets = Arc::clone(&self.installed_provider_targets);
        let workspace_store_root = self.workspace_store_root.clone();
        let query_generation_authority = self.query_generation_authority.clone();
        let mut query_generation = query_generation_authority.subscribe();
        let telemetry_sender = self.telemetry_sender.clone();
        let cancellations = Arc::clone(&self.cancellations);
        let dispatch_budget = dispatch_budget_for_method(&request.method);
        Box::pin(async move {
            let project_workspace_key = RuntimeProjectWorkspaceKey::new(
                request.project_id.clone(),
                request.workspace_id.clone(),
            );
            let operation = async {
                if request.method == agent_semantic_client_protocol::CANCELLATION_PROBE_METHOD {
                    return std::future::pending::<
                        Result<serde_json::Value, AspClientOperationError>,
                    >()
                    .await;
                }
                if request.method == AGENT_SESSION_REGISTER_METHOD {
                    let params: AgentSessionRegisterRequest =
                        serde_json::from_value(request.params).map_err(|error| {
                            format!("decode child registration request: {error}")
                        })?;
                    params.validate()?;
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
                            "ASP client request requires an initialized workspace root".to_owned()
                        })?;
                    let state = agent_semantic_client_core::state_core::ResolvedState::resolve(
                        &initialized.project_root,
                    )?;
                    if state.repo.repo_id.as_str() != request.project_id.as_str()
                        || state.workspace.workspace_id.as_str() != request.workspace_id.as_str()
                    {
                        return Err(AspClientOperationError::Message(
                            "child registration project/workspace identity mismatch".to_owned(),
                        ));
                    }
                    let now = agent_semantic_client_db::agent_session_unix_timestamp()?;
                    let metadata = serde_json::json!({
                        "event": "child-registration-command",
                        "schemaId": AGENT_SESSION_REGISTER_RESPONSE_SCHEMA_ID,
                        "schemaVersion": 1,
                        "platform": "codex",
                        "nativeEnvironment": true,
                        "rootSessionId": params.root_session_id,
                        "parentThreadId": params.parent_thread_id,
                        "childThreadId": params.child_thread_id,
                        "agentType": params.agent_name,
                        "configuredRoute": params.route_key,
                        "messageTargetBinding": {
                            "source": "codex-child-registration-command",
                            "boundRootSessionId": params.root_session_id,
                            "childThreadId": params.child_thread_id,
                            "messageTargetId": params.agent_path
                        }
                    });
                    let registered = agent_session_registry
                        .register_session_from_runtime_owner(
                            agent_semantic_client_db::agent_session_registry::AgentSessionRegisterRequest {
                                project_id: request.project_id.as_str().into(),
                                root_session_id: params.root_session_id.as_str().into(),
                                session_id: params.child_thread_id.as_str().into(),
                                message_target_id: Some(params.agent_path.as_str().into()),
                                parent_session_id: Some(params.parent_thread_id.as_str().into()),
                                name: params.route_key.as_str().into(),
                                role: params.route_key.as_str().into(),
                                model_observation: None,
                                status: "active".into(),
                                expires_at: None,
                                metadata_json: metadata.to_string().into(),
                                now,
                            },
                        )
                        .await?;
                    let physical_generation =
                        registered.physical_generation.try_into().map_err(|_| {
                            "child registration physical generation is invalid".to_owned()
                        })?;
                    let receipt = AgentSessionRegisterReceipt {
                        schema_id: AGENT_SESSION_REGISTER_RESPONSE_SCHEMA_ID.to_owned(),
                        schema_version: 1,
                        state: "registered".to_owned(),
                        platform: "codex".to_owned(),
                        project_id: request.project_id.as_str().to_owned(),
                        root_session_id: params.root_session_id,
                        parent_thread_id: params.parent_thread_id,
                        child_thread_id: params.child_thread_id,
                        agent_name: params.agent_name,
                        agent_path: params.agent_path,
                        route_key: params.route_key,
                        physical_generation,
                        registry_owner: "runtime-server-agent-session-registry".to_owned(),
                        transport: "grpc-client-frame".to_owned(),
                    };
                    receipt.validate()?;
                    return serde_json::to_value(receipt)
                        .map_err(|error| error.to_string())
                        .map_err(AspClientOperationError::Message);
                }
                if request.method == SCHEMA_BUNDLE_METHOD {
                    let params: SchemaBundleRequest = serde_json::from_value(request.params)
                        .map_err(|error| format!("decode schema bundle request: {error}"))?;
                    params.validate()?;
                    let response = schema_bundles.project(&params);
                    response.validate()?;
                    return serde_json::to_value(response.as_ref())
                        .map_err(|error| error.to_string())
                        .map_err(AspClientOperationError::Message);
                }
                if request.method == LIVE_CORPUS_CACHE_STATE_METHOD {
                    let started = tokio::time::Instant::now();
                    let params: LiveCorpusCacheStateRequest =
                        serde_json::from_value(request.params).map_err(|error| {
                            format!("decode Live Corpus cache-state request: {error}")
                        })?;
                    params.validate()?;
                    let installed_provider_matches =
                        installed_provider_targets
                            .iter()
                            .any(|(language_id, provider_id)| {
                                language_id == &params.language_id
                                    && provider_id == &params.provider_id
                            });
                    if !installed_provider_matches {
                        return Err(AspClientOperationError::Message(format!(
                            "Live Corpus cache-state provider identity is not installed: languageId={} providerId={}",
                            params.language_id, params.provider_id
                        )));
                    }
                    let workspace_identity = request.workspace_id.as_str();
                    let resident_generation_evicted = match params.cache_state.as_str() {
                        "cold-build" => {
                            query_generation_authority
                                .require_workspace_absent(&project_workspace_key)?;
                            false
                        }
                        "cold-load" => {
                            let expected_generation_digest = params
                                .expected_generation_digest
                                .as_deref()
                                .expect("validated cold-load generation digest");
                            let expected_root_digest = params
                                .expected_root_digest
                                .as_deref()
                                .expect("validated cold-load root digest");
                            query_generation_authority.evict_ready_exact(
                                &project_workspace_key,
                                expected_generation_digest,
                                expected_root_digest,
                            )?;
                            let initialized = initialized_workspaces
                                .lock()
                                .map_err(|_| {
                                    "ASP client workspace-root registry poisoned".to_owned()
                                })?
                                .get(&(
                                    request.project_id.as_str().to_owned(),
                                    workspace_identity.to_owned(),
                                    request.session_id.as_str().to_owned(),
                                ))
                                .cloned()
                                .ok_or_else(|| {
                                    "Live Corpus cache state requires an initialized workspace root"
                                        .to_owned()
                                })?;
                            let pointer_path = agent_semantic_client_db::runtime_server_workspace::workspace_generation_pointer_path(
                                &workspace_store_root,
                                workspace_identity,
                                &initialized.project_root,
                            )?;
                            let reopened = query_generation_authority
                                .ensure_ready(
                                    &project_workspace_key,
                                    &pointer_path,
                                    &initialized.project_root,
                                    expected_generation_digest,
                                )
                                .await?;
                            if reopened.resident().source_root_digest() != expected_root_digest {
                                return Err(AspClientOperationError::Message(
                                    "Live Corpus cold-load reopened a different source root"
                                        .to_owned(),
                                ));
                            }
                            true
                        }
                        "warm-read" => {
                            query_generation_authority.verify_ready_exact(
                                &project_workspace_key,
                                params
                                    .expected_generation_digest
                                    .as_deref()
                                    .expect("validated warm-read generation digest"),
                                params
                                    .expected_root_digest
                                    .as_deref()
                                    .expect("validated warm-read root digest"),
                            )?;
                            false
                        }
                        "released" => {
                            query_generation_authority.evict_ready_exact(
                                &project_workspace_key,
                                params
                                    .expected_generation_digest
                                    .as_deref()
                                    .expect("validated release generation digest"),
                                params
                                    .expected_root_digest
                                    .as_deref()
                                    .expect("validated release root digest"),
                            )?;
                            true
                        }
                        _ => unreachable!("validated Live Corpus cache state"),
                    };
                    let receipt = LiveCorpusCacheStateReceipt {
                        schema_id: agent_semantic_client_protocol::LIVE_CORPUS_CACHE_STATE_RECEIPT_SCHEMA_ID
                            .to_owned(),
                        schema_version: "1".to_owned(),
                        operation_id: params.operation_id,
                        state: "ready".to_owned(),
                        cache_state: params.cache_state,
                        project_id: request.project_id.as_str().to_owned(),
                        workspace_id: workspace_identity.to_owned(),
                        generation_digest: params.expected_generation_digest,
                        root_digest: params.expected_root_digest,
                        resident_generation_evicted,
                        client_session_evicted: false,
                        source_workspace_mutation_count: 0,
                        filesystem_delete_count: 0,
                        elapsed_micros: elapsed_micros(started),
                    };
                    receipt.validate()?;
                    return serde_json::to_value(receipt)
                        .map_err(|error| error.to_string())
                        .map_err(AspClientOperationError::Message);
                }
                if request.method == GRAPH_TIMELINE_METHOD {
                    let params: agent_semantic_client_protocol::AspClientGraphsTimelineRequest =
                        serde_json::from_value(request.params).map_err(|error| {
                            AspClientOperationError::Message(format!(
                                "decode graph timeline request: {error}"
                            ))
                        })?;
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
                            "ASP client request requires an initialized workspace root".to_owned()
                        })?;
                    let cancellation =
                        agent_semantic_client_db::runtime_generation_cancellation::GenerationCancellation::new();
                    let timeline_payload = serde_json::json!({
                        "schemaId": params.schema_id,
                        "schemaVersion": params.schema_version,
                        "eventPacket": params.event_packet,
                        "arguments": params.arguments,
                    });
                    return runtime_search_service
                        .graphs_timeline(
                            initialized.project_root,
                            request.request_id.as_str().to_owned(),
                            timeline_payload,
                            cancellation,
                        )
                        .await
                        .map_err(AspClientOperationError::Message);
                }
                if request.method
                    == agent_semantic_client_protocol::WORKSPACE_GENERATION_ENSURE_READY_METHOD
                {
                    #[derive(serde::Deserialize)]
                    #[serde(rename_all = "camelCase", deny_unknown_fields)]
                    struct EnsureReadyParams {
                        language_id: Option<String>,
                    }
                    let params: EnsureReadyParams = serde_json::from_value(request.params)
                        .map_err(|error| {
                            AspClientOperationError::Message(format!(
                                "invalid workspace generation ensure-ready parameters: {error}"
                            ))
                        })?;
                    let provider_target = match params.language_id {
                        None => None,
                        Some(language_id) => {
                            let provider_id = installed_provider_targets
                                .iter()
                                .find_map(|(installed_language_id, provider_id)| {
                                    (installed_language_id == &language_id)
                                        .then(|| provider_id.clone())
                                })
                                .ok_or_else(|| {
                                    AspClientOperationError::Message(format!(
                                        "installed provider target missing for languageId={language_id}"
                                    ))
                                })?;
                            Some(agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationProviderTarget {
                                language_id,
                                provider_id: Some(provider_id),
                            })
                        }
                    };
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
                            "workspace generation ensure-ready requires an initialized workspace root"
                                .to_owned()
                        })?;
                    let terminal = generation_admission
                        .ensure_runtime_generation_ready_for_provider(
                            request.workspace_id.as_str().to_owned(),
                            initialized.project_root.clone(),
                            provider_target,
                        )
                        .await?;
                    install_runtime_query_generation_terminal(
                        &terminal,
                        &query_generation_authority,
                        workspace_registry.as_ref(),
                        &project_workspace_key,
                        request.workspace_id.as_str(),
                        &initialized.project_root,
                    )
                    .await
                    .map_err(|error| error.message)?;
                    return serde_json::to_value(terminal)
                        .map_err(|error| error.to_string())
                        .map_err(AspClientOperationError::Message);
                }
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
                    let validated_params = ResidentGraphEvaluationRequestV1::from_value(
                        request.params,
                    )
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
                            format!(
                                "installed provider target missing for languageId={language_id}"
                            )
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
                            "ASP client graph request requires an initialized workspace root"
                                .to_owned()
                        })?
                        .project_root
                        .clone();
                    let dispatch_started = tokio::time::Instant::now();
                    let generation_state = query_generation
                        .borrow()
                        .get(&project_workspace_key)
                        .cloned();
                    let generation = match generation_state {
                        Some(RuntimeQueryGenerationState::Ready(generation)) => generation,
                        Some(RuntimeQueryGenerationState::Failed { reason, .. }) => {
                            let readiness_submission = request_runtime_query_generation_ready(
                                generation_admission.as_ref(),
                                &query_generation_authority,
                                &workspace_registry,
                                &project_workspace_key,
                                request.workspace_id.as_str().to_owned(),
                                project_root.clone(),
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
                let agent_semantic_client_protocol::ResolvedServerClientMethod::Language {
                    language_id,
                    route,
                } = resolved
                else {
                    return Err(AspClientOperationError::Message(
                        "unsupported server-owned client method".to_owned(),
                    ));
                };
                let provider_id = installed_provider_targets
                    .iter()
                    .find_map(|(installed_language_id, provider_id)| {
                        (installed_language_id == &language_id).then(|| provider_id.clone())
                    })
                    .ok_or_else(|| {
                        format!("installed provider target missing for languageId={language_id}")
                    })?;
                let exact_query_params = if matches!(&route, ServerClientRoute::ExactQuery) {
                    let params: AspClientExactQueryRequest =
                        serde_json::from_value(request.params.clone())
                            .map_err(|error| error.to_string())?;
                    params.validate_schema_identity()?;
                    Some(params)
                } else {
                    None
                };
                let search_params = if matches!(&route, ServerClientRoute::Search) {
                    let params: AspClientSearchRequest =
                        serde_json::from_value(request.params.clone())
                            .map_err(|error| error.to_string())?;
                    params.validate_schema_identity()?;
                    Some(params)
                } else {
                    None
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
                    .ok_or_else(|| {
                        "ASP client request requires an initialized workspace root".to_owned()
                    })?
                    .project_root
                    .clone();
                let params = request.params;
                let generation_state = query_generation
                    .borrow()
                    .get(&project_workspace_key)
                    .cloned();
                let generation = 'generation_resolution: {
                    match generation_state {
                        Some(RuntimeQueryGenerationState::Ready(generation)) => generation,
                        Some(RuntimeQueryGenerationState::Failed { reason, .. }) => {
                            let readiness_submission = request_runtime_query_generation_ready(
                                generation_admission.as_ref(),
                                &query_generation_authority,
                                &workspace_registry,
                                &project_workspace_key,
                                request.workspace_id.as_str().to_owned(),
                                project_root.clone(),
                            );
                            let submission_error = readiness_submission.err();
                            if submission_error.is_none()
                                && let Some(search) = search_params.as_ref()
                            {
                                let requested =
                                    std::time::Duration::from_millis(search.deadline_ms);
                                let remaining = requested
                                    .min(RUNTIME_CLIENT_DISPATCH_BUDGET)
                                    .saturating_sub(dispatch_started.elapsed());
                                if let Some(observed) = wait_for_runtime_query_generation_change(
                                    &mut query_generation,
                                    &project_workspace_key,
                                    remaining,
                                )
                                .await
                                {
                                    match observed {
                                        RuntimeQueryGenerationState::Ready(generation) => {
                                            break 'generation_resolution generation;
                                        }
                                        RuntimeQueryGenerationState::Failed { reason, .. } => {
                                            return Err(AspClientOperationError::Terminal(
                                                query_generation_not_ready_error(
                                                    QueryNotReadyContext {
                                                        operation_id: request.request_id.as_str(),
                                                        project_id: request.project_id.as_str(),
                                                        workspace_id: request.workspace_id.as_str(),
                                                        language_id: &language_id,
                                                        provider_id: &provider_id,
                                                        exact_query: None,
                                                        generation_state: "failed",
                                                        publication_error: Some(reason.as_ref()),
                                                        elapsed_micros: elapsed_micros(
                                                            dispatch_started,
                                                        ),
                                                    },
                                                )?,
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
                                            exact_query: None,
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
                            if readiness_submission_error.is_none()
                                && let Some(search) = search_params.as_ref()
                            {
                                let requested =
                                    std::time::Duration::from_millis(search.deadline_ms);
                                let remaining = requested
                                    .min(RUNTIME_CLIENT_DISPATCH_BUDGET)
                                    .saturating_sub(dispatch_started.elapsed());
                                if let Some(observed) = wait_for_runtime_query_generation(
                                    &mut query_generation,
                                    &project_workspace_key,
                                    remaining,
                                )
                                .await
                                {
                                    match observed {
                                        RuntimeQueryGenerationState::Ready(generation) => {
                                            break 'generation_resolution generation;
                                        }
                                        RuntimeQueryGenerationState::Failed { reason, .. } => {
                                            return Err(AspClientOperationError::Terminal(
                                                query_generation_not_ready_error(
                                                    QueryNotReadyContext {
                                                        operation_id: request.request_id.as_str(),
                                                        project_id: request.project_id.as_str(),
                                                        workspace_id: request.workspace_id.as_str(),
                                                        language_id: &language_id,
                                                        provider_id: &provider_id,
                                                        exact_query: None,
                                                        generation_state: "failed",
                                                        publication_error: Some(reason.as_ref()),
                                                        elapsed_micros: elapsed_micros(
                                                            dispatch_started,
                                                        ),
                                                    },
                                                )?,
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
                                            exact_query: None,
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
                match route {
                    agent_semantic_client_protocol::ServerClientRoute::AgentSessionRegister => {
                        unreachable!(
                            "server-owned AgentSession route is handled before language dispatch"
                        )
                    }
                    agent_semantic_client_protocol::ServerClientRoute::MultiAgentHostEvent => {
                        unreachable!("server-owned Host event route requires its typed dispatcher")
                    }
                    agent_semantic_client_protocol::ServerClientRoute::MultiAgentChildren => {
                        unreachable!(
                            "server-owned child projection route requires its typed dispatcher"
                        )
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
                        unreachable!(
                            "workspace generation ensure-ready is handled before language dispatch"
                        )
                    }
                    agent_semantic_client_protocol::ServerClientRoute::Search => {
                        let params = search_params.expect("Search route decoded its request");
                        let language =
                            agent_semantic_client_core::LanguageId::try_from(language_id.as_str())
                                .map_err(|error| format!("decode language id: {error}"))?;
                        let authority = agent_semantic_search::ResidentSearchAuthority {
                            language_id: language.clone(),
                            provider_id: provider_id.as_str().into(),
                        };
                        let intent = agent_semantic_search::ResidentSearchIntent::parse(&params.intent)
                        .map_err(AspClientOperationError::Message)?;
                        let mut fusion_capabilities = generation.fusion_capabilities();
                        if intent == agent_semantic_search::ResidentSearchIntent::Relationship {
                            // The optional Python worker is admitted lazily below under the
                            // exact immutable generation request. Its successful typed receipt,
                            // not the base generation receipt, proves this capability.
                            fusion_capabilities.python_graph = true;
                        }
                        let execution_plan = agent_semantic_search::plan_resident_search_execution(
                            intent,
                            fusion_capabilities,
                        )
                        .map_err(|message| {
                            AspClientOperationError::Terminal(AspClientDispatchError {
                                reason_kind: "query-not-ready".to_owned(),
                                message,
                                details: Some(serde_json::json!({
                                    "phase": "resident-fusion-plan",
                                    "intent": intent.as_str(),
                                })),
                            })
                        })?;
                        let resident_started = tokio::time::Instant::now();
                        let owner_scope = params.scope.strip_prefix("owner:");
                        let mut cold_rg = None;
                        let lookup = if execution_plan.use_cold_rg_candidates {
                            let remaining = std::time::Duration::from_millis(params.deadline_ms)
                                .min(RUNTIME_CLIENT_DISPATCH_BUDGET)
                                .saturating_sub(dispatch_started.elapsed());
                            let cold = crate::runtime_cold_rg::execute_runtime_cold_rg(
                                generation.resident().cold_rg_corpus(),
                                &params.query,
                                params.max_owners,
                                remaining,
                            )
                            .await
                            .map_err(|message| {
                                AspClientOperationError::Terminal(AspClientDispatchError {
                                    reason_kind: "cold-rg-execution-failed".to_owned(),
                                    message,
                                    details: Some(serde_json::json!({
                                        "phase": "immutable-generation-cold-rg",
                                        "intent": intent.as_str(),
                                    })),
                                })
                            })?;
                            let mut owner_paths = cold.candidate_owner_paths.clone();
                            if let Some(owner_path) = owner_scope {
                                owner_paths.retain(|candidate| candidate == owner_path);
                            }
                            let lookup = generation.resident().read_cold_rg_candidates(
                                &params.query,
                                &owner_paths,
                                Some(&authority),
                                params.max_owners,
                            );
                            cold_rg = Some(agent_semantic_search::SearchPlaybookColdRgExecution {
                                generation_digest: cold.content_generation_digest,
                                coverage_input_digest: cold.coverage_input_digest,
                                candidate_owner_ids: owner_paths,
                                elapsed_micros: cold.elapsed_micros,
                                process_count: cold.process_count,
                            });
                            lookup
                        } else {
                            match (execution_plan.verify_rg_bytes, owner_scope) {
                            (true, Some(owner_path)) => generation
                                .resident()
                                .read_byte_evidence_for_owner_scope(
                                    &params.query,
                                    owner_path,
                                    Some(&authority),
                                    params.max_owners,
                                ),
                            (true, None) => generation.resident().read_byte_evidence(
                                &params.query,
                                Some(&authority),
                                params.max_owners,
                            ),
                            (false, Some(owner_path)) => generation
                                .resident()
                                .read_source_index_for_owner_scope(
                                    &params.query,
                                    owner_path,
                                    Some(&authority),
                                    params.max_owners,
                                ),
                            (false, None) => generation.resident().read_source_index(
                                &params.query,
                                Some(&authority),
                                params.max_owners,
                            ),
                            }
                        }
                        .map_err(|message| {
                            let reason_kind = if message.starts_with("query-not-ready:") {
                                "query-not-ready"
                            } else {
                                "resident-search-read-failed"
                            };
                            AspClientOperationError::Terminal(AspClientDispatchError {
                                reason_kind: reason_kind.to_owned(),
                                message,
                                details: Some(serde_json::json!({
                                    "phase": "resident-fused-read",
                                    "intent": intent.as_str(),
                                })),
                            })
                        })?;
                        let resident_read_elapsed_micros = elapsed_micros(resident_started);
                        let graph_stage = if execution_plan.project_resident_graph {
                            crate::runtime_search_graph::rank_resident_search_frontier(
                                request.request_id.as_str(),
                                intent,
                                &params.query,
                                &language_id,
                                &provider_id,
                                generation.generation_digest(),
                                generation.resident(),
                                &lookup.hits,
                            )
                            .map_err(|error| {
                                AspClientOperationError::Terminal(AspClientDispatchError {
                                    reason_kind: error.reason_kind.to_owned(),
                                    message: error.message,
                                    details: error.details,
                                })
                            })?
                        } else {
                            agent_semantic_search::ResidentGraphSearchStage {
                                generation_digest: generation.generation_digest().to_owned(),
                                result_digest: format!(
                                    "blake3-256:{}",
                                    blake3::hash(b"asp.search.resident-graph.unavailable.v1")
                                        .to_hex()
                                ),
                                ranked_owner_paths: Vec::new(),
                                elapsed_micros: 0,
                                work: agent_semantic_search::ResidentGraphSearchWork::default(),
                            }
                        };
                        let python_graph = if execution_plan.require_python_graph {
                            Some(
                                crate::runtime_search_graph::evaluate_python_relationship_graph(
                                    request.request_id.as_str(),
                                    &language_id,
                                    &params.query,
                                    generation.resident(),
                                    &lookup.hits,
                                    &runtime_search_service,
                                )
                                .await
                                .map_err(|error| {
                                    AspClientOperationError::Terminal(AspClientDispatchError {
                                        reason_kind: error.reason_kind.to_owned(),
                                        message: error.message,
                                        details: error.details,
                                    })
                                })?,
                            )
                        } else {
                            None
                        };
                        let mut selector_owner_paths =
                            agent_semantic_search::bounded_ranked_selector_owner_paths(
                                &lookup.hits,
                                execution_plan.project_resident_graph.then_some(&graph_stage),
                            );
                        if let Some(python_graph) = &python_graph {
                            selector_owner_paths
                                .extend(python_graph.candidate_owner_ids.iter().cloned());
                            selector_owner_paths.sort();
                            selector_owner_paths.dedup();
                            selector_owner_paths.truncate(
                                agent_semantic_search::RUNTIME_SEARCH_SELECTOR_OWNER_LIMIT,
                            );
                        }
                        let parser_owned_selector_pairs = generation
                            .parser_owned_callable_selector_pairs(&selector_owner_paths)?;
                        let native_syntax_started = tokio::time::Instant::now();
                        let fused_generation = &generation
                            .resident()
                            .search_generation_authority()
                            .content_search_generation;
                        fused_generation.validate()?;
                        let native_syntax_state = generation.native_syntax_state();
                        let (native_syntax_projections, native_syntax_relations) =
                            if native_syntax_state == "ready" {
                                generation.native_syntax_playbook_projection(&selector_owner_paths)?
                            } else {
                                (Vec::new(), Vec::new())
                            };
                        let native_syntax_stage_artifact_digest =
                            if native_syntax_state == "ready" {
                                agent_semantic_search::build_native_syntax_stage(
                                    fused_generation.identity().clone(),
                                    native_syntax_projections.clone(),
                                    native_syntax_relations.clone(),
                                )?
                                .artifact_digest
                            } else {
                                format!(
                                    "blake3-256:{}",
                                    blake3::hash(
                                        format!(
                                            "native-syntax-generation-v1\0{}\0{native_syntax_state}",
                                            fused_generation.content_generation_digest
                                        )
                                        .as_bytes()
                                    )
                                    .to_hex()
                                )
                            };
                        let native_syntax_elapsed_micros =
                            elapsed_micros(native_syntax_started);
                        record_runtime_route_performance(
                            &telemetry_sender,
                            request.workspace_id.as_str(),
                            &language_id,
                            generation.generation_digest(),
                            request.request_id.as_str(),
                            "search",
                            "runtime-graph-rank",
                            &graph_stage.result_digest,
                            graph_stage.elapsed_micros,
                        )?;
                        record_runtime_route_performance(
                            &telemetry_sender,
                            request.workspace_id.as_str(),
                            &language_id,
                            generation.generation_digest(),
                            request.request_id.as_str(),
                            "search",
                            "runtime-source-index-read",
                            &params.intent,
                            resident_read_elapsed_micros,
                        )?;
                        let runtime_receipt = agent_semantic_search::build_runtime_provider_search_receipt_with_graph(
                            request.request_id.as_str().to_owned(),
                            language,
                            vec![agent_semantic_search::RuntimeSearchSource::shared(
                                "resident", std::sync::Arc::clone(&lookup),
                            )],
                            resident_read_elapsed_micros,
                            parser_owned_selector_pairs,
                            execution_plan
                                .project_resident_graph
                                .then_some(graph_stage.clone()),
                        )
                        .await?;
                        let receipt = agent_semantic_search::build_search_playbook_receipt(
                            agent_semantic_search::SearchPlaybookReceiptInput {
                                workspace_identity: request.workspace_id.as_str().to_owned(),
                                query: params.query,
                                intent: params.intent,
                                coverage: params.coverage,
                                max_owners: params.max_owners,
                                deadline_ms: params.deadline_ms,
                                indexed_owner_count: generation.resident().indexed_owner_count(),
                                indexed_lexical_executed: execution_plan.use_tantivy_candidates,
                                byte_evidence_executed: execution_plan.verify_rg_bytes,
                                cold_rg,
                                resident_graph_executed: execution_plan.project_resident_graph,
                                source_acquisition_stage_artifact_digest: fused_generation
                                    .acquisition
                                    .artifact_digest
                                    .clone(),
                                native_syntax_state: native_syntax_state.to_owned(),
                                native_syntax_stage_artifact_digest,
                                native_syntax_projections,
                                native_syntax_relations,
                                native_syntax_elapsed_micros,
                                runtime: runtime_receipt,
                                graph: graph_stage,
                                python_graph,
                            },
                        )?;
                        Ok(serde_json::to_value(receipt)
                            .map_err(|error| format!("encode search receipt: {error}"))?)
                    }
                    agent_semantic_client_protocol::ServerClientRoute::SourceIndexLookup => {
                        let started = tokio::time::Instant::now();
                        let params: agent_semantic_client_protocol::AspClientSourceIndexLookupRequest =
                            serde_json::from_value(params).map_err(|error| {
                                format!("decode ASP client source-index lookup request: {error}")
                            })?;
                        params.validate_schema_identity()?;
                        if params.query.trim().is_empty() || !(1..=100).contains(&params.limit) {
                            return Err(AspClientOperationError::Message(
                                "source-index lookup requires a non-empty query and limit in 1..=100"
                                    .to_owned(),
                            ));
                        }
                        let requested_root = std::path::PathBuf::from(&params.index_root)
                            .canonicalize()
                            .map_err(|error| {
                                format!("canonicalize source-index indexRoot: {error}")
                            })?;
                        let admitted_root = project_root.canonicalize().map_err(|error| {
                            format!("canonicalize admitted workspace root: {error}")
                        })?;
                        if requested_root != admitted_root {
                            return Err(AspClientOperationError::Terminal(
                                AspClientDispatchError {
                                    reason_kind: "source-index-workspace-mismatch".to_owned(),
                                    message:
                                        "source-index indexRoot is not the initialized workspace"
                                            .to_owned(),
                                    details: Some(serde_json::json!({
                                        "requestedIndexRoot": requested_root,
                                        "admittedWorkspaceRoot": admitted_root,
                                    })),
                                },
                            ));
                        }
                        let language =
                            agent_semantic_client_core::LanguageId::try_from(language_id.as_str())
                                .map_err(|error| format!("decode language id: {error}"))?;
                        let authority = agent_semantic_search::ResidentSearchAuthority {
                            language_id: language,
                            provider_id: provider_id.as_str().into(),
                        };
                        let lookup = generation.resident().read_source_index(
                            &params.query,
                            Some(&authority),
                            params.limit,
                        )?;
                        record_runtime_route_performance(
                            &telemetry_sender,
                            request.workspace_id.as_str(),
                            &language_id,
                            generation.generation_digest(),
                            request.request_id.as_str(),
                            "source-index",
                            "runtime-source-index-read",
                            "lookup",
                            elapsed_micros(started),
                        )?;
                        Ok(serde_json::to_value(lookup.as_ref())
                            .map_err(|error| format!("encode source-index lookup: {error}"))?)
                    }
                    agent_semantic_client_protocol::ServerClientRoute::ExactQuery => {
                        let started = tokio::time::Instant::now();
                        let params: AspClientExactQueryRequest = serde_json::from_value(params)
                            .map_err(|error| {
                                format!("decode ASP client exact-query request: {error}")
                            })?;
                        params.validate_schema_identity()?;
                        let projection_kind = agent_semantic_client_db::runtime_server_workspace::ExactProjectionKind::try_from(
                            params.projection.as_str(),
                        )?;
                        let resident_started = tokio::time::Instant::now();
                        if generation.native_syntax_state() != "ready" {
                            return Err(AspClientOperationError::Terminal(
                                AspClientDispatchError {
                                    reason_kind: "native-syntax-not-ready".to_owned(),
                                    message: "parser-owned exact projections are not ready for the admitted content generation".to_owned(),
                                    details: Some(serde_json::json!({
                                        "projectId": request.project_id.as_str(),
                                        "workspaceId": request.workspace_id.as_str(),
                                        "generationDigest": generation.generation_digest(),
                                        "contentGenerationDigest": generation.content_generation_digest(),
                                        "attachmentState": generation.native_syntax_state(),
                                    })),
                                },
                            ));
                        }
                        let projection = generation
                            .read_runtime_selector(projection_kind, &params.selector)?;
                        let resident_read_elapsed_micros = elapsed_micros(resident_started);
                        let elapsed_micros = elapsed_micros(started);
                        let service_elapsed_micros =
                            elapsed_micros.saturating_sub(resident_read_elapsed_micros);
                        record_runtime_route_performance(
                            &telemetry_sender,
                            request.workspace_id.as_str(),
                            &language_id,
                            generation.generation_digest(),
                            request.request_id.as_str(),
                            "query",
                            "runtime-selector-read",
                            projection_kind.as_str(),
                            elapsed_micros,
                        )?;
                        if let Some(failure) = classify_exact_query_failure(&projection) {
                            let selector_details = serde_json::to_value(&projection)
                                .map_err(|error| error.to_string())?;
                            let selector_state = selector_details
                                .get("state")
                                .and_then(serde_json::Value::as_str)
                                .ok_or_else(|| {
                                    "Runtime selector failure has no typed state".to_owned()
                                })?;
                            let failure_terminal = AspClientExactQueryFailure {
                                schema_id:
                                    "agent.semantic-protocols.asp-client-exact-query-failure"
                                        .to_owned(),
                                schema_version: "1".to_owned(),
                                state: "failed".to_owned(),
                                operation_id: request.request_id.as_str().to_owned(),
                                project_id: request.project_id.as_str().to_owned(),
                                workspace_id: request.workspace_id.as_str().to_owned(),
                                language_id: language_id.clone(),
                                provider_id: provider_id.clone(),
                                requested_selector: Some(params.selector),
                                resolved_selector: failure.resolved_selector,
                                projection_kind: Some(params.projection),
                                phase: "resident-selector-read".to_owned(),
                                reason_kind: failure.reason_kind.to_owned(),
                                generation_digest: Some(generation.generation_digest().to_owned()),
                                root_digest: Some(
                                    generation.resident().owner_merkle_root_digest(),
                                ),
                                recommended_next: failure.recommended_next,
                                resident_read_elapsed_micros,
                                service_elapsed_micros,
                                elapsed_micros,
                                work_counters: AspClientRuntimeWorkCounters::default(),
                                details: serde_json::json!({
                                    "selectorState": selector_state,
                                    "selectorRead": selector_details,
                                }),
                            };
                            failure_terminal.validate()?;
                            let details = serde_json::to_value(failure_terminal)
                                .map_err(|error| error.to_string())?;
                            return Err(AspClientOperationError::Terminal(
                                AspClientDispatchError {
                                    reason_kind: failure.reason_kind.to_owned(),
                                    message: format!(
                                        "exact projection terminal: {}",
                                        failure.reason_kind
                                    ),
                                    details: Some(details),
                                },
                            ));
                        }
                        let response = AspClientExactQueryResponse {
                            schema_id: "agent.semantic-protocols.asp-client-exact-query-response"
                                .to_owned(),
                            schema_version: "1".to_owned(),
                            operation_id: request.request_id.as_str().to_owned(),
                            project_id: request.project_id.as_str().to_owned(),
                            workspace_id: request.workspace_id.as_str().to_owned(),
                            language_id: language_id.clone(),
                            provider_id: provider_id.clone(),
                            generation_digest: generation.generation_digest().to_owned(),
                            root_digest: generation.resident().owner_merkle_root_digest(),
                            result: serde_json::to_value(projection)
                                .map_err(|error| format!("encode query result: {error}"))?,
                            resident_read_elapsed_micros,
                            service_elapsed_micros,
                            elapsed_micros,
                            work_counters: AspClientRuntimeWorkCounters::default(),
                        };
                        response.validate()?;
                        Ok(serde_json::to_value(response)
                            .map_err(|error| format!("encode query response: {error}"))?)
                    }
                }
            };
            let result = tokio::select! {
                            result = operation => result.map_err(|error| match error {
                                AspClientOperationError::Message(message) => AspClientDispatchError {
                                    reason_kind: "client-method-dispatch-failed".to_owned(),
                                    message,
                                    details: None,
                                },
                                AspClientOperationError::Terminal(error) => error,
                            }),
                            _ = async {
                                match dispatch_budget {
                                    Some(budget) => tokio::time::sleep(budget).await,
                                    None => std::future::pending::<()>().await,
                                }
                            } => {
            let budget = dispatch_budget.expect("deadline branch requires an interactive budget");
            Err(AspClientDispatchError {
                reason_kind: "client-request-deadline-exceeded".to_owned(),
                message: format!(
                    "Runtime ClientFrame dispatch exceeded its {}ms interactive deadline",
                    budget.as_millis(),
                ),
                details: Some(serde_json::json!({
                    "schemaId": "agent.semantic-protocols.asp-client-dispatch-failure",
                    "schemaVersion": "1",
                    "state": "failed",
                    "phase": "runtime-client-dispatch",
                    "reasonKind": "client-request-deadline-exceeded",
                    "budgetMs": budget.as_millis(),
                })),
            })
                            }
                            changed = cancelled.changed() => {
                                let _ = changed;
            Err(AspClientDispatchError {
                reason_kind: "client-request-cancelled".to_owned(),
                message: "client request was cancelled".to_owned(),
                details: None,
            })
                            }
                        };
            cancellations
                .lock()
                .expect("ASP Client Protocol cancellation registry poisoned")
                .remove(&key);
            result
        })
    }

    fn cancel(
        &self,
        project_id: &ClientProjectId,
        workspace_identity: &ClientWorkspaceIdentity,
        session_id: &ClientSessionId,
        request_id: &ClientRequestId,
    ) -> agent_semantic_client_server::AspClientCancelFuture {
        let key = (
            project_id.clone(),
            workspace_identity.clone(),
            session_id.clone(),
            request_id.clone(),
        );
        let cancellations = Arc::clone(&self.cancellations);
        let cancellation_admitted = Arc::clone(&self.cancellation_admitted);
        Box::pin(async move {
            loop {
                let admitted = cancellation_admitted.notified();
                let sender = cancellations
                    .lock()
                    .expect("ASP Client Protocol cancellation registry poisoned")
                    .remove(&key);
                if let Some(sender) = sender {
                    return sender.send(true).is_ok();
                }
                admitted.await;
            }
        })
    }
}

fn elapsed_micros(started: tokio::time::Instant) -> u64 {
    started.elapsed().as_micros().min(u128::from(u64::MAX)) as u64
}

#[allow(clippy::too_many_arguments)]
fn record_runtime_route_performance(
    telemetry_sender: &agent_semantic_client_db::runtime_telemetry_bus::RuntimeTelemetryBusSender,
    workspace_identity: &str,
    language_id: &str,
    generation_digest: &str,
    operation_id: &str,
    surface: &str,
    stage: &str,
    requested_projection: &str,
    elapsed_micros: u64,
) -> Result<(), String> {
    let budget_micros = 1_000;
    let mut observation =
        agent_semantic_client_db::runtime_server_opentelemetry::RuntimePerformanceObservation::new(
            surface,
            stage,
            elapsed_micros,
            budget_micros,
            if elapsed_micros <= budget_micros {
                "within-budget"
            } else {
                "budget-exceeded"
            },
        );
    observation.workspace_identity = Some(workspace_identity.to_owned());
    observation.language_id = Some(language_id.to_owned());
    observation.generation_digest = Some(generation_digest.to_owned());
    observation.operation_id = Some(operation_id.to_owned());
    observation.requested_projection = Some(requested_projection.to_owned());
    observation.memory_search_turso_opens = Some(0);
    observation.memory_search_source_bytes_read = Some(0);
    observation.memory_search_provider_spawns = Some(0);
    observation.memory_search_socket_connects = Some(0);
    telemetry_sender.try_record_performance(observation)
}
