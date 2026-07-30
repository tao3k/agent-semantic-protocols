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

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeServerEvent {
    ConnectionRejected(String),
    ConnectionTaskFailed(String),
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
}

impl RuntimeServer {
    pub fn workspace_registry(
        &self,
    ) -> &std::sync::Arc<crate::runtime_server_workspace::RuntimeServerWorkspaceRegistry> {
        &self.workspace_registry
    }

    pub async fn serve(self) -> Result<RuntimeServerExit, String> {
        let workspace_registry = std::sync::Arc::clone(&self.workspace_registry);
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
            crate::runtime_server_control::RuntimeServerState::Healthy,
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
        })
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
        } = self;
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
                    let connection_drain = drain_receiver.clone();
                    connections.spawn(async move {
                        crate::workspace_db_ipc_server::serve_runtime_server_workspace_stream(
                            &mut stream,
                            &endpoint,
                            &registry,
                            &memory_registry,
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
