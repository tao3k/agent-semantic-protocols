use agent_semantic_provider_protocol::{ProviderRegisterRequest, ProviderRegisterResponse};

pub async fn call_runtime_provider_register(
    endpoint: &crate::runtime_server_control::RuntimeServerEndpoint,
    request: &ProviderRegisterRequest,
) -> Result<ProviderRegisterResponse, String> {
    endpoint.validate()?;
    request.validate()?;
    let mut stream = tokio::net::UnixStream::connect(&endpoint.provider_plane_socket_path)
        .await
        .map_err(|error| format!("failed to connect Runtime Server provider plane: {error}"))?;
    crate::workspace_db_ipc::write_frame(&mut stream, request).await?;
    let response: ProviderRegisterResponse =
        crate::workspace_db_ipc::read_frame(&mut stream).await?;
    response.validate()?;
    Ok(response)
}
