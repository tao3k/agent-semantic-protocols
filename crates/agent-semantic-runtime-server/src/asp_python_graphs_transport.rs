//! Bounded, multiplexed gRPC transport to the ASP Server-owned Python graphs service.

use std::collections::HashMap;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

use serde_json::Value;
use tokio::sync::Mutex;
use tokio::sync::mpsc;
use tokio::sync::oneshot;

#[path = "asp_python_graphs_generated.rs"]
pub mod generated;

use generated::GraphsEnvelope;
use generated::asp_python_graphs_client::AspPythonGraphsClient;

fn encode_graphs_request(request: &Value) -> Result<GraphsEnvelope, String> {
    serde_json::to_vec(request)
        .map(|json| GraphsEnvelope { json })
        .map_err(|error| format!("encode asp-python-graphs request: {error}"))
}

fn wire_terminal_message(
    reason: agent_semantic_provider_transport::PriorityJsonStreamTerminal,
) -> String {
    match reason {
        agent_semantic_provider_transport::PriorityJsonStreamTerminal::SequenceOverflow => {
            "state=failed reasonKind=asp-python-graphs-wire-sequence-overflow".to_owned()
        }
        agent_semantic_provider_transport::PriorityJsonStreamTerminal::EncodeFailed(error) => {
            format!("state=failed reasonKind=asp-python-graphs-wire-encode-failed error={error}")
        }
    }
}

const DEFAULT_QUEUE_CAPACITY: usize = 32;
pub const ASP_PYTHON_GRAPHS_NAMESPACE: &str = "asp.python.graphs";
pub const ASP_PYTHON_GRAPHS_SESSION_SCHEMA_ID: &str =
    "agent.semantic-protocols.asp-python-graphs-session";
pub const ASP_PYTHON_GRAPHS_SESSION_SCHEMA_VERSION: &str = "1";

type PendingReceipt = oneshot::Sender<Result<Value, String>>;

async fn terminalize_pending(
    terminal: &Mutex<Option<String>>,
    pending: &Mutex<HashMap<String, PendingReceipt>>,
    message: String,
) {
    let mut terminal = terminal.lock().await;
    if terminal.is_some() {
        return;
    }
    *terminal = Some(message.clone());
    drop(terminal);
    let pending = std::mem::take(&mut *pending.lock().await);
    for (_, sender) in pending {
        let _ = sender.send(Err(message.clone()));
    }
}

struct ResponseReader(tokio::task::JoinHandle<()>);

impl Drop for ResponseReader {
    fn drop(&mut self) {
        self.0.abort();
    }
}

/// One long-lived service session. Clones share the same bounded sender,
/// response reader, and request-id correlation registry.
#[derive(Clone)]
pub struct AspPythonGraphsTransport {
    outbound: mpsc::Sender<Value>,
    control: mpsc::Sender<Value>,
    pending: Arc<Mutex<HashMap<String, PendingReceipt>>>,
    cancelled: Arc<Mutex<HashSet<String>>>,
    late_cancellations: Arc<AtomicU64>,
    terminal: Arc<Mutex<Option<String>>>,
    _response_reader: Arc<ResponseReader>,
}

impl AspPythonGraphsTransport {
    pub async fn connect_unix(socket_path: impl Into<PathBuf>) -> Result<Self, String> {
        Self::connect_unix_with_capacity(socket_path.into(), DEFAULT_QUEUE_CAPACITY).await
    }

