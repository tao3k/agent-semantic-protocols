use std::future::Future;
use std::pin::Pin;

use bytes::Bytes;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio::sync::watch;

use crate::ProviderRuntimeContractReceipt;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProviderRuntimeActorState {
    Starting,
    Warming,
    Ready(ProviderRuntimeContractReceipt),
    Draining,
    Failed(String),
    Stopped,
}

enum ProviderRuntimeActorCommand {
    Request {
        request_id: u64,
        operation: String,
        payload: Bytes,
        response: oneshot::Sender<Result<Bytes, String>>,
    },
    Cancel {
        request_id: u64,
    },
    Drain {
        response: oneshot::Sender<()>,
    },
    Shutdown {
        response: oneshot::Sender<()>,
    },
}

#[derive(Clone)]
pub struct ProviderRuntimeActorClient {
    commands: mpsc::Sender<ProviderRuntimeActorCommand>,
    state: watch::Receiver<ProviderRuntimeActorState>,
    next_request_id: std::sync::Arc<std::sync::atomic::AtomicU64>,
}

pub struct ProviderRuntimeRequest {
    request_id: u64,
    response: oneshot::Receiver<Result<Bytes, String>>,
    commands: mpsc::Sender<ProviderRuntimeActorCommand>,
    terminal: bool,
}

impl Future for ProviderRuntimeRequest {
    type Output = Result<Bytes, String>;

    fn poll(
        self: std::pin::Pin<&mut Self>,
        context: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let request = self.get_mut();
        match std::pin::Pin::new(&mut request.response).poll(context) {
            std::task::Poll::Ready(Ok(result)) => {
                request.terminal = true;
                std::task::Poll::Ready(result)
            }
            std::task::Poll::Ready(Err(_)) => {
                request.terminal = true;
                std::task::Poll::Ready(Err(
                    "asp-client-server-request-terminal: state=response-dropped".to_owned(),
                ))
            }
            std::task::Poll::Pending => std::task::Poll::Pending,
        }
    }
}

impl Drop for ProviderRuntimeRequest {
    fn drop(&mut self) {
        if self.terminal {
            return;
        }
        let _ = self.commands.try_send(ProviderRuntimeActorCommand::Cancel {
            request_id: self.request_id,
        });
    }
}

impl ProviderRuntimeActorClient {
    pub fn states(&self) -> tokio_stream::wrappers::WatchStream<ProviderRuntimeActorState> {
        tokio_stream::wrappers::WatchStream::new(self.state.clone())
    }

    pub fn current(&self) -> ProviderRuntimeActorState {
        self.state.borrow().clone()
    }

    pub fn lifecycle_states(
        &self,
        expected: ProviderRuntimeContractReceipt,
    ) -> impl tokio_stream::Stream<Item = Result<crate::AspClientServerLifecycleReceipt, String>>
    {
        tokio_stream::StreamExt::map(self.states(), move |state| {
            crate::AspClientServerLifecycleReceipt::from_actor_state(&expected, state)
        })
    }

    pub fn current_lifecycle(
        &self,
        expected: &ProviderRuntimeContractReceipt,
    ) -> Result<crate::AspClientServerLifecycleReceipt, String> {
        crate::AspClientServerLifecycleReceipt::from_actor_state(expected, self.current())
    }

    pub async fn wait_ready(&mut self) -> Result<ProviderRuntimeContractReceipt, String> {
        loop {
            match self.current() {
                ProviderRuntimeActorState::Ready(receipt) => return Ok(receipt),
                ProviderRuntimeActorState::Failed(reason) => return Err(reason),
                ProviderRuntimeActorState::Stopped => {
                    return Err("asp-client-server-stopped-before-ready".to_owned());
                }
                ProviderRuntimeActorState::Starting | ProviderRuntimeActorState::Warming => {}
                ProviderRuntimeActorState::Draining => {
                    return Err("asp-client-server-drained-before-ready".to_owned());
                }
            }
            self.state
                .changed()
                .await
                .map_err(|_| "asp-client-server-state-channel-closed-before-ready".to_owned())?;
        }
    }

    pub async fn request(
        &self,
        operation: impl Into<String>,
        payload: impl Into<Bytes>,
    ) -> Result<Bytes, String> {
        self.begin_request(operation, payload).await?.await
    }

