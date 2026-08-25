//! Public ASP Client Protocol binding for the resident Runtime Server.

use std::collections::{HashMap, hash_map::Entry};
use std::sync::{Arc, Mutex};

use crate::query_generation::{RuntimeQueryGenerationAuthority, RuntimeQueryGenerationState};
use agent_semantic_client_db::runtime_search_service::RuntimeSearchServiceHandle;
use agent_semantic_client_db::runtime_server_workspace::RuntimeServerWorkspaceRegistry;
use agent_semantic_client_protocol::{
    AspClientExactQueryRequest, AspClientExactQueryResponse, AspClientOwnerSearchRequest,
    AspClientRuntimeWorkCounters, AspClientSearchRequest, ClientRequestId, ClientSessionId,
    ClientWorkspaceIdentity,
};
use agent_semantic_client_server::{
    AspClientDispatchError, AspClientDispatchFuture, AspClientDispatchRequest, AspClientDispatcher,
    AspClientProtocolHttpService,
};

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

pub async fn bind_http_listener() -> Result<(tokio::net::TcpListener, String), String> {
    let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
        .await
        .map_err(|error| format!("failed to bind ASP Client Protocol HTTP endpoint: {error}"))?;
    let endpoint = format!(
        "http://{}",
        listener
            .local_addr()
            .map_err(|error| format!("failed to resolve ASP Client Protocol endpoint: {error}"))?
    );
    Ok((listener, endpoint))
}

pub fn build_http_service(
    runtime_search_service: RuntimeSearchServiceHandle,
    workspace_registry: Arc<RuntimeServerWorkspaceRegistry>,
    client_catalog_generation: String,
    installed_provider_targets: Arc<[(String, String)]>,
    generation_admission: Arc<
        agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationAdmission,
    >,
    query_generation_authority: RuntimeQueryGenerationAuthority,
    telemetry_sender: agent_semantic_client_db::runtime_telemetry_bus::RuntimeTelemetryBusSender,
) -> Result<Arc<AspClientProtocolHttpService<RuntimeAspClientDispatcher>>, String> {
    let query_generation = query_generation_authority.subscribe();
    let initialized_workspaces = Arc::new(Mutex::new(HashMap::new()));
    let catalog_generation = client_catalog_generation;
    let catalog_provider_targets = Arc::clone(&installed_provider_targets);
    let dispatcher = Arc::new(RuntimeAspClientDispatcher::new(
        runtime_search_service,
        workspace_registry,
        Arc::clone(&initialized_workspaces),
        installed_provider_targets,
        generation_admission,
        query_generation_authority,
        query_generation,
        telemetry_sender,
    ));
    Ok(Arc::new(AspClientProtocolHttpService::new_async(
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
                    vec![agent_semantic_client_protocol::ClientTransport::HttpJson],
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
                })
            });
        }
        self.cancellation_admitted.notify_waiters();

        let workspace_registry = Arc::clone(&self.workspace_registry);
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
                    return std::future::pending().await;
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
                if !query_generation_ready {
                    generation_admission
                    .submit_query_demand_for_candidate(
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
                    )
                    .await?;
                }
                let terminal = generation_admission
                    .wait_terminal(request.workspace_identity.as_str(), &project_root)
                    .await?;
                if terminal.state
                    != agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationAdmissionState::Ready
                {
                    return Err(terminal.error.unwrap_or_else(|| {
                        format!(
                            "workspace generation admission reached terminal state {:?}",
                            terminal.state
                        )
                    }));
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
                    return Err(
                        "active-workspace-generation-required schemaVersion=1 state=Failed"
                            .to_owned(),
                    );
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
                            return Err("ASP client search operation must not be empty".to_owned());
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
                        serde_json::to_value(receipt)
                            .map_err(|error| format!("encode search receipt: {error}"))
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
                        serde_json::to_value(read)
                            .map_err(|error| format!("encode owner response: {error}"))
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
                        serde_json::to_value(response)
                            .map_err(|error| format!("encode query response: {error}"))
                    }
                }
            };
            let result = tokio::select! {
                result = operation => result.map_err(|message| AspClientDispatchError {
                    reason_kind: "client-method-dispatch-failed".to_owned(),
                    message,
                }),
                changed = cancelled.changed() => {
                    let _ = changed;
                    Err(AspClientDispatchError {
                        reason_kind: "client-request-cancelled".to_owned(),
                        message: "client request was cancelled".to_owned(),
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
