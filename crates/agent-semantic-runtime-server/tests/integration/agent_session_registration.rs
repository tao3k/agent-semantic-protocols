// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Agent session registration integration tests.

use std::sync::Arc;

async fn test_generation_admission(
    project_root: &std::path::Path,
    workspace_id: &str,
) -> (
    Arc<agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationAdmission>,
    String,
) {
    use agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationAdmission;
    use agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationBuildFailure;
    use agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationFailureStage;

    let project_id = agent_semantic_client_core::state_core::ResolvedState::resolve(project_root)
        .expect("resolve fixture ProjectId")
        .repo
        .repo_id
        .to_string();
    let catalog =
        agent_semantic_client_db::runtime_server_admission_catalog::RuntimeWorkspaceAdmissionCatalog::load(
            project_root.parent().expect("fixture parent").join("workspace-admissions.v1.json"),
        )
        .await
        .expect("workspace admission catalog");
    catalog
        .record(
            agent_semantic_client_db::runtime_server_admission_catalog::RuntimeWorkspaceAdmissionCatalogEntry {
                project_id: project_id.clone(),
                workspace_identity: workspace_id.to_owned(),
                project_root: project_root.to_path_buf(),
            },
        )
        .await
        .expect("admit fixture project/workspace");
    let admission = WorkspaceGenerationAdmission::new(Arc::new(
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
    ))
    .with_catalog(catalog);
    (Arc::new(admission), project_id)
}

use agent_semantic_client_protocol::AGENT_SESSION_REGISTER_METHOD;
use agent_semantic_client_protocol::AGENT_SESSION_REGISTER_REQUEST_SCHEMA_ID;
use agent_semantic_client_protocol::AgentChildThreadId;
use agent_semantic_client_protocol::AgentName;
use agent_semantic_client_protocol::AgentParentThreadId;
use agent_semantic_client_protocol::AgentPath;
use agent_semantic_client_protocol::AgentRootSessionId;
use agent_semantic_client_protocol::AgentRouteKey;
use agent_semantic_client_protocol::AgentSessionRegisterReceipt;
use agent_semantic_client_protocol::AgentSessionRegisterRequest;
use agent_semantic_client_protocol::AgentSessionTransport;
use agent_semantic_client_protocol::ClientFrame;
use agent_semantic_client_protocol::ClientFrameBase;
use agent_semantic_client_protocol::ClientInfo;
use agent_semantic_client_protocol::ClientProjectId;
use agent_semantic_client_protocol::ClientRequestId;
use agent_semantic_client_protocol::ClientSchemaId;
use agent_semantic_client_protocol::ClientSessionId;
use agent_semantic_client_protocol::ClientWorkspaceIdentity;
use agent_semantic_client_protocol::protocol_identity::CLIENT_FRAME_SCHEMA_ID;
use agent_semantic_client_protocol::protocol_identity::CLIENT_PROTOCOL_ID;
use agent_semantic_client_protocol::protocol_identity::CLIENT_PROTOCOL_VERSION;
use agent_semantic_client_protocol::protocol_identity::SCHEMA_VERSION;
use agent_semantic_client_server::AspClientGrpcTransport;
use agent_semantic_client_server::bind_asp_client_grpc_tcp;
use agent_semantic_client_server::serve_asp_client_grpc_tcp;
use agent_semantic_schema_manager::SchemaManager;

fn digest(character: char) -> String {
    format!("blake3-256:{}", character.to_string().repeat(64))
}

