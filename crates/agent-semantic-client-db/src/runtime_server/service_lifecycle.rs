// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

use super::core::runtime_server_shutdown_signal;
use super::core::{RuntimeServer, RuntimeServerExit, RuntimeServerShutdownHandle};
use crate::WorkspaceDbRegistry;
use crate::runtime_server_control::status_memory::RuntimeServerStatusMemoryWriter;
use crate::runtime_server_control::{RuntimeServerEndpoint, publish_runtime_server_endpoint};
use std::path::Path;
use std::sync::Arc;
use tokio::sync::watch;

fn validate_runtime_server_bind_authorities(
    endpoint: &RuntimeServerEndpoint,
    workspace_store: &crate::runtime_server_workspace::RuntimeServerWorkspaceStore,
    artifact_catalog: &agent_semantic_artifacts::runtime_artifact_catalog::RuntimeArtifactCatalog,
) -> Result<(), String> {
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
    Ok(())
}

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
            agent_semantic_artifacts::runtime_artifact_catalog::RuntimeArtifactCatalog,
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
            agent_semantic_artifacts::runtime_artifact_catalog::RuntimeArtifactCatalog,
        >,
    ) -> Result<Self, String> {
        validate_runtime_server_bind_authorities(
            &endpoint,
            &workspace_store,
            artifact_catalog.as_ref(),
        )?;
        let listener = crate::runtime_server_control::bind_runtime_server_listener_at(
            &endpoint.control_endpoint,
        )
        .await?;
        Self::bind_validated_artifact_catalog(
            endpoint,
            listener,
            registry,
            workspace_store,
            artifact_catalog,
        )
        .await
    }

    pub async fn bind_with_artifact_catalog_and_listener(
        endpoint: RuntimeServerEndpoint,
        listener: tokio::net::TcpListener,
        registry: Arc<WorkspaceDbRegistry>,
        workspace_store: crate::runtime_server_workspace::RuntimeServerWorkspaceStore,
        artifact_catalog: Arc<
            agent_semantic_artifacts::runtime_artifact_catalog::RuntimeArtifactCatalog,
        >,
    ) -> Result<Self, String> {
        validate_runtime_server_bind_authorities(
            &endpoint,
            &workspace_store,
            artifact_catalog.as_ref(),
        )?;
        let observed = listener
            .local_addr()
            .map_err(|error| format!("inspect Runtime control listener: {error}"))?;
        if observed != endpoint.control_endpoint.socket_addr() {
            return Err("Runtime control listener and endpoint publication differ".to_owned());
        }
        Self::bind_validated_artifact_catalog(
            endpoint,
            listener,
            registry,
            workspace_store,
            artifact_catalog,
        )
        .await
    }

    async fn bind_validated_artifact_catalog(
        endpoint: RuntimeServerEndpoint,
        listener: tokio::net::TcpListener,
        registry: Arc<WorkspaceDbRegistry>,
        workspace_store: crate::runtime_server_workspace::RuntimeServerWorkspaceStore,
        artifact_catalog: Arc<
            agent_semantic_artifacts::runtime_artifact_catalog::RuntimeArtifactCatalog,
        >,
    ) -> Result<Self, String> {
        let provider_register_state_path = std::path::PathBuf::from(&endpoint.workspace_store_path)
            .join("provider-register.v1.json");
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
        let provider_seed = agent_semantic_provider_protocol::builtin_provider_registrations()?;
        let provider_register = if artifact_catalog.runtime_bundle_digest().is_some() {
            crate::runtime_provider_register::RuntimeProviderRegister::from_verified_seed_with_store(
                provider_seed,
                provider_register_state_path,
                artifact_catalog.active_provider_targets(),
            )
            .await?
        } else {
            crate::runtime_provider_register::RuntimeProviderRegister::from_seed_with_store(
                provider_seed,
                provider_register_state_path,
            )
            .await?
        };
        Ok(Self {
            artifact_catalog,
            workspace_registry,
            endpoint,
            listener,
            provider_register: Arc::new(provider_register),
            registry,
            workspace_count,
            shutdown,
            shutdown_handle: RuntimeServerShutdownHandle {
                sender: shutdown_sender,
            },
            readiness_sender,
            generation_publication:
                crate::runtime_server_publication::WorkspaceGenerationPublication::new(),
            durability_tasks: Arc::new(tokio::sync::Mutex::new(tokio::task::JoinSet::new())),
            status_memory,
            events: None,
            generation_admission: None,
            runtime_search_service: None,
            asp_python_graphs_status: None,
            agent_session_registry_owner: None,
            agent_session_status: None,
            telemetry_sender: None,
        })
    }

    pub fn shutdown_handle(&self) -> RuntimeServerShutdownHandle {
        self.shutdown_handle.clone()
    }

    /// Publish the resident endpoint only after every required transport plane
    /// has been bound by its owning package.
    pub async fn publish_endpoint_after_required_planes(
        &self,
        endpoint_path: &std::path::Path,
    ) -> Result<(), String> {
        if let Err(error) = self.endpoint.validate_service_reachability().await {
            self.cleanup_bound_artifacts().await;
            return Err(error);
        }
        if let Err(error) = publish_runtime_server_endpoint(endpoint_path, &self.endpoint).await {
            self.cleanup_bound_artifacts().await;
            return Err(error);
        }
        Ok(())
    }
}
