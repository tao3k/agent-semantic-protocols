//! Public ASP Client Protocol binding for the resident Runtime Server.

use std::collections::{HashMap, hash_map::Entry};
use std::sync::{Arc, Mutex};

use crate::query_generation::{RuntimeQueryGenerationAuthority, RuntimeQueryGenerationState};
use agent_semantic_client_db::runtime_search_service::RuntimeSearchServiceHandle;
use agent_semantic_client_db::runtime_server_workspace::RuntimeServerWorkspaceRegistry;
use agent_semantic_client_protocol::{
    AspClientExactQueryFailure, AspClientExactQueryRequest, AspClientExactQueryResponse,
    AspClientOwnerSearchRequest, AspClientRuntimeWorkCounters, AspClientSearchRequest,
    ClientRequestId, ClientSessionId, ClientWorkspaceIdentity, SCHEMA_BUNDLE_METHOD,
    SchemaBundleRequest, ServerClientRoute,
};
use agent_semantic_client_server::{
    AspClientDispatchError, AspClientDispatchFuture, AspClientDispatchRequest, AspClientDispatcher,
    AspClientFrameService,
};

enum AspClientOperationError {
    Message(String),
    Terminal(AspClientDispatchError),
}

impl From<String> for AspClientOperationError {
    fn from(message: String) -> Self {
        Self::Message(message)
    }
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

fn generation_admission_reason_kind(
    stage: Option<
        &agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationFailureStage,
    >,
) -> &'static str {
    use agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationFailureStage;