fn test_project_workspace_resolver()
-> agent_semantic_runtime_server::HostWorkspaceInitializationBindingResolver {
    Arc::new(|_| {
        let project_workspace = agent_semantic_content_identity::ProjectWorkspaceBinding::new(
            "git+file:///runtime-test.git#workspace/main",
            ".",
            "local-only",
            Vec::new(),
        )
        .map_err(|error| error.to_string())?;
        agent_semantic_content_identity::HostWorkspaceInitializationBinding::new(
            project_workspace,
            "worktree-main",
        )
        .map_err(|error| error.to_string())
    })
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
    let (generation_admission, project_id) =
        test_generation_admission(&project_root, &workspace_identity).await;
    let workspace_registry = Arc::new(
        agent_semantic_client_db::runtime_server_workspace::RuntimeServerWorkspaceRegistry::new(
            directory.path().join("workspace-generations"),
        )
        .expect("workspace registry"),
    );
    let service = agent_semantic_runtime_server::build_frame_service(
        agent_semantic_runtime_server::RuntimeSchemaBundleCatalog::load(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."),
        )
        .await
        .expect("verified schema bundle catalog"),
        Arc::clone(&agent_session_registry),
        runtime_search_service,
        generation_admission,
        workspace_registry,
        digest('a'),
        Arc::from(registered_language_provider_pairs()),
        Arc::new(
            agent_semantic_client_db::runtime_provider_register::RuntimeProviderRegister::from_seed(
                Vec::new(),
            )
            .expect("empty provider register"),
        ),
        directory.path().join("workspace-store"),
        test_project_workspace_resolver(),
        agent_semantic_runtime_server::RuntimeQueryGenerationAuthority::new(),
        telemetry.sender,
    )
    .expect("frame service");
    let listener = bind_asp_client_grpc_tcp()
        .await
        .expect("bind ASP Client gRPC endpoint");
    let endpoint = listener.local_addr().expect("ASP Client endpoint");
    let (shutdown, shutdown_receiver) = tokio::sync::watch::channel(false);
    let server = tokio::spawn(serve_asp_client_grpc_tcp(
        listener,
        service,
        shutdown_receiver,
    ));
    let client = AspClientGrpcTransport::connect_tcp(endpoint)
        .await
        .expect("connect ASP Client gRPC transport");
    let base = ClientFrameBase {
        schema_id: CLIENT_FRAME_SCHEMA_ID.to_owned(),
        schema_version: SCHEMA_VERSION.to_owned(),
        protocol_id: CLIENT_PROTOCOL_ID.to_owned(),
        protocol_version: CLIENT_PROTOCOL_VERSION.to_owned(),
        session_id: ClientSessionId::new("client-session").expect("session id"),
        project_id: ClientProjectId::new(project_id.clone()).expect("project id"),
        workspace_id: ClientWorkspaceIdentity::new(workspace_identity.clone())
            .expect("workspace identity"),
        trace_context: None,
    };
    let params = serde_json::to_value(AgentSessionRegisterRequest {
        schema_id: ClientSchemaId::new(AGENT_SESSION_REGISTER_REQUEST_SCHEMA_ID)
            .expect("request schema id"),
        schema_version: 1,
        root_session_id: AgentRootSessionId::new("root-1").expect("root session id"),
        parent_thread_id: AgentParentThreadId::new("parent-1").expect("parent thread id"),
        child_thread_id: AgentChildThreadId::new("child-1").expect("child thread id"),
        agent_name: AgentName::new("asp_testing").expect("agent name"),
        agent_path: AgentPath::new("/root/asp_testing").expect("agent path"),
        route_key: AgentRouteKey::new("asp_testing").expect("route key"),
    })
    .expect("encode registration request");
    let initialized = client
        .call(ClientFrame::Initialize {
            base: base.clone(),
            request_id: ClientRequestId::new("initialize").expect("request id"),
            client_info: ClientInfo {
                name: "runtime-test".to_owned(),
                version: "1".to_owned(),
            },
            capabilities: serde_json::json!({}),
        })
        .await
        .expect("initialize response");
    let ClientFrame::Response {
        catalog: Some(catalog),
        ..
    } = initialized
    else {
        panic!("initialize must return the exact Runtime catalog")
    };
    let response = client
        .call(ClientFrame::Request {
            base,
            request_id: ClientRequestId::new("register-child").expect("request id"),
            catalog_generation: catalog.catalog_generation,
            workspace_generation: catalog.workspace_generation,
            method: AGENT_SESSION_REGISTER_METHOD.to_owned(),
            params,
            client_timing_witness: None,
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
    assert_eq!(receipt.project_id.as_str(), project_id);
    assert_eq!(receipt.root_session_id.as_str(), "root-1");
    assert_eq!(receipt.parent_thread_id.as_str(), "parent-1");
    assert_eq!(receipt.child_thread_id.as_str(), "child-1");
    assert_eq!(receipt.agent_path.as_str(), "/root/asp_testing");
    assert_eq!(receipt.transport, AgentSessionTransport::GrpcClientFrame);
    let stored = agent_session_registry
        .session_by_id(&project_id, "child-1")
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
