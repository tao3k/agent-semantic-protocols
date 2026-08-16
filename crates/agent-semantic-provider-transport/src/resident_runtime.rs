use std::future::Future;
use std::pin::Pin;

use bytes::Bytes;
use tokio::sync::{mpsc, oneshot, watch};

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
        operation: String,
        payload: Bytes,
        response: oneshot::Sender<Result<Bytes, String>>,
    },
    Shutdown {
        response: oneshot::Sender<()>,
    },
}

#[derive(Clone)]
pub struct ProviderRuntimeActorClient {
    commands: mpsc::Sender<ProviderRuntimeActorCommand>,
    state: watch::Receiver<ProviderRuntimeActorState>,
}

impl ProviderRuntimeActorClient {
    pub fn states(&self) -> tokio_stream::wrappers::WatchStream<ProviderRuntimeActorState> {
        tokio_stream::wrappers::WatchStream::new(self.state.clone())
    }

    pub fn current(&self) -> ProviderRuntimeActorState {
        self.state.borrow().clone()
    }

    pub async fn wait_ready(&mut self) -> Result<ProviderRuntimeContractReceipt, String> {
        loop {
            match self.current() {
                ProviderRuntimeActorState::Ready(receipt) => return Ok(receipt),
                ProviderRuntimeActorState::Failed(reason) => return Err(reason),
                ProviderRuntimeActorState::Stopped => {
                    return Err("provider runtime actor stopped before Ready".to_owned());
                }
                ProviderRuntimeActorState::Starting | ProviderRuntimeActorState::Warming => {}
                ProviderRuntimeActorState::Draining => {
                    return Err("provider runtime actor drained before Ready".to_owned());
                }
            }
            self.state.changed().await.map_err(|_| {
                "provider runtime actor state channel closed before Ready".to_owned()
            })?;
        }
    }

    pub async fn request(
        &self,
        operation: impl Into<String>,
        payload: impl Into<Bytes>,
    ) -> Result<Bytes, String> {
        let operation = operation.into();
        let payload = payload.into();
        let receipt = match self.current() {
            ProviderRuntimeActorState::Ready(receipt) => receipt,
            ProviderRuntimeActorState::Starting => {
                return Err("provider-runtime-not-ready: state=starting".to_owned());
            }
            ProviderRuntimeActorState::Warming => {
                return Err("provider-runtime-not-ready: state=warming".to_owned());
            }
            ProviderRuntimeActorState::Draining => {
                return Err("provider-runtime-not-ready: state=draining".to_owned());
            }
            ProviderRuntimeActorState::Failed(reason) => {
                return Err(format!(
                    "provider-runtime-not-ready: state=failed reason={reason}"
                ));
            }
            ProviderRuntimeActorState::Stopped => {
                return Err("provider-runtime-not-ready: state=stopped".to_owned());
            }
        };
        if !receipt
            .operations
            .iter()
            .any(|candidate| candidate.operation == operation)
        {
            return Err(format!(
                "provider-runtime-operation-not-admitted: operation={operation}"
            ));
        }

        let (response, result) = oneshot::channel();
        self.commands
            .send(ProviderRuntimeActorCommand::Request {
                operation,
                payload,
                response,
            })
            .await
            .map_err(|_| "provider-runtime-not-ready: admission-closed".to_owned())?;
        result
            .await
            .map_err(|_| "provider runtime actor dropped request response".to_owned())?
    }
}

pub struct ProviderRuntimeActorAuthority {
    client: ProviderRuntimeActorClient,
    task: tokio::task::JoinHandle<()>,
}

pub trait ProviderRuntimePeer: Send + 'static {
    fn handshake(
        &mut self,
    ) -> Pin<Box<dyn Future<Output = Result<ProviderRuntimeContractReceipt, String>> + Send + '_>>;

    fn request(
        &mut self,
        operation: String,
        payload: Bytes,
    ) -> Pin<Box<dyn Future<Output = Result<Bytes, String>> + Send + '_>>;

    fn shutdown(&mut self) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send + '_>>;
}

impl ProviderRuntimeActorAuthority {
    pub fn client(&self) -> ProviderRuntimeActorClient {
        self.client.clone()
    }

    pub async fn shutdown(self) -> Result<(), String> {
        let (response, stopped) = oneshot::channel();
        let _ = self
            .client
            .commands
            .send(ProviderRuntimeActorCommand::Shutdown { response })
            .await;
        let _ = stopped.await;
        self.task
            .await
            .map_err(|error| format!("provider runtime actor task failed: {error}"))
    }
}