    pub async fn begin_request(
        &self,
        operation: impl Into<String>,
        payload: impl Into<Bytes>,
    ) -> Result<ProviderRuntimeRequest, String> {
        let operation = operation.into();
        let payload = payload.into();
        let receipt = match self.current() {
            ProviderRuntimeActorState::Ready(receipt) => receipt,
            ProviderRuntimeActorState::Starting => {
                return Err("asp-client-server-not-ready: state=starting".to_owned());
            }
            ProviderRuntimeActorState::Warming => {
                return Err("asp-client-server-not-ready: state=warming".to_owned());
            }
            ProviderRuntimeActorState::Draining => {
                return Err("asp-client-server-not-ready: state=draining".to_owned());
            }
            ProviderRuntimeActorState::Failed(reason) => {
                return Err(format!(
                    "asp-client-server-not-ready: state=failed reason={reason}"
                ));
            }
            ProviderRuntimeActorState::Stopped => {
                return Err("asp-client-server-not-ready: state=stopped".to_owned());
            }
        };
        if !receipt
            .operations
            .iter()
            .any(|candidate| candidate.operation == operation)
        {
            return Err(format!(
                "asp-client-server-operation-not-admitted: operation={operation}"
            ));
        }

        let request_id = self
            .next_request_id
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let (response, result) = oneshot::channel();
        self.commands
            .send(ProviderRuntimeActorCommand::Request {
                request_id,
                operation,
                payload,
                response,
            })
            .await
            .map_err(|_| "asp-client-server-not-ready: admission-closed".to_owned())?;
        Ok(ProviderRuntimeRequest {
            request_id,
            response: result,
            commands: self.commands.clone(),
            terminal: false,
        })
    }
}

pub struct ProviderRuntimeActorAuthority {
    client: ProviderRuntimeActorClient,
    task: tokio::task::JoinHandle<()>,
    worker_abort: tokio::task::AbortHandle,
}

pub trait ProviderRuntimePeer: Send + Sync + 'static {
    fn handshake(
        &self,
    ) -> Pin<Box<dyn Future<Output = Result<ProviderRuntimeContractReceipt, String>> + Send + '_>>;

    fn request(
        &self,
        operation: String,
        payload: Bytes,
    ) -> Pin<Box<dyn Future<Output = Result<Bytes, String>> + Send + '_>>;

    fn shutdown(&self) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send + '_>>;

    fn wait_terminated(&self) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send + '_>>;
}

fn publish_actor_failure(state_writer: &watch::Sender<ProviderRuntimeActorState>, reason: String) {
    let published = state_writer.send_if_modified(|state| {
        if matches!(
            state,
            ProviderRuntimeActorState::Failed(_) | ProviderRuntimeActorState::Stopped
        ) {
            false
        } else {
            *state = ProviderRuntimeActorState::Failed(reason.clone());
            true
        }
    });
    if published {
        let reason_kind = reason
            .split_whitespace()
            .find_map(|field| field.strip_prefix("reasonKind="))
            .unwrap_or("provider-runtime-actor-failed");
        let phase = reason
            .split_whitespace()
            .find_map(|field| field.strip_prefix("phase="))
            .unwrap_or("actor-supervisor");
        tracing::error!(
            target: "asp.client_server",
            reason_kind,
            phase,
            terminal_state = "failed",
            error = %reason,
        );
    }
}

fn supervise_provider_runtime_actor(
    client: ProviderRuntimeActorClient,
    state_writer: watch::Sender<ProviderRuntimeActorState>,
    worker: tokio::task::JoinHandle<()>,
) -> ProviderRuntimeActorAuthority {
    let worker_abort = worker.abort_handle();
    let task = tokio::spawn(async move {
        match worker.await {
            Ok(()) => publish_actor_failure(
                &state_writer,
                "state=provider-runtime-terminal reasonKind=provider-runtime-actor-task-exited-without-terminal"
                    .to_owned(),
            ),
            Err(error) => {
                let reason_kind = if error.is_cancelled() {
                    "provider-runtime-actor-task-aborted"
                } else if error.is_panic() {
                    "provider-runtime-actor-task-panicked"
                } else {
                    "provider-runtime-actor-task-join-failed"
                };
                publish_actor_failure(
                    &state_writer,
                    format!(
                        "state=provider-runtime-terminal reasonKind={reason_kind} joinError={error}"
                    ),
                );
            }
        }
    });
    ProviderRuntimeActorAuthority {
        client,
        task,
        worker_abort,
    }
}

