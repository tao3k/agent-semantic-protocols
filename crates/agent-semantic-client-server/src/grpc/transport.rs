// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Loopback TCP gRPC transport for multiplexed public ASP `ClientFrame` sessions.

use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use std::task::Context;
use std::task::Poll;

use agent_semantic_client_protocol::ClientDispatchClass;
use agent_semantic_client_protocol::ClientFrame;
use agent_semantic_client_protocol::ClientRequestId;
use agent_semantic_client_protocol::classify_client_dispatch;
use futures_util::StreamExt as FuturesStreamExt;
use futures_util::stream::FuturesUnordered;
use parking_lot::Mutex;
use prost::Message;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio::sync::watch;
use tokio_stream::Stream;
use tokio_stream::wrappers::ReceiverStream;
use tonic::Request;
use tonic::Response;
use tonic::Status;
use tonic::Streaming;

use crate::AspClientDispatcher;
use crate::AspClientFrameService;

use super::generated::ClientFrameEnvelope;
use super::generated::asp_client_protocol_client::AspClientProtocolClient;
use super::generated::asp_client_protocol_server::AspClientProtocol;
use super::generated::asp_client_protocol_server::AspClientProtocolServer;
use super::wire::decode_frame;
use super::wire::encode_frame;

const CLIENT_FRAME_RESPONSE_BUDGET: std::time::Duration = std::time::Duration::from_secs(10);
const CLIENT_SESSION_CONNECT_BUDGET: std::time::Duration = std::time::Duration::from_secs(2);
pub const CLIENT_FRAME_SESSION_CAPACITY: usize = 32;
pub const CLIENT_FRAME_SESSION_CONTROL_RESERVE: usize = 1;
const CLIENT_FRAME_TRANSPORT_CAPACITY: usize =
    CLIENT_FRAME_SESSION_CAPACITY + CLIENT_FRAME_SESSION_CONTROL_RESERVE;
pub const CLIENT_FRAME_PARTITION_BYTES: usize = 256 * 1024;
pub const CLIENT_FRAME_LOGICAL_RESPONSE_BYTES: usize = 16 * 1024 * 1024;

/// Connect tonic to a Host-granted, already-connected Runtime descriptor.
///
/// Sandboxed clients must not discover a global socket path and call
/// `connect(2)`: the Host opens the Runtime-owned endpoint outside the sandbox
/// and transfers ownership of the connected descriptor. The normal
/// ClientFrame initialize/catalog exchange still validates endpoint and
/// protocol identity after the transport is established.
#[cfg(unix)]
pub fn admit_asp_client_grpc_inherited_descriptor(
    descriptor: std::os::fd::OwnedFd,
) -> Result<tokio::net::UnixStream, String> {
    let stream: std::os::unix::net::UnixStream = descriptor.into();
    stream
        .set_nonblocking(true)
        .map_err(|error| format!("prepare inherited Runtime descriptor: {error}"))?;
    tokio::net::UnixStream::from_std(stream)
        .map_err(|error| format!("admit inherited Runtime descriptor: {error}"))
}

#[cfg(unix)]
pub async fn connect_asp_client_grpc_inherited_descriptor(
    descriptor: std::os::fd::OwnedFd,
) -> Result<tonic::transport::Channel, String> {
    let stream = admit_asp_client_grpc_inherited_descriptor(descriptor)?;
    let stream = Arc::new(Mutex::new(Some(stream)));

    tonic::transport::Endpoint::try_from("http://[::]:50051")
        .map_err(|error| error.to_string())?
        .connect_with_connector(tower::service_fn(move |_| {
            let stream = Arc::clone(&stream);
            async move {
                stream
                    .lock()
                    .take()
                    .map(hyper_util::rt::TokioIo::new)
                    .ok_or_else(|| {
                        std::io::Error::new(
                            std::io::ErrorKind::NotConnected,
                            "inherited Runtime descriptor was already consumed",
                        )
                    })
            }
        }))
        .await
        .map_err(|error| format!("connect inherited Runtime descriptor: {error}"))
}