pub fn spawn_provider_runtime_actor<Handshake, HandshakeFuture, Handler, HandlerFuture>(
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
    let client = ProviderRuntimeActorClient { commands, state };
    let task = tokio::spawn(async move {
        let startup_started = std::time::Instant::now();
        tracing::info!(target: "asp.provider_server", state = "starting");
        state_writer.send_replace(ProviderRuntimeActorState::Warming);
        tracing::info!(target: "asp.provider_server", state = "warming");
        let receipt = tokio::select! {
            result = handshake() => match result.and_then(|receipt| {
                receipt.validate()?;
                Ok(receipt)
            }) {
                Ok(receipt) => receipt,
            Err(reason) => {
                tracing::error!(
                    target: "asp.provider_server",
                    state = "failed",
                    error = %reason,
                    startup_elapsed_micros = startup_started.elapsed().as_micros() as u64,
                );
                state_writer.send_replace(ProviderRuntimeActorState::Failed(reason));
                    return;
                }
            },
            command = receiver.recv() => {
                if let Some(ProviderRuntimeActorCommand::Shutdown { response }) = command {
                    state_writer.send_replace(ProviderRuntimeActorState::Stopped);
                    let _ = response.send(());
                }
                return;
            }
        };
        state_writer.send_replace(ProviderRuntimeActorState::Ready(receipt));

        while let Some(command) = receiver.recv().await {
            match command {
                ProviderRuntimeActorCommand::Request {
                    operation,
                    payload,
                    response,
                } => {
                    let _ = response.send(handler(operation, payload).await);
                }
                ProviderRuntimeActorCommand::Shutdown { response } => {
                    state_writer.send_replace(ProviderRuntimeActorState::Stopped);
                    let _ = response.send(());
                    return;
                }
            }
        }
        state_writer.send_replace(ProviderRuntimeActorState::Stopped);
    });
    ProviderRuntimeActorAuthority { client, task }
}

pub fn spawn_provider_runtime_peer_actor<Peer>(
    capacity: usize,
    mut peer: Peer,
) -> ProviderRuntimeActorAuthority
where
    Peer: ProviderRuntimePeer,
{
    let (commands, mut receiver) = mpsc::channel(capacity.max(1));
    let (state_writer, state) = watch::channel(ProviderRuntimeActorState::Starting);
    let client = ProviderRuntimeActorClient { commands, state };
    let task = tokio::spawn(async move {
        let startup_started = std::time::Instant::now();
        tracing::info!(target: "asp.provider_server", state = "starting");
        state_writer.send_replace(ProviderRuntimeActorState::Warming);
        tracing::info!(target: "asp.provider_server", state = "warming");
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
                if let Some(ProviderRuntimeActorCommand::Shutdown { response }) = command {
                    state_writer.send_replace(ProviderRuntimeActorState::Draining);
                    tracing::info!(target: "asp.provider_server", state = "draining");
                    let _ = peer.shutdown().await;
                    state_writer.send_replace(ProviderRuntimeActorState::Stopped);
                tracing::info!(target: "asp.provider_server", state = "stopped", residual_tasks = 0_u64);
                    let _ = response.send(());
                    }
                    return;
                }
        };
        tracing::info!(
                target: "asp.provider_server",
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

        while let Some(command) = receiver.recv().await {
            match command {
                ProviderRuntimeActorCommand::Request {
                    operation,
                    payload,
                    response,
                } => {
                    let started = std::time::Instant::now();
                    let result = peer.request(operation.clone(), payload).await;
                    tracing::info!(
                        target: "asp.provider_server",
                    event = "request",
                    provider_id = %telemetry_provider_id,
                    language_id = %telemetry_language_id,
                    transport = %telemetry_transport,
                    attempt = 1_u64,
                    publication_epoch = 1_u64,
                    active_requests = 1_u64,
                        operation = %operation,
                        outcome = if result.is_ok() { "ready" } else { "error" },
                        elapsed_micros = started.elapsed().as_micros() as u64,
                    );
                    let _ = response.send(result);
                }
                ProviderRuntimeActorCommand::Shutdown { response } => {
                    state_writer.send_replace(ProviderRuntimeActorState::Draining);
                    tracing::info!(target: "asp.provider_server", state = "draining");
                    let _ = peer.shutdown().await;
                    state_writer.send_replace(ProviderRuntimeActorState::Stopped);
                    tracing::info!(target: "asp.provider_server", state = "stopped", residual_tasks = 0_u64);
                    let _ = response.send(());
                    return;
                }
            }
        }
        let _ = peer.shutdown().await;
        state_writer.send_replace(ProviderRuntimeActorState::Stopped);
        tracing::info!(target: "asp.provider_server", state = "stopped", residual_tasks = 0_u64);
    });
    ProviderRuntimeActorAuthority { client, task }
}

#[cfg(test)]
#[path = "../tests/unit/resident_runtime.rs"]
mod tests;
