//! Public ASP Client Protocol binding for the resident Runtime Server.

use std::collections::{HashMap, hash_map::Entry};
use std::sync::{Arc, Mutex};

use crate::query_generation::{RuntimeQueryGenerationAuthority, RuntimeQueryGenerationState};
use agent_semantic_client_db::runtime_provider_register::RuntimeProviderRegister;
use agent_semantic_client_db::runtime_search_service::RuntimeSearchServiceHandle;
use agent_semantic_client_db::runtime_server_workspace::RuntimeServerWorkspaceRegistry;
use agent_semantic_client_protocol::{
    AspClientExactQueryRequest, AspClientOwnerSearchRequest, AspClientSearchRequest,
    ClientRequestId, ClientSessionId, ClientWorkspaceIdentity,
};
use agent_semantic_client_server::{
    AspClientDispatchError, AspClientDispatchFuture, AspClientDispatchRequest, AspClientDispatcher,
    AspClientProtocolHttpService,
};

type ClientRequestKey = (ClientWorkspaceIdentity, ClientSessionId, ClientRequestId);

#[derive(Clone)]
pub struct RuntimeAspClientDispatcher {
    workspace_registry: Arc<RuntimeServerWorkspaceRegistry>,
    provider_register: Arc<RuntimeProviderRegister>,
    query_generation:
        tokio::sync::watch::Receiver<Arc<HashMap<String, RuntimeQueryGenerationState>>>,
    telemetry_sender: agent_semantic_client_db::runtime_telemetry_bus::RuntimeTelemetryBusSender,
    cancellations: Arc<Mutex<HashMap<ClientRequestKey, tokio::sync::watch::Sender<bool>>>>,
}

