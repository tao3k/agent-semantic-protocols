use super::core::runtime_server_shutdown_signal;
use super::core::{RuntimeServer, RuntimeServerExit, RuntimeServerShutdownHandle};
use crate::WorkspaceDbRegistry;
use crate::runtime_server_control::status_memory::RuntimeServerStatusMemoryWriter;
use crate::runtime_server_control::{
    RuntimeServerEndpoint, bind_runtime_server_listener, publish_runtime_server_endpoint,
};
use std::path::Path;
use std::sync::Arc;
use tokio::sync::watch;

impl RuntimeServer {
    pub async fn serve(self) -> Result<RuntimeServerExit, String> {
        let workspace_registry = std::sync::Arc::clone(&self.workspace_registry);
        let generation_admission = self.generation_admission.clone();
        let shutdown_handle = self.shutdown_handle();
        let serve = self.serve_inner();
        tokio::pin!(serve);
        let serve_result = tokio::select! {
            result = &mut serve => result,
            signal = runtime_server_shutdown_signal() => {
                match signal {
                    Ok(()) => {
                        shutdown_handle.shutdown();
                        // Startup restore may be waiting on a generation build.
                        // Do not await that request-owned future during drain;
                        // dropping `serve` lets the Server cleanup below cancel
                        // admission builds and flush the exit receipt.
                        Ok(RuntimeServerExit::ShutdownRequested)
                    }
                    Err(error) => Err(error),
                }
            }
        };
        let shutdown_result = workspace_registry.shutdown().await;
        let admission_shutdown_result = match generation_admission {
            Some(admission) => admission.shutdown().await.map(|_| ()),
            None => Ok(()),
        };
        let shutdown_result = match (shutdown_result, admission_shutdown_result) {
            (Ok(_), Ok(())) => Ok(()),
            (Err(error), Ok(())) | (Ok(_), Err(error)) => Err(error),
            (Err(workspace_error), Err(admission_error)) => Err(format!(
                "workspace lane shutdown failed: {workspace_error}; admission lane shutdown failed: {admission_error}"
            )),
        };
        match (serve_result, shutdown_result) {
            (Ok(exit), Ok(_)) => Ok(exit),
            (Err(error), Ok(_)) => Err(error),
            (Ok(_), Err(error)) => Err(error),
            (Err(serve_error), Err(shutdown_error)) => Err(format!(
                "Runtime Server failed and workspace lanes did not drain: serve={serve_error}; shutdown={shutdown_error}"
            )),
        }
    }

    pub async fn bind_with_catalog(
        endpoint: RuntimeServerEndpoint,
        registry: Arc<WorkspaceDbRegistry>,
        artifact_catalog: Arc<
            agent_semantic_runtime::runtime_artifact_catalog::RuntimeArtifactCatalog,
        >,
    ) -> Result<Self, String> {
        let workspace_store =
            crate::runtime_server_workspace::prepare_runtime_server_workspace_store_at_root(
                endpoint.workspace_store_path.clone().into(),
            )
            .await?;
        Self::bind_with_artifact_catalog(endpoint, registry, workspace_store, artifact_catalog)
            .await
    }