    pub async fn connect_unix_with_capacity(
        socket_path: PathBuf,
        capacity: usize,
    ) -> Result<Self, String> {
        if capacity == 0 {
            return Err("asp-python-graphs queue capacity must be positive".to_owned());
        }
        let channel = connect_unix_channel(socket_path).await?;
        let mut client = AspPythonGraphsClient::new(channel);
        let (outbound, receiver) = mpsc::channel(capacity);
        let (control, control_receiver) = mpsc::channel(8);
        let (wire_terminal, mut wire_terminal_rx) = tokio::sync::watch::channel(None);
        let mut inbound = client
            .session(tonic::Request::new(
                agent_semantic_provider_transport::PriorityJsonStream::new(
                    control_receiver,
                    receiver,
                    0,
                    encode_graphs_request,
                    wire_terminal,
                ),
            ))
            .await
            .map_err(|error| error.to_string())?
            .into_inner();
        let pending: Arc<Mutex<HashMap<String, PendingReceipt>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let terminal = Arc::new(Mutex::new(None));
        let cancelled = Arc::new(Mutex::new(HashSet::new()));
        let late_cancellations = Arc::new(AtomicU64::new(0));
        let response_pending = Arc::clone(&pending);
        let response_cancelled = Arc::clone(&cancelled);
        let response_late_cancellations = Arc::clone(&late_cancellations);
        let response_terminal = Arc::clone(&terminal);
        let response_reader = tokio::spawn(async move {
            let mut wire_terminal_open = true;
            loop {
                let receipt = tokio::select! {
                    changed = wire_terminal_rx.changed(), if wire_terminal_open => {
                        match changed {
                            Ok(()) => match wire_terminal_rx.borrow_and_update().clone() {
                                Some(reason) => Err(wire_terminal_message(reason)),
                                None => continue,
                            },
                            Err(_) => {
                                wire_terminal_open = false;
                                continue;
                            }
                        }
                    }
                    message = inbound.message() => match message {
                        Ok(Some(envelope)) => decode_receipt(&envelope.json),
                        Ok(None) => Err("asp-python-graphs gRPC stream closed".to_owned()),
                        Err(error) => Err(error.to_string()),
                    }
                };
                let request_id = receipt
                    .as_ref()
                    .ok()
                    .and_then(|value| value.get("requestId"))
                    .and_then(Value::as_str)
                    .map(str::to_owned);
                if let Some(request_id) = request_id {
                    if let Some(sender) = response_pending.lock().await.remove(&request_id) {
                        let _ = sender.send(receipt);
                    } else if response_cancelled.lock().await.remove(&request_id) {
                        response_late_cancellations.fetch_add(1, Ordering::AcqRel);
                    }
                    continue;
                }
                let message = receipt
                    .err()
                    .unwrap_or_else(|| "asp-python-graphs receipt has no requestId".to_owned());
                terminalize_pending(&response_terminal, &response_pending, message).await;
                break;
            }
        });
        Ok(Self {
            outbound,
            control,
            pending,
            cancelled,
            late_cancellations,
            terminal,
            _response_reader: Arc::new(ResponseReader(response_reader)),
        })
    }

    pub async fn call(&self, request: Value) -> Result<Value, String> {
        self.call_with_cancellation(request, None).await
    }

    pub async fn call_cancellable(
        &self,
        request: Value,
        cancellation: agent_semantic_client_db::runtime_generation_cancellation::GenerationCancellation,
    ) -> Result<Value, String> {
        self.call_with_cancellation(request, Some(cancellation))
            .await
    }

