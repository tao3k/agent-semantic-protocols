//! Unix gRPC transport for multiplexed public ASP `ClientFrame` sessions.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use agent_semantic_client_protocol::{ClientFrame, ClientRequestId};
use tokio::sync::{Mutex, mpsc, oneshot, watch};
use tokio_stream::{Stream, StreamExt, wrappers::ReceiverStream};
use tonic::{Request, Response, Status, Streaming};

use crate::{AspClientDispatcher, AspClientFrameService};

use super::generated::{
    ClientFrameEnvelope,
    asp_client_protocol_client::AspClientProtocolClient,
    asp_client_protocol_server::{AspClientProtocol, AspClientProtocolServer},
};

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
        let (outbound, receiver) = mpsc::channel(32);
        let supervisor = tokio::spawn(async move {
            let mut requests = tokio::task::JoinSet::new();
            while let Some(envelope) = inbound.next().await {
                let envelope = match envelope {
                    Ok(envelope) => envelope,
                    Err(error) => {
                        let _ = outbound.send(Err(error)).await;
                        break;
                    }
                };
                let frame: ClientFrame = match serde_json::from_slice(&envelope.client_frame_json) {
                    Ok(frame) => frame,
                    Err(error) => {
                        let _ = outbound
                            .send(Err(Status::invalid_argument(format!(
                                "decode ASP ClientFrame: {error}"
                            ))))
                            .await;
                        continue;
                    }
                };
                let frames = Arc::clone(&frames);
                let outbound = outbound.clone();
                requests.spawn(async move {
                    let result = match frames.handle_frame(frame).await {
                        Ok(Some(response)) => serde_json::to_vec(&response)
                            .map(|client_frame_json| ClientFrameEnvelope { client_frame_json })
                            .map_err(|error| Status::internal(error.to_string())),
                        Ok(None) => return,
                        Err(error) => Err(Status::failed_precondition(error)),
                    };
                    let _ = outbound.send(result).await;
                });
            }
            while requests.join_next().await.is_some() {}
        });
        Ok(Response::new(Box::pin(GrpcResponseStream {
            receiver: ReceiverStream::new(receiver),
            supervisor,
        })))
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
    pub async fn connect_unix(socket_path: impl Into<PathBuf>) -> Result<Self, String> {
        let channel = connect_unix_channel(socket_path.into()).await?;
        let mut client = AspClientProtocolClient::new(channel);
        let (outbound, receiver) = mpsc::channel(32);
        let mut inbound = client
            .session(Request::new(ReceiverStream::new(receiver)))
            .await
            .map_err(|error| error.to_string())?
            .into_inner();
        let pending: Arc<Mutex<HashMap<ClientRequestId, PendingResponse>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let response_pending = Arc::clone(&pending);
        let response_reader = tokio::spawn(async move {
            loop {
                let response = match inbound.message().await {
                    Ok(Some(envelope)) => {
                        serde_json::from_slice::<ClientFrame>(&envelope.client_frame_json)
                            .map_err(|error| format!("decode ASP ClientFrame response: {error}"))
                    }
                    Ok(None) => Err("ASP Client Protocol gRPC stream closed".to_owned()),
                    Err(error) => Err(error.to_string()),
                };
                let request_id = response.as_ref().ok().and_then(frame_request_id).cloned();
                if let Some(request_id) = request_id {
                    if let Some(sender) = response_pending.lock().await.remove(&request_id) {
                        let _ = sender.send(response);
                    }
                    continue;
                }
                let message = response
                    .err()
                    .unwrap_or_else(|| "ASP Client Protocol response has no requestId".to_owned());
                let pending = std::mem::take(&mut *response_pending.lock().await);
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
        let request_id = frame_request_id(&frame)
            .cloned()
            .ok_or_else(|| "ASP Client Protocol call frame requires requestId".to_owned())?;
        let (sender, receiver) = oneshot::channel();
        if self
            .pending
            .lock()
            .await
            .insert(request_id.clone(), sender)
            .is_some()
        {
            return Err(format!(
                "ASP Client Protocol requestId is already pending: {}",
                request_id.as_str()
            ));
        }
        let client_frame_json = serde_json::to_vec(&frame)
            .map_err(|error| format!("encode ASP ClientFrame request: {error}"))?;
        if self
            .outbound
            .send(ClientFrameEnvelope { client_frame_json })
            .await
            .is_err()
        {
            self.pending.lock().await.remove(&request_id);
            return Err("ASP Client Protocol gRPC stream is closed".to_owned());
        }
        receiver
            .await
            .map_err(|_| "ASP Client Protocol response channel closed".to_owned())?
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
