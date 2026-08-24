pub mod generated {
    tonic::include_proto!("asp.provider.stream");
}
use generated::{
    ProviderRegisterPacket, ProviderStreamEnvelope, provider_session_client::ProviderSessionClient,
};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

/// Owned long-lived provider session; callers retain the bounded sender and
/// explicitly drop it to drain the bidi stream.
pub struct GrpcProviderSessionClient {
    outbound: mpsc::Sender<ProviderStreamEnvelope>,
    inbound: tonic::Streaming<ProviderStreamEnvelope>,
}

impl GrpcProviderSessionClient {
    pub async fn connect_unix(
        socket_path: impl Into<std::path::PathBuf>,
    ) -> Result<Self, tonic::Status> {
        let channel = connect_unix_channel(socket_path.into()).await?;
        let mut client = ProviderSessionClient::new(channel);
        let (outbound, receiver) = mpsc::channel(32);
        let response = client
            .session(tonic::Request::new(ReceiverStream::new(receiver)))
            .await?;
        Ok(Self {
            outbound,
            inbound: response.into_inner(),
        })
    }

    pub async fn send(&self, envelope: ProviderStreamEnvelope) -> Result<(), tonic::Status> {
        self.outbound
            .send(envelope)
            .await
            .map_err(|_| tonic::Status::cancelled("provider session is closed"))
    }

    pub async fn recv(&mut self) -> Result<Option<ProviderStreamEnvelope>, tonic::Status> {
        self.inbound.message().await
    }
}

pub async fn call_runtime_provider_register(
    socket_path: impl Into<std::path::PathBuf>,
    request: &agent_semantic_provider_protocol::ProviderRegisterRequest,
) -> Result<agent_semantic_provider_protocol::ProviderRegisterResponse, String> {
    request.validate()?;
    let channel = connect_unix_channel(socket_path.into())
        .await
        .map_err(|status| status.to_string())?;
    let mut client = ProviderSessionClient::new(channel);
    let packet = ProviderRegisterPacket {
        schema_id: agent_semantic_provider_protocol::PROVIDER_REGISTER_REQUEST_SCHEMA_ID.to_owned(),
        schema_version: agent_semantic_provider_protocol::PROVIDER_REGISTER_SCHEMA_VERSION
            .to_owned(),
        payload: serde_json::to_vec(request)
            .map_err(|error| format!("encode provider register request: {error}"))?,
    };
    let response = client
        .register(tonic::Request::new(packet))
        .await
        .map_err(|status| status.to_string())?
        .into_inner();
    if response.schema_id != agent_semantic_provider_protocol::PROVIDER_REGISTER_RESPONSE_SCHEMA_ID
        || response.schema_version
            != agent_semantic_provider_protocol::PROVIDER_REGISTER_SCHEMA_VERSION
    {
        return Err("invalid provider register response packet schema identity".to_owned());
    }
    let response: agent_semantic_provider_protocol::ProviderRegisterResponse =
        serde_json::from_slice(&response.payload)
            .map_err(|error| format!("decode provider register response: {error}"))?;
    response.validate()?;
    Ok(response)
}

async fn connect_unix_channel(
    socket_path: std::path::PathBuf,
) -> Result<tonic::transport::Channel, tonic::Status> {
    tonic::transport::Endpoint::try_from("http://[::]:50051")
        .map_err(|error| tonic::Status::invalid_argument(error.to_string()))?
        .connect_with_connector(tower::service_fn(move |_| {
            let socket_path = socket_path.clone();
            async move {
                tokio::net::UnixStream::connect(socket_path)
                    .await
                    .map(hyper_util::rt::TokioIo::new)
            }
        }))
        .await
        .map_err(|error| tonic::Status::unavailable(error.to_string()))
}