    async fn call_with_cancellation(
        &self,
        request: Value,
        cancellation: Option<
            agent_semantic_client_db::runtime_generation_cancellation::GenerationCancellation,
        >,
    ) -> Result<Value, String> {
        if let Some(message) = self.terminal.lock().await.clone() {
            return Err(format!("asp-python-graphs terminal: {message}"));
        }
        let request_id = request
            .get("requestId")
            .and_then(Value::as_str)
            .filter(|request_id| !request_id.is_empty())
            .ok_or_else(|| "asp-python-graphs request requires requestId".to_owned())?
            .to_owned();
        let (sender, receiver) = oneshot::channel();
        if self
            .pending
            .lock()
            .await
            .insert(request_id.clone(), sender)
            .is_some()
        {
            return Err(format!(
                "asp-python-graphs requestId is already pending: {request_id}"
            ));
        }
        if self.outbound.send(request).await.is_err() {
            self.pending.lock().await.remove(&request_id);
            return Err("asp-python-graphs request queue is closed".to_owned());
        }
        match cancellation {
            None => receiver
                .await
                .map_err(|_| "asp-python-graphs response channel closed".to_owned())?,
            Some(cancellation) => tokio::select! {
                response = receiver => response
                    .map_err(|_| "asp-python-graphs response channel closed".to_owned())?,
                _ = cancellation.cancelled() => {
                    self.pending.lock().await.remove(&request_id);
                    let mut cancelled = self.cancelled.lock().await;
                    if cancelled.len() >= 1024 {
                        if let Some(oldest) = cancelled.iter().next().cloned() {
                            cancelled.remove(&oldest);
                        }
                    }
                    cancelled.insert(request_id.clone());
                    drop(cancelled);
                    let cancel_request = serde_json::json!({
                            "schemaId": ASP_PYTHON_GRAPHS_SESSION_SCHEMA_ID,
                            "schemaVersion": ASP_PYTHON_GRAPHS_SESSION_SCHEMA_VERSION,
                            "sessionId": "transport",
                            "serviceEpoch": "runtime-server",
                            "requestId": format!("cancel-{request_id}"),
                            "messageKind": "cancel",
                            "cancellationId": request_id,
                            "payloadSchemaId": ASP_PYTHON_GRAPHS_SESSION_SCHEMA_ID,
                            "payload": {},
                        });
                    if self.control.send(cancel_request).await.is_err() {
                        return Err("asp-python-graphs cancellation control queue is closed".to_owned());
                    }
                    Err("asp-python-graphs request cancelled".to_owned())
                }
            },
        }
    }

