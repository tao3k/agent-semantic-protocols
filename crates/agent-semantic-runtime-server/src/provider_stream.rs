use agent_semantic_provider_protocol::validate_provider_stream_envelope;
use tokio_stream::{Stream, StreamExt};
use tonic::{Request, Response, Status, Streaming};

pub mod generated {
    tonic::include_proto!("asp.provider.stream");
}
use generated::{
    ProviderRegisterPacket, ProviderStreamEnvelope, provider_session_server::ProviderSession,
};

pub struct RuntimeStreamService {
    provider_register: std::sync::Arc<
        agent_semantic_client_db::runtime_provider_register::RuntimeProviderRegister,
    >,
}

impl RuntimeStreamService {
    pub fn new(
        provider_register: std::sync::Arc<
            agent_semantic_client_db::runtime_provider_register::RuntimeProviderRegister,
        >,
    ) -> Self {
        Self { provider_register }
    }
}

#[tonic::async_trait]
impl ProviderSession for RuntimeStreamService {
    type SessionStream =
        std::pin::Pin<Box<dyn Stream<Item = Result<ProviderStreamEnvelope, Status>> + Send>>;

    async fn register(
        &self,
        request: Request<ProviderRegisterPacket>,
    ) -> Result<Response<ProviderRegisterPacket>, Status> {
        let packet = request.into_inner();
        if packet.schema_id != agent_semantic_provider_protocol::PROVIDER_REGISTER_REQUEST_SCHEMA_ID
            || packet.schema_version
                != agent_semantic_provider_protocol::PROVIDER_REGISTER_SCHEMA_VERSION
        {
            return Err(Status::invalid_argument(
                "invalid provider register packet schema identity",
            ));
        }
        let request: agent_semantic_provider_protocol::ProviderRegisterRequest =
            serde_json::from_slice(&packet.payload)
                .map_err(|error| Status::invalid_argument(error.to_string()))?;
        request.validate().map_err(Status::invalid_argument)?;
        let response = self
            .provider_register
            .apply(request)
            .await
            .map_err(Status::failed_precondition)?;
        response.validate().map_err(Status::internal)?;
        let payload =
            serde_json::to_vec(&response).map_err(|error| Status::internal(error.to_string()))?;
        Ok(Response::new(ProviderRegisterPacket {
            schema_id: agent_semantic_provider_protocol::PROVIDER_REGISTER_RESPONSE_SCHEMA_ID
                .to_owned(),
            schema_version: agent_semantic_provider_protocol::PROVIDER_REGISTER_SCHEMA_VERSION
                .to_owned(),
            payload,
        }))
    }

    async fn session(
        &self,
        request: Request<Streaming<ProviderStreamEnvelope>>,
    ) -> Result<Response<Self::SessionStream>, Status> {
        let output = request.into_inner().map(|message| {
            let envelope = message?;
            validate_provider_stream_envelope(
                &envelope.schema_id,
                &envelope.schema_version,
                &envelope.session_id,
                &envelope.request_id,
                &envelope.workspace_identity,
                &envelope.generation_digest,
                &envelope.provider_id,
                &envelope.language_id,
                &envelope.kind,
                &envelope.payload_schema_id,
            )
            .map_err(Status::invalid_argument)?;
            Ok(envelope)
        });
        Ok(Response::new(Box::pin(output)))
    }
}

pub async fn bind_provider_stream_listener(
    socket_path: &std::path::Path,
) -> Result<tokio::net::UnixListener, String> {
    tokio::net::UnixListener::bind(socket_path).map_err(|error| {
        format!(
            "failed to bind Runtime Server provider stream {}: {error}",
            socket_path.display()
        )
    })
}

pub async fn serve_provider_stream(
    listener: tokio::net::UnixListener,
    provider_register: std::sync::Arc<
        agent_semantic_client_db::runtime_provider_register::RuntimeProviderRegister,
    >,
    mut shutdown: tokio::sync::watch::Receiver<bool>,
) -> Result<(), String> {
    tonic::transport::Server::builder()
        .add_service(
            generated::provider_session_server::ProviderSessionServer::new(
                RuntimeStreamService::new(provider_register),
            ),
        )
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
        .map_err(|error| format!("Runtime Server provider stream failed: {error}"))
}
