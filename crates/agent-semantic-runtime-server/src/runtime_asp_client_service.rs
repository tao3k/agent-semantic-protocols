// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;
use std::sync::Mutex;

use crate::RuntimeQueryGenerationAuthority;
use agent_semantic_client_db::runtime_search_service::RuntimeSearchServiceHandle;
use agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationAdmission;
use agent_semantic_client_protocol::ClientProjectId;
use agent_semantic_client_protocol::ClientRequestId;
use agent_semantic_client_protocol::ClientSessionId;
use agent_semantic_client_protocol::ClientWorkspaceIdentity;
use agent_semantic_client_server::AspClientFrameService;
use agent_semantic_search::WorkspaceSearchProvider;

pub(super) type ClientRequestKey = (
    ClientProjectId,
    ClientWorkspaceIdentity,
    ClientSessionId,
    ClientRequestId,
);
pub(super) type ClientWorkspaceKey = (String, String, String);

/// Capture the complete provider facts needed by workspace Search planning.
///
/// The Runtime computes this once from its admitted Provider Register during
/// daemon bootstrap.  It is deliberately not reconstructed from CLI arguments
/// or Hook configuration on the client path.
pub fn workspace_search_providers_from_provider_register(
    register: &agent_semantic_client_db::runtime_provider_register::RuntimeProviderRegister,
    schema_bundles: &crate::schema_bundle::RuntimeSchemaBundleCatalog,
) -> Result<Arc<[WorkspaceSearchProvider]>, String> {
    let snapshot = register.snapshot();
    let mut providers = snapshot
        .providers
        .iter()
        .map(|provider| {
            let inventory = provider.source_inventory()?;
            let search_supported = provider
                .registration_field("searchCapabilities")?
                .get("ownerItems")
                .and_then(serde_json::Value::as_bool)
                .ok_or_else(|| {
                    format!(
                        "provider searchCapabilities.ownerItems is invalid: languageId={}",
                        provider.language_id
                    )
                })?;
            let mut source_extensions = inventory
                .source_extensions
                .into_iter()
                .map(|extension| extension.trim_start_matches('.').to_owned())
                .collect::<Vec<_>>();
            source_extensions.sort_unstable();
            source_extensions.dedup();
            if source_extensions.iter().any(String::is_empty) {
                return Err(format!(
                    "provider sourceInventory has an empty public extension: languageId={}",
                    provider.language_id
                ));
            }
            let producer_axes = schema_bundles
                .search_producer_axes(&provider.language_id)
                .ok_or_else(|| {
                    format!(
                        "provider has no admitted schema profile: languageId={}",
                        provider.language_id
                    )
                })?
                .iter()
                .map(|axis| match axis {
                    agent_semantic_schema_manager::SearchProducerAxis::Language => {
                        agent_semantic_search::WorkspaceSearchProducerAxis::Language
                    }
                    agent_semantic_schema_manager::SearchProducerAxis::Document => {
                        agent_semantic_search::WorkspaceSearchProducerAxis::Document
                    }
                })
                .collect();
            Ok(WorkspaceSearchProvider {
                language_id: provider.language_id.clone(),
                provider_id: provider.provider_id.clone(),
                source_extensions,
                search_supported,
                producer_axes,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    providers.sort_by(|left, right| left.language_id.cmp(&right.language_id));
    if providers
        .windows(2)
        .any(|pair| pair[0].language_id == pair[1].language_id)
    {
        return Err(
            "Runtime workspace Search provider snapshot has duplicate languages".to_owned(),
        );
    }
    Ok(Arc::from(providers))
}

#[derive(Clone)]
pub(super) struct InitializedWorkspace {
    pub(super) project_root: std::path::PathBuf,
    pub(super) host_workspace: agent_semantic_content_identity::HostWorkspaceInitializationBinding,
}

pub type HostWorkspaceInitializationBindingResolver = Arc<
    dyn Fn(
            &std::path::Path,
        )
            -> Result<agent_semantic_content_identity::HostWorkspaceInitializationBinding, String>
        + Send
        + Sync,
>;

impl InitializedWorkspace {
    fn from_project_root(
        project_root: std::path::PathBuf,
        resolver: &HostWorkspaceInitializationBindingResolver,
    ) -> Result<Self, String> {
        let host_workspace = resolver(&project_root)?;
        host_workspace
            .validate()
            .map_err(|error| format!("invalid Host workspace initialization binding: {error}"))?;
        Ok(Self {
            project_root,
            host_workspace,
        })
    }
}

#[derive(Clone)]
pub struct RuntimeAspClientDispatcher {
    pub(super) schema_bundles: crate::schema_bundle::RuntimeSchemaBundleCatalog,
    pub(super) agent_session_registry: Arc<agent_semantic_client_db::AgentSessionRegistry>,
    pub(super) initialized_workspaces:
        Arc<Mutex<HashMap<ClientWorkspaceKey, InitializedWorkspace>>>,
    pub(super) runtime_search_service: RuntimeSearchServiceHandle,
    pub(super) generation_admission: Arc<WorkspaceGenerationAdmission>,
    pub(super) workspace_registry:
        Arc<agent_semantic_client_db::runtime_server_workspace::RuntimeServerWorkspaceRegistry>,
    pub(super) active_provider_targets: Arc<[(String, String)]>,
    /// Runtime-owned provider authority. Each Search/Query request derives one
    /// immutable snapshot, so a committed provider refresh is visible to the
    /// next request without client inference or daemon restart.
    pub(super) provider_register:
        Arc<agent_semantic_client_db::runtime_provider_register::RuntimeProviderRegister>,
    pub(super) workspace_store_root: std::path::PathBuf,
    pub(super) query_generation_authority: RuntimeQueryGenerationAuthority,
    pub(super) telemetry_sender:
        agent_semantic_client_db::runtime_telemetry_bus::RuntimeTelemetryBusSender,
    pub(super) telemetry_traces: Arc<
        Mutex<
            HashMap<
                ClientRequestKey,
                agent_semantic_client_db::runtime_server_opentelemetry::RuntimeSearchTelemetryTrace,
            >,
        >,
    >,
    pub(super) active_telemetry_trace_count: Arc<std::sync::atomic::AtomicUsize>,
    pub(super) cancellations:
        Arc<Mutex<HashMap<ClientRequestKey, tokio::sync::watch::Sender<bool>>>>,
    pub(super) cancellation_admitted: Arc<tokio::sync::Notify>,
    pub(super) resident_request_seen:
        Arc<Mutex<HashSet<(ClientProjectId, ClientWorkspaceIdentity)>>>,
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
        active_provider_targets: Arc<[(String, String)]>,
        provider_register: Arc<
            agent_semantic_client_db::runtime_provider_register::RuntimeProviderRegister,
        >,
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
            active_provider_targets,
            provider_register,
            workspace_store_root,
            query_generation_authority,
            telemetry_sender,
            telemetry_traces: Arc::new(Mutex::new(HashMap::new())),
            active_telemetry_trace_count: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            cancellations: Arc::new(Mutex::new(HashMap::new())),
            cancellation_admitted: Arc::new(tokio::sync::Notify::new()),
            resident_request_seen: Arc::new(Mutex::new(HashSet::new())),
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
    active_provider_targets: Arc<[(String, String)]>,
    provider_register: Arc<
        agent_semantic_client_db::runtime_provider_register::RuntimeProviderRegister,
    >,
    workspace_store_root: std::path::PathBuf,
    host_workspace_resolver: HostWorkspaceInitializationBindingResolver,
    query_generation_authority: RuntimeQueryGenerationAuthority,
    telemetry_sender: agent_semantic_client_db::runtime_telemetry_bus::RuntimeTelemetryBusSender,
) -> Result<Arc<AspClientFrameService<RuntimeAspClientDispatcher>>, String> {
    let initialized_workspaces = Arc::new(Mutex::new(HashMap::new()));
    let catalog_generation = client_catalog_generation;
    let catalog_provider_targets = Arc::clone(&active_provider_targets);
    let dispatcher = Arc::new(RuntimeAspClientDispatcher::new(
        schema_bundles,
        agent_session_registry,
        runtime_search_service,
        Arc::clone(&generation_admission),
        workspace_registry,
        Arc::clone(&initialized_workspaces),
        active_provider_targets,
        provider_register,
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
            let host_workspace_resolver = Arc::clone(&host_workspace_resolver);
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
                let initialized = InitializedWorkspace::from_project_root(
                    project_root.clone(),
                    &host_workspace_resolver,
                )?;
                let mut workspaces = initialized_workspaces
                    .lock()
                    .map_err(|_| "ASP client workspace-root registry poisoned".to_owned())?;
                if let Some(current) = workspaces.get(&key)
                    && (current.project_root != project_root
                        || current.host_workspace != initialized.host_workspace)
                {
                    return Err(
                        "client session was rebound to a different projectRoot or Project Topology manifest"
                            .to_owned(),
                    );
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

#[cfg(test)]
#[path = "../tests/unit/runtime_asp_client_service.rs"]
mod tests;