    pub fn late_cancellation_count(&self) -> u64 {
        self.late_cancellations.load(Ordering::Acquire)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AspPythonGraphsLifecycleState {
    Starting,
    Ready,
    Failed(String),
    Draining,
    Stopped,
}

struct LifecycleState {
    transport: Option<AspPythonGraphsTransport>,
    child_shutdown: Option<tokio::sync::watch::Sender<bool>>,
    child_supervisor: Option<tokio::task::JoinHandle<()>>,
    starting: Option<Arc<tokio::sync::Notify>>,
    terminal: Option<String>,
    draining: bool,
}

/// ASP Server-owned lifecycle for the one Python Graphs process connection.
///
/// The process itself is selected by the Runtime Server control plane. This
/// type owns exactly one long-lived UDS gRPC session;
/// callers never connect or spawn a provider per request.
#[derive(Clone)]
pub struct AspPythonGraphsServer {
    socket_path: Arc<PathBuf>,
    capacity: usize,
    artifact: Option<Arc<crate::asp_python_graphs_artifact::VerifiedAspPythonGraphsArtifact>>,
    request_id_sequence: Arc<AtomicU64>,
    state: Arc<Mutex<LifecycleState>>,
}

pub type AspPythonGraphsLifecycle = AspPythonGraphsServer;

impl AspPythonGraphsServer {
    pub fn new(socket_path: impl Into<PathBuf>, capacity: usize) -> Result<Self, String> {
        if capacity == 0 {
            return Err("asp-python-graphs queue capacity must be positive".to_owned());
        }
        Ok(Self {
            socket_path: Arc::new(socket_path.into()),
            capacity,
            artifact: None,
            request_id_sequence: Arc::new(AtomicU64::new(0)),
            state: Arc::new(Mutex::new(LifecycleState {
                transport: None,
                child_shutdown: None,
                child_supervisor: None,
                starting: None,
                terminal: None,
                draining: false,
            })),
        })
    }

    pub fn from_artifact(
        artifact: crate::asp_python_graphs_artifact::VerifiedAspPythonGraphsArtifact,
        socket_path: impl Into<PathBuf>,
        capacity: usize,
    ) -> Result<Self, String> {
        let mut server = Self::new(socket_path, capacity)?;
        server.artifact = Some(Arc::new(artifact));
        Ok(server)
    }

    pub fn unavailable(
        socket_path: impl Into<PathBuf>,
        capacity: usize,
        reason: impl Into<String>,
    ) -> Result<Self, String> {
        let server = Self::new(socket_path, capacity)?;
        if let Ok(mut state) = server.state.try_lock() {
            state.terminal = Some(reason.into());
        }
        Ok(server)
    }

    pub fn state(&self) -> AspPythonGraphsLifecycleState {
        // This is a diagnostic snapshot. Async callers should use `state_async`.
        match self.state.try_lock() {
            Ok(state) if state.draining => AspPythonGraphsLifecycleState::Draining,
            Ok(state) if state.terminal.is_some() => {
                AspPythonGraphsLifecycleState::Failed(state.terminal.clone().unwrap_or_default())
            }
            Ok(state) if state.transport.is_some() => AspPythonGraphsLifecycleState::Ready,
            Ok(_) => AspPythonGraphsLifecycleState::Starting,
            Err(_) => AspPythonGraphsLifecycleState::Starting,
        }
    }

    pub async fn state_async(&self) -> AspPythonGraphsLifecycleState {
        let state = self.state.lock().await;
        if state.draining {
            AspPythonGraphsLifecycleState::Draining
        } else if let Some(reason) = &state.terminal {
            AspPythonGraphsLifecycleState::Failed(reason.clone())
        } else if state.transport.is_some() {
            AspPythonGraphsLifecycleState::Ready
        } else {
            AspPythonGraphsLifecycleState::Starting
        }
    }

    pub async fn ensure_started(&self) -> Result<(), String> {
        loop {
            let wait = {
                let mut state = self.state.lock().await;
                if state.draining {
                    return Err("asp-python-graphs lifecycle is draining".to_owned());
                }
                if let Some(reason) = &state.terminal {
                    return Err(format!("asp-python-graphs terminal: {reason}"));
                }
                if state.transport.is_some() {
                    return Ok(());
                }
                if let Some(wait) = &state.starting {
                    Some(Arc::clone(wait))
                } else {
                    let wait = Arc::new(tokio::sync::Notify::new());
                    state.starting = Some(Arc::clone(&wait));
                    let server = self.clone();
                    let completion = Arc::clone(&wait);
                    tokio::spawn(async move { server.start_once(completion).await });
                    Some(wait)
                }
            };
            // The first caller's task owns the single start attempt. Waiting
            // callers use the shared notify, and a dropped caller cannot
            // cancel that attempt.
            let wait = wait.expect("start wait is always present");
            let notified = wait.notified();
            notified.await;
        }
    }

    async fn start_once(&self, completion: Arc<tokio::sync::Notify>) {
        let result = async {
            let artifact = self.artifact.as_ref().ok_or_else(|| {
                "state=unavailable reasonKind=asp-python-graphs-bundle-member-not-installed"
                    .to_owned()
            })?;
            let child = artifact.command_for_socket(&self.socket_path)?.spawn().map_err(|error| {
                format!("state=unavailable reasonKind=asp-python-graphs-bundle-member-spawn-failed error={error}")
            })?;
            let (child_shutdown, child_shutdown_receiver) = tokio::sync::watch::channel(false);
            let server = self.clone();
            let child_supervisor = tokio::spawn(async move {
                supervise_child(child, child_shutdown_receiver, server).await;
            });
            {
                let mut state = self.state.lock().await;
                state.child_shutdown = Some(child_shutdown);
                state.child_supervisor = Some(child_supervisor);
            }
            let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(10);
            let transport = loop {
                if let Some(reason) = self.state.lock().await.terminal.clone() {
                    return Err(format!("state=failed reasonKind=asp-python-graphs-child-terminal error={reason}"));
                }
                match AspPythonGraphsTransport::connect_unix_with_capacity(
                    (*self.socket_path).clone(), self.capacity,
                ).await {
                    Ok(transport) => break transport,
                    Err(error) if tokio::time::Instant::now() >= deadline => {
                        return Err(format!("state=failed reasonKind=asp-python-graphs-readiness-timeout error={error}"));
                    }
                    Err(_) => tokio::time::sleep(std::time::Duration::from_millis(25)).await,
                }
            };
            let (runtime_digest, execution_digest) = (
                artifact.content_digest(),
                artifact.execution_command_digest(),
            );
            let request_id = self.next_id("hello");
            let hello = serde_json::json!({
                "schemaId": ASP_PYTHON_GRAPHS_SESSION_SCHEMA_ID,
                "schemaVersion": ASP_PYTHON_GRAPHS_SESSION_SCHEMA_VERSION,
                "sessionId": "runtime-server",
                "serviceEpoch": "runtime-server",
                "requestId": request_id,
                "messageKind": "hello",
                "runtimeArtifactDigest": runtime_digest,
                "executionArtifactDigest": execution_digest,
                "payloadSchemaId": ASP_PYTHON_GRAPHS_SESSION_SCHEMA_ID,
                "payload": {},
            });
            match transport.call(hello).await {
                Ok(receipt) if receipt_state(&receipt) == Some("ready") => Ok(transport),
                Ok(receipt) => Err(format!("asp-python-graphs hello rejected: {receipt}")),
                Err(error) => Err(format!("asp-python-graphs hello failed: {error}")),
            }
        }.await;
        if result.is_err() {
            let (shutdown, supervisor) = {
                let mut state = self.state.lock().await;
                (state.child_shutdown.take(), state.child_supervisor.take())
            };
            if let Some(shutdown) = shutdown {
                let _ = shutdown.send(true);
            }
            if let Some(supervisor) = supervisor {
                let _ = supervisor.await;
            }
        }
        let result = match result {
            Ok(transport) => Ok(transport),
            Err(error) => Err(error),
        };
        let mut state = self.state.lock().await;
        state.starting = None;
        match result {
            Ok(transport) => {
                if state.terminal.is_none() {
                    state.transport = Some(transport);
                }
            }
            Err(error) => state.terminal = Some(error),
        }
        if state.transport.is_none() {
            state.child_shutdown = None;
            state.child_supervisor = None;
        }
        completion.notify_waiters();
    }

    pub async fn recover_after_terminal(&self) -> Result<(), String> {
        let mut state = self.state.lock().await;
        state.transport = None;
        state.terminal = None;
        state.draining = false;
        Ok(())
    }

    /// Evaluate a server-owned, non-generation graph operation.  History and
    /// timeline analysis uses this path so it shares the one
    /// `asp-python-graphs` process, bounded transport, cancellation registry,
    /// and terminal authority with generation-bound evaluation.
    pub async fn timeline(
        &self,
        request_id: String,
        payload: Value,
        cancellation: agent_semantic_client_db::runtime_generation_cancellation::GenerationCancellation,
    ) -> Result<Value, String> {
        self.ensure_started().await?;
        let transport = self
            .state
            .lock()
            .await
            .transport
            .clone()
            .ok_or_else(|| "asp-python-graphs transport is absent".to_owned())?;
        let internal_request_id = self.next_id("request");
        let mut request = self.service_envelope_with_request_id(
            "runtime-server",
            "timeline",
            internal_request_id.clone(),
            payload,
        );
        request["clientRequestId"] = Value::String(request_id);
        let transport_result = transport.call_cancellable(request, cancellation).await;
        let transport_error = transport_result.as_ref().err().cloned();
        let result = transport_result.and_then(|receipt| {
            if receipt.get("requestId").and_then(Value::as_str)
                != Some(internal_request_id.as_str())
            {
                return Err("asp-python-graphs timeline receipt requestId mismatch".to_owned());
            }
            if receipt_state(&receipt) != Some("completed") {
                return Err(format!("asp-python-graphs timeline rejected: {receipt}"));
            }
            receipt
                .get("payload")
                .and_then(Value::as_object)
                .and_then(|payload| payload.get("result"))
                .cloned()
                .ok_or_else(|| "asp-python-graphs timeline receipt lacks payload.result".to_owned())
        });
        if let Some(error) = transport_error {
            if !error.contains("cancelled") {
                self.mark_terminal(error).await;
            }
        }
        result
    }

    /// Construct and content-bind the graph stage of one Search generation.
    /// The Runtime owns transport and lifecycle only; the request and receipt
    /// contract are owned by `agent-semantic-search`.
    pub async fn generation_graph(
        &self,
        request_id: String,
        payload: Value,
        cancellation: agent_semantic_client_db::runtime_generation_cancellation::GenerationCancellation,
    ) -> Result<Value, String> {
        self.ensure_started().await?;
        let transport = self
            .state
            .lock()
            .await
            .transport
            .clone()
            .ok_or_else(|| "asp-python-graphs transport is absent".to_owned())?;
        let internal_request_id = self.next_id("generation-graph");
        let mut request = self.service_envelope_with_request_id(
            "runtime-server",
            "generation-graph",
            internal_request_id.clone(),
            payload,
        );
        request["clientRequestId"] = Value::String(request_id);
        agent_semantic_search_projection::bind_graph_generation_identity(&mut request)?;
        let transport_result = transport.call_cancellable(request, cancellation).await;
        let transport_error = transport_result.as_ref().err().cloned();
        let result = transport_result.and_then(|receipt| {
            if receipt.get("requestId").and_then(Value::as_str)
                != Some(internal_request_id.as_str())
            {
                return Err("asp-python-graphs generation receipt requestId mismatch".to_owned());
            }
            if receipt_state(&receipt) != Some("completed") {
                return Err(format!(
                    "asp-python-graphs generation graph rejected: {receipt}"
                ));
            }
            receipt
                .get("payload")
                .and_then(Value::as_object)
                .and_then(|payload| payload.get("result"))
                .cloned()
                .ok_or_else(|| {
                    "asp-python-graphs generation receipt lacks payload.result".to_owned()
                })
        });
        if let Some(error) = transport_error {
            if !error.contains("cancelled") {
                self.mark_terminal(error).await;
            }
        }
        result
    }

    /// Evaluate one intent-only request against an already retained exact
    /// workspace generation. The service envelope, not the intent payload,
    /// carries the durable generation identity.
    pub async fn evaluate_resident(
        &self,
        workspace_identity: String,
        generation_digest: String,
        request_id: String,
        payload: Value,
        cancellation: agent_semantic_client_db::runtime_generation_cancellation::GenerationCancellation,
    ) -> Result<Value, String> {
        self.ensure_started().await?;
        let transport = self
            .state
            .lock()
            .await
            .transport
            .clone()
            .ok_or_else(|| "asp-python-graphs transport is absent".to_owned())?;
        let internal_request_id = self.next_id("evaluate-resident");
        let mut request = self.service_envelope_with_request_id(
            "runtime-server",
            "evaluate-resident",
            internal_request_id.clone(),
            payload,
        );
        request["clientRequestId"] = Value::String(request_id);
        request["workspaceIdentity"] = Value::String(workspace_identity.clone());
        request["generationDigest"] = Value::String(generation_digest.clone());
        let transport_result = transport.call_cancellable(request, cancellation).await;
        let transport_error = transport_result.as_ref().err().cloned();
        let result = transport_result.and_then(|receipt| {
            agent_semantic_search_projection::validate_graph_generation_receipt_identity(
                &receipt,
                &internal_request_id,
                "completed",
                &workspace_identity,
                &generation_digest,
            )?;
            receipt
                .get("payload")
                .and_then(Value::as_object)
                .cloned()
                .map(Value::Object)
                .ok_or_else(|| {
                    "asp-python-graphs resident evaluation receipt lacks payload".to_owned()
                })
        });
        if let Some(error) = transport_error {
            if !error.contains("cancelled") {
                self.mark_terminal(error).await;
            }
        }
        result
    }

    /// Release exactly one retained workspace generation. A missing or drifted
    /// identity remains a typed terminal and cannot release another entry.
    pub async fn release_generation(
        &self,
        workspace_identity: String,
        generation_digest: String,
        request_id: String,
    ) -> Result<Value, String> {
        self.ensure_started().await?;
        let transport = self
            .state
            .lock()
            .await
            .transport
            .clone()
            .ok_or_else(|| "asp-python-graphs transport is absent".to_owned())?;
        let internal_request_id = self.next_id("release-generation");
        let mut request = self.service_envelope_with_request_id(
            "runtime-server",
            "release-generation",
            internal_request_id.clone(),
            serde_json::json!({}),
        );
        request["clientRequestId"] = Value::String(request_id);
        request["workspaceIdentity"] = Value::String(workspace_identity.clone());
        request["generationDigest"] = Value::String(generation_digest.clone());
        let receipt = transport.call(request).await?;
        agent_semantic_search_projection::validate_graph_generation_receipt_identity(
            &receipt,
            &internal_request_id,
            "released",
            &workspace_identity,
            &generation_digest,
        )?;
        Ok(receipt)
    }

    async fn mark_terminal(&self, reason: String) {
        let mut state = self.state.lock().await;
        state.terminal = Some(reason);
        state.transport = None;
    }

    fn service_envelope_with_request_id(
        &self,
        session_id: &str,
        kind: &str,
        request_id: String,
        payload: Value,
    ) -> Value {
        serde_json::json!({
            "schemaId": ASP_PYTHON_GRAPHS_SESSION_SCHEMA_ID,
            "schemaVersion": ASP_PYTHON_GRAPHS_SESSION_SCHEMA_VERSION,
            "sessionId": session_id,
            "serviceEpoch": "runtime-server",
            "requestId": request_id,
            "messageKind": kind,
            "payloadSchemaId": payload.get("schemaId").and_then(Value::as_str).unwrap_or(ASP_PYTHON_GRAPHS_SESSION_SCHEMA_ID),
            "payload": payload,
        })
    }

    fn next_id(&self, prefix: &str) -> String {
        let sequence = self.request_id_sequence.fetch_add(1, Ordering::AcqRel) + 1;
        format!("{prefix}-{sequence}")
    }

    pub async fn drain(&self) -> Result<(), String> {
        self.state.lock().await.draining = true;
        Ok(())
    }

    pub async fn shutdown(&self) -> Result<(), String> {
        let (transport, child_shutdown, supervisor) = {
            let mut state = self.state.lock().await;
            state.draining = true;
            state.terminal = Some("asp-python-graphs lifecycle stopped".to_owned());
            (
                state.transport.take(),
                state.child_shutdown.take(),
                state.child_supervisor.take(),
            )
        };
        drop(transport);
        if let Some(shutdown) = child_shutdown {
            let _ = shutdown.send(true);
        }
        if let Some(supervisor) = supervisor {
            let _ = supervisor.await;
        }
        Ok(())
    }
}

fn decode_receipt(bytes: &[u8]) -> Result<Value, String> {
    let receipt: Value = serde_json::from_slice(bytes)
        .map_err(|error| format!("decode asp-python-graphs receipt: {error}"))?;
    if !receipt.is_object() {
        return Err("asp-python-graphs receipt must be a JSON object".to_owned());
    }
    Ok(receipt)
}

fn receipt_state(receipt: &Value) -> Option<&str> {
    receipt
        .get("payload")
        .and_then(Value::as_object)
        .and_then(|payload| payload.get("state"))
        .and_then(Value::as_str)
}

async fn supervise_child(
    mut child: tokio::process::Child,
    mut shutdown: tokio::sync::watch::Receiver<bool>,
    server: AspPythonGraphsServer,
) {
    let status = tokio::select! {
        status = child.wait() => status,
        _ = shutdown.changed() => {
            let _ = child.kill().await;
            child.wait().await
        }
    };
    let mut state = server.state.lock().await;
    if !state.draining {
        state.transport = None;
        state.child_shutdown = None;
        state.child_supervisor = None;
        state.terminal = Some(format!(
            "state=failed reasonKind=asp-python-graphs-process-exited status={status:?}"
        ));
    }
}

async fn connect_unix_channel(socket_path: PathBuf) -> Result<tonic::transport::Channel, String> {
    tonic::transport::Endpoint::try_from("http://[::]:50051")
        .map_err(|error| error.to_string())?
        .connect_with_connector(tower::service_fn(move |_| {
            let socket_path = socket_path.clone();
            async move {
                tokio::net::UnixStream::connect(socket_path)
                    .await
                    .map(hyper_util::rt::TokioIo::new)
            }
        }))
        .await
        .map_err(|error| error.to_string())
}

#[cfg(test)]
#[path = "../tests/unit/asp_python_graphs_transport.rs"]
mod tests;