impl ProviderRuntimeActorAuthority {
    pub fn client(&self) -> ProviderRuntimeActorClient {
        self.client.clone()
    }

    pub async fn shutdown(self) -> Result<(), String> {
        let Self {
            client,
            task,
            worker_abort: _,
        } = self;
        let (response, stopped) = oneshot::channel();
        let _ = client
            .commands
            .send(ProviderRuntimeActorCommand::Shutdown { response })
            .await;
        let _ = stopped.await;
        task.await
            .map_err(|error| format!("provider runtime actor supervisor task failed: {error}"))
    }

    pub fn abort_actor(&self) {
        self.worker_abort.abort();
    }

    pub async fn join(self) -> Result<(), String> {
        self.task
            .await
            .map_err(|error| format!("provider runtime actor supervisor task failed: {error}"))
    }

    pub async fn drain(self) -> Result<(), String> {
        let Self {
            client,
            task,
            worker_abort: _,
        } = self;
        let (response, drained) = oneshot::channel();
        client
            .commands
            .send(ProviderRuntimeActorCommand::Drain { response })
            .await
            .map_err(|_| "asp-client-server-drain: admission-closed".to_owned())?;
        drained
            .await
            .map_err(|_| "asp-client-server-drain: receipt-dropped".to_owned())?;
        task.await
            .map_err(|error| format!("provider runtime actor supervisor task failed: {error}"))
    }
}

