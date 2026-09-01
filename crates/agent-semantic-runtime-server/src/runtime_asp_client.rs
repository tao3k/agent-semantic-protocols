//! Public ASP Client Protocol binding for the resident Runtime Server.

use std::collections::{HashMap, hash_map::Entry};
use std::sync::{Arc, Mutex};

use crate::query_generation::{RuntimeQueryGenerationAuthority, RuntimeQueryGenerationState};
use agent_semantic_client_db::runtime_search_service::RuntimeSearchServiceHandle;
use agent_semantic_client_db::runtime_server_admission::{
    WorkspaceGenerationAdmission, WorkspaceGenerationAdmissionState,
    WorkspaceGenerationProviderTarget, discover_workspace_generation_candidate,
};
use agent_semantic_client_protocol::{
    AGENT_SESSION_REGISTER_METHOD, AGENT_SESSION_REGISTER_RESPONSE_SCHEMA_ID,
    AgentSessionRegisterReceipt, AgentSessionRegisterRequest, AspClientExactQueryFailure,
    AspClientExactQueryRequest, AspClientExactQueryResponse, AspClientOwnerSearchRequest,
    AspClientOwnerSearchResponse, AspClientOwnerSearchSeed, AspClientRuntimeWorkCounters,
    AspClientSearchRequest, ClientRequestId, ClientSessionId, ClientWorkspaceIdentity,
    GRAPH_TIMELINE_METHOD, LIVE_CORPUS_CACHE_STATE_METHOD, LiveCorpusCacheStateReceipt,
    LiveCorpusCacheStateRequest, SCHEMA_BUNDLE_METHOD, SchemaBundleRequest, ServerClientRoute,
};
use agent_semantic_client_server::{
    AspClientDispatchError, AspClientDispatchFuture, AspClientDispatchRequest, AspClientDispatcher,
    AspClientFrameService,
};
use agent_semantic_search_projection::GraphTurboEvaluationRequest;

enum AspClientOperationError {
    Message(String),
    Terminal(AspClientDispatchError),
}

impl From<String> for AspClientOperationError {
    fn from(message: String) -> Self {
        Self::Message(message)
    }
}

const OWNER_SEARCH_SEED_LIMIT: usize = 100;
const RUNTIME_CLIENT_DISPATCH_BUDGET: std::time::Duration = std::time::Duration::from_secs(5);