    match stage {
        Some(WorkspaceGenerationFailureStage::GenerationBuilder) => {
            "runtime-generation-builder-failed"
        }
        Some(WorkspaceGenerationFailureStage::GenerationBuilderSupervision) => {
            "runtime-generation-builder-supervision-failed"
        }
        Some(WorkspaceGenerationFailureStage::WorkspaceBootstrap) => {
            "runtime-workspace-bootstrap-failed"
        }
        Some(WorkspaceGenerationFailureStage::DurableRestore) => {
            "runtime-generation-restore-failed"
        }
        Some(WorkspaceGenerationFailureStage::SourceBuilder) => "runtime-source-builder-failed",
        Some(WorkspaceGenerationFailureStage::SourceIndexCommit) => {
            "runtime-source-index-commit-failed"
        }
        Some(WorkspaceGenerationFailureStage::CanonicalGenerationPublication) => {
            "runtime-generation-publication-failed"
        }
        Some(WorkspaceGenerationFailureStage::AdmissionValidation) => {
            "runtime-generation-admission-validation-failed"
        }
        None => "runtime-generation-admission-failed",
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
}

type ClientRequestKey = (ClientWorkspaceIdentity, ClientSessionId, ClientRequestId);
type ClientWorkspaceKey = (String, String);

#[derive(Clone)]
struct InitializedWorkspace {
    project_root: std::path::PathBuf,
    candidate:
        agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationCandidateIdentity,
}

#[derive(Clone)]
pub struct RuntimeAspClientDispatcher {
    schema_bundles: crate::schema_bundle::RuntimeSchemaBundleCatalog,
    workspace_registry: Arc<RuntimeServerWorkspaceRegistry>,
    initialized_workspaces: Arc<Mutex<HashMap<ClientWorkspaceKey, InitializedWorkspace>>>,
    installed_provider_targets: Arc<[(String, String)]>,
    generation_admission:
        Arc<agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationAdmission>,
    query_generation_authority: RuntimeQueryGenerationAuthority,
    query_generation:
        tokio::sync::watch::Receiver<Arc<HashMap<String, RuntimeQueryGenerationState>>>,
    telemetry_sender: agent_semantic_client_db::runtime_telemetry_bus::RuntimeTelemetryBusSender,
    cancellations: Arc<Mutex<HashMap<ClientRequestKey, tokio::sync::watch::Sender<bool>>>>,
    cancellation_admitted: Arc<tokio::sync::Notify>,
}

impl RuntimeAspClientDispatcher {
    fn new(
        schema_bundles: crate::schema_bundle::RuntimeSchemaBundleCatalog,
        _runtime_search_service: RuntimeSearchServiceHandle,
        workspace_registry: Arc<RuntimeServerWorkspaceRegistry>,
        initialized_workspaces: Arc<Mutex<HashMap<ClientWorkspaceKey, InitializedWorkspace>>>,
        installed_provider_targets: Arc<[(String, String)]>,
        generation_admission: Arc<
            agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationAdmission,
        >,
        query_generation_authority: RuntimeQueryGenerationAuthority,
        query_generation: tokio::sync::watch::Receiver<
            Arc<HashMap<String, RuntimeQueryGenerationState>>,
        >,
        telemetry_sender: agent_semantic_client_db::runtime_telemetry_bus::RuntimeTelemetryBusSender,
    ) -> Self {
        Self {
            schema_bundles,
            workspace_registry,
            initialized_workspaces,
            installed_provider_targets,
            generation_admission,
            query_generation_authority,
            query_generation,
            telemetry_sender,
            cancellations: Arc::new(Mutex::new(HashMap::new())),
            cancellation_admitted: Arc::new(tokio::sync::Notify::new()),
        }
    }
}

/// Build the sole Runtime-owned public ClientFrame service. HTTP and Unix
/// gRPC bindings both mount this same admission/dispatch owner.
pub fn build_frame_service(
    schema_bundles: crate::schema_bundle::RuntimeSchemaBundleCatalog,
    runtime_search_service: RuntimeSearchServiceHandle,
    workspace_registry: Arc<RuntimeServerWorkspaceRegistry>,
    client_catalog_generation: String,
    installed_provider_targets: Arc<[(String, String)]>,
    generation_admission: Arc<
        agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationAdmission,
    >,
    query_generation_authority: RuntimeQueryGenerationAuthority,
    telemetry_sender: agent_semantic_client_db::runtime_telemetry_bus::RuntimeTelemetryBusSender,
) -> Result<Arc<AspClientFrameService<RuntimeAspClientDispatcher>>, String> {
    let query_generation = query_generation_authority.subscribe();
    let initialized_workspaces = Arc::new(Mutex::new(HashMap::new()));
    let catalog_generation = client_catalog_generation;
    let catalog_provider_targets = Arc::clone(&installed_provider_targets);
    let dispatcher = Arc::new(RuntimeAspClientDispatcher::new(
        schema_bundles,
        runtime_search_service,
        workspace_registry,
        Arc::clone(&initialized_workspaces),
        installed_provider_targets,
        generation_admission,
        query_generation_authority,
        query_generation,
        telemetry_sender,
    ));
    Ok(Arc::new(AspClientFrameService::new_async(
        dispatcher,
        move |workspace_identity, session_id, project_root| {
            let initialized_workspaces = Arc::clone(&initialized_workspaces);
            let catalog_generation = catalog_generation.clone();
            let provider_targets = Arc::clone(&catalog_provider_targets);
            async move {
                let project_root = std::path::PathBuf::from(project_root);
                if !project_root.is_absolute() {
                    return Err("client initialize projectRoot must be absolute".to_owned());
                }
                let expected_identity =
                    agent_semantic_client_db::AgentSessionRegistry::workspace_id(&project_root)?;
                if expected_identity != workspace_identity {
                    return Err(
                        "client initialize workspaceIdentity does not match projectRoot".to_owned(),
                    );
                }
                let candidate = agent_semantic_client_db::runtime_server_admission::discover_workspace_generation_candidate(&project_root).await?;
                let workspace_generation =
                    client_generation_digest(candidate.candidate_generation.digest.as_str())?;
                let key = (workspace_identity.clone(), session_id);
                let initialized = InitializedWorkspace {
                    project_root: project_root.clone(),
                    candidate,
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
                    vec![
                        agent_semantic_client_protocol::ClientTransport::HttpJson,
                        agent_semantic_client_protocol::ClientTransport::RuntimeIpc,
                    ],
                    provider_targets
                        .iter()
                        .map(|(language_id, _)| language_id.clone()),
                )
            }
        },
    )))
}

fn client_generation_digest(candidate_digest: &str) -> Result<String, String> {
    let hex = candidate_digest
        .strip_prefix("blake3:")
        .or_else(|| candidate_digest.strip_prefix("blake3-256:"))
        .ok_or_else(|| "workspace candidate uses an unsupported digest".to_owned())?;
    if hex.len() != 64
        || !hex
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err("workspace candidate digest is invalid".to_owned());
    }
    Ok(format!("blake3-256:{hex}"))
}

