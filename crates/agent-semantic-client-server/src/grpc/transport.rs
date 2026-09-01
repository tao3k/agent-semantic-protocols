//! Unix gRPC transport for multiplexed public ASP `ClientFrame` sessions.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use agent_semantic_client_protocol::{ClientFrame, ClientRequestId};
use futures_util::{StreamExt as FuturesStreamExt, stream::FuturesUnordered};
use parking_lot::Mutex;
use tokio::sync::{mpsc, oneshot, watch};
use tokio_stream::{Stream, wrappers::ReceiverStream};
use tonic::{Request, Response, Status, Streaming};

use crate::{AspClientDispatcher, AspClientFrameService};

use super::generated::{
    ClientFrameEnvelope,
    asp_client_protocol_client::AspClientProtocolClient,
    asp_client_protocol_server::{AspClientProtocol, AspClientProtocolServer},
};
use super::wire::{decode_frame, encode_frame};

const CLIENT_FRAME_RESPONSE_BUDGET: std::time::Duration = std::time::Duration::from_secs(10);
const CLIENT_SESSION_CONNECT_BUDGET: std::time::Duration = std::time::Duration::from_secs(2);
pub const CLIENT_FRAME_SESSION_CAPACITY: usize = 32;
pub const CLIENT_FRAME_SESSION_CONTROL_RESERVE: usize = 1;
const CLIENT_FRAME_TRANSPORT_CAPACITY: usize =
    CLIENT_FRAME_SESSION_CAPACITY + CLIENT_FRAME_SESSION_CONTROL_RESERVE;

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
                        if let Some(Some(result)) = completed
                            && outbound.send(result).await.is_err()
                        {
                            break;
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
) -> Option<Result<ClientFrameEnvelope, Status>> {
    match frames.handle_frame(frame).await {
        Ok(Some(response)) => {
            Some(encode_frame(response).map_err(|error| Status::internal(error.to_string())))
        }
        Ok(None) => None,
        Err(error) => Some(Err(Status::failed_precondition(error))),
    }
}

pub async fn bind_asp_client_grpc_unix(
    socket_path: &Path,
) -> Result<tokio::net::UnixListener, String> {
    tokio::net::UnixListener::bind(socket_path).map_err(|error| {
        format!(
            "failed to bind ASP Client Protocol gRPC socket {}: {error}",
            socket_path.display()
        )
    })
}

pub async fn serve_asp_client_grpc_unix<D: AspClientDispatcher>(
    listener: tokio::net::UnixListener,
    service: Arc<AspClientFrameService<D>>,
    mut shutdown: watch::Receiver<bool>,
) -> Result<(), String> {
    tonic::transport::Server::builder()
        .add_service(AspClientProtocolServer::new(AspClientGrpcService::new(
            service,
        )))
        .serve_with_incoming_shutdown(
            tokio_stream::wrappers::UnixListenerStream::new(listener),
            async move {
                while !*shutdown.borrow() {
                    if shutdown.changed().await.is_err() {
                        break;
                    }
                }
            },
        )
        .await
        .map_err(|error| format!("ASP Client Protocol gRPC server failed: {error}"))
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
}

impl AspClientPendingCall {
    #[must_use]
    pub fn request_id(&self) -> &ClientRequestId {
        &self.request_id
    }

    pub async fn wait(mut self) -> Result<ClientFrame, String> {
        let receiver = self
            .receiver
            .take()
            .expect("pending ClientFrame call receiver must exist");
        match tokio::time::timeout(CLIENT_FRAME_RESPONSE_BUDGET, receiver).await {
            Ok(response) => {
                response.map_err(|_| "ASP Client Protocol response channel closed".to_owned())?
            }
            Err(_) => {
                self.pending.lock().remove(&self.request_id);
                Err(format!(
                    "reasonKind=runtime-client-response-deadline-exceeded requestId={} budgetMs={}",
                    self.request_id.as_str(),
                    CLIENT_FRAME_RESPONSE_BUDGET.as_millis(),
                ))
            }
        }
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

    pub async fn connect_unix(socket_path: impl Into<PathBuf>) -> Result<Self, String> {
        let channel = connect_unix_channel(socket_path.into()).await?;
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
            loop {
                let response = match inbound.message().await {
                    Ok(Some(envelope)) => decode_frame(envelope)
                        .map_err(|error| format!("decode ASP ClientFrame response: {error}")),
                    Ok(None) => Err("ASP Client Protocol gRPC stream closed".to_owned()),
                    Err(error) => Err(error.to_string()),
                };
                let request_id = response.as_ref().ok().and_then(frame_request_id).cloned();
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
        | ClientFrame::Dispatch { request_id, .. }
        | ClientFrame::Request { request_id, .. }
        | ClientFrame::Cancel { request_id, .. }
        | ClientFrame::Shutdown { request_id, .. }
        | ClientFrame::Response { request_id, .. } => Some(request_id),
        ClientFrame::Exit { .. } | ClientFrame::Event { .. } => None,
    }
}

async fn connect_unix_channel(socket_path: PathBuf) -> Result<tonic::transport::Channel, String> {
    let endpoint = tonic::transport::Endpoint::try_from("http://[::]:50051")
        .map_err(|error| error.to_string())?;
    let connection = endpoint.connect_with_connector(tower::service_fn(move |_| {
        let socket_path = socket_path.clone();
        async move {
            tokio::net::UnixStream::connect(socket_path)
                .await
                .map(hyper_util::rt::TokioIo::new)
        }
    }));
    tokio::time::timeout(CLIENT_SESSION_CONNECT_BUDGET, connection)
        .await
        .map_err(|_| {
            format!(
                "reasonKind=runtime-client-connect-deadline-exceeded budgetMs={} retryAdmitted=false",
                CLIENT_SESSION_CONNECT_BUDGET.as_millis()
            )
        })?
        .map_err(|error| error.to_string())
}