pub struct AspClientGrpcService<D> {
    frames: Arc<AspClientFrameService<D>>,
}

impl<D> AspClientGrpcService<D> {
    pub fn new(frames: Arc<AspClientFrameService<D>>) -> Self {
        Self { frames }
    }
}

#[tonic::async_trait]
impl<D: AspClientDispatcher> AspClientProtocol for AspClientGrpcService<D> {
    type SessionStream =
        Pin<Box<dyn Stream<Item = Result<ClientFrameEnvelope, Status>> + Send + 'static>>;

    async fn session(
        &self,
        request: Request<Streaming<ClientFrameEnvelope>>,
    ) -> Result<Response<Self::SessionStream>, Status> {
        let mut inbound = request.into_inner();
        let frames = Arc::clone(&self.frames);
        let (outbound, receiver) = mpsc::channel(CLIENT_FRAME_TRANSPORT_CAPACITY);
        let supervisor = tokio::spawn(async move {
            let mut inbound_open = true;
            let mut requests = FuturesUnordered::new();
            loop {
                if !inbound_open && requests.is_empty() {
                    break;
                }
                tokio::select! {
                    envelope = inbound.next(), if inbound_open && requests.len() < CLIENT_FRAME_TRANSPORT_CAPACITY => {
                        match envelope {
                            Some(Ok(envelope)) => match decode_frame(envelope) {
                                Ok(frame) => requests.push(dispatch_client_frame(Arc::clone(&frames), frame)),
                                Err(error) => {
                                    if outbound.send(Err(Status::invalid_argument(format!(
                                        "decode ASP ClientFrame: {error}"
                                    )))).await.is_err() {
                                        break;
                                    }
                                }
                            },
                            Some(Err(error)) => {
                                let _ = outbound.send(Err(error)).await;
                                break;
                            }
                            None => inbound_open = false,
                        }
                    }
                    completed = requests.next(), if !requests.is_empty() => {
                        if let Some(Some(result)) = completed {
                            match result {
                                Ok(encoded) => {
                                    let egress_started = tokio::time::Instant::now();
                                    let mut delivered = true;
                                    for envelope in encoded.envelopes {
                                        if outbound.send(Ok(envelope)).await.is_err() {
                                            delivered = false;
                                            break;
                                        }
                                    }
                                    frames.terminal_egressed(
                                        &encoded.telemetry,
                                        elapsed_micros(egress_started),
                                        delivered,
                                    );
                                    if !delivered {
                                        return;
                                    }
                                }
                                Err(error) => {
                                    if outbound.send(Err(error)).await.is_err() {
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        });
        Ok(Response::new(Box::pin(GrpcResponseStream {
            receiver: ReceiverStream::new(receiver),
            supervisor,
        })))
    }
}

async fn dispatch_client_frame<D: AspClientDispatcher>(
    frames: Arc<AspClientFrameService<D>>,
    frame: ClientFrame,
) -> Option<Result<EncodedClientResponse, Status>> {
    match frames.handle_frame(frame).await {
        Ok(Some(response)) => {
            let telemetry = response_telemetry(&response)?;
            let serialization_started = tokio::time::Instant::now();
            let encoded = encode_response_partitions(response)
                .map_err(|error| Status::resource_exhausted(error.to_string()));
            if encoded.is_ok() {
                frames.response_serialized(&telemetry, elapsed_micros(serialization_started));
            } else {
                frames.terminal_egressed(&telemetry, elapsed_micros(serialization_started), false);
            }
            Some(encoded.map(|envelopes| EncodedClientResponse {
                envelopes,
                telemetry,
            }))
        }
        Ok(None) => None,
        Err(error) => Some(Err(Status::failed_precondition(error))),
    }
}

struct EncodedClientResponse {
    envelopes: Vec<ClientFrameEnvelope>,
    telemetry: crate::AspClientResponseTelemetry,
}

fn response_telemetry(frame: &ClientFrame) -> Option<crate::AspClientResponseTelemetry> {
    let ClientFrame::Response {
        base,
        request_id,
        outcome,
        ..
    } = frame
    else {
        return None;
    };
    Some(crate::AspClientResponseTelemetry {
        project_id: base.project_id.clone(),
        workspace_id: base.workspace_id.clone(),
        session_id: base.session_id.clone(),
        request_id: request_id.clone(),
        outcome: *outcome,
    })
}

fn elapsed_micros(started: tokio::time::Instant) -> u64 {
    started.elapsed().as_micros().min(u128::from(u64::MAX)) as u64
}

fn encode_response_partitions(frame: ClientFrame) -> Result<Vec<ClientFrameEnvelope>, String> {
    let request_id = frame_request_id(&frame)
        .ok_or_else(|| "partitioned ClientFrame response requires requestId".to_owned())?
        .as_str()
        .to_owned();
    let envelope = encode_frame(frame)?;
    let encoded = envelope.encode_to_vec();
    if encoded.len() <= CLIENT_FRAME_PARTITION_BYTES {
        return Ok(vec![envelope]);
    }
    if encoded.len() > CLIENT_FRAME_LOGICAL_RESPONSE_BYTES {
        return Err(format!(
            "reasonKind=client-frame-logical-response-too-large encodedBytes={} limitBytes={}",
            encoded.len(),
            CLIENT_FRAME_LOGICAL_RESPONSE_BYTES,
        ));
    }
    let base = envelope
        .base
        .clone()
        .ok_or_else(|| "partitioned ClientFrame response requires base".to_owned())?;
    let response_digest = format!(
        "blake3-256:{}",
        agent_semantic_content_identity::exact_selector_merkle::blake3_content_digest_v1(&encoded)
            .as_str()
    );
    let partition_count = encoded.len().div_ceil(CLIENT_FRAME_PARTITION_BYTES);
    let partition_count = u32::try_from(partition_count)
        .map_err(|_| "ClientFrame response partition count exceeds u32".to_owned())?;
    encoded
        .chunks(CLIENT_FRAME_PARTITION_BYTES)
        .enumerate()
        .map(|(partition_index, bytes)| {
            Ok(ClientFrameEnvelope {
                base: Some(base.clone()),
                frame: Some(
                    super::generated::client_frame_envelope::Frame::ResponsePartition(
                        super::generated::ResponsePartitionFrame {
                            request_id: request_id.clone(),
                            partition_index: u32::try_from(partition_index).map_err(|_| {
                                "ClientFrame response partition index exceeds u32".to_owned()
                            })?,
                            partition_count,
                            response_digest: response_digest.clone(),
                            encoded_response: bytes.to_vec(),
                        },
                    ),
                ),
            })
        })
        .collect()
}

pub async fn bind_asp_client_grpc_tcp() -> Result<tokio::net::TcpListener, String> {
    tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
        .await
        .map_err(|error| format!("failed to bind ASP Client Protocol loopback gRPC: {error}"))
}

pub async fn serve_asp_client_grpc_tcp<D: AspClientDispatcher>(
    listener: tokio::net::TcpListener,
    service: Arc<AspClientFrameService<D>>,
    mut shutdown: watch::Receiver<bool>,
) -> Result<(), String> {
    tonic::transport::Server::builder()
        .add_service(AspClientProtocolServer::new(AspClientGrpcService::new(
            service,
        )))
        .serve_with_incoming_shutdown(
            tokio_stream::wrappers::TcpListenerStream::new(listener),
            async move {
                while !*shutdown.borrow() {
                    if shutdown.changed().await.is_err() {
                        break;
                    }
                }
            },
        )
        .await
        .map_err(|error| format!("ASP Client Protocol loopback gRPC server failed: {error}"))
}

type PendingResponse = oneshot::Sender<Result<ClientFrame, String>>;

/// One in-flight ClientFrame call whose request identity remains registered
/// until a typed terminal arrives, the bounded wait expires, or the handle is
/// dropped.  Keeping registration separate from waiting lets callers send the
/// protocol `Cancel` frame without creating a second response authority.
pub struct AspClientPendingCall {
    request_id: ClientRequestId,
    receiver: Option<oneshot::Receiver<Result<ClientFrame, String>>>,
    pending: Arc<Mutex<HashMap<ClientRequestId, PendingResponse>>>,
    response_budget: Option<std::time::Duration>,
}

impl AspClientPendingCall {
    #[must_use]
    pub fn request_id(&self) -> &ClientRequestId {
        &self.request_id
    }

    #[must_use]
    pub fn has_response_deadline(&self) -> bool {
        self.response_budget.is_some()
    }

    pub async fn wait(mut self) -> Result<ClientFrame, String> {
        let receiver = self
            .receiver
            .take()
            .expect("pending ClientFrame call receiver must exist");
        let response = match self.response_budget {
            Some(budget) => match tokio::time::timeout(budget, receiver).await {
                Ok(response) => response,
                Err(_) => {
                    self.pending.lock().remove(&self.request_id);
                    return Err(format!(
                        "reasonKind=runtime-client-response-deadline-exceeded requestId={} budgetMs={}",
                        self.request_id.as_str(),
                        budget.as_millis(),
                    ));
                }
            },
            None => receiver.await,
        };
        response.map_err(|_| "ASP Client Protocol response channel closed".to_owned())?
    }
}

fn response_budget_for_frame(frame: &ClientFrame) -> Option<std::time::Duration> {
    match frame {
        ClientFrame::Request { method, .. }
            if classify_client_dispatch(method) == ClientDispatchClass::ColdGenerationAdmission =>
        {
            None
        }
        _ => Some(CLIENT_FRAME_RESPONSE_BUDGET),
    }
}

impl Drop for AspClientPendingCall {
    fn drop(&mut self) {
        if self.receiver.is_some() {
            self.pending.lock().remove(&self.request_id);
        }
    }
}

struct GrpcResponseStream {
    receiver: ReceiverStream<Result<ClientFrameEnvelope, Status>>,
    supervisor: tokio::task::JoinHandle<()>,
}

impl Stream for GrpcResponseStream {
    type Item = Result<ClientFrameEnvelope, Status>;

    fn poll_next(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.receiver).poll_next(context)
    }
}

impl Drop for GrpcResponseStream {
    fn drop(&mut self) {
        self.supervisor.abort();
    }
}

struct ClientResponseReader(tokio::task::JoinHandle<()>);

impl Drop for ClientResponseReader {
    fn drop(&mut self) {
        self.0.abort();
    }
}

struct ResponsePartitionAssembly {
    partition_count: u32,
    response_digest: String,
    encoded_response: Vec<u8>,
    next_partition_index: u32,
}

impl ResponsePartitionAssembly {
    fn new(partition: &super::generated::ResponsePartitionFrame) -> Result<Self, String> {
        if partition.partition_count == 0 {
            return Err("ClientFrame response partition_count must be nonzero".to_owned());
        }
        if partition.partition_index != 0 {
            return Err("ClientFrame response partitions must begin at index zero".to_owned());
        }
        Ok(Self {
            partition_count: partition.partition_count,
            response_digest: partition.response_digest.clone(),
            encoded_response: Vec::new(),
            next_partition_index: 0,
        })
    }

    fn push(
        &mut self,
        partition: super::generated::ResponsePartitionFrame,
    ) -> Result<Option<ClientFrame>, String> {
        if partition.partition_count != self.partition_count
            || partition.response_digest != self.response_digest
        {
            return Err("ClientFrame response partition identity changed".to_owned());
        }
        if partition.partition_index != self.next_partition_index {
            return Err(format!(
                "ClientFrame response partition out of order: expected={} actual={}",
                self.next_partition_index, partition.partition_index,
            ));
        }
        if partition.encoded_response.is_empty()
            || partition.encoded_response.len() > CLIENT_FRAME_PARTITION_BYTES
        {
            return Err(format!(
                "ClientFrame response partition byte budget violated: bytes={} limit={}",
                partition.encoded_response.len(),
                CLIENT_FRAME_PARTITION_BYTES,
            ));
        }
        let next_len = self
            .encoded_response
            .len()
            .checked_add(partition.encoded_response.len())
            .ok_or_else(|| "ClientFrame response byte count overflow".to_owned())?;
        if next_len > CLIENT_FRAME_LOGICAL_RESPONSE_BYTES {
            return Err(format!(
                "ClientFrame logical response byte budget violated: bytes={next_len} limit={CLIENT_FRAME_LOGICAL_RESPONSE_BYTES}"
            ));
        }
        self.encoded_response
            .extend_from_slice(&partition.encoded_response);
        self.next_partition_index += 1;
        if self.next_partition_index != self.partition_count {
            return Ok(None);
        }
        let actual_digest = format!(
            "blake3-256:{}",
            agent_semantic_content_identity::exact_selector_merkle::blake3_content_digest_v1(
                &self.encoded_response,
            )
            .as_str()
        );
        if actual_digest != self.response_digest {
            return Err(format!(
                "ClientFrame response digest mismatch: expected={} actual={actual_digest}",
                self.response_digest,
            ));
        }
        let envelope = ClientFrameEnvelope::decode(self.encoded_response.as_slice())
            .map_err(|error| format!("decode partitioned ASP ClientFrame response: {error}"))?;
        decode_frame(envelope)
            .map(Some)
            .map_err(|error| format!("decode partitioned ASP ClientFrame response: {error}"))
    }
}

/// Cloneable, bounded, request-id multiplexed transport for one public ASP
/// Client Protocol bidirectional stream.
#[derive(Clone)]
pub struct AspClientGrpcTransport {
    outbound: mpsc::Sender<ClientFrameEnvelope>,
    pending: Arc<Mutex<HashMap<ClientRequestId, PendingResponse>>>,
    _response_reader: Arc<ClientResponseReader>,
}

impl AspClientGrpcTransport {
    #[must_use]
    pub fn pending_call_count(&self) -> usize {
        self.pending.lock().len()
    }

    pub async fn connect_tcp(address: std::net::SocketAddr) -> Result<Self, String> {
        if !address.ip().is_loopback() || address.port() == 0 {
            return Err("ASP Client Protocol requires a nonzero loopback endpoint".to_owned());
        }
        let endpoint = tonic::transport::Endpoint::from_shared(format!("http://{address}"))
            .map_err(|error| error.to_string())?;
        let channel = tokio::time::timeout(CLIENT_SESSION_CONNECT_BUDGET, endpoint.connect())
            .await
            .map_err(|_| {
                format!(
                    "reasonKind=runtime-client-connect-deadline-exceeded budgetMs={} retryAdmitted=false",
                    CLIENT_SESSION_CONNECT_BUDGET.as_millis()
                )
            })?
            .map_err(|error| {
                format!(
                    "reasonKind=transport-unavailable bindingKind=loopback-tcp-http2 retryAdmitted=false error={error}"
                )
            })?;
        Self::connect_channel(channel).await
    }

    #[cfg(unix)]
    pub async fn connect_inherited_descriptor(
        descriptor: std::os::fd::OwnedFd,
    ) -> Result<Self, String> {
        let channel = connect_asp_client_grpc_inherited_descriptor(descriptor).await?;
        Self::connect_channel(channel).await
    }

    async fn connect_channel(channel: tonic::transport::Channel) -> Result<Self, String> {
        let mut client = AspClientProtocolClient::new(channel);
        let (outbound, receiver) = mpsc::channel(CLIENT_FRAME_TRANSPORT_CAPACITY);
        let mut inbound = tokio::time::timeout(
            CLIENT_SESSION_CONNECT_BUDGET,
            client.session(Request::new(ReceiverStream::new(receiver))),
        )
        .await
        .map_err(|_| {
            format!(
                "reasonKind=runtime-client-session-deadline-exceeded budgetMs={} retryAdmitted=false",
                CLIENT_SESSION_CONNECT_BUDGET.as_millis()
            )
        })?
        .map_err(|error| error.to_string())?
        .into_inner();
        let pending: Arc<Mutex<HashMap<ClientRequestId, PendingResponse>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let response_pending = Arc::clone(&pending);
        let response_reader = tokio::spawn(async move {
            let mut assemblies: HashMap<ClientRequestId, ResponsePartitionAssembly> =
                HashMap::new();
            loop {
                let (request_id, response) = match inbound.message().await {
                    Ok(Some(envelope)) => match envelope.frame {
                        Some(
                            super::generated::client_frame_envelope::Frame::ResponsePartition(
                                partition,
                            ),
                        ) => {
                            let request_id = match ClientRequestId::new(&partition.request_id) {
                                Ok(request_id) => request_id,
                                Err(error) => {
                                    let message = format!(
                                        "decode ASP ClientFrame response partition requestId: {error}"
                                    );
                                    let pending = std::mem::take(&mut *response_pending.lock());
                                    for (_, sender) in pending {
                                        let _ = sender.send(Err(message.clone()));
                                    }
                                    break;
                                }
                            };
                            let result = if let Some(assembly) = assemblies.get_mut(&request_id) {
                                assembly.push(partition)
                            } else {
                                match ResponsePartitionAssembly::new(&partition) {
                                    Ok(mut assembly) => {
                                        let result = assembly.push(partition);
                                        assemblies.insert(request_id.clone(), assembly);
                                        result
                                    }
                                    Err(error) => Err(error),
                                }
                            };
                            match result {
                                Ok(Some(frame)) => {
                                    assemblies.remove(&request_id);
                                    (Some(request_id), Ok(frame))
                                }
                                Ok(None) => continue,
                                Err(error) => {
                                    assemblies.remove(&request_id);
                                    (Some(request_id), Err(error))
                                }
                            }
                        }
                        _ => {
                            let response = decode_frame(envelope).map_err(|error| {
                                format!("decode ASP ClientFrame response: {error}")
                            });
                            let request_id =
                                response.as_ref().ok().and_then(frame_request_id).cloned();
                            (request_id, response)
                        }
                    },
                    Ok(None) => (
                        None,
                        Err("ASP Client Protocol gRPC stream closed".to_owned()),
                    ),
                    Err(error) => (None, Err(error.to_string())),
                };
                if let Some(request_id) = request_id {
                    if let Some(sender) = response_pending.lock().remove(&request_id) {
                        let _ = sender.send(response);
                    }
                    continue;
                }
                let message = response
                    .err()
                    .unwrap_or_else(|| "ASP Client Protocol response has no requestId".to_owned());
                let pending = std::mem::take(&mut *response_pending.lock());
                for (_, sender) in pending {
                    let _ = sender.send(Err(message.clone()));
                }
                break;
            }
        });
        Ok(Self {
            outbound,
            pending,
            _response_reader: Arc::new(ClientResponseReader(response_reader)),
        })
    }

    pub async fn call(&self, frame: ClientFrame) -> Result<ClientFrame, String> {
        self.begin_call(frame).await?.wait().await
    }

    /// Register and send one call without waiting for its terminal response.
    /// This is the sole supported way to retain a request while a correlated
    /// cancellation frame is sent on the same bounded gRPC session.
    pub async fn begin_call(&self, frame: ClientFrame) -> Result<AspClientPendingCall, String> {
        let response_budget = response_budget_for_frame(&frame);
        let request_id = frame_request_id(&frame)
            .cloned()
            .ok_or_else(|| "ASP Client Protocol call frame requires requestId".to_owned())?;
        let envelope = encode_frame(frame)
            .map_err(|error| format!("encode ASP ClientFrame request: {error}"))?;
        let (sender, receiver) = oneshot::channel();
        {
            let mut pending = self.pending.lock();
            if pending.len() >= CLIENT_FRAME_SESSION_CAPACITY {
                return Err(format!(
                    "reasonKind=client-session-backpressure capacity={} pending={} reservedControlSlots={} retryAdmitted=false",
                    CLIENT_FRAME_SESSION_CAPACITY,
                    pending.len(),
                    CLIENT_FRAME_SESSION_CONTROL_RESERVE,
                ));
            }
            match pending.entry(request_id.clone()) {
                std::collections::hash_map::Entry::Vacant(entry) => {
                    entry.insert(sender);
                }
                std::collections::hash_map::Entry::Occupied(_) => {
                    return Err(format!(
                        "ASP Client Protocol requestId is already pending: {}",
                        request_id.as_str()
                    ));
                }
            }
        }
        if self.outbound.send(envelope).await.is_err() {
            self.pending.lock().remove(&request_id);
            return Err("ASP Client Protocol gRPC stream is closed".to_owned());
        }
        Ok(AspClientPendingCall {
            request_id,
            receiver: Some(receiver),
            pending: Arc::clone(&self.pending),
            response_budget,
        })
    }

    /// Send a correlated cancellation frame for an already registered call.
    /// Its typed `Cancelled` response completes the original pending handle;
    /// no second pending entry or retry is created.
    pub async fn cancel_pending(
        &self,
        base: agent_semantic_client_protocol::ClientFrameBase,
        request_id: ClientRequestId,
    ) -> Result<(), String> {
        if !self.pending.lock().contains_key(&request_id) {
            return Err(format!(
                "ASP Client Protocol cancellation requires a pending requestId: {}",
                request_id.as_str()
            ));
        }
        let envelope = encode_frame(ClientFrame::Cancel { base, request_id })
            .map_err(|error| format!("encode ASP ClientFrame cancellation: {error}"))?;
        self.outbound
            .send(envelope)
            .await
            .map_err(|_| "ASP Client Protocol gRPC stream is closed".to_owned())
    }

    /// Send a protocol frame that intentionally has no terminal response.
    /// Correlated frames must use `call`, `begin_call`, or `cancel_pending` so
    /// their exactly-one response ownership cannot be lost.
    pub async fn send_oneway(&self, frame: ClientFrame) -> Result<(), String> {
        if let Some(request_id) = frame_request_id(&frame) {
            return Err(format!(
                "ASP Client Protocol one-way frame cannot carry requestId: {}",
                request_id.as_str()
            ));
        }
        let envelope = encode_frame(frame)
            .map_err(|error| format!("encode ASP ClientFrame one-way request: {error}"))?;
        self.outbound
            .send(envelope)
            .await
            .map_err(|_| "ASP Client Protocol gRPC stream is closed".to_owned())
    }
}

fn frame_request_id(frame: &ClientFrame) -> Option<&ClientRequestId> {
    match frame {
        ClientFrame::Initialize { request_id, .. }
        | ClientFrame::Request { request_id, .. }
        | ClientFrame::Cancel { request_id, .. }
        | ClientFrame::Shutdown { request_id, .. }
        | ClientFrame::Response { request_id, .. } => Some(request_id),
        ClientFrame::Exit { .. } | ClientFrame::Event { .. } => None,
    }
}

#[cfg(test)]
#[path = "../../tests/unit/grpc_transport.rs"]
mod tests;