impl AspClientDispatcher for RuntimeAspClientDispatcher {
    fn dispatch(&self, request: AspClientDispatchRequest) -> AspClientDispatchFuture {
        let key = (
            request.workspace_identity.clone(),
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

        let workspace_registry = Arc::clone(&self.workspace_registry);
        let schema_bundles = self.schema_bundles.clone();
        let initialized_workspaces = Arc::clone(&self.initialized_workspaces);
        let installed_provider_targets = Arc::clone(&self.installed_provider_targets);
        let generation_admission = Arc::clone(&self.generation_admission);
        let query_generation_authority = self.query_generation_authority.clone();
        let query_generation = self.query_generation.clone();
        let telemetry_sender = self.telemetry_sender.clone();
        let cancellations = Arc::clone(&self.cancellations);
        Box::pin(async move {
            let operation = async {
                if request.method == agent_semantic_client_protocol::CANCELLATION_PROBE_METHOD {
                    return std::future::pending::<
                        Result<serde_json::Value, AspClientOperationError>,
                    >()
                    .await;
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
                let (language_id, route) =
                    agent_semantic_client_protocol::resolve_server_client_method(
                        &request.method,
                        installed_provider_targets
                            .iter()
                            .map(|(language_id, _)| language_id.clone()),
                    )?;
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
                let admission_started = tokio::time::Instant::now();
                let initialized = initialized_workspaces
                    .lock()
                    .map_err(|_| "ASP client workspace-root registry poisoned".to_owned())?
                    .get(&(
                        request.workspace_identity.as_str().to_owned(),
                        request.session_id.as_str().to_owned(),
                    ))
                    .cloned()
                    .ok_or_else(|| {
                        "ASP client request requires an initialized workspace root".to_owned()
                    })?;
                let project_root = initialized.project_root;
                let query_generation_ready = query_generation
                    .borrow()
                    .get(request.workspace_identity.as_str())
                    .is_some_and(|state| matches!(state, RuntimeQueryGenerationState::Ready(_)));
                let queued_receipt = if !query_generation_ready {
                    let queued_receipt = generation_admission
                    .enqueue_query_demand_for_candidate(
                        request.workspace_identity.as_str().to_owned(),
                        project_root.clone(),
                        initialized.candidate,
                        Vec::new(),
                        Some(
                            agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationProviderTarget {
                                language_id: language_id.clone(),
                                provider_id: Some(provider_id.clone()),
                            },
                        ),
                    )?;
                    Some(queued_receipt)
                } else {
                    None
                };
                let terminal = match queued_receipt.or_else(|| {
                    generation_admission.status(request.workspace_identity.as_str(), &project_root)
                }) {
                    Some(observed) => observed,
                    None if exact_query_params.is_some() => {
                        let params = exact_query_params
                            .as_ref()
                            .expect("ExactQuery params were checked above");
                        let elapsed_micros = elapsed_micros(admission_started);
                        let failure = AspClientExactQueryFailure {
                            schema_id: "agent.semantic-protocols.asp-client-exact-query-failure"
                                .to_owned(),
                            schema_version: "1".to_owned(),
                            state: "failed".to_owned(),
                            operation_id: request.request_id.as_str().to_owned(),
                            language_id: language_id.clone(),
                            provider_id: provider_id.clone(),
                            requested_selector: Some(params.selector.clone()),
                            resolved_selector: None,
                            projection_kind: Some(params.projection.clone()),
                            phase: "workspace-generation-admission".to_owned(),
                            reason_kind: "runtime-generation-admission-receipt-missing".to_owned(),
                            generation_digest: None,
                            root_digest: None,
                            recommended_next: serde_json::json!({
                                "action": "inspect-runtime-generation-admission",
                                "workspaceIdentity": request.workspace_identity.as_str(),
                            }),
                            resident_read_elapsed_micros: 0,
                            service_elapsed_micros: elapsed_micros,
                            elapsed_micros,
                            work_counters: AspClientRuntimeWorkCounters::default(),
                            details: serde_json::json!({
                                "admissionState": "receipt-missing",
                                "languageId": language_id,
                                "providerId": provider_id,
                            }),
                        };
                        failure.validate()?;
                        return Err(AspClientOperationError::Terminal(AspClientDispatchError {
                            reason_kind: failure.reason_kind.clone(),
                            message: "workspace generation admission has no current receipt"
                                .to_owned(),
                            details: Some(
                                serde_json::to_value(failure).map_err(|error| error.to_string())?,
                            ),
                        }));
                    }
                    None => {
                        return Err(AspClientOperationError::Message(
                            "workspace generation admission has no current receipt".to_owned(),
                        ));
                    }
                };
                if terminal.state
                    != agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationAdmissionState::Ready
                {
                    if let Some(params) = exact_query_params.as_ref() {
                        let elapsed_micros = elapsed_micros(admission_started);
                        let candidate_generation_digest = client_generation_digest(
                            terminal.candidate_generation.digest.as_str(),
                        )?;
                        let message = terminal.error.unwrap_or_else(|| {
                            format!(
                                "workspace generation admission reached terminal state {:?}",
                                terminal.state
                            )
                        });
                        let reason_kind = match terminal.state {
                            agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationAdmissionState::Queued => {
                                "runtime-generation-queued"
                            }
                            agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationAdmissionState::Building => {
                                "runtime-generation-building"
                            }
                            agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationAdmissionState::Cancelled => {
                                "runtime-generation-admission-cancelled"
                            }
                            agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationAdmissionState::Failed => {
                                generation_admission_reason_kind(terminal.failure_stage.as_ref())
                            }
                            agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationAdmissionState::Ready => {
                                unreachable!("Ready admission bypasses failure terminal")
                            }
                        };
                        let recommended_action = match terminal.state {
                            agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationAdmissionState::Queued => {
                                "observe-runtime-dispatch"
                            }
                            agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationAdmissionState::Building => {
                                "observe-runtime-generation"
                            }
                            agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationAdmissionState::Failed
                            | agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationAdmissionState::Cancelled => {
                                "inspect-runtime-generation-admission"
                            }
                            agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationAdmissionState::Ready => {
                                unreachable!("Ready admission bypasses failure terminal")
                            }
                        };
                        let failure = AspClientExactQueryFailure {
                            schema_id:
                                "agent.semantic-protocols.asp-client-exact-query-failure"
                                    .to_owned(),
                            schema_version: "1".to_owned(),
                            state: "failed".to_owned(),
                            operation_id: request.request_id.as_str().to_owned(),
                            language_id: language_id.clone(),
                            provider_id: provider_id.clone(),
                            requested_selector: Some(params.selector.clone()),
                            resolved_selector: None,
                            projection_kind: Some(params.projection.clone()),
                            phase: "workspace-generation-admission".to_owned(),
                            reason_kind: reason_kind.to_owned(),
                            generation_digest: None,
                            root_digest: None,
                            recommended_next: serde_json::json!({
                                "action": recommended_action,
                                "workspaceIdentity": request.workspace_identity.as_str(),
                            }),
                            resident_read_elapsed_micros: 0,
                            service_elapsed_micros: elapsed_micros,
                            elapsed_micros,
                            work_counters: AspClientRuntimeWorkCounters::default(),
                            details: serde_json::json!({
                                "admissionState": format!("{:?}", terminal.state),
                                "attempt": terminal.attempt,
                                "candidateGenerationDigest": candidate_generation_digest,
                                "failureStage": terminal.failure_stage,
                                "languageId": language_id,
                                "providerId": provider_id,
                            }),
                        };
                        failure.validate()?;
                        return Err(AspClientOperationError::Terminal(AspClientDispatchError {
                            reason_kind: failure.reason_kind.clone(),
                            message,
                            details: Some(
                                serde_json::to_value(failure)
                                    .map_err(|error| error.to_string())?,
                            ),
                        }));
                    }
                    return Err(AspClientOperationError::Message(
                                terminal.error.unwrap_or_else(|| {
                                    format!(
                                        "workspace generation admission reached terminal state {:?}",
                                        terminal.state
                                    )
                                }),
                            ));
                }
                let commit = terminal.commit.ok_or_else(|| {
                    "workspace generation admission reached Ready without a commit".to_owned()
                })?;
                let pointer_path = agent_semantic_client_db::runtime_server_workspace::workspace_generation_pointer_path(
                    workspace_registry.root(),
                    request.workspace_identity.as_str(),
                    &project_root,
                )?;
                let query_generation_is_current = query_generation
                    .borrow()
                    .get(request.workspace_identity.as_str())
                    .is_some_and(|state| {
                        matches!(
                            state,
                            RuntimeQueryGenerationState::Ready(generation)
                                if generation.generation_digest() == commit.generation_digest
                        )
                    });
                if !query_generation_is_current {
                    query_generation_authority.clear_workspace(request.workspace_identity.as_str());
                    query_generation_authority
                        .ensure_ready(
                            request.workspace_identity.as_str(),
                            &pointer_path,
                            &project_root,
                            &commit.generation_digest,
                        )
                        .await
                        .map_err(|error| format!(
                            "state=generation-failed reasonKind=runtime-query-generation-publication-failed message={error}"
                        ))?;
                }
                let params = request.params;
                let generation = query_generation
                    .borrow()
                    .get(request.workspace_identity.as_str())
                    .cloned()
                    .ok_or_else(|| {
                        "active-workspace-generation-required schemaVersion=1 state=Building"
                            .to_owned()
                    })?;
                let RuntimeQueryGenerationState::Ready(generation) = generation else {
                    if let Some(params) = exact_query_params.as_ref() {
                        let elapsed_micros = elapsed_micros(admission_started);
                        let failure = AspClientExactQueryFailure {
                            schema_id: "agent.semantic-protocols.asp-client-exact-query-failure"
                                .to_owned(),
                            schema_version: "1".to_owned(),
                            state: "failed".to_owned(),
                            operation_id: request.request_id.as_str().to_owned(),
                            language_id: language_id.clone(),
                            provider_id: provider_id.clone(),
                            requested_selector: Some(params.selector.clone()),
                            resolved_selector: None,
                            projection_kind: Some(params.projection.clone()),
                            phase: "runtime-generation-authority".to_owned(),
                            reason_kind: "active-workspace-generation-required".to_owned(),
                            generation_digest: None,
                            root_digest: None,
                            recommended_next: serde_json::json!({
                                "action": "inspect-runtime-generation-authority",
                                "workspaceIdentity": request.workspace_identity.as_str(),
                            }),
                            resident_read_elapsed_micros: 0,
                            service_elapsed_micros: elapsed_micros,
                            elapsed_micros,
                            work_counters: AspClientRuntimeWorkCounters::default(),
                            details: serde_json::json!({
                                "generationState": "not-ready",
                            }),
                        };
                        failure.validate()?;
                        return Err(AspClientOperationError::Terminal(AspClientDispatchError {
                            reason_kind: failure.reason_kind.clone(),
                            message: "active workspace generation is not Ready".to_owned(),
                            details: Some(
                                serde_json::to_value(failure).map_err(|error| error.to_string())?,
                            ),
                        }));
                    }
                    return Err(AspClientOperationError::Message(
                        "active-workspace-generation-required schemaVersion=1 state=Failed"
                            .to_owned(),
                    ));
                };
                match route {
                    agent_semantic_client_protocol::ServerClientRoute::CancellationProbe => {
                        unreachable!("cancellation probe is owned by the lifecycle route")
                    }
                    agent_semantic_client_protocol::ServerClientRoute::Search => {
                        let started = tokio::time::Instant::now();
                        let params: AspClientSearchRequest = serde_json::from_value(params)
                            .map_err(|error| {
                                format!("decode ASP client search request: {error}")
                            })?;
                        params.validate_schema_identity()?;
                        if params.operation.is_empty() {
                            return Err(AspClientOperationError::Message(
                                "ASP client search operation must not be empty".to_owned(),
                            ));
                        }
                        let language =
                            agent_semantic_client_core::LanguageId::try_from(language_id.as_str())
                                .map_err(|error| format!("decode language id: {error}"))?;
                        let authority = agent_semantic_search::ResidentSearchAuthority {
                            language_id: language.clone(),
                            provider_id: provider_id.as_str().into(),
                        };
                        let lookup = generation.resident().read_source_index(
                            &params.query,
                            Some(&authority),
                            100,
                        )?;
                        let elapsed_micros = elapsed_micros(started);
                        record_runtime_route_performance(
                            &telemetry_sender,
                            request.workspace_identity.as_str(),
                            &language_id,
                            generation.generation_digest(),
                            request.request_id.as_str(),
                            "search",
                            "runtime-source-index-read",
                            &params.operation,
                            elapsed_micros,
                        )?;
                        let receipt = agent_semantic_search::build_runtime_provider_search_receipt(
                            request.request_id.as_str().to_owned(),
                            language,
                            vec![agent_semantic_search::RuntimeSearchSource::once(
                                "resident", lookup,
                            )],
                            elapsed_micros,
                            Vec::new(),
                        )
                        .await?;
                        Ok(serde_json::to_value(receipt)
                            .map_err(|error| format!("encode search receipt: {error}"))?)
                    }
                    agent_semantic_client_protocol::ServerClientRoute::OwnerSearch => {
                        let started = tokio::time::Instant::now();
                        let params: AspClientOwnerSearchRequest = serde_json::from_value(params)
                            .map_err(|error| {
                                format!("decode ASP client owner-search request: {error}")
                            })?;
                        params.validate_schema_identity()?;
                        let read = generation
                            .resident()
                            .read_runtime_owner(&params.owner_path)?;
                        record_runtime_route_performance(
                            &telemetry_sender,
                            request.workspace_identity.as_str(),
                            &language_id,
                            generation.generation_digest(),
                            request.request_id.as_str(),
                            "search",
                            "runtime-owner-read",
                            &params.view,
                            elapsed_micros(started),
                        )?;
                        Ok(serde_json::to_value(read)
                            .map_err(|error| format!("encode owner response: {error}"))?)
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
                        let projection = generation
                            .resident()
                            .read_runtime_selector(projection_kind, &params.selector)?;
                        let resident_read_elapsed_micros = elapsed_micros(resident_started);
                        let elapsed_micros = elapsed_micros(started);
                        let service_elapsed_micros =
                            elapsed_micros.saturating_sub(resident_read_elapsed_micros);
                        record_runtime_route_performance(
                            &telemetry_sender,
                            request.workspace_identity.as_str(),
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
                                language_id: language_id.clone(),
                                provider_id: provider_id.clone(),
                                requested_selector: Some(params.selector),
                                resolved_selector: failure.resolved_selector,
                                projection_kind: Some(params.projection),
                                phase: "resident-selector-read".to_owned(),
                                reason_kind: failure.reason_kind.to_owned(),
                                generation_digest: Some(generation.generation_digest().to_owned()),
                                root_digest: Some(generation.resident().root_digest()),
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
                            language_id: language_id.clone(),
                            provider_id: provider_id.clone(),
                            generation_digest: generation.generation_digest().to_owned(),
                            root_digest: generation.resident().root_digest(),
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
        workspace_identity: &ClientWorkspaceIdentity,
        session_id: &ClientSessionId,
        request_id: &ClientRequestId,
    ) -> agent_semantic_client_server::AspClientCancelFuture {
        let key = (
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