pub fn spawn_in_process_provider_runtime_actor<Handshake, HandshakeFuture, Handler, HandlerFuture>(
    capacity: usize,
    handshake: Handshake,
    mut handler: Handler,
) -> ProviderRuntimeActorAuthority
where
    Handshake: FnOnce() -> HandshakeFuture + Send + 'static,
    HandshakeFuture:
        Future<Output = Result<ProviderRuntimeContractReceipt, String>> + Send + 'static,
    Handler: FnMut(String, Bytes) -> HandlerFuture + Send + 'static,
    HandlerFuture: Future<Output = Result<Bytes, String>> + Send + 'static,
{
    let (commands, mut receiver) = mpsc::channel(capacity.max(1));
    let (state_writer, state) = watch::channel(ProviderRuntimeActorState::Starting);
    let client = ProviderRuntimeActorClient {
        commands,
        state,
        next_request_id: std::sync::Arc::new(std::sync::atomic::AtomicU64::new(1)),
    };
    let worker_state_writer = state_writer.clone();
    let worker = tokio::spawn(async move {
        let state_writer = worker_state_writer;
        let startup_started = std::time::Instant::now();
        tracing::info!(target: "asp.client_server", state = "starting");
        state_writer.send_replace(ProviderRuntimeActorState::Warming);
        tracing::info!(target: "asp.client_server", state = "warming");
        let receipt = tokio::select! {
            result = handshake() => match result.and_then(|receipt| {
                receipt.validate()?;
                Ok(receipt)
            }) {
                Ok(receipt) => receipt,
            Err(reason) => {
                tracing::error!(
                    target: "asp.client_server",
                    state = "failed",
                    error = %reason,
                    startup_elapsed_micros = startup_started.elapsed().as_micros() as u64,
                );
                state_writer.send_replace(ProviderRuntimeActorState::Failed(reason));
                    return;
                }
            },
            command = receiver.recv() => {
                if let Some(
                    ProviderRuntimeActorCommand::Shutdown { response }
                    | ProviderRuntimeActorCommand::Drain { response },
                ) = command
                {
                    state_writer.send_replace(ProviderRuntimeActorState::Stopped);
                    let _ = response.send(());
                }
                return;
            }
        };
        state_writer.send_replace(ProviderRuntimeActorState::Ready(receipt));

        let mut requests = tokio::task::JoinSet::new();
        let mut admitted = std::collections::HashMap::new();
        let mut drain_response = None;
        loop {
            tokio::select! {
                command = receiver.recv() => match command {
                    Some(ProviderRuntimeActorCommand::Request {
                        request_id,
                        operation,
                        payload,
                        response,
                    }) => {
                        if drain_response.is_some() {
                            let _ = response.send(Err(
                                "asp-client-server-not-ready: state=draining".to_owned(),
                            ));
                            continue;
                        }
                        tracing::info!(
                            target: "asp.client_server",
                            event = "request-admitted",
                            request_id,
                            operation = %operation,
                        );
                        let request = handler(operation, payload);
                        let task = requests.spawn(async move { (request_id, request.await) });
                        admitted.insert(request_id, (response, task));
                        tracing::info!(
                            target: "asp.client_server",
                            event = "lease-acquired",
                            request_id,
                            active_requests = admitted.len() as u64,
                        );
                    }
                    Some(ProviderRuntimeActorCommand::Cancel { request_id }) => {
                        tracing::info!(
                            target: "asp.client_server",
                            event = "cancellation-requested",
                            request_id,
                        );
                        if let Some((response, task)) = admitted.remove(&request_id) {
                            task.abort();
                            let _ = response.send(Err(
                                "asp-client-server-request-terminal: state=cancelled".to_owned(),
                            ));
                            tracing::info!(
                                target: "asp.client_server",
                                event = "request-terminal",
                                request_id,
                                state = "cancelled",
                            );
                            tracing::info!(
                                target: "asp.client_server",
                                event = "lease-released",
                                request_id,
                                active_requests = admitted.len() as u64,
                            );
                        }
                    }
                    Some(ProviderRuntimeActorCommand::Drain { response }) => {
                        state_writer.send_replace(ProviderRuntimeActorState::Draining);
                        drain_response = Some(response);
                        if admitted.is_empty() {
                            let _ = drain_response.take().expect("drain response").send(());
                            break;
                        }
                    }
                    Some(ProviderRuntimeActorCommand::Shutdown { response }) => {
                        state_writer.send_replace(ProviderRuntimeActorState::Draining);
                        for (request_id, (request_response, task)) in admitted.drain() {
                            task.abort();
                            let _ = request_response.send(Err(
                                "asp-client-server-request-terminal: state=draining".to_owned(),
                            ));
                            tracing::info!(
                                target: "asp.client_server",
                                event = "request-terminal",
                                request_id,
                                state = "draining",
                            );
                        }
                        while requests.join_next().await.is_some() {}
                        state_writer.send_replace(ProviderRuntimeActorState::Stopped);
                        let _ = response.send(());
                        return;
                    }
                    None => break,
                },
                completed = requests.join_next(), if !requests.is_empty() => {
                    if let Some(Ok((request_id, result))) = completed {
                        if let Some((response, _task)) = admitted.remove(&request_id) {
                            let state = if result.is_ok() { "completed" } else { "failed" };
                            let _ = response.send(result);
                            tracing::info!(
                                target: "asp.client_server",
                                event = "request-terminal",
                                request_id,
                                state,
                            );
                            tracing::info!(
                                target: "asp.client_server",
                                event = "lease-released",
                                request_id,
                                active_requests = admitted.len() as u64,
                            );
                            if admitted.is_empty() {
                                if let Some(response) = drain_response.take() {
                                    let _ = response.send(());
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }
        for (request_id, (response, task)) in admitted.drain() {
            task.abort();
            let _ = response.send(Err(
                "asp-client-server-request-terminal: state=admission-closed".to_owned(),
            ));
            tracing::info!(
                target: "asp.client_server",
                event = "request-terminal",
                request_id,
                state = "admission-closed",
            );
        }
        while requests.join_next().await.is_some() {}
        tracing::info!(target: "asp.client_server", event = "idle-closed");
        state_writer.send_replace(ProviderRuntimeActorState::Stopped);
    });
    supervise_provider_runtime_actor(client, state_writer, worker)
}

pub fn spawn_provider_runtime_peer_actor<Peer>(
    capacity: usize,
    peer: Peer,
) -> ProviderRuntimeActorAuthority
where
    Peer: ProviderRuntimePeer,
{
    let (commands, mut receiver) = mpsc::channel(capacity.max(1));
    let (state_writer, state) = watch::channel(ProviderRuntimeActorState::Starting);
    let client = ProviderRuntimeActorClient {
        commands,
        state,
        next_request_id: std::sync::Arc::new(std::sync::atomic::AtomicU64::new(1)),
    };
    let peer = std::sync::Arc::new(peer);
    let worker_state_writer = state_writer.clone();
    let worker = tokio::spawn(async move {
        let state_writer = worker_state_writer;
        let startup_started = std::time::Instant::now();
        tracing::info!(target: "asp.client_server", state = "starting");
        state_writer.send_replace(ProviderRuntimeActorState::Warming);
        tracing::info!(target: "asp.client_server", state = "warming");
        let receipt = tokio::select! {
                result = peer.handshake() => match result.and_then(|receipt| {
                    receipt.validate()?;
                    Ok(receipt)
                }) {
                    Ok(receipt) => receipt,
                    Err(reason) => {
                        state_writer.send_replace(ProviderRuntimeActorState::Failed(reason));
                        let _ = peer.shutdown().await;
                        return;
                    }
                },
                command = receiver.recv() => {
                    if let Some(
                        ProviderRuntimeActorCommand::Shutdown { response }
                        | ProviderRuntimeActorCommand::Drain { response },
                    ) = command
                    {
                        let reason =
                            "state=provider-runtime-terminal reasonKind=provider-runtime-startup-cancelled"
                                .to_owned();
                        publish_actor_failure(&state_writer, reason);
                        tracing::info!(target: "asp.client_server", state = "cancelled");
                        let _ = peer.shutdown().await;
                        let _ = response.send(());
                    }
                    return;
                },
                terminal = peer.wait_terminated() => {
                    let reason = match terminal {
                        Ok(()) => "state=provider-runtime-terminal reasonKind=provider-runtime-peer-eof".to_owned(),
                        Err(error) => format!(
                            "state=provider-runtime-terminal reasonKind=provider-runtime-peer-lifecycle-failed error={error}"
                        ),
                    };
                    publish_actor_failure(&state_writer, reason);
                    let _ = peer.shutdown().await;
                    return;
                },
        };
        tracing::info!(
            target: "asp.client_server",
            state = "ready",
            attempt = 1_u64,
            publication_epoch = 1_u64,
                provider_id = %receipt.provider_id,
                language_id = %receipt.language_id,
                contract_digest = %receipt.contract_digest,
            startup_elapsed_micros = startup_started.elapsed().as_micros() as u64,
        );
        let telemetry_provider_id = receipt.provider_id.clone();
        let telemetry_language_id = receipt.language_id.clone();
        let telemetry_transport = format!("{:?}", receipt.transport);
        state_writer.send_replace(ProviderRuntimeActorState::Ready(receipt));

        let mut requests = tokio::task::JoinSet::new();
        let mut admitted = std::collections::HashMap::new();
        let mut drain_response = None;
        loop {
            tokio::select! {
                command = receiver.recv() => match command {
                    Some(ProviderRuntimeActorCommand::Request {
                        request_id,
                        operation,
                        payload,
                        response,
                    }) => {
                        if drain_response.is_some() {
                            let _ = response.send(Err(
                                "asp-client-server-not-ready: state=draining".to_owned(),
                            ));
                            continue;
                        }
                        let request_peer = std::sync::Arc::clone(&peer);
                        let request_operation = operation.clone();
                        let started = std::time::Instant::now();
                        let task = requests.spawn(async move {
                            let result = request_peer.request(request_operation, payload).await;
                            (request_id, operation, started, result)
                        });
                        admitted.insert(request_id, (response, task));
                        tracing::info!(
                            target: "asp.client_server",
                            event = "lease-acquired",
                            request_id,
                            active_requests = admitted.len() as u64,
                        );
                        tracing::info!(
                            target: "asp.client_server",
                            event = "request-admitted",
                            request_id,
                            active_requests = admitted.len() as u64,
                        );
                    }
                    Some(ProviderRuntimeActorCommand::Cancel { request_id }) => {
                        tracing::info!(
                            target: "asp.client_server",
                            event = "cancellation-requested",
                            request_id,
                        );
                        if let Some((response, task)) = admitted.remove(&request_id) {
                            task.abort();
                            let _ = response.send(Err(
                                "asp-client-server-request-terminal: state=cancelled".to_owned(),
                            ));
                            tracing::info!(
                                target: "asp.client_server",
                                event = "request-terminal",
                                request_id,
                                outcome = "cancelled",
                                active_requests = admitted.len() as u64,
                            );
                            tracing::info!(
                                target: "asp.client_server",
                                event = "lease-released",
                                request_id,
                                active_requests = admitted.len() as u64,
                            );
                        }
                    }
                    Some(ProviderRuntimeActorCommand::Drain { response }) => {
                        state_writer.send_replace(ProviderRuntimeActorState::Draining);
                        tracing::info!(target: "asp.client_server", state = "draining");
                        drain_response = Some(response);
                        if admitted.is_empty() {
                            let _ = drain_response.take().expect("drain response").send(());
                            break;
                        }
                    }
                    Some(ProviderRuntimeActorCommand::Shutdown { response }) => {
                        state_writer.send_replace(ProviderRuntimeActorState::Draining);
                        tracing::info!(target: "asp.client_server", state = "draining");
                        for (request_id, (request_response, task)) in admitted.drain() {
                            task.abort();
                            let _ = request_response.send(Err(
                                "asp-client-server-request-terminal: state=draining".to_owned(),
                            ));
                            tracing::info!(
                                target: "asp.client_server",
                                event = "request-terminal",
                                request_id,
                                outcome = "draining",
                            );
                        }
                        while requests.join_next().await.is_some() {}
                        let _ = peer.shutdown().await;
                        state_writer.send_replace(ProviderRuntimeActorState::Stopped);
                        tracing::info!(target: "asp.client_server", state = "stopped", residual_tasks = 0_u64);
                        let _ = response.send(());
                        return;
                    }
                    None => break,
                },
                completed = requests.join_next(), if !requests.is_empty() => {
                    if let Some(Ok((request_id, operation, started, result))) = completed {
                        if let Some((response, _task)) = admitted.remove(&request_id) {
                            tracing::info!(
                                target: "asp.client_server",
                                event = "request-terminal",
                                request_id,
                                provider_id = %telemetry_provider_id,
                                language_id = %telemetry_language_id,
                                transport = %telemetry_transport,
                                attempt = 1_u64,
                                publication_epoch = 1_u64,
                                active_requests = admitted.len() as u64,
                                operation = %operation,
                                outcome = if result.is_ok() { "ready" } else { "error" },
                                elapsed_micros = started.elapsed().as_micros() as u64,
                            );
                            let _ = response.send(result);
                            tracing::info!(
                                target: "asp.client_server",
                                event = "lease-released",
                                request_id,
                                active_requests = admitted.len() as u64,
                            );
                            if admitted.is_empty() {
                                if let Some(response) = drain_response.take() {
                                    let _ = response.send(());
                                    break;
                                }
                            }
                        }
                    }
                },
                terminal = peer.wait_terminated() => {
                    let reason = match terminal {
                        Ok(()) => "state=provider-runtime-terminal reasonKind=provider-runtime-peer-eof phase=ready".to_owned(),
                        Err(error) => format!(
                            "state=provider-runtime-terminal reasonKind=provider-runtime-peer-lifecycle-failed phase=ready error={error}"
                        ),
                    };
                    publish_actor_failure(&state_writer, reason.clone());
                    for (request_id, (response, task)) in admitted.drain() {
                        task.abort();
                        let _ = response.send(Err(reason.clone()));
                        tracing::info!(
                            target: "asp.client_server",
                            event = "request-terminal",
                            request_id,
                            outcome = "peer-terminal",
                        );
                    }
                    while requests.join_next().await.is_some() {}
                    return;
                }
            }
        }
        for (_request_id, (response, task)) in admitted.drain() {
            task.abort();
            let _ = response.send(Err(
                "asp-client-server-request-terminal: state=admission-closed".to_owned(),
            ));
        }
        while requests.join_next().await.is_some() {}
        let _ = peer.shutdown().await;
        state_writer.send_replace(ProviderRuntimeActorState::Stopped);
        tracing::info!(target: "asp.client_server", event = "idle-closed");
        tracing::info!(target: "asp.client_server", state = "stopped", residual_tasks = 0_u64);
    });
    supervise_provider_runtime_actor(client, state_writer, worker)
}

#[cfg(test)]
#[path = "../tests/unit/resident_runtime.rs"]
mod tests;

#[cfg(test)]
#[path = "../tests/unit/request_lifecycle.rs"]
mod request_lifecycle_tests;