impl RuntimeAspClientDispatcher {
    fn new(
        _runtime_search_service: RuntimeSearchServiceHandle,
        workspace_registry: Arc<RuntimeServerWorkspaceRegistry>,
        provider_register: Arc<RuntimeProviderRegister>,
        query_generation: tokio::sync::watch::Receiver<
            Arc<HashMap<String, RuntimeQueryGenerationState>>,
        >,
        telemetry_sender: agent_semantic_client_db::runtime_telemetry_bus::RuntimeTelemetryBusSender,
    ) -> Self {
        Self {
            workspace_registry,
            provider_register,
            query_generation,
            telemetry_sender,
            cancellations: Arc::new(Mutex::new(HashMap::new())),
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
    provider_register: Arc<RuntimeProviderRegister>,
    generation_admission: Arc<
        agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationAdmission,
    >,
    query_generation_authority: RuntimeQueryGenerationAuthority,
    telemetry_sender: agent_semantic_client_db::runtime_telemetry_bus::RuntimeTelemetryBusSender,
) -> Result<Arc<AspClientProtocolHttpService<RuntimeAspClientDispatcher>>, String> {
    let query_generation = query_generation_authority.subscribe();
    let catalog_workspace_registry = Arc::clone(&workspace_registry);
    let catalog_provider_register = Arc::clone(&provider_register);
    let catalog_generation_admission = Arc::clone(&generation_admission);
    let catalog_query_generation = query_generation.clone();
    let catalog_query_generation_authority = query_generation_authority.clone();
    let dispatcher = Arc::new(RuntimeAspClientDispatcher::new(
        runtime_search_service,
        workspace_registry,
        provider_register,
        query_generation,
        telemetry_sender,
    ));
    Ok(Arc::new(AspClientProtocolHttpService::new_async(
        dispatcher,
        move |workspace_identity, project_root| {
            let workspace_registry = Arc::clone(&catalog_workspace_registry);
            let provider_register = Arc::clone(&catalog_provider_register);
            let generation_admission = Arc::clone(&catalog_generation_admission);
            let mut query_generation = catalog_query_generation.clone();
            let query_generation_authority = catalog_query_generation_authority.clone();
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
                if workspace_registry
                    .unique_resident_scope(&workspace_identity)
                    .is_err()
                {
                    let candidate = agent_semantic_client_db::runtime_server_admission::discover_workspace_generation_candidate(&project_root).await?;
                    generation_admission
                        .ensure(&workspace_identity, &project_root, candidate)
                        .await
                        .map_err(|error| error.to_string())?;
                }
                if let Some(current) =
                    generation_admission.current(&workspace_identity, &project_root)
                {
                    let terminal = if current.state
                        == agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationAdmissionState::Building
                    {
                        generation_admission
                            .wait_terminal(&workspace_identity, &project_root)
                            .await?
                    } else {
                        current
                    };
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
                }
                let (_, generation_digest) =
                    workspace_registry.unique_resident_scope(&workspace_identity)?;
                let pointer_path = agent_semantic_client_db::runtime_server_workspace::workspace_generation_pointer_path(
                    workspace_registry.root(),
                    &workspace_identity,
                    &project_root,
                )?;
                query_generation_authority
                    .ensure_ready(
                        &workspace_identity,
                        &pointer_path,
                        &project_root,
                        &generation_digest,
                    )
                    .await
                    .map_err(|error| format!(
                        "state=generation-failed reasonKind=runtime-query-generation-publication-failed message={error}"
                    ))?;
                loop {
                    let state = query_generation.borrow().get(&workspace_identity).cloned();
                    match state {
                        Some(RuntimeQueryGenerationState::Ready(_)) => break,
                        Some(RuntimeQueryGenerationState::Failed(reason)) => {
                            return Err(format!(
                                "state=generation-failed reasonKind=runtime-query-generation-publication-failed message={reason}"
                            ));
                        }
                        None => query_generation.changed().await.map_err(|_| {
                            "Runtime query-generation publication channel closed".to_owned()
                        })?,
                    }
                }
                provider_register.client_protocol_catalog(
                    generation_digest,
                    vec![agent_semantic_client_protocol::ClientTransport::HttpJson],
                )
            }
        },
    )))
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

        let workspace_registry = Arc::clone(&self.workspace_registry);
        let provider_register = Arc::clone(&self.provider_register);
        let query_generation = self.query_generation.clone();
        let telemetry_sender = self.telemetry_sender.clone();
        let cancellations = Arc::clone(&self.cancellations);
        Box::pin(async move {
            let operation = async {
                let (_project_root, _) = workspace_registry
                    .unique_resident_scope(request.workspace_identity.as_str())?;
                let (language_id, route, _) =
                    provider_register.resolve_client_method(&request.method)?;
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
                match route.as_str() {
                    "search" => {
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
                        let lookup = generation.resident().read_source_index(
                            &params.query,
                            Some(&language),
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
                    "search.owner" => {
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
                    "query" => {
                        let started = tokio::time::Instant::now();
                        let params: AspClientExactQueryRequest = serde_json::from_value(params)
                            .map_err(|error| {
                                format!("decode ASP client exact-query request: {error}")
                            })?;
                        params.validate_schema_identity()?;
                        let projection_kind = agent_semantic_client_db::runtime_server_workspace::ExactProjectionKind::try_from(
                            params.projection.as_str(),
                        )?;
                        let projection = generation
                            .resident()
                            .read_runtime_selector(projection_kind, &params.selector)?;
                        record_runtime_route_performance(
                            &telemetry_sender,
                            request.workspace_identity.as_str(),
                            &language_id,
                            generation.generation_digest(),
                            request.request_id.as_str(),
                            "query",
                            "runtime-selector-read",
                            projection_kind.as_str(),
                            elapsed_micros(started),
                        )?;
                        serde_json::to_value(projection)
                            .map_err(|error| format!("encode query response: {error}"))
                    }
                    route => Err(format!("unsupported Runtime route: {route}")),
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
    ) -> bool {
        let key = (
            workspace_identity.clone(),
            session_id.clone(),
            request_id.clone(),
        );
        let sender = self
            .cancellations
            .lock()
            .expect("ASP Client Protocol cancellation registry poisoned")
            .remove(&key);
        sender.is_some_and(|sender| sender.send(true).is_ok())
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
