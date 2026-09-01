//! Bounded, multiplexed gRPC transport to the ASP Server-owned Python graphs service.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

use serde_json::Value;
use tokio::sync::{Mutex, mpsc, oneshot};

pub mod generated {
    tonic::include_proto!("asp.python.graphs");
}

use generated::{GraphsEnvelope, asp_python_graphs_client::AspPythonGraphsClient};

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

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct GraphGenerationIdentity {
    pub workspace_identity: String,
    pub generation_digest: String,
    pub publication_token: u64,
}

impl GraphGenerationIdentity {
    pub fn new(
        workspace_identity: impl Into<String>,
        generation_digest: impl Into<String>,
    ) -> Result<Self, String> {
        let identity = Self {
            workspace_identity: workspace_identity.into(),
            generation_digest: generation_digest.into(),
            publication_token: 0,
        };
        if identity.workspace_identity.is_empty() || identity.generation_digest.is_empty() {
            return Err(
                "asp-python-graphs generation identity requires workspace and generation"
                    .to_owned(),
            );
        }
        Ok(identity)
    }

    pub fn new_with_token(
        workspace_identity: impl Into<String>,
        generation_digest: impl Into<String>,
        publication_token: u64,
    ) -> Result<Self, String> {
        let mut identity = Self::new(workspace_identity, generation_digest)?;
        identity.publication_token = publication_token;
        Ok(identity)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GenerationAdmission {
    new: bool,
}

impl GenerationAdmission {
    pub fn is_new(self) -> bool {
        self.new
    }
}

#[derive(Default)]
pub struct GraphGenerationLedger {
    refs: BTreeMap<GraphGenerationIdentity, usize>,
}

impl GraphGenerationLedger {
    pub fn admit(&mut self, identity: GraphGenerationIdentity) -> GenerationAdmission {
        let entry = self.refs.entry(identity).or_insert(0);
        let new = *entry == 0;
        *entry += 1;
        GenerationAdmission { new }
    }

    pub fn refs(&self, identity: &GraphGenerationIdentity) -> Option<usize> {
        self.refs.get(identity).copied()
    }

    pub fn release(&mut self, identity: &GraphGenerationIdentity) -> bool {
        let Some(entry) = self.refs.get_mut(identity) else {
            return false;
        };
        *entry -= 1;
        if *entry == 0 {
            self.refs.remove(identity);
            true
        } else {
            false
        }
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

struct GenerationSlot {
    session_id: String,
    refs: usize,
    graph_payload_digest: String,
    graph_payload: Arc<Value>,
    retiring: bool,
    status: GenerationSlotStatus,
}

enum GenerationSlotStatus {
    Opening(Arc<tokio::sync::Notify>),
    Ready,
    Failed(String),
}

struct LifecycleState {
    transport: Option<AspPythonGraphsTransport>,
    child_shutdown: Option<tokio::sync::watch::Sender<bool>>,
    child_supervisor: Option<tokio::task::JoinHandle<()>>,
    starting: Option<Arc<tokio::sync::Notify>>,
    terminal: Option<String>,
    draining: bool,
    generations: BTreeMap<GraphGenerationIdentity, GenerationSlot>,
    changed: Arc<tokio::sync::Notify>,
}

/// ASP Server-owned lifecycle for the one Python Graphs process connection.
///
/// The process itself is selected by the Runtime Server control plane. This
/// type owns exactly one long-lived UDS gRPC session and all generation leases;
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

pub struct GraphGenerationLease {
    server: AspPythonGraphsServer,
    identity: GraphGenerationIdentity,
    session_id: String,
    released: bool,
}

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
                generations: BTreeMap::new(),
                changed: Arc::new(tokio::sync::Notify::new()),
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
                "state=unavailable reasonKind=asp-python-graphs-artifact-receipt-missing".to_owned()
            })?;
            let child = artifact.command_for_socket(&self.socket_path)?.spawn().map_err(|error| {
                format!("state=unavailable reasonKind=asp-python-graphs-artifact-spawn-failed error={error}")
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
                &artifact.descriptor().content_digest,
                &artifact.descriptor().execution_command_digest,
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
        if !state.generations.is_empty() {
            return Err("cannot recover asp-python-graphs while generations are leased".to_owned());
        }
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

    pub async fn open_generation_with_token(
        &self,
        identity: GraphGenerationIdentity,
        publication_token: u64,
        graph_payload: Value,
        cancellation: agent_semantic_client_db::runtime_generation_cancellation::GenerationCancellation,
    ) -> Result<GraphGenerationLease, String> {
        let artifact =
            agent_semantic_content_identity::ArtifactJson::from_serializable(&graph_payload)
                .map_err(|error| format!("canonicalize asp-python-graphs generation: {error}"))?;
        let graph_payload_digest = format!(
            "blake3-256:{}",
            agent_semantic_content_identity::hash_normalized_json(&artifact).value
        );
        self.open_generation_shared_with_token(
            identity,
            publication_token,
            graph_payload_digest,
            Arc::new(graph_payload),
            cancellation,
        )
        .await
    }

    /// Acquire a generation lease without cloning or re-hashing the immutable
    /// graph on every query. The Runtime search mmap owns the canonical Arc and
    /// digest; only the first admission sends the payload to Python.
    pub async fn open_generation_shared_with_token(
        &self,
        mut identity: GraphGenerationIdentity,
        publication_token: u64,
        graph_payload_digest: String,
        graph_payload: Arc<Value>,
        cancellation: agent_semantic_client_db::runtime_generation_cancellation::GenerationCancellation,
    ) -> Result<GraphGenerationLease, String> {
        if publication_token == 0 {
            return Err("state=stale-generation reasonKind=missing-publication-token".to_owned());
        }
        if !graph_payload_digest.starts_with("blake3-256:") {
            return Err("asp-python-graphs generation payload digest is invalid".to_owned());
        }
        identity.publication_token = publication_token;
        self.ensure_started().await?;
        let (notify, start_open, session_id) = {
            let mut state = self.state.lock().await;
            if state.draining {
                return Err("asp-python-graphs lifecycle is draining".to_owned());
            }
            if let Some(slot) = state.generations.get_mut(&identity) {
                if let GenerationSlotStatus::Failed(error) = &slot.status {
                    return Err(error.clone());
                }
                if slot.retiring {
                    return Err("asp-python-graphs generation is retiring".to_owned());
                }
                if slot.graph_payload_digest != graph_payload_digest {
                    return Err("asp-python-graphs generation graph payload conflicts with the admitted snapshot".to_owned());
                }
                slot.refs += 1;
                let notify = match &slot.status {
                    GenerationSlotStatus::Opening(notify) => Some(Arc::clone(notify)),
                    GenerationSlotStatus::Ready => {
                        return Ok(GraphGenerationLease {
                            server: self.clone(),
                            identity,
                            session_id: slot.session_id.clone(),
                            released: false,
                        });
                    }
                    GenerationSlotStatus::Failed(error) => return Err(error.clone()),
                };
                (notify, false, slot.session_id.clone())
            } else {
                let session_id = self.next_id("session");
                let notify = Arc::new(tokio::sync::Notify::new());
                state.generations.insert(
                    identity.clone(),
                    GenerationSlot {
                        session_id: session_id.clone(),
                        refs: 1,
                        graph_payload_digest,
                        graph_payload,
                        retiring: false,
                        status: GenerationSlotStatus::Opening(Arc::clone(&notify)),
                    },
                );
                (Some(notify), true, session_id)
            }
        };
        if start_open {
            let server = self.clone();
            let open_identity = identity.clone();
            tokio::spawn(async move { server.finish_open(open_identity).await });
        }
        let notify = notify.expect("generation open always has a terminal notification");
        loop {
            let (terminal, notified) = {
                let state = self.state.lock().await;
                let terminal = state
                    .generations
                    .get(&identity)
                    .map(|slot| match &slot.status {
                        GenerationSlotStatus::Opening(_) => None,
                        GenerationSlotStatus::Ready => Some(Ok(())),
                        GenerationSlotStatus::Failed(error) => Some(Err(error.clone())),
                    });
                (terminal, notify.notified())
            };
            match terminal.flatten() {
                Some(Ok(())) => {
                    return Ok(GraphGenerationLease {
                        server: self.clone(),
                        identity,
                        session_id,
                        released: false,
                    });
                }
                Some(Err(error)) => {
                    let _ = self.release_generation(&identity, &session_id).await;
                    return Err(error);
                }
                None => tokio::select! {
                    _ = notified => {},
                    _ = cancellation.cancelled() => {
                        let _ = self.release_generation(&identity, &session_id).await;
                        return Err("asp-python-graphs generation open cancelled".to_owned());
                    }
                },
            }
        }
    }

    async fn finish_open(&self, identity: GraphGenerationIdentity) {
        let session_id = {
            let state = self.state.lock().await;
            state
                .generations
                .get(&identity)
                .map(|slot| slot.session_id.clone())
        };
        let Some(session_id) = session_id else { return };
        let graph_payload = {
            let state = self.state.lock().await;
            state
                .generations
                .get(&identity)
                .map(|slot| Arc::clone(&slot.graph_payload))
        };
        let Some(graph_payload) = graph_payload else {
            return;
        };
        let request = self.envelope(
            &session_id,
            "open-generation",
            &identity,
            graph_payload.as_ref().clone(),
        );
        let (result, process_terminal) = async {
            let transport = self
                .state
                .lock()
                .await
                .transport
                .clone()
                .ok_or_else(|| "asp-python-graphs transport is absent".to_owned());
            let transport = match transport {
                Ok(transport) => transport,
                Err(error) => return (Err(error), true),
            };
            match transport.call(request).await {
                Err(error) => (Err(error), true),
                Ok(response) if receipt_state(&response) == Some("ready") => (Ok(()), false),
                Ok(response) => (
                    Err(format!(
                        "asp-python-graphs generation open rejected: {response}"
                    )),
                    false,
                ),
            }
        }
        .await;
        let (notify, retired) = {
            let mut state = self.state.lock().await;
            let Some(slot) = state.generations.get_mut(&identity) else {
                return;
            };
            let notify = match &slot.status {
                GenerationSlotStatus::Opening(notify) => Arc::clone(notify),
                _ => return,
            };
            let failed = result.as_ref().err().cloned();
            let succeeded = result.is_ok();
            slot.status = match result {
                Ok(()) => GenerationSlotStatus::Ready,
                Err(error) => GenerationSlotStatus::Failed(error),
            };
            if process_terminal {
                if let Some(error) = failed {
                    state.terminal = Some(error);
                }
            }
            let highest_token = state
                .generations
                .iter()
                .filter(|(existing, _)| existing.workspace_identity == identity.workspace_identity)
                .map(|(existing, _)| existing.publication_token)
                .max()
                .unwrap_or(identity.publication_token);
            let equal_token_conflict = succeeded
                && state.generations.iter().any(|(existing, _)| {
                    existing.workspace_identity == identity.workspace_identity
                        && existing != &identity
                        && existing.publication_token == identity.publication_token
                });
            let stale =
                succeeded && (identity.publication_token < highest_token || equal_token_conflict);
            if stale {
                if let Some(slot) = state.generations.get_mut(&identity) {
                    slot.retiring = true;
                    slot.status = GenerationSlotStatus::Failed(
                        "asp-python-graphs stale generation publication".to_owned(),
                    );
                }
            }
            let retired = if succeeded && !stale {
                state
                    .generations
                    .iter_mut()
                    .filter(|(existing, _)| {
                        existing.workspace_identity == identity.workspace_identity
                            && *existing != &identity
                    })
                    .filter_map(|(_, slot)| {
                        slot.retiring = true;
                        (slot.refs == 0).then(|| slot.session_id.clone())
                    })
                    .collect::<Vec<_>>()
            } else {
                Vec::new()
            };
            (notify, retired)
        };
        notify.notify_waiters();
        for session in retired {
            self.retire_zero_ref_generation(&identity.workspace_identity, &session)
                .await;
        }
    }

    async fn mark_terminal(&self, reason: String) {
        let mut state = self.state.lock().await;
        state.terminal = Some(reason);
        state.transport = None;
    }

    fn envelope(
        &self,
        session_id: &str,
        kind: &str,
        identity: &GraphGenerationIdentity,
        payload: Value,
    ) -> Value {
        self.envelope_with_request_id(session_id, kind, identity, self.next_id("request"), payload)
    }

    fn envelope_with_request_id(
        &self,
        session_id: &str,
        kind: &str,
        identity: &GraphGenerationIdentity,
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
            "workspaceIdentity": identity.workspace_identity,
            "generationDigest": identity.generation_digest,
            "generationToken": identity.publication_token,
            "payloadSchemaId": payload.get("schemaId").and_then(Value::as_str).unwrap_or(ASP_PYTHON_GRAPHS_SESSION_SCHEMA_ID),
            "payload": payload,
        })
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

    async fn release_generation(
        &self,
        identity: &GraphGenerationIdentity,
        session_id: &str,
    ) -> Result<(), String> {
        let should_release = {
            let mut state = self.state.lock().await;
            let Some(slot) = state.generations.get_mut(identity) else {
                return Ok(());
            };
            if slot.refs > 1 {
                slot.refs -= 1;
                false
            } else if !slot.retiring {
                slot.refs = 0;
                state.changed.notify_waiters();
                false
            } else {
                state.generations.remove(identity);
                state.changed.notify_waiters();
                true
            }
        };
        if should_release {
            let transport = self
                .state
                .lock()
                .await
                .transport
                .clone()
                .ok_or_else(|| "asp-python-graphs transport is absent".to_owned())?;
            let receipt = transport
                .call(self.envelope(
                    session_id,
                    "release-generation",
                    identity,
                    serde_json::json!({}),
                ))
                .await?;
            if receipt_state(&receipt) != Some("completed") {
                return Err(format!(
                    "asp-python-graphs generation release rejected: {receipt}"
                ));
            }
        }
        Ok(())
    }

    async fn retire_zero_ref_generation(&self, workspace_identity: &str, session_id: &str) {
        let identity = {
            let mut state = self.state.lock().await;
            let identity = state
                .generations
                .iter()
                .find(|(identity, slot)| {
                    identity.workspace_identity == workspace_identity
                        && slot.session_id == session_id
                        && slot.retiring
                        && slot.refs == 0
                })
                .map(|(identity, _)| identity.clone());
            if let Some(identity) = &identity {
                state.generations.remove(identity);
                state.changed.notify_waiters();
            }
            identity
        };
        if let Some(identity) = identity {
            if let Some(transport) = self.state.lock().await.transport.clone() {
                let _ = transport
                    .call(self.envelope(
                        session_id,
                        "release-generation",
                        &identity,
                        serde_json::json!({}),
                    ))
                    .await;
            }
        }
    }

    pub async fn drain(&self) -> Result<(), String> {
        {
            let mut state = self.state.lock().await;
            state.draining = true;
            for slot in state.generations.values_mut() {
                slot.retiring = true;
            }
        }
        loop {
            let (retired, changed, remaining) = {
                let mut state = self.state.lock().await;
                let retired = state
                    .generations
                    .iter()
                    .filter(|(_, slot)| slot.refs == 0)
                    .map(|(identity, slot)| (identity.clone(), slot.session_id.clone()))
                    .collect::<Vec<_>>();
                for (identity, _) in &retired {
                    state.generations.remove(identity);
                }
                let remaining = !state.generations.is_empty();
                (retired, Arc::clone(&state.changed), remaining)
            };
            for (identity, session_id) in retired {
                let transport = self.state.lock().await.transport.clone();
                if let Some(transport) = transport {
                    let _ = transport
                        .call(self.envelope(
                            &session_id,
                            "release-generation",
                            &identity,
                            serde_json::json!({}),
                        ))
                        .await;
                }
            }
            if !remaining {
                break;
            }
            changed.notified().await;
        }
        Ok(())
    }

    pub async fn shutdown(&self) -> Result<(), String> {
        let (transport, child_shutdown, supervisor, releases) = {
            let mut state = self.state.lock().await;
            state.draining = true;
            state.terminal = Some("asp-python-graphs lifecycle stopped".to_owned());
            let releases = state
                .generations
                .iter()
                .map(|(identity, slot)| (identity.clone(), slot.session_id.clone()))
                .collect::<Vec<_>>();
            state.generations.clear();
            state.changed.notify_waiters();
            (
                state.transport.take(),
                state.child_shutdown.take(),
                state.child_supervisor.take(),
                releases,
            )
        };
        if let Some(transport_ref) = transport.as_ref() {
            for (identity, session_id) in releases {
                let _ = transport_ref
                    .call(self.envelope(
                        &session_id,
                        "release-generation",
                        &identity,
                        serde_json::json!({}),
                    ))
                    .await;
            }
        }
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

impl GraphGenerationLease {
    pub fn identity(&self) -> &GraphGenerationIdentity {
        &self.identity
    }

    pub async fn evaluate(
        &self,
        payload: Value,
        cancellation: agent_semantic_client_db::runtime_generation_cancellation::GenerationCancellation,
    ) -> Result<Value, String> {
        self.evaluate_with_request_id(payload, self.server.next_id("request"), cancellation)
            .await
    }

    pub async fn evaluate_with_request_id(
        &self,
        payload: Value,
        request_id: String,
        cancellation: agent_semantic_client_db::runtime_generation_cancellation::GenerationCancellation,
    ) -> Result<Value, String> {
        if self.released {
            return Err("asp-python-graphs generation lease is released".to_owned());
        }
        let transport = self
            .server
            .state
            .lock()
            .await
            .transport
            .clone()
            .ok_or_else(|| "asp-python-graphs transport is absent".to_owned())?;
        let internal_request_id = self.server.next_id("request");
        let mut request = self.server.envelope_with_request_id(
            &self.session_id,
            "evaluate",
            &self.identity,
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
                return Err("asp-python-graphs receipt requestId mismatch".to_owned());
            }
            if receipt_state(&receipt) != Some("completed") {
                return Err(format!("asp-python-graphs evaluation rejected: {receipt}"));
            }
            receipt
                .get("payload")
                .and_then(Value::as_object)
                .and_then(|payload| payload.get("result"))
                .cloned()
                .ok_or_else(|| {
                    "asp-python-graphs evaluation receipt lacks payload.result".to_owned()
                })
        });
        if let Some(error) = transport_error {
            if !error.contains("cancelled") {
                self.server.mark_terminal(error).await;
            }
        }
        result
    }

    pub async fn release(mut self) -> Result<(), String> {
        if self.released {
            return Ok(());
        }
        self.released = true;
        self.server
            .release_generation(&self.identity, &self.session_id)
            .await
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
mod tests {
    use serde_json::json;

    use super::{
        AspPythonGraphsServer, GraphGenerationIdentity, GraphGenerationLedger, decode_receipt,
        terminalize_pending, wire_terminal_message,
    };

    #[test]
    fn receipt_requires_json_object() {
        assert!(decode_receipt(br#"{"requestId":"request-1"}"#).is_ok());
        assert!(decode_receipt(br#"[]"#).is_err());
    }

    #[test]
    fn generation_ledger_reuses_one_open_and_releases_at_zero() {
        let mut ledger = GraphGenerationLedger::default();
        let identity = GraphGenerationIdentity::new("workspace", "generation").unwrap();

        assert!(ledger.admit(identity.clone()).is_new());
        assert!(!ledger.admit(identity.clone()).is_new());
        assert_eq!(ledger.refs(&identity), Some(2));
        assert!(!ledger.release(&identity));
        assert!(ledger.release(&identity));
        assert_eq!(ledger.refs(&identity), None);
    }

    #[test]
    fn server_envelopes_leave_wire_sequence_to_transport() {
        let server = AspPythonGraphsServer::new("/tmp/asp-python-graphs.sock", 1).unwrap();
        let identity =
            GraphGenerationIdentity::new_with_token("workspace", "generation", 7).unwrap();

        let first = server.envelope("session", "evaluate", &identity, json!({}));
        let second = server.envelope("session", "evaluate", &identity, json!({}));

        assert!(first.get("sequence").is_none());
        assert!(second.get("sequence").is_none());
        assert_eq!(first["requestId"], "request-1");
        assert_eq!(second["requestId"], "request-2");
    }

    #[test]
    fn generic_wire_failures_map_to_stable_graph_protocol_terminals() {
        assert_eq!(
            wire_terminal_message(
                agent_semantic_provider_transport::PriorityJsonStreamTerminal::SequenceOverflow,
            ),
            "state=failed reasonKind=asp-python-graphs-wire-sequence-overflow"
        );
        assert_eq!(
            wire_terminal_message(
                agent_semantic_provider_transport::PriorityJsonStreamTerminal::EncodeFailed(
                    "injected".to_owned(),
                ),
            ),
            "state=failed reasonKind=asp-python-graphs-wire-encode-failed error=injected"
        );
    }

    #[tokio::test]
    async fn typed_wire_terminal_drains_pending_exactly_once() {
        let terminal = tokio::sync::Mutex::new(None);
        let (first_tx, first_rx) = tokio::sync::oneshot::channel();
        let (second_tx, second_rx) = tokio::sync::oneshot::channel();
        let pending = tokio::sync::Mutex::new(std::collections::HashMap::from([
            ("first".to_owned(), first_tx),
            ("second".to_owned(), second_tx),
        ]));
        let message = "state=failed reasonKind=asp-python-graphs-wire-closed".to_owned();

        terminalize_pending(&terminal, &pending, message.clone()).await;
        terminalize_pending(&terminal, &pending, "second-terminal".to_owned()).await;

        assert_eq!(terminal.lock().await.as_deref(), Some(message.as_str()));
        assert_eq!(first_rx.await.unwrap().unwrap_err(), message);
        assert_eq!(second_rx.await.unwrap().unwrap_err(), message);
        assert!(pending.lock().await.is_empty());
    }
}
