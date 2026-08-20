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

    pub async fn drain(self) -> Result<(), String> {
        let (response, drained) = oneshot::channel();
        self.client
            .commands
            .send(ProviderRuntimeActorCommand::Drain { response })
            .await
            .map_err(|_| "asp-client-server-drain: admission-closed".to_owned())?;
        drained
            .await
            .map_err(|_| "asp-client-server-drain: receipt-dropped".to_owned())?;
        self.task
            .await
            .map_err(|error| format!("provider runtime actor task failed: {error}"))
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
    let task = tokio::spawn(async move {
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
    ProviderRuntimeActorAuthority { client, task }
}

#[cfg(test)]
mod request_lifecycle_tests {
    use super::*;

    fn ready_receipt() -> ProviderRuntimeContractReceipt {
        ProviderRuntimeContractReceipt::new(
            "asp-rust",
            "rust",
            format!("blake3-256:{}", "a".repeat(64)),
            format!("blake3-256:{}", "b".repeat(64)),
            crate::runtime_contract::ProviderRuntimeContractTransport::RuntimeIpc,
            vec![crate::runtime_contract::ProviderRuntimeContractOperation {
                operation: "owner-items".to_owned(),
                request_schema_id: "agent.semantic-protocols.asp-client-server-request".to_owned(),
                response_schema_id: "agent.semantic-protocols.asp-client-server-response"
                    .to_owned(),
            }],
        )
        .expect("runtime contract")
    }

    #[tokio::test]
    async fn actor_executes_admitted_requests_concurrently() {
        let barrier = std::sync::Arc::new(tokio::sync::Barrier::new(2));
        let authority =
            spawn_in_process_provider_runtime_actor(8, || async { Ok(ready_receipt()) }, {
                let barrier = std::sync::Arc::clone(&barrier);
                move |_operation, payload| {
                    let barrier = std::sync::Arc::clone(&barrier);
                    async move {
                        barrier.wait().await;
                        Ok(payload)
                    }
                }
            });
        let mut client = authority.client();
        client.wait_ready().await.expect("ready actor");
        let first = client
            .begin_request("owner-items", Bytes::from_static(b"first"))
            .await
            .expect("first request");
        let second = client
            .begin_request("owner-items", Bytes::from_static(b"second"))
            .await
            .expect("second request");
        let (first, second) = tokio::time::timeout(std::time::Duration::from_millis(100), async {
            tokio::join!(first, second)
        })
        .await
        .expect("requests must not serialize");
        assert_eq!(first.expect("first response"), Bytes::from_static(b"first"));
        assert_eq!(
            second.expect("second response"),
            Bytes::from_static(b"second")
        );
        authority.shutdown().await.expect("shutdown actor");
    }

    #[tokio::test]
    async fn peer_actor_concurrency_and_idle_drain_share_the_tokio_authority() {
        struct ConcurrentPeer {
            barrier: std::sync::Arc<tokio::sync::Barrier>,
            stopped: std::sync::Arc<std::sync::atomic::AtomicBool>,
        }

        impl ProviderRuntimePeer for ConcurrentPeer {
            fn handshake(
                &self,
            ) -> Pin<
                Box<
                    dyn Future<Output = Result<ProviderRuntimeContractReceipt, String>> + Send + '_,
                >,
            > {
                Box::pin(async { Ok(ready_receipt()) })
            }

            fn request(
                &self,
                _operation: String,
                payload: Bytes,
            ) -> Pin<Box<dyn Future<Output = Result<Bytes, String>> + Send + '_>> {
                let barrier = std::sync::Arc::clone(&self.barrier);
                Box::pin(async move {
                    barrier.wait().await;
                    Ok(payload)
                })
            }

            fn shutdown(&self) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send + '_>> {
                Box::pin(async {
                    self.stopped
                        .store(true, std::sync::atomic::Ordering::SeqCst);
                    Ok(())
                })
            }
        }

        let barrier = std::sync::Arc::new(tokio::sync::Barrier::new(2));
        let stopped = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let authority = spawn_provider_runtime_peer_actor(
            8,
            ConcurrentPeer {
                barrier,
                stopped: std::sync::Arc::clone(&stopped),
            },
        );
        let mut client = authority.client();
        client.wait_ready().await.expect("ready peer actor");
        let first = client
            .begin_request("owner-items", Bytes::from_static(b"first"))
            .await
            .expect("first peer request");
        let second = client
            .begin_request("owner-items", Bytes::from_static(b"second"))
            .await
            .expect("second peer request");
        let (first, second) = tokio::time::timeout(std::time::Duration::from_millis(100), async {
            tokio::join!(first, second)
        })
        .await
        .expect("peer requests must not serialize");
        assert_eq!(first.expect("first response"), Bytes::from_static(b"first"));
        assert_eq!(
            second.expect("second response"),
            Bytes::from_static(b"second")
        );
        authority.drain().await.expect("idle drain peer actor");
        assert!(stopped.load(std::sync::atomic::Ordering::SeqCst));
        assert_eq!(client.current(), ProviderRuntimeActorState::Stopped);
    }

    #[tokio::test]
    async fn dropping_request_cancels_handler_and_releases_actor_lease() {
        struct ActiveGuard(std::sync::Arc<std::sync::atomic::AtomicUsize>);
        impl Drop for ActiveGuard {
            fn drop(&mut self) {
                self.0.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
            }
        }

        let active = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let authority =
            spawn_in_process_provider_runtime_actor(8, || async { Ok(ready_receipt()) }, {
                let active = std::sync::Arc::clone(&active);
                move |_operation, _payload| {
                    let active = std::sync::Arc::clone(&active);
                    async move {
                        active.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                        let _guard = ActiveGuard(active);
                        std::future::pending::<Result<Bytes, String>>().await
                    }
                }
            });
        let mut client = authority.client();
        client.wait_ready().await.expect("ready actor");
        let request = client
            .begin_request("owner-items", Bytes::new())
            .await
            .expect("admitted request");
        tokio::time::timeout(std::time::Duration::from_millis(100), async {
            while active.load(std::sync::atomic::Ordering::SeqCst) != 1 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("handler started");
        drop(request);
        tokio::time::timeout(std::time::Duration::from_millis(100), async {
            while active.load(std::sync::atomic::Ordering::SeqCst) != 0 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("cancelled handler released its lease");
        authority.shutdown().await.expect("shutdown actor");
    }

    #[tokio::test]
    async fn drain_rejects_new_work_and_waits_for_existing_request_lease() {
        let release = std::sync::Arc::new(tokio::sync::Notify::new());
        let authority =
            spawn_in_process_provider_runtime_actor(8, || async { Ok(ready_receipt()) }, {
                let release = std::sync::Arc::clone(&release);
                move |_operation, payload| {
                    let release = std::sync::Arc::clone(&release);
                    async move {
                        release.notified().await;
                        Ok(payload)
                    }
                }
            });
        let mut client = authority.client();
        client.wait_ready().await.expect("ready actor");
        let admitted = client
            .begin_request("owner-items", Bytes::from_static(b"admitted"))
            .await
            .expect("admitted request");
        let drain = tokio::spawn(authority.drain());
        tokio::time::timeout(std::time::Duration::from_millis(100), async {
            while client.current() != ProviderRuntimeActorState::Draining {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("actor entered draining");
        let rejected = match client.begin_request("owner-items", Bytes::new()).await {
            Ok(_) => panic!("draining actor accepted new work"),
            Err(error) => error,
        };
        assert_eq!(rejected, "asp-client-server-not-ready: state=draining");
        release.notify_waiters();
        assert_eq!(
            admitted.await.expect("admitted response"),
            Bytes::from_static(b"admitted")
        );
        drain.await.expect("drain task").expect("drain receipt");
        assert_eq!(client.current(), ProviderRuntimeActorState::Stopped);
    }
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
    let task = tokio::spawn(async move {
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
                    state_writer.send_replace(ProviderRuntimeActorState::Draining);
                tracing::info!(target: "asp.client_server", state = "draining");
                    let _ = peer.shutdown().await;
                    state_writer.send_replace(ProviderRuntimeActorState::Stopped);
                tracing::info!(target: "asp.client_server", state = "stopped", residual_tasks = 0_u64);
                    let _ = response.send(());
                    }
                    return;
                }
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
    ProviderRuntimeActorAuthority { client, task }
}

#[cfg(test)]
#[path = "../tests/unit/resident_runtime.rs"]
mod tests;