    pub async fn bind_with_artifact_catalog(
        endpoint: RuntimeServerEndpoint,
        registry: Arc<WorkspaceDbRegistry>,
        workspace_store: crate::runtime_server_workspace::RuntimeServerWorkspaceStore,
        artifact_catalog: Arc<
            agent_semantic_runtime::runtime_artifact_catalog::RuntimeArtifactCatalog,
        >,
    ) -> Result<Self, String> {
        endpoint.validate()?;
        if Path::new(&endpoint.workspace_store_path) != workspace_store.root() {
            return Err(format!(
                "Runtime Server endpoint workspace store authority mismatch: endpoint={} server={}",
                endpoint.workspace_store_path,
                workspace_store.root().display()
            ));
        }
        if endpoint.artifact_mode != artifact_catalog.mode_label()
            || endpoint.artifact_catalog_digest != artifact_catalog.digest()
        {
            return Err(format!(
                "Runtime Server endpoint artifact catalog mismatch: endpointMode={} runtimeMode={} endpointDigest={} runtimeDigest={}",
                endpoint.artifact_mode,
                artifact_catalog.mode_label(),
                endpoint.artifact_catalog_digest,
                artifact_catalog.digest()
            ));
        }
        let listener = bind_runtime_server_listener(Path::new(&endpoint.socket_path))?;
        let data_listener =
            bind_runtime_server_listener(Path::new(&endpoint.data_plane_socket_path))?;
        let provider_register_state_path =
            crate::runtime_server_control::provider_register_state_path(Path::new(
                &endpoint.provider_plane_socket_path,
            ))?;
        let mut status_memory = RuntimeServerStatusMemoryWriter::create(&endpoint).await?;
        let entry_counts = registry.workspace_entry_counts();
        let slot_count = entry_counts.slot_count;
        let loaded_entry_count = entry_counts.loaded_entry_count;
        status_memory.publish(
            crate::runtime_server_control::RuntimeServerState::Starting,
            slot_count.max(loaded_entry_count),
        )?;
        let workspace_registry = std::sync::Arc::new(
            crate::runtime_server_workspace::RuntimeServerWorkspaceRegistry::new(
                workspace_store.root().to_path_buf(),
            )?,
        );
        let workspace_count = workspace_registry.subscribe_workspace_count();
        let (shutdown_sender, shutdown) = watch::channel(false);
        let (readiness_sender, _readiness) =
            watch::channel(crate::runtime_server_control::RuntimeServerState::Starting);
        Ok(Self {
            artifact_catalog,
            workspace_registry,
            endpoint,
            listener,
            data_listener,
            provider_register: Arc::new(
                crate::runtime_provider_register::RuntimeProviderRegister::from_seed_with_store(
                    agent_semantic_provider_protocol::builtin_provider_registrations()?,
                    provider_register_state_path,
                )
                .await?,
            ),
            registry,
            workspace_count,
            shutdown,
            shutdown_handle: RuntimeServerShutdownHandle {
                sender: shutdown_sender,
            },
            readiness_sender,
            generation_publication:
                crate::runtime_server_publication::WorkspaceGenerationPublication::new(),
            status_memory,
            events: None,
            generation_admission: None,
            graph_turbo_evaluation_builder: None,
            runtime_search_service: None,
            graph_turbo_resident_status: None,
            agent_session_registry_owner: None,
            session_control_plane_runtime_registry: Arc::new(
                crate::SessionControlPlaneRuntimeRegistry::default(),
            ),
            agent_session_status: None,
            codex_multi_agent_control_plane_owner: Arc::new(
                crate::codex_multi_agent_control_plane_owner::CodexMultiAgentControlPlaneOwner::new(
                ),
            ),
            telemetry_sender: None,
        })
    }

    pub fn shutdown_handle(&self) -> RuntimeServerShutdownHandle {
        self.shutdown_handle.clone()
    }

    pub async fn bind_and_publish_with_catalog(
        endpoint: RuntimeServerEndpoint,
        registry: Arc<WorkspaceDbRegistry>,
        endpoint_path: &std::path::Path,
        artifact_catalog: Arc<
            agent_semantic_runtime::runtime_artifact_catalog::RuntimeArtifactCatalog,
        >,
    ) -> Result<Self, String> {
        let server = Self::bind_with_catalog(endpoint, registry, artifact_catalog).await?;
        if let Err(error) = publish_runtime_server_endpoint(endpoint_path, &server.endpoint).await {
            server.cleanup_bound_artifacts().await;
            return Err(error);
        }
        Ok(server)
    }

    pub async fn bind_and_publish_with_artifact_catalog(
        endpoint: RuntimeServerEndpoint,
        registry: Arc<WorkspaceDbRegistry>,
        endpoint_path: &std::path::Path,
        workspace_store: crate::runtime_server_workspace::RuntimeServerWorkspaceStore,
        artifact_catalog: Arc<
            agent_semantic_runtime::runtime_artifact_catalog::RuntimeArtifactCatalog,
        >,
    ) -> Result<Self, String> {
        let server =
            Self::bind_with_artifact_catalog(endpoint, registry, workspace_store, artifact_catalog)
                .await?;
        if let Err(error) = publish_runtime_server_endpoint(endpoint_path, &server.endpoint).await {
            server.cleanup_bound_artifacts().await;
            return Err(error);
        }
        Ok(server)
    }
}
