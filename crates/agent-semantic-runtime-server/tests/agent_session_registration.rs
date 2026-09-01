use std::sync::Arc;

fn test_generation_admission()
-> Arc<agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationAdmission> {
    use agent_semantic_client_db::runtime_server_admission::{
        WorkspaceGenerationAdmission, WorkspaceGenerationBuildFailure,
        WorkspaceGenerationFailureStage,
    };

    Arc::new(WorkspaceGenerationAdmission::new(Arc::new(
        |_workspace_identity,
         _project_root,
         _candidate,
         _build_mode,
         _changed_paths,
         _provider_target,
         _cancellation| {
            Box::pin(async {
                Err(WorkspaceGenerationBuildFailure::new(
                    WorkspaceGenerationFailureStage::GenerationBuilder,
                    "test generation builder is intentionally unavailable",
                ))
            })
        },
    )))
}

use agent_semantic_client_protocol::{
    AGENT_SESSION_REGISTER_METHOD, AGENT_SESSION_REGISTER_REQUEST_SCHEMA_ID,
    AgentSessionRegisterReceipt, AgentSessionRegisterRequest, CLIENT_FRAME_SCHEMA_ID,
    CLIENT_PROTOCOL_ID, CLIENT_PROTOCOL_VERSION, ClientFrame, ClientFrameBase, ClientInfo,
    ClientRequestId, ClientSessionId, ClientWorkspaceIdentity, SCHEMA_VERSION,
};
use agent_semantic_client_server::{
    AspClientGrpcTransport, bind_asp_client_grpc_unix, serve_asp_client_grpc_unix,
};
use agent_semantic_schema_manager::SchemaManager;

fn digest(character: char) -> String {
    format!("blake3-256:{}", character.to_string().repeat(64))
}

fn registered_language_provider_pairs() -> Vec<(String, String)> {
    let workspace_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    SchemaManager::new(workspace_root)
        .registered_language_profiles()
        .expect("registered language profiles")
        .into_iter()
        .map(|profile| {
            let provider_id = format!("asp-{}", profile.language_id);
            (profile.language_id, provider_id)
        })
        .collect()
}

#[tokio::test]
async fn child_registration_uses_the_grpc_client_frame_and_runtime_registry_owner() {
    let directory = tempfile::tempdir().expect("temporary workspace");
    let project_root = directory.path().join("project");
    std::fs::create_dir_all(&project_root).expect("create project root");
    let workspace_identity =
        agent_semantic_client_db::AgentSessionRegistry::workspace_id(&project_root)
            .expect("workspace identity");
    agent_semantic_client_db::AgentSessionRegistry::mark_runtime_server_owner_process();
    let agent_session_registry = Arc::new(
        agent_semantic_client_db::AgentSessionRegistry::open_or_create_state_root_async(
            directory.path().join("agent-sessions"),
        )
        .await
        .expect("agent session registry"),
    );
    let (runtime_search_service, _runtime_search_requests) =
        agent_semantic_client_db::runtime_search_service::runtime_search_service_channel();
    let telemetry = agent_semantic_client_db::runtime_telemetry_bus::RuntimeTelemetryBus::new();
    let service = agent_semantic_runtime_server::build_frame_service(
        agent_semantic_runtime_server::RuntimeSchemaBundleCatalog::load(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."),
        )
        .await
        .expect("verified schema bundle catalog"),
        Arc::clone(&agent_session_registry),
        runtime_search_service,
        test_generation_admission(),
        digest('a'),
        Arc::from(registered_language_provider_pairs()),
        directory.path().join("workspace-store"),
        agent_semantic_runtime_server::query_generation::RuntimeQueryGenerationAuthority::new(),
        telemetry.sender,
    )
    .expect("frame service");
    let socket_dir = tempfile::tempdir().expect("client socket directory");
    let socket_path = socket_dir.path().join("asp-client.sock");
    let listener = bind_asp_client_grpc_unix(&socket_path)
        .await
        .expect("bind ASP Client gRPC socket");
    let (shutdown, shutdown_receiver) = tokio::sync::watch::channel(false);
    let server = tokio::spawn(serve_asp_client_grpc_unix(
        listener,
        service,
        shutdown_receiver,
    ));
    let client = AspClientGrpcTransport::connect_unix(&socket_path)
        .await
        .expect("connect ASP Client gRPC transport");
    let base = ClientFrameBase {
        schema_id: CLIENT_FRAME_SCHEMA_ID.to_owned(),
        schema_version: SCHEMA_VERSION.to_owned(),
        protocol_id: CLIENT_PROTOCOL_ID.to_owned(),
        protocol_version: CLIENT_PROTOCOL_VERSION.to_owned(),
        session_id: ClientSessionId::new("client-session").expect("session id"),
        workspace_identity: ClientWorkspaceIdentity::new(workspace_identity.clone())
            .expect("workspace identity"),
        trace_context: None,
    };
    let params = serde_json::to_value(AgentSessionRegisterRequest {
        schema_id: AGENT_SESSION_REGISTER_REQUEST_SCHEMA_ID.to_owned(),
        schema_version: 1,
        root_session_id: "root-1".to_owned(),
        parent_thread_id: "parent-1".to_owned(),
        child_thread_id: "child-1".to_owned(),
        agent_name: "asp_testing".to_owned(),
        agent_path: "/root/asp_testing".to_owned(),
        route_key: "asp_testing".to_owned(),
    })
    .expect("encode registration request");
    let response = client
        .call(ClientFrame::Dispatch {
            base,
            request_id: ClientRequestId::new("register-child").expect("request id"),
            project_root: project_root.to_string_lossy().into_owned(),
            client_info: ClientInfo {
                name: "runtime-test".to_owned(),
                version: "1".to_owned(),
            },
            method: AGENT_SESSION_REGISTER_METHOD.to_owned(),
            params,
        })
        .await
        .expect("registration response");
    let ClientFrame::Response {
        outcome: agent_semantic_client_protocol::ClientOutcome::Ready,
        result: Some(result),
        error: None,
        ..
    } = response
    else {
        panic!("registration must return a ready response: {response:?}");
    };
    let receipt: AgentSessionRegisterReceipt =
        serde_json::from_value(result).expect("decode registration receipt");
    receipt.validate().expect("valid registration receipt");
    assert_eq!(receipt.project_id, workspace_identity);
    assert_eq!(receipt.root_session_id, "root-1");
    assert_eq!(receipt.parent_thread_id, "parent-1");
    assert_eq!(receipt.child_thread_id, "child-1");
    assert_eq!(receipt.agent_path, "/root/asp_testing");
    assert_eq!(receipt.transport, "grpc-client-frame");
    let stored = agent_session_registry
        .session_by_id(&receipt.project_id, "child-1")
        .await
        .expect("query registered child")
        .expect("registered child exists");
    assert_eq!(stored.root_session_id.as_str(), "root-1");
    assert_eq!(stored.parent_session_id.as_deref(), Some("parent-1"));
    assert_eq!(
        stored.message_target_id.as_deref(),
        Some("/root/asp_testing")
    );
    drop(client);
    shutdown.send(true).expect("shutdown gRPC service");
    server.await.expect("server task").expect("server shutdown");
}
