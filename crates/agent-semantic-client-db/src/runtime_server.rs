use std::path::Path;
use std::sync::Arc;

use tokio::net::{UnixListener, UnixStream};
use tokio::sync::{mpsc, watch};
use tokio::task::JoinSet;

use crate::WorkspaceDbRegistry;
use crate::runtime_server_control::status_memory::RuntimeServerStatusMemoryWriter;
use crate::runtime_server_control::{
    RuntimeServerControlReceipt, RuntimeServerEndpoint, RuntimeServerRequestReadError,
    bind_runtime_server_listener, read_runtime_server_requests, write_runtime_server_receipts,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeServerExit {
    ListenerClosed,
    RestartRequested,
    ShutdownRequested,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(tag = "kind", content = "detail", rename_all = "kebab-case")]
pub enum RuntimeServerEvent {
    ConnectionRejected(String),
    ConnectionTaskFailed(String),
    WorkspaceGenerationRestoreFailed {
        workspace_identity: String,
        error: String,
    },
}

#[derive(Clone)]
pub struct RuntimeServerShutdownHandle {
    sender: watch::Sender<bool>,
}

impl RuntimeServerShutdownHandle {
    pub fn shutdown(&self) {
        self.sender.send_replace(true);
    }
}

pub struct RuntimeServer {
    workspace_registry:
        std::sync::Arc<crate::runtime_server_workspace::RuntimeServerWorkspaceRegistry>,
    endpoint: RuntimeServerEndpoint,
    listener: UnixListener,
    data_listener: UnixListener,
    registry: Arc<WorkspaceDbRegistry>,
    workspace_count: watch::Receiver<usize>,
    shutdown: watch::Receiver<bool>,
    shutdown_handle: RuntimeServerShutdownHandle,
    status_memory: RuntimeServerStatusMemoryWriter,
    events: Option<mpsc::UnboundedSender<RuntimeServerEvent>>,
    generation_admission:
        Option<Arc<crate::runtime_server_admission::WorkspaceGenerationAdmission>>,
    owner_projection_builder:
        Option<crate::runtime_server_workspace::WorkspaceOwnerProjectionBuilder>,
}

impl RuntimeServer {
    pub fn workspace_registry(
        &self,
    ) -> &std::sync::Arc<crate::runtime_server_workspace::RuntimeServerWorkspaceRegistry> {
        &self.workspace_registry
    }

    /// Returns the supervised workspace generation admission plane when configured.
    pub fn workspace_generation_admission(
        &self,
    ) -> Option<Arc<crate::runtime_server_admission::WorkspaceGenerationAdmission>> {
        self.generation_admission.clone()
    }

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
                        serve.await
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

    pub async fn bind(
        endpoint: RuntimeServerEndpoint,
        registry: Arc<WorkspaceDbRegistry>,
    ) -> Result<Self, String> {
        endpoint.validate()?;
        let listener = bind_runtime_server_listener(Path::new(&endpoint.socket_path))?;
        let data_listener =
            bind_runtime_server_listener(Path::new(&endpoint.data_plane_socket_path))?;
        let mut status_memory = RuntimeServerStatusMemoryWriter::create(&endpoint).await?;
        let (slot_count, loaded_entry_count) = registry.workspace_entry_counts();
        status_memory.publish(
            crate::runtime_server_control::RuntimeServerState::Starting,
            slot_count.max(loaded_entry_count),
        )?;
        let workspace_registry_root = std::path::Path::new(&endpoint.socket_path)
            .parent()
            .ok_or_else(|| "Runtime Server socket path has no parent".to_owned())?
            .join("workspaces");
        let workspace_registry = std::sync::Arc::new(
            crate::runtime_server_workspace::RuntimeServerWorkspaceRegistry::new(
                workspace_registry_root,
            )?,
        );
        let workspace_count = workspace_registry.subscribe_workspace_count();
        let (shutdown_sender, shutdown) = watch::channel(false);
        Ok(Self {
            workspace_registry,
            endpoint,
            listener,
            data_listener,
            registry,
            workspace_count,
            shutdown,
            shutdown_handle: RuntimeServerShutdownHandle {
                sender: shutdown_sender,
            },
            status_memory,
            events: None,
            generation_admission: None,
            owner_projection_builder: None,
        })
    }

    pub fn with_workspace_owner_projection_builder(
        mut self,
        builder: crate::runtime_server_workspace::WorkspaceOwnerProjectionBuilder,
    ) -> Self {
        self.owner_projection_builder = Some(builder);
        self
    }

    pub fn shutdown_handle(&self) -> RuntimeServerShutdownHandle {
        self.shutdown_handle.clone()
    }

    pub async fn bind_and_publish(
        endpoint: RuntimeServerEndpoint,
        registry: Arc<WorkspaceDbRegistry>,
        endpoint_path: &std::path::Path,
    ) -> Result<Self, String> {
        let server = Self::bind(endpoint, registry).await?;
        if let Err(error) = crate::runtime_server_control::publish_runtime_server_endpoint(
            endpoint_path,
            &server.endpoint,
        )
        .await
        {
            server.cleanup_bound_artifacts().await;
            return Err(error);
        }
        Ok(server)
    }

    async fn cleanup_bound_artifacts(&self) {
        for path in [
            &self.endpoint.socket_path,
            &self.endpoint.data_plane_socket_path,
            &self.endpoint.status_memory_path,
        ] {
            let _ = tokio::fs::remove_file(path).await;
        }
    }

    pub fn with_event_sender(mut self, events: mpsc::UnboundedSender<RuntimeServerEvent>) -> Self {
        self.events = Some(events);
        self
    }

    pub fn with_workspace_generation_builder(
        self,
        source_builder: crate::runtime_server_admission::WorkspaceGenerationCandidateBuilder,
    ) -> Self {
        self.configure_workspace_generation_builder(source_builder, None)
    }

    pub fn with_workspace_generation_builder_and_catalog(
        self,
        source_builder: crate::runtime_server_admission::WorkspaceGenerationCandidateBuilder,
        catalog: crate::runtime_server_admission_catalog::RuntimeWorkspaceAdmissionCatalog,
    ) -> Self {
        self.configure_workspace_generation_builder(source_builder, Some(catalog))
    }

    /// Restores only canonical generations already committed to the workspace Turso authority.
    /// Missing generations remain scope-local failures and must be published by the writer lane.
    pub fn with_workspace_generation_catalog(
        self,
        catalog: crate::runtime_server_admission_catalog::RuntimeWorkspaceAdmissionCatalog,
    ) -> Self {
        self.configure_workspace_generation_builder(None, Some(catalog))
    }

    fn configure_workspace_generation_builder(
        mut self,
        source_builder: impl Into<
            Option<crate::runtime_server_admission::WorkspaceGenerationCandidateBuilder>,
        >,
        catalog: Option<crate::runtime_server_admission_catalog::RuntimeWorkspaceAdmissionCatalog>,
    ) -> Self {
        let source_builder = source_builder.into();
        let durable_registry = Arc::clone(&self.registry);
        let memory_registry = Arc::clone(&self.workspace_registry);
        let builder = Arc::new(
            move |workspace_identity: String, project_root: std::path::PathBuf| {
                let durable_registry = Arc::clone(&durable_registry);
                let memory_registry = Arc::clone(&memory_registry);
                let source_builder = source_builder.clone();
                let workspace_for_build = workspace_identity.clone();
                Box::pin(async move {
                    let session = crate::workspace_db_ipc_server::admitted_or_bootstrap_workspace(
                        &durable_registry,
                        &workspace_identity,
                        &project_root,
                    )
                    .await?;
                    match session
                        .load_active_workspace_generation_materialization_state(&project_root)
                        .await?
                    {
                        crate::runtime_server_workspace::WorkspaceCanonicalMaterializationLoad::Ready(materialization) => {
                            materialization.validate_persisted(&workspace_identity)?;
                            memory_registry
                                .ensure_canonical_generation(
                                    format!("daemon-admission-restore-{workspace_identity}"),
                                    &workspace_identity,
                                    materialization,
                                )
                                .await?;
                            return Ok(());
                        }
                        crate::runtime_server_workspace::WorkspaceCanonicalMaterializationLoad::Missing
                        | crate::runtime_server_workspace::WorkspaceCanonicalMaterializationLoad::Incompatible { .. } => {}
                    }
                    let source_builder = source_builder.as_ref().ok_or_else(|| {
                        format!(
                            "canonical workspace generation is unavailable; writer lane publication is required before admission: workspaceIdentity={workspace_identity}"
                        )
                    })?;
                    let build = source_builder(workspace_for_build, project_root.clone()).await?;
                    build
                        .materialization
                        .validate_refresh_request(&workspace_identity, &build.refresh)?;
                    let committed_materialization = build.materialization.clone();
                    let durable = session
                        .commit_source_index_generation(build.refresh, build.materialization)
                        .await?;
                    committed_materialization.validate_persisted(&workspace_identity)?;
                    if !durable
                        .source_snapshot
                        .has_same_content_identity(&committed_materialization.source_snapshot)
                    {
                        return Err(format!(
                            "Turso generation evidence differs from canonical materialization: durable={:?} materialized={:?}",
                            durable.source_snapshot, committed_materialization.source_snapshot
                        ));
                    }
                    memory_registry
                        .ensure_canonical_generation(
                            format!("daemon-admission-build-{workspace_identity}"),
                            &workspace_identity,
                            committed_materialization,
                        )
                        .await?;
                    Ok(())
                })
                    as crate::runtime_server_admission::WorkspaceGenerationBuildFuture
            },
        );
        let admission = crate::runtime_server_admission::WorkspaceGenerationAdmission::new(builder);
        self.generation_admission = Some(Arc::new(match catalog {
            Some(catalog) => admission.with_catalog(catalog),
            None => admission,
        }));
        self
    }

    async fn serve_inner(self) -> Result<RuntimeServerExit, String> {
        let Self {
            workspace_registry,
            endpoint,
            listener,
            data_listener,
            registry,
            mut workspace_count,
            mut shutdown,
            shutdown_handle: _,
            mut status_memory,
            events,
            generation_admission,
            owner_projection_builder,
        } = self;
        let (slot_count, loaded_entry_count) = registry.workspace_entry_counts();
        status_memory.publish(
            crate::runtime_server_control::RuntimeServerState::Healthy,
            slot_count
                .max(loaded_entry_count)
                .max(*workspace_count.borrow()),
        )?;
        let restore_task = generation_admission.clone().map(|admission| {
            let events = events.clone();
            tokio::spawn(async move {
                match admission.restore_registered().await {
                    Ok(report) => {
                        for receipt in report.failed {
                            publish_event(
                                events.as_ref(),
                                RuntimeServerEvent::WorkspaceGenerationRestoreFailed {
                                    workspace_identity: receipt.workspace_identity,
                                    error: receipt.error.unwrap_or_else(|| "unknown".to_owned()),
                                },
                            );
                        }
                    }
                    Err(error) => publish_event(
                        events.as_ref(),
                        RuntimeServerEvent::ConnectionTaskFailed(format!(
                            "workspace generation restore task failed: {error}"
                        )),
                    ),
                }
            })
        });
        let mut connections = JoinSet::new();
        let (drain_sender, drain_receiver) = watch::channel(false);
        let exit = loop {
            tokio::select! {
                connection = listener.accept() => {
                    let (stream, _) = connection.map_err(|error| {
                        format!("failed to accept runtime server request: {error}")
                    })?;
                    connections.spawn(serve_connection(
                        stream,
                        endpoint.clone(),
                        Arc::clone(&registry),
                        drain_receiver.clone(),
                    ));
                }
                connection = data_listener.accept() => {
                    let (mut stream, _) = connection.map_err(|error| {
                        format!("failed to accept Runtime Server data-plane request: {error}")
                    })?;
                    let endpoint = endpoint.clone();
                    let registry = Arc::clone(&registry);
                    let memory_registry = Arc::clone(&workspace_registry);
                    let generation_admission = generation_admission.clone();
                    let owner_projection_builder = owner_projection_builder.clone();
                    let connection_drain = drain_receiver.clone();
                    connections.spawn(async move {
                        crate::workspace_db_ipc_server::serve_runtime_server_workspace_stream(
                            &mut stream,
                            &endpoint,
                            &registry,
                            &memory_registry,
                            generation_admission.as_deref(),
                            owner_projection_builder.as_ref(),
                            connection_drain,
                        )
                        .await?;
                        Ok(false)
                    });
                }
                completed = connections.join_next(), if !connections.is_empty() => {
                    match completed {
                        Some(Ok(Ok(true))) => {
                            let (slot_count, loaded_entry_count) =
                                registry.workspace_entry_counts();
                            status_memory.publish(
                                crate::runtime_server_control::RuntimeServerState::Draining,
                                slot_count
                                    .max(loaded_entry_count)
                                    .max(*workspace_count.borrow()),
                            )?;
                            let _ = drain_sender.send(true);
                            break RuntimeServerExit::RestartRequested;
                        }
                        Some(Ok(Ok(false))) => {}
                        Some(Ok(Err(error))) => {
                            publish_event(
                                events.as_ref(),
                                RuntimeServerEvent::ConnectionRejected(error),
                            );
                        }
                        Some(Err(error)) => {
                            publish_event(
                                events.as_ref(),
                                RuntimeServerEvent::ConnectionTaskFailed(error.to_string()),
                            );
                        }
                        None => break RuntimeServerExit::ListenerClosed,
                    }
                }
                changed = workspace_count.changed() => {
                    if changed.is_err() {
                        break RuntimeServerExit::ListenerClosed;
                    }
                    let (slot_count, loaded_entry_count) = registry.workspace_entry_counts();
                    status_memory.publish(
                        crate::runtime_server_control::RuntimeServerState::Healthy,
                        slot_count
                            .max(loaded_entry_count)
                            .max(*workspace_count.borrow_and_update()),
                    )?;
                }
                changed = shutdown.changed() => {
                    if changed.is_err() || *shutdown.borrow_and_update() {
                        let (slot_count, loaded_entry_count) =
                            registry.workspace_entry_counts();
                        status_memory.publish(
                            crate::runtime_server_control::RuntimeServerState::Draining,
                            slot_count
                                .max(loaded_entry_count)
                                .max(*workspace_count.borrow()),
                        )?;
                        let _ = drain_sender.send(true);
                        break RuntimeServerExit::ShutdownRequested;
                    }
                }
            }
        };
        let _ = drain_sender.send(true);
        drop(listener);
        drop(data_listener);
        while let Some(completed) = connections.join_next().await {
            match completed {
                Ok(Ok(_)) => {}
                Ok(Err(error)) => publish_event(
                    events.as_ref(),
                    RuntimeServerEvent::ConnectionRejected(error),
                ),
                Err(error) => publish_event(
                    events.as_ref(),
                    RuntimeServerEvent::ConnectionTaskFailed(error.to_string()),
                ),
            }
        }
        if let Some(restore_task) = restore_task {
            restore_task.abort();
            let _ = restore_task.await;
        }
        Ok(exit)
    }
}

#[cfg(unix)]
async fn runtime_server_shutdown_signal() -> Result<(), String> {
    let mut terminate = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        .map_err(|error| {
        format!("failed to install Runtime Server SIGTERM handler: {error}")
    })?;
    tokio::select! {
        result = tokio::signal::ctrl_c() => {
            result.map_err(|error| format!("failed to await Runtime Server Ctrl-C signal: {error}"))
        }
        _ = terminate.recv() => Ok(()),
    }
}

#[cfg(not(unix))]
async fn runtime_server_shutdown_signal() -> Result<(), String> {
    tokio::signal::ctrl_c()
        .await
        .map_err(|error| format!("failed to await Runtime Server Ctrl-C signal: {error}"))
}

fn publish_event(
    events: Option<&mpsc::UnboundedSender<RuntimeServerEvent>>,
    event: RuntimeServerEvent,
) {
    if let Some(events) = events {
        let _ = events.send(event);
    }
}

async fn serve_connection(
    mut stream: UnixStream,
    endpoint: RuntimeServerEndpoint,
    registry: Arc<WorkspaceDbRegistry>,
    mut drain: watch::Receiver<bool>,
) -> Result<bool, String> {
    loop {
        let requests = tokio::select! {
            requests = read_runtime_server_requests(&mut stream) => match requests {
                Ok(requests) => requests,
                Err(RuntimeServerRequestReadError::Closed) => return Ok(false),
                Err(RuntimeServerRequestReadError::Invalid(error)) => return Err(error),
            },
            changed = drain.changed() => {
                let _ = changed;
                return Ok(false);
            }
        };
        let (slot_count, loaded_entry_count) = registry.workspace_entry_counts();
        let workspace_entry_count = slot_count.max(loaded_entry_count);
        let mut restart = false;
        let mut receipts = Vec::with_capacity(requests.len());
        for request in requests {
            let request_restart = request.requires_restart(&endpoint)?;
            restart |= request_restart;
            receipts.push(if request_restart {
                RuntimeServerControlReceipt::draining(
                    request.request_id,
                    &endpoint,
                    workspace_entry_count,
                )
            } else {
                RuntimeServerControlReceipt::healthy(
                    request.request_id,
                    &endpoint,
                    workspace_entry_count,
                )
            });
        }
        write_runtime_server_receipts(&mut stream, &receipts).await?;
        if restart {
            return Ok(true);
        }
    }
}
