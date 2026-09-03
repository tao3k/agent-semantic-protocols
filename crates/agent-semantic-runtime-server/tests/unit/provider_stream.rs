use agent_semantic_provider_protocol::{PROVIDER_STREAM_SCHEMA_ID, PROVIDER_STREAM_SCHEMA_VERSION};
use agent_semantic_provider_transport::GrpcProviderSessionClient;
use agent_semantic_provider_transport::grpc_session::generated::ProviderStreamEnvelope;

fn envelope(kind: &str, sequence: u64) -> ProviderStreamEnvelope {
    ProviderStreamEnvelope {
        schema_id: PROVIDER_STREAM_SCHEMA_ID.to_owned(),
        schema_version: PROVIDER_STREAM_SCHEMA_VERSION.to_owned(),
        session_id: "session-g1".to_owned(),
        request_id: format!("request-{sequence}"),
        workspace_identity: "workspace-g1".to_owned(),
        generation_digest: "generation-g1".to_owned(),
        provider_id: "provider-g1".to_owned(),
        language_id: "rust".to_owned(),
        sequence,
        kind: kind.to_owned(),
        payload_schema_id: "provider-stream.payload".to_owned(),
        payload: kind.as_bytes().to_vec(),
    }
}

async fn provider_register(
    directory: &tempfile::TempDir,
) -> std::sync::Arc<agent_semantic_client_db::runtime_provider_register::RuntimeProviderRegister> {
    std::sync::Arc::new(
        agent_semantic_client_db::runtime_provider_register::RuntimeProviderRegister::from_seed_with_store(
            agent_semantic_provider_protocol::builtin_provider_registrations().unwrap(),
            directory.path().join("provider-register.json"),
        )
        .await
        .unwrap(),
    )
}

#[tokio::test]
async fn provider_stream_register_activate_facts_ready_cancel_drain_is_one_bidi_stream() {
    let directory = tempfile::tempdir().unwrap();
    let provider_register = provider_register(&directory).await;
    let listener = agent_semantic_runtime_server::bind_provider_stream_tcp()
        .await
        .unwrap();
    let endpoint = listener.local_addr().unwrap();
    let (shutdown, shutdown_rx) = tokio::sync::watch::channel(false);
    let server = tokio::spawn(async move {
        agent_semantic_runtime_server::serve_provider_stream_tcp(
            listener,
            provider_register,
            shutdown_rx,
        )
        .await
        .unwrap();
    });

    let mut client = GrpcProviderSessionClient::connect_tcp(endpoint)
        .await
        .unwrap();
    for (sequence, kind) in [
        (1, "Register"),
        (2, "Activate"),
        (3, "Facts"),
        (4, "Ready"),
        (5, "Cancel"),
        (6, "Drain"),
    ] {
        client.send(envelope(kind, sequence)).await.unwrap();
        let response = client.recv().await.unwrap().unwrap();
        assert_eq!(response.kind, kind);
        assert_eq!(response.sequence, sequence);
        assert_eq!(response.session_id, "session-g1");
        assert_eq!(response.workspace_identity, "workspace-g1");
        assert_eq!(response.generation_digest, "generation-g1");
    }
    drop(client);
    shutdown.send(true).unwrap();
    server.await.unwrap();
}

#[tokio::test]
async fn provider_stream_rejects_invalid_schema_version() {
    let directory = tempfile::tempdir().unwrap();
    let provider_register = provider_register(&directory).await;
    let listener = agent_semantic_runtime_server::bind_provider_stream_tcp()
        .await
        .unwrap();
    let endpoint = listener.local_addr().unwrap();
    let (shutdown, shutdown_rx) = tokio::sync::watch::channel(false);
    let server = tokio::spawn(async move {
        agent_semantic_runtime_server::serve_provider_stream_tcp(
            listener,
            provider_register,
            shutdown_rx,
        )
        .await
        .unwrap();
    });
    let mut client = GrpcProviderSessionClient::connect_tcp(endpoint)
        .await
        .unwrap();
    let mut invalid = envelope("Register", 1);
    invalid.schema_version = "2".to_owned();
    client.send(invalid).await.unwrap();
    assert!(client.recv().await.is_err());
    drop(client);
    shutdown.send(true).unwrap();
    server.await.unwrap();
}

#[tokio::test]
async fn provider_register_uses_the_same_grpc_provider_plane() {
    let directory = tempfile::tempdir().unwrap();
    let provider_register = provider_register(&directory).await;
    let listener = agent_semantic_runtime_server::bind_provider_stream_tcp()
        .await
        .unwrap();
    let endpoint = listener.local_addr().unwrap();
    let (shutdown, shutdown_rx) = tokio::sync::watch::channel(false);
    let server = tokio::spawn(async move {
        agent_semantic_runtime_server::serve_provider_stream_tcp(
            listener,
            provider_register,
            shutdown_rx,
        )
        .await
        .unwrap();
    });
    let response =
        agent_semantic_provider_transport::grpc_session::call_runtime_provider_register_tcp(
            endpoint,
            &agent_semantic_provider_protocol::ProviderRegisterRequest {
                schema_id: agent_semantic_provider_protocol::PROVIDER_REGISTER_REQUEST_SCHEMA_ID
                    .to_owned(),
                schema_version: agent_semantic_provider_protocol::PROVIDER_REGISTER_SCHEMA_VERSION
                    .to_owned(),
                expected_generation: None,
                request: agent_semantic_provider_protocol::ProviderRegisterOperation::List,
            },
        )
        .await
        .unwrap();
    response.validate().unwrap();
    assert!(matches!(
        response.result,
        agent_semantic_provider_protocol::ProviderRegisterResult::Snapshot { .. }
    ));
    shutdown.send(true).unwrap();
    server.await.unwrap();
}
