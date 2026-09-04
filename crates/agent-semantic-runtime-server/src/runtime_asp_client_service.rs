use std::collections::HashMap;
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
            Ok(WorkspaceSearchProvider {
                language_id: provider.language_id.clone(),
                provider_id: provider.provider_id.clone(),
                source_extensions,
                search_supported,
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
    pub(super) installed_provider_targets: Arc<[(String, String)]>,
    /// Immutable provider facts captured once when this Runtime daemon starts.
    /// Workspace Search planning reads this snapshot; it never asks a client to
    /// supply or infer a language/provider mapping.
    pub(super) workspace_search_providers: Arc<[WorkspaceSearchProvider]>,
    pub(super) workspace_store_root: std::path::PathBuf,
    pub(super) query_generation_authority: RuntimeQueryGenerationAuthority,
    pub(super) telemetry_sender:
        agent_semantic_client_db::runtime_telemetry_bus::RuntimeTelemetryBusSender,
    pub(super) cancellations:
        Arc<Mutex<HashMap<ClientRequestKey, tokio::sync::watch::Sender<bool>>>>,
    pub(super) cancellation_admitted: Arc<tokio::sync::Notify>,
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
        workspace_search_providers: Arc<[WorkspaceSearchProvider]>,
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
            workspace_search_providers,
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
    workspace_search_providers: Arc<[WorkspaceSearchProvider]>,
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
        workspace_search_providers,
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

#[cfg(test)]
mod tests {
    use super::workspace_search_providers_from_provider_register;

    #[test]
    fn workspace_search_provider_snapshot_uses_admitted_register_facts() {
        let register = agent_semantic_client_db::runtime_provider_register::RuntimeProviderRegister::from_seed(
            agent_semantic_provider_protocol::builtin_provider_registrations()
                .expect("builtin provider registrations"),
        )
        .expect("validated provider register");
        let providers = workspace_search_providers_from_provider_register(&register)
            .expect("immutable workspace Search provider snapshot");
        assert!(!providers.is_empty());
        assert!(providers.iter().all(|provider| {
            provider.search_supported
                && !provider.source_extensions.is_empty()
                && provider
                    .source_extensions
                    .iter()
                    .all(|extension| !extension.starts_with('.'))
        }));
        assert!(
            providers
                .windows(2)
                .all(|pair| pair[0].language_id < pair[1].language_id)
        );
    }
}