fn bounded_owner_search_response(
    request: &AspClientOwnerSearchRequest,
    read: agent_semantic_client_db::runtime_server_workspace::WorkspaceRuntimeOwnerSearchRead,
) -> Result<AspClientOwnerSearchResponse, String> {
    if request.view != "seeds" {
        return Err(format!(
            "owner search view is not supported by the active schema: view={}",
            request.view
        ));
    }
    let (generation_digest, root_digest, owner) = match read {
        agent_semantic_client_db::runtime_server_workspace::WorkspaceRuntimeOwnerSearchRead::Owner {
            generation_digest,
            root_digest,
            owner,
        } => (generation_digest, root_digest, Some(owner)),
        agent_semantic_client_db::runtime_server_workspace::WorkspaceRuntimeOwnerSearchRead::OwnerMissing {
            generation_digest,
            root_digest,
        } => (generation_digest, root_digest, None),
    };
    let Some(owner) = owner else {
        return Ok(AspClientOwnerSearchResponse {
            schema_id: "agent.semantic-protocols.asp-client-owner-search-response".to_owned(),
            schema_version: "1".to_owned(),
            state: "owner-missing".to_owned(),
            generation_digest,
            root_digest,
            owner_path: request.owner_path.clone(),
            content_digest: None,
            query: request.query.clone(),
            view: request.view.clone(),
            candidate_count: 0,
            returned_count: 0,
            selectors: Vec::new(),
        });
    };

    let matching = owner
        .selectors
        .into_iter()
        .map(|selector| AspClientOwnerSearchSeed {
            selector: selector.selector,
            byte_start: selector.byte_start,
            byte_end: selector.byte_end,
        })
        .collect::<Vec<_>>();
    let returned_count = matching.len();
    Ok(AspClientOwnerSearchResponse {
        schema_id: "agent.semantic-protocols.asp-client-owner-search-response".to_owned(),
        schema_version: "1".to_owned(),
        state: "owner".to_owned(),
        generation_digest,
        root_digest,
        owner_path: owner.owner_path,
        content_digest: Some(owner.content_digest),
        query: request.query.clone(),
        view: request.view.clone(),
        candidate_count: owner.candidate_count,
        returned_count,
        selectors: matching,
    })
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
mod owner_search_tests {
    use super::*;
    use agent_semantic_client_db::runtime_server_workspace::{
        WorkspaceOwnerSearchSeedSnapshot, WorkspaceOwnerSearchSnapshot, WorkspaceOwnerSnapshot,
        WorkspaceRuntimeOwnerSearchRead, WorkspaceSelectorSnapshot,
    };

    fn request(query: &str) -> AspClientOwnerSearchRequest {
        AspClientOwnerSearchRequest {
            schema_id: "agent.semantic-protocols.asp-client-owner-search-request".to_owned(),
            schema_version: "1".to_owned(),
            owner_path: "src/lib.rs".to_owned(),
            query: query.to_owned(),
            view: "seeds".to_owned(),
        }
    }

    #[test]
    fn owner_search_returns_committed_bounded_seeds_and_omits_owner_payloads() {
        let read = WorkspaceRuntimeOwnerSearchRead::Owner {
            generation_digest: "generation-1".to_owned(),
            root_digest: "root-1".to_owned(),
            owner: WorkspaceOwnerSearchSnapshot {
                owner_path: "src/lib.rs".to_owned(),
                content_digest: "blake3-256:owner".to_owned(),
                candidate_count: 1,
                selectors: vec![WorkspaceOwnerSearchSeedSnapshot {
                    selector: "rust://src/lib.rs#item/function/compare".to_owned(),
                    byte_start: 0,
                    byte_end: 15,
                }],
            },
        };

        let response = bounded_owner_search_response(&request("compare"), read)
            .expect("bounded owner response");
        assert_eq!(response.state, "owner");
        assert_eq!(response.candidate_count, 1);
        assert_eq!(response.returned_count, 1);
        assert_eq!(
            response.selectors[0].selector,
            "rust://src/lib.rs#item/function/compare"
        );
        let encoded = serde_json::to_value(response).expect("encode owner response");
        assert!(encoded.get("bytes").is_none());
        assert!(encoded.get("derivedProjections").is_none());
        assert!(encoded["selectors"][0].get("queryKeys").is_none());
    }

    #[test]
    fn owner_search_does_not_treat_selector_text_as_a_query_key() {
        let read = WorkspaceRuntimeOwnerSearchRead::Owner {
            generation_digest: "generation-1".to_owned(),
            root_digest: "root-1".to_owned(),
            owner: WorkspaceOwnerSearchSnapshot {
                owner_path: "src/lib.rs".to_owned(),
                content_digest: "blake3-256:owner".to_owned(),
                candidate_count: 0,
                selectors: Vec::new(),
            },
        };

        let response = bounded_owner_search_response(&request("compare"), read)
            .expect("bounded owner response");
        assert_eq!(response.candidate_count, 0);
        assert!(response.selectors.is_empty());
    }

    #[test]
    fn compact_owner_search_removes_at_least_two_orders_of_payload_amplification() {
        let selector = "rust://src/lib.rs#item/function/compare";
        let full_owner = WorkspaceOwnerSnapshot {
            owner_path: "src/lib.rs".to_owned(),
            authority: None,
            content_digest: "blake3-256:owner".to_owned(),
            bytes: vec![b'x'; 128 * 1024],
            selectors: vec![WorkspaceSelectorSnapshot {
                selector: selector.to_owned(),
                byte_start: 0,
                byte_end: 15,
                query_keys: vec!["compare".to_owned()],
                derived_projections: Vec::new(),
            }],
        };
        let compact = bounded_owner_search_response(
            &request("compare"),
            WorkspaceRuntimeOwnerSearchRead::Owner {
                generation_digest: "generation-1".to_owned(),
                root_digest: "root-1".to_owned(),
                owner: WorkspaceOwnerSearchSnapshot {
                    owner_path: "src/lib.rs".to_owned(),
                    content_digest: "blake3-256:owner".to_owned(),
                    candidate_count: 1,
                    selectors: vec![WorkspaceOwnerSearchSeedSnapshot {
                        selector: selector.to_owned(),
                        byte_start: 0,
                        byte_end: 15,
                    }],
                },
            },
        )
        .expect("compact owner response");
        let full_bytes = serde_json::to_vec(&full_owner).expect("encode full owner snapshot");
        let compact_bytes = serde_json::to_vec(&compact).expect("encode compact owner response");
        let reduction = full_bytes.len() / compact_bytes.len();
        println!(
            "{{\"schemaId\":\"agent.semantic-protocols.owner-search-payload-reduction-receipt\",\"schemaVersion\":\"1\",\"fullBytes\":{},\"compactBytes\":{},\"reductionFactor\":{reduction}}}",
            full_bytes.len(),
            compact_bytes.len(),
        );
        assert!(
            reduction >= 100,
            "compact owner-search response must remove at least 100x payload amplification: {reduction}x"
        );
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
    fn unpublished_generation_returns_typed_query_not_ready_without_admission() {
        let request = AspClientExactQueryRequest {
            schema_id: "agent.semantic-protocols.asp-client-exact-query-request".to_owned(),
            schema_version: "1".to_owned(),
            selector: "rust://src/lib.rs#item/function/ready".to_owned(),
            projection: "source".to_owned(),
        };
        let error = query_generation_not_ready_error(QueryNotReadyContext {
            operation_id: "request-1",
            workspace_identity: "workspace-1",
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
}

struct QueryNotReadyContext<'a> {
    operation_id: &'a str,
    workspace_identity: &'a str,
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
        workspace_identity,
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
        "workspaceIdentity": workspace_identity,
    });
    if let Some(exact_query) = exact_query {
        let failure = AspClientExactQueryFailure {
            schema_id: "agent.semantic-protocols.asp-client-exact-query-failure".to_owned(),
            schema_version: "1".to_owned(),
            state: "failed".to_owned(),
            operation_id: operation_id.to_owned(),
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
            "workspaceIdentity": workspace_identity,
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

type ClientRequestKey = (ClientWorkspaceIdentity, ClientSessionId, ClientRequestId);
type ClientWorkspaceKey = (String, String);

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

/// Build the sole Runtime-owned public ClientFrame service. HTTP and Unix
/// gRPC bindings both mount this same admission/dispatch owner.
pub fn build_frame_service(
    schema_bundles: crate::schema_bundle::RuntimeSchemaBundleCatalog,
    agent_session_registry: Arc<agent_semantic_client_db::AgentSessionRegistry>,
    runtime_search_service: RuntimeSearchServiceHandle,
    generation_admission: Arc<WorkspaceGenerationAdmission>,
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
        generation_admission,
        Arc::clone(&initialized_workspaces),
        installed_provider_targets,
        workspace_store_root,
        query_generation_authority,
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
                // Client initialization binds identity only. Language generation admission is
                // intentionally deferred to language methods so Multi-Agent lifecycle calls do
                // not depend on a provider project entry or Source Index generation.
                let workspace_generation = format!(
                    "blake3-256:{}",
                    blake3::hash(workspace_identity.as_bytes()).to_hex()
                );
                let key = (workspace_identity.clone(), session_id);
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

        let schema_bundles = self.schema_bundles.clone();
        let agent_session_registry = Arc::clone(&self.agent_session_registry);
        let initialized_workspaces = Arc::clone(&self.initialized_workspaces);
        let runtime_search_service = self.runtime_search_service.clone();
        let generation_admission = Arc::clone(&self.generation_admission);
        let installed_provider_targets = Arc::clone(&self.installed_provider_targets);
        let workspace_store_root = self.workspace_store_root.clone();
        let query_generation_authority = self.query_generation_authority.clone();
        let query_generation = query_generation_authority.subscribe();
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
                            request.workspace_identity.as_str().to_owned(),
                            request.session_id.as_str().to_owned(),
                        ))
                        .cloned()
                        .ok_or_else(|| {
                            "ASP client request requires an initialized workspace root".to_owned()
                        })?;
                    let project_id = agent_semantic_client_db::AgentSessionRegistry::workspace_id(
                        &initialized.project_root,
                    )?;
                    if project_id != request.workspace_identity.as_str() {
                        return Err(AspClientOperationError::Message(
                            "child registration workspace identity mismatch".to_owned(),
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
                                project_id: project_id.as_str().into(),
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
                        project_id,
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
                    let workspace_identity = request.workspace_identity.as_str();
                    let resident_generation_evicted = match params.cache_state.as_str() {
                        "cold-build" => {
                            query_generation_authority
                                .require_workspace_absent(workspace_identity)?;
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
                                workspace_identity,
                                expected_generation_digest,
                                expected_root_digest,
                            )?;
                            let initialized = initialized_workspaces
                                .lock()
                                .map_err(|_| {
                                    "ASP client workspace-root registry poisoned".to_owned()
                                })?
                                .get(&(
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
                                    workspace_identity,
                                    &pointer_path,
                                    &initialized.project_root,
                                    expected_generation_digest,
                                )
                                .await?;
                            if reopened.resident().root_digest() != expected_root_digest {
                                return Err(AspClientOperationError::Message(
                                    "Live Corpus cold-load reopened a different source root"
                                        .to_owned(),
                                ));
                            }
                            true
                        }
                        "warm-read" => {
                            query_generation_authority.verify_ready_exact(
                                workspace_identity,
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
                                workspace_identity,
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
                        workspace_identity: workspace_identity.to_owned(),
                        generation_digest: params.expected_generation_digest,
                        root_digest: params.expected_root_digest,
                        resident_generation_evicted,
                        client_session_evicted: false,
                        source_workspace_mutation_count: 0,
                        global_cache_mutation_count: 0,
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
                            request.workspace_identity.as_str().to_owned(),
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
                let resolved = agent_semantic_client_protocol::resolve_server_client_method_owner(
                    &request.method,
                    installed_provider_targets
                        .iter()
                        .map(|(language_id, _)| language_id.clone()),
                )?;
                if let agent_semantic_client_protocol::ResolvedServerClientMethod::Server(
                    ServerClientRoute::GraphsEvaluate,
                ) = resolved
                {
                    let validated_params = GraphTurboEvaluationRequest::from_value(request.params)
                        .map_err(|error| {
                            AspClientOperationError::Message(format!(
                                "invalid graph evaluation request: {error}"
                            ))
                        })?
                        .into_value();
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
                    let (generation_digest, source_root_digest, generation_token) =
                        query_generation
                            .borrow()
                            .get(request.workspace_identity.as_str())
                            .and_then(|state| match state {
                                RuntimeQueryGenerationState::Ready(generation) => Some((
                                    generation.generation_digest().to_owned(),
                                    generation.resident().root_digest(),
                                    generation.generation_token(),
                                )),
                                _ => None,
                            })
                            .ok_or_else(|| {
                                "graph evaluation requires an active Ready workspace generation"
                                    .to_owned()
                            })?;
                    let (graph_generation_digest, graph_open_payload) =
                        graph_open_payload(&validated_params)?;
                    let receipt = runtime_search_service
                        .graphs_evaluate(
                            initialized.project_root,
                            request.workspace_identity.as_str().to_owned(),
                            generation_digest,
                            source_root_digest,
                            generation_token,
                            graph_generation_digest,
                            graph_open_payload,
                            request.request_id.as_str().to_owned(),
                            validated_params,
                        )
                        .await?;
                    return Ok(receipt);
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
                let dispatch_started = tokio::time::Instant::now();
                let project_root = initialized_workspaces
                    .lock()
                    .map_err(|_| "ASP client workspace-root registry poisoned".to_owned())?
                    .get(&(
                        request.workspace_identity.as_str().to_owned(),
                        request.session_id.as_str().to_owned(),
                    ))
                    .ok_or_else(|| {
                        "ASP client request requires an initialized workspace root".to_owned()
                    })?
                    .project_root
                    .clone();
                let params = request.params;
                let expected_candidate = discover_workspace_generation_candidate(&project_root)
                    .await
                    .map_err(AspClientOperationError::Message)?;
                let admission_receipt = generation_admission
                    .enqueue_query_demand_for_candidate(
                        request.workspace_identity.as_str().to_owned(),
                        project_root.clone(),
                        expected_candidate.clone(),
                        Vec::new(),
                        Some(WorkspaceGenerationProviderTarget {
                            language_id: language_id.clone(),
                            provider_id: Some(provider_id.clone()),
                        }),
                    )
                    .await
                    .map_err(|error| {
                        AspClientOperationError::Terminal(
                            query_generation_not_ready_error(QueryNotReadyContext {
                                operation_id: request.request_id.as_str(),
                                workspace_identity: request.workspace_identity.as_str(),
                                language_id: &language_id,
                                provider_id: &provider_id,
                                exact_query: exact_query_params.as_ref(),
                                generation_state: "admission-failed",
                                publication_error: Some(&error),
                                elapsed_micros: elapsed_micros(dispatch_started),
                            })
                            .unwrap_or_else(|render_error| {
                                AspClientDispatchError {
                                    reason_kind: "query-not-ready".to_owned(),
                                    message: render_error,
                                    details: None,
                                }
                            }),
                        )
                    })?;
                let admission_ready = admission_receipt.accepted
                    && admission_receipt.state == WorkspaceGenerationAdmissionState::Ready
                    && admission_receipt.candidate_generation
                        == expected_candidate.candidate_generation
                    && admission_receipt.policy_overlay_digest
                        == expected_candidate.policy_overlay_digest;
                if !admission_ready {
                    let admission_error = admission_receipt
                        .error
                        .as_deref()
                        .unwrap_or("workspace generation admission is pending");
                    return Err(AspClientOperationError::Terminal(
                        query_generation_not_ready_error(QueryNotReadyContext {
                            operation_id: request.request_id.as_str(),
                            workspace_identity: request.workspace_identity.as_str(),
                            language_id: &language_id,
                            provider_id: &provider_id,
                            exact_query: exact_query_params.as_ref(),
                            generation_state: if matches!(
                                admission_receipt.state,
                                WorkspaceGenerationAdmissionState::Failed
                                    | WorkspaceGenerationAdmissionState::Cancelled
                            ) {
                                "admission-failed"
                            } else {
                                "admission-pending"
                            },
                            publication_error: Some(admission_error),
                            elapsed_micros: elapsed_micros(dispatch_started),
                        })?,
                    ));
                }
                let committed_generation_digest = admission_receipt
                    .commit
                    .as_ref()
                    .ok_or_else(|| {
                        AspClientOperationError::Message(
                            "accepted query admission terminal is missing its commit".to_owned(),
                        )
                    })?
                    .generation_digest
                    .clone();
                let resident_matches_commit = query_generation
                    .borrow()
                    .get(request.workspace_identity.as_str())
                    .is_some_and(|state| {
                        matches!(
                            state,
                            RuntimeQueryGenerationState::Ready(generation)
                                if generation.generation_digest()
                                    == committed_generation_digest
                        )
                    });
                if !resident_matches_commit {
                    return Err(AspClientOperationError::Terminal(
                        query_generation_not_ready_error(QueryNotReadyContext {
                            operation_id: request.request_id.as_str(),
                            workspace_identity: request.workspace_identity.as_str(),
                            language_id: &language_id,
                            provider_id: &provider_id,
                            exact_query: exact_query_params.as_ref(),
                            generation_state: "publication-pending",
                            publication_error: None,
                            elapsed_micros: elapsed_micros(dispatch_started),
                        })?,
                    ));
                }
                let generation_state = query_generation
                    .borrow()
                    .get(request.workspace_identity.as_str())
                    .cloned();
                let generation = match generation_state {
                    Some(RuntimeQueryGenerationState::Ready(generation)) => generation,
                    Some(RuntimeQueryGenerationState::Failed { reason, .. }) => {
                        return Err(AspClientOperationError::Terminal(
                            query_generation_not_ready_error(QueryNotReadyContext {
                                operation_id: request.request_id.as_str(),
                                workspace_identity: request.workspace_identity.as_str(),
                                language_id: &language_id,
                                provider_id: &provider_id,
                                exact_query: exact_query_params.as_ref(),
                                generation_state: "failed",
                                publication_error: Some(reason.as_ref()),
                                elapsed_micros: elapsed_micros(dispatch_started),
                            })?,
                        ));
                    }
                    None => {
                        return Err(AspClientOperationError::Terminal(
                            query_generation_not_ready_error(QueryNotReadyContext {
                                operation_id: request.request_id.as_str(),
                                workspace_identity: request.workspace_identity.as_str(),
                                language_id: &language_id,
                                provider_id: &provider_id,
                                exact_query: exact_query_params.as_ref(),
                                generation_state: "unpublished",
                                publication_error: None,
                                elapsed_micros: elapsed_micros(dispatch_started),
                            })?,
                        ));
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
                    agent_semantic_client_protocol::ServerClientRoute::GraphsEvaluate => {
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
                    agent_semantic_client_protocol::ServerClientRoute::Search => {
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
                        let resident_started = tokio::time::Instant::now();
                        let lookup = generation.resident().read_source_index(
                            &params.query,
                            Some(&authority),
                            100,
                        )?;
                        let resident_read_elapsed_micros = elapsed_micros(resident_started);
                        let owner_paths = lookup
                            .hits
                            .iter()
                            .map(|hit| hit.owner_path.clone())
                            .collect::<Vec<_>>();
                        let parser_owned_selector_pairs = generation
                            .resident()
                            .parser_owned_callable_selector_pairs(&owner_paths)?;
                        let graph_stage =
                            crate::runtime_search_graph::rank_resident_search_frontier(
                                &runtime_search_service,
                                &project_root,
                                request.workspace_identity.as_str(),
                                request.request_id.as_str(),
                                &params.operation,
                                &params.query,
                                &language_id,
                                &provider_id,
                                generation.generation_digest(),
                                generation.generation_token(),
                                generation.resident(),
                                &lookup.hits,
                            )
                            .await
                            .map_err(|error| {
                                AspClientOperationError::Terminal(AspClientDispatchError {
                                    reason_kind: error.reason_kind.to_owned(),
                                    message: error.message,
                                    details: error.details,
                                })
                            })?;
                        if let Some(graph_stage) = graph_stage.as_ref() {
                            record_runtime_route_performance(
                                &telemetry_sender,
                                request.workspace_identity.as_str(),
                                &language_id,
                                generation.generation_digest(),
                                request.request_id.as_str(),
                                "search",
                                "runtime-graph-rank",
                                &graph_stage.result_digest,
                                graph_stage.elapsed_micros,
                            )?;
                        }
                        record_runtime_route_performance(
                            &telemetry_sender,
                            request.workspace_identity.as_str(),
                            &language_id,
                            generation.generation_digest(),
                            request.request_id.as_str(),
                            "search",
                            "runtime-source-index-read",
                            &params.operation,
                            resident_read_elapsed_micros,
                        )?;
                        let receipt = agent_semantic_search::build_runtime_provider_search_receipt_with_graph(
                            request.request_id.as_str().to_owned(),
                            language,
                            vec![agent_semantic_search::RuntimeSearchSource::once(
                                "resident", lookup,
                            )],
                            resident_read_elapsed_micros,
                            parser_owned_selector_pairs,
                            graph_stage,
                        )
                        .await?;
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
                            request.workspace_identity.as_str(),
                            &language_id,
                            generation.generation_digest(),
                            request.request_id.as_str(),
                            "source-index",
                            "runtime-source-index-read",
                            "lookup",
                            elapsed_micros(started),
                        )?;
                        Ok(serde_json::to_value(lookup)
                            .map_err(|error| format!("encode source-index lookup: {error}"))?)
                    }
                    agent_semantic_client_protocol::ServerClientRoute::OwnerSearch => {
                        let started = tokio::time::Instant::now();
                        let params: AspClientOwnerSearchRequest = serde_json::from_value(params)
                            .map_err(|error| {
                                format!("decode ASP client owner-search request: {error}")
                            })?;
                        params.validate_schema_identity()?;
                        let query_terms =
                            agent_semantic_search::source_index_lookup_terms(&params.query);
                        let read = generation.resident().read_runtime_owner_search(
                            &params.owner_path,
                            &query_terms,
                            OWNER_SEARCH_SEED_LIMIT,
                        )?;
                        let response = bounded_owner_search_response(&params, read)?;
                        response.validate()?;
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
                        Ok(serde_json::to_value(response)
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
                            _ = tokio::time::sleep(RUNTIME_CLIENT_DISPATCH_BUDGET) => {
            Err(AspClientDispatchError {
                reason_kind: "client-request-deadline-exceeded".to_owned(),
                message: format!(
                    "Runtime ClientFrame dispatch exceeded its {}ms interactive deadline",
                    RUNTIME_CLIENT_DISPATCH_BUDGET.as_millis(),
                ),
                details: Some(serde_json::json!({
                    "schemaId": "agent.semantic-protocols.asp-client-dispatch-failure",
                    "schemaVersion": "1",
                    "state": "failed",
                    "phase": "runtime-client-dispatch",
                    "reasonKind": "client-request-deadline-exceeded",
                    "budgetMs": RUNTIME_CLIENT_DISPATCH_BUDGET.as_millis(),
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

fn graph_open_payload(
    request: &serde_json::Value,
) -> Result<(String, Arc<serde_json::Value>), String> {
    let graph = request
        .get("graph")
        .cloned()
        .ok_or_else(|| "graph evaluation requires graph".to_owned())?;
    let payload = serde_json::json!({
        "graph": graph,
        "sourceSnapshot": request.get("sourceSnapshot").cloned().unwrap_or(serde_json::Value::Null),
        "workspaceGeneration": request.get("workspaceGeneration").cloned().unwrap_or(serde_json::Value::Null),
    });
    let artifact = agent_semantic_content_identity::ArtifactJson::from_serializable(&payload)
        .map_err(|error| format!("canonicalize graph generation: {error}"))?;
    let digest = format!(
        "blake3-256:{}",
        agent_semantic_content_identity::hash_normalized_json(&artifact).value
    );
    Ok((digest, Arc::new(payload)))
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
