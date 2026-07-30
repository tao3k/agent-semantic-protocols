use std::path::Path;
use std::sync::Arc;

use tokio::net::{UnixListener, UnixStream};
use tokio::sync::{mpsc, watch};
use tokio::task::JoinSet;

use crate::WorkspaceDbRegistry;
use crate::runtime_server_control::{
    RuntimeServerControlReceipt, RuntimeServerEndpoint, RuntimeServerRequestReadError,
    bind_runtime_server_listener, read_runtime_server_requests, write_runtime_server_receipts,
};
use crate::runtime_server_control::status_memory::RuntimeServerStatusMemoryWriter;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeServerExit {
    ListenerClosed,
    RestartRequested,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeServerEvent {
    ConnectionRejected(String),
    ConnectionTaskFailed(String),
}

pub struct RuntimeServer {
    endpoint: RuntimeServerEndpoint,
    listener: UnixListener,
    registry: Arc<WorkspaceDbRegistry>,
    status_memory: RuntimeServerStatusMemoryWriter,
    events: Option<mpsc::UnboundedSender<RuntimeServerEvent>>,
}

impl RuntimeServer {
    pub async fn bind(
        endpoint: RuntimeServerEndpoint,
        registry: Arc<WorkspaceDbRegistry>,
    ) -> Result<Self, String> {
        let listener = bind_runtime_server_listener(Path::new(&endpoint.socket_path))?;
        let mut status_memory = RuntimeServerStatusMemoryWriter::create(&endpoint).await?;
        let (slot_count, loaded_entry_count) = registry.workspace_entry_counts();
        status_memory.publish(
            crate::runtime_server_control::RuntimeServerState::Healthy,
            slot_count.max(loaded_entry_count),
        )?;
        Ok(Self {
            endpoint,
            listener,
            registry,
            status_memory,
            events: None,
        })
    }

    pub fn with_event_sender(
        mut self,
        events: mpsc::UnboundedSender<RuntimeServerEvent>,
    ) -> Self {
        self.events = Some(events);
        self
    }

    pub async fn serve(self) -> Result<RuntimeServerExit, String> {
        let Self {
            endpoint,
            listener,
            registry,
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
                completed = connections.join_next(), if !connections.is_empty() => {
                    match completed {
                        Some(Ok(Ok(true))) => {
                            status_memory.publish(
                                crate::runtime_server_control::RuntimeServerState::Draining,
                                registry.workspace_entry_counts().0,
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
            }
        };
        let _ = drain_sender.send(true);
        drop(listener);
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
