use std::sync::Arc;

use agent_semantic_client_protocol::{
    CLIENT_FRAME_SCHEMA_ID, CLIENT_PROTOCOL_ID, CLIENT_PROTOCOL_VERSION, ClientFrame,
    ClientFrameBase, ClientInfo, ClientRequestId, ClientSessionId, ClientWorkspaceIdentity,
    SCHEMA_BUNDLE_REQUEST_SCHEMA_ID, SCHEMA_VERSION, SchemaBundleRequest, SchemaBundleResponse,
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

fn assert_exact_query_not_ready_terminal(response_frame: &ClientFrame, params: &serde_json::Value) {
    let ClientFrame::Response {
        outcome: agent_semantic_client_protocol::ClientOutcome::Error,
        result: None,
        error: Some(error),
        ..
    } = response_frame
    else {
        panic!("exact query generation failure must be typed: {response_frame:?}");
    };
    let terminal = &error["terminal"];
    assert_eq!(error["reasonKind"], "query-not-ready");
    assert_eq!(
        terminal["schemaId"],
        "agent.semantic-protocols.asp-client-exact-query-failure"
    );
    assert_eq!(terminal["phase"], "runtime-generation-authority");
    assert_eq!(terminal["generationDigest"], serde_json::Value::Null);
    assert_eq!(terminal["rootDigest"], serde_json::Value::Null);
    assert_eq!(terminal["requestedSelector"], params["selector"]);
    assert_eq!(terminal["details"]["generationState"], "unpublished");
    assert_eq!(
        terminal["details"]["publicationError"],
        serde_json::Value::Null
    );
    assert_eq!(
        terminal["recommendedNext"]["action"],
        "publish-complete-workspace-generation"
    );
    assert_eq!(
        terminal["elapsedMicros"],
        terminal["residentReadElapsedMicros"].as_u64().unwrap()
            + terminal["serviceElapsedMicros"].as_u64().unwrap()
    );
    assert_eq!(terminal["workCounters"]["filesystemReadCount"], 0);
    assert_eq!(terminal["workCounters"]["databaseReadCount"], 0);
    assert_eq!(terminal["workCounters"]["providerProcessCount"], 0);
}

async fn warm_dispatch_without_resident_generation_returns_query_not_ready(
    request_id: &str,
    method: &str,
    params: serde_json::Value,
) {
    let directory = tempfile::tempdir().expect("temporary workspace");
    let project_root = directory.path().join("project");
    std::fs::create_dir_all(&project_root).expect("create project root");
    std::fs::write(project_root.join("lib.rs"), "pub fn ready() {}\n")
        .expect("write source fixture");
    let workspace_identity =
        agent_semantic_client_db::AgentSessionRegistry::workspace_id(&project_root)
            .expect("workspace identity");
    let (runtime_search_service, _runtime_search_requests) =
        agent_semantic_client_db::runtime_search_service::runtime_search_service_channel();
    let telemetry = agent_semantic_client_db::runtime_telemetry_bus::RuntimeTelemetryBus::new();
    let schema_bundles = agent_semantic_runtime_server::RuntimeSchemaBundleCatalog::load(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."),
    )
    .await
    .expect("verified schema bundle catalog");
    agent_semantic_client_db::AgentSessionRegistry::mark_runtime_server_owner_process();
    let agent_session_registry = Arc::new(
        agent_semantic_client_db::AgentSessionRegistry::open_or_create_state_root_async(
            directory.path().join("agent-sessions"),
        )
        .await
        .expect("agent session registry"),
    );
    let service = agent_semantic_runtime_server::build_frame_service(
        schema_bundles,
        agent_session_registry,
        runtime_search_service,
        test_generation_admission(),
        digest('a'),
        Arc::from(registered_language_provider_pairs()),
        directory.path().join("workspace-store"),
        agent_semantic_runtime_server::query_generation::RuntimeQueryGenerationAuthority::new(),
        telemetry.sender,
    )
    .expect("frame service");
    let base = ClientFrameBase {
        schema_id: CLIENT_FRAME_SCHEMA_ID.to_owned(),
        schema_version: SCHEMA_VERSION.to_owned(),
        protocol_id: CLIENT_PROTOCOL_ID.to_owned(),
        protocol_version: CLIENT_PROTOCOL_VERSION.to_owned(),
        session_id: ClientSessionId::new("session-initialize").expect("session id"),
        workspace_identity: ClientWorkspaceIdentity::new(workspace_identity)
            .expect("workspace identity"),
        trace_context: None,
    };
    let request = ClientFrame::Dispatch {
        base: base.clone(),
        request_id: ClientRequestId::new(request_id).expect("request id"),
        project_root: project_root.to_string_lossy().into_owned(),
        client_info: ClientInfo {
            name: "runtime-test".to_owned(),
            version: "1".to_owned(),
        },
        method: method.to_owned(),
        params: params.clone(),
    };
    // This is a dispatcher contract test, not a transport test. Exercise the
    // frame service directly so semantic coverage does not require the test
    // process to bind an AF_UNIX listener that a caller sandbox may forbid.
    // The dedicated client-server integration suite owns real UDS/gRPC I/O.
    let response_frame = service
        .handle_frame(request)
        .await
        .expect("dispatch frame")
        .expect("dispatch response");

    if method.ends_with(".query") {
        assert_exact_query_not_ready_terminal(&response_frame, &params);
    }

    assert!(
        matches!(response_frame, ClientFrame::Response { error: Some(_), .. }),
        "generation failure must be a typed client response: {response_frame:?}"
    );
    let ClientFrame::Response {
        error: Some(error), ..
    } = &response_frame
    else {
        unreachable!("typed error response was asserted above")
    };
    assert_eq!(error["reasonKind"], "query-not-ready");
    assert_eq!(error["terminal"]["phase"], "runtime-generation-authority");
    assert_eq!(error["terminal"]["workCounters"]["filesystemReadCount"], 0);
    assert_eq!(error["terminal"]["workCounters"]["providerProcessCount"], 0);
}

#[tokio::test]
async fn search_dispatch_requires_a_committed_generation_admission() {
    warm_dispatch_without_resident_generation_returns_query_not_ready(
        "request-search",
        "rust.search",
        serde_json::json!({
            "schemaId": "agent.semantic-protocols.asp-client-search-request",
            "schemaVersion": "1",
            "operation": "pipe"
        }),
    )
    .await;
}

#[tokio::test]
async fn exact_query_dispatch_requires_a_committed_generation_admission() {
    warm_dispatch_without_resident_generation_returns_query_not_ready(
        "request-query",
        "rust.query",
        serde_json::json!({
            "schemaId": "agent.semantic-protocols.asp-client-exact-query-request",
            "schemaVersion": "1",
            "selector": "rust://src/lib.rs#item/function/ready",
            "projection": "source"
        }),
    )
    .await;
}

#[tokio::test]
async fn resident_graph_evaluation_requires_a_committed_generation_without_provider_work() {
    warm_dispatch_without_resident_generation_returns_query_not_ready(
        "request-resident-graph",
        "asp.graph.evaluate",
        serde_json::json!({
            "schemaId": "agent.semantic-protocols.semantic-graph-resident-evaluation-request",
            "schemaVersion": "1",
            "protocolId": "agent.semantic-protocols.search",
            "protocolVersion": "1",
            "packetKind": "resident-graph-evaluation-request",
            "languageId": "rust",
            "surface": "search-pipe",
            "queryTerms": ["ready"],
            "profile": "structural",
            "seedIds": [],
            "budget": {"maxDepth": 4, "maxNodes": 64, "maxEdges": 128, "maxResults": 32}
        }),
    )
    .await;
}

#[tokio::test]
async fn registered_language_search_routes_share_query_readiness_gate() {
    for (language_id, _) in registered_language_provider_pairs() {
        warm_dispatch_without_resident_generation_returns_query_not_ready(
            &format!("request-{language_id}-search"),
            &format!("{language_id}.search"),
            serde_json::json!({
                "schemaId": "agent.semantic-protocols.asp-client-search-request",
                "schemaVersion": "1",
                "operation": "pipe"
            }),
        )
        .await;
    }
}

#[tokio::test]
async fn registered_language_exact_query_routes_share_typed_query_readiness_terminal() {
    for (language_id, _) in registered_language_provider_pairs() {
        warm_dispatch_without_resident_generation_returns_query_not_ready(
            &format!("request-{language_id}-query"),
            &format!("{language_id}.query"),
            serde_json::json!({
                "schemaId": "agent.semantic-protocols.asp-client-exact-query-request",
                "schemaVersion": "1",
                "selector": format!("{language_id}://source#item/function/missing"),
                "projection": "source"
            }),
        )
        .await;
    }
}

#[derive(Debug)]
struct ProjectionLatencyReceipt {
    p50_nanos: u128,
    p95_nanos: u128,
    p99_nanos: u128,
    max_nanos: u128,
}

fn measure_projection_latency(
    catalog: &agent_semantic_runtime_server::RuntimeSchemaBundleCatalog,
    request: &SchemaBundleRequest,
) -> ProjectionLatencyReceipt {
    const SAMPLE_COUNT: usize = 128;
    let mut samples = Vec::with_capacity(SAMPLE_COUNT);
    for _ in 0..SAMPLE_COUNT {
        let started = std::time::Instant::now();
        let response = std::hint::black_box(catalog.project(std::hint::black_box(request)));
        samples.push(started.elapsed().as_nanos());
        std::hint::black_box(response);
    }
    samples.sort_unstable();
    ProjectionLatencyReceipt {
        p50_nanos: samples[SAMPLE_COUNT * 50 / 100],
        p95_nanos: samples[SAMPLE_COUNT * 95 / 100],
        p99_nanos: samples[SAMPLE_COUNT * 99 / 100],
        max_nanos: samples[SAMPLE_COUNT - 1],
    }
}

#[tokio::test]
async fn canonical_profiles_project_ready_and_unchanged_in_process_under_one_millisecond() {
    let workspace_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let profiles = SchemaManager::new(&workspace_root)
        .registered_language_profiles()
        .expect("registered profiles");
    let catalog = agent_semantic_runtime_server::RuntimeSchemaBundleCatalog::load(&workspace_root)
        .await
        .expect("verified schema bundle catalog");

    for profile in profiles {
        let request = SchemaBundleRequest {
            schema_id: SCHEMA_BUNDLE_REQUEST_SCHEMA_ID.to_owned(),
            schema_version: SCHEMA_VERSION.to_owned(),
            language_id: profile.language_id.clone(),
            root_set_ids: profile.root_sets,
            known_bundle_digest: None,
        };
        let ready = catalog.project(&request);
        ready.validate().expect("valid Ready response");
        let SchemaBundleResponse::Ready {
            receipt,
            entries,
            documents,
            ..
        } = ready.as_ref()
        else {
            panic!("registered profile must be Ready")
        };
        assert_eq!(entries.len(), documents.len());

        let unchanged_request = SchemaBundleRequest {
            known_bundle_digest: Some(receipt.bundle_digest.clone()),
            ..request.clone()
        };
        let unchanged = catalog.project(&unchanged_request);
        unchanged.validate().expect("valid Unchanged response");
        assert!(matches!(
            unchanged.as_ref(),
            SchemaBundleResponse::Unchanged { .. }
        ));

        for (state, measured_request) in [("ready", &request), ("unchanged", &unchanged_request)] {
            let latency = measure_projection_latency(&catalog, measured_request);
            eprintln!(
                "schemaBundleLatency language={} state={state} samples=128 p50Nanos={} p95Nanos={} p99Nanos={} maxNanos={}",
                profile.language_id,
                latency.p50_nanos,
                latency.p95_nanos,
                latency.p99_nanos,
                latency.max_nanos,
            );
            assert!(
                latency.max_nanos < 1_000_000,
                "schema projection exceeded serviceElapsedMicros gate: language={} state={state} p50Nanos={} p95Nanos={} p99Nanos={} maxNanos={}",
                profile.language_id,
                latency.p50_nanos,
                latency.p95_nanos,
                latency.p99_nanos,
                latency.max_nanos,
            );
        }
    }

    let failed = catalog.project(&SchemaBundleRequest {
        schema_id: SCHEMA_BUNDLE_REQUEST_SCHEMA_ID.to_owned(),
        schema_version: SCHEMA_VERSION.to_owned(),
        language_id: "unregistered-language".to_owned(),
        root_set_ids: vec!["client-protocol".to_owned()],
        known_bundle_digest: None,
    });
    failed.validate().expect("valid Failed response");
    assert!(matches!(
        failed.as_ref(),
        SchemaBundleResponse::Failed { reason_kind, .. }
            if reason_kind == "schema-bundle-language-unregistered"
    ));
}

#[tokio::test]
async fn host_uds_schema_bundle_route_bypasses_workspace_generation() {
    let directory = tempfile::tempdir().expect("temporary workspace");
    let project_root = directory.path().join("project");
    std::fs::create_dir_all(&project_root).expect("create project root");
    std::fs::write(project_root.join("lib.rs"), "pub fn ready() {}\n")
        .expect("write source fixture");
    let workspace_identity =
        agent_semantic_client_db::AgentSessionRegistry::workspace_id(&project_root)
            .expect("workspace identity");
    let (runtime_search_service, _requests) =
        agent_semantic_client_db::runtime_search_service::runtime_search_service_channel();
    let telemetry = agent_semantic_client_db::runtime_telemetry_bus::RuntimeTelemetryBus::new();
    let workspace_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let profile = SchemaManager::new(&workspace_root)
        .registered_language_profiles()
        .expect("registered profiles")
        .into_iter()
        .next()
        .expect("at least one registered profile");
    agent_semantic_client_db::AgentSessionRegistry::mark_runtime_server_owner_process();
    let agent_session_root = tempfile::tempdir().expect("agent session root");
    let agent_session_registry = Arc::new(
        agent_semantic_client_db::AgentSessionRegistry::open_or_create_state_root_async(
            agent_session_root.path(),
        )
        .await
        .expect("agent session registry"),
    );
    let service = agent_semantic_runtime_server::build_frame_service(
        agent_semantic_runtime_server::RuntimeSchemaBundleCatalog::load(&workspace_root)
            .await
            .expect("verified schema bundle catalog"),
        agent_session_registry,
        runtime_search_service,
        test_generation_admission(),
        digest('a'),
        Arc::from(registered_language_provider_pairs()),
        directory.path().join("workspace-store"),
        agent_semantic_runtime_server::query_generation::RuntimeQueryGenerationAuthority::new(),
        telemetry.sender,
    )
    .expect("frame service");
    let socket_dir = tempfile::tempdir().expect("socket directory");
    let socket_path = socket_dir.path().join("asp-client.sock");
    let listener = bind_asp_client_grpc_unix(&socket_path)
        .await
        .expect("bind UDS");
    let (shutdown, shutdown_receiver) = tokio::sync::watch::channel(false);
    let server = tokio::spawn(serve_asp_client_grpc_unix(
        listener,
        service,
        shutdown_receiver,
    ));
    let client = AspClientGrpcTransport::connect_unix(&socket_path)
        .await
        .expect("connect UDS");
    let request = SchemaBundleRequest {
        schema_id: SCHEMA_BUNDLE_REQUEST_SCHEMA_ID.to_owned(),
        schema_version: SCHEMA_VERSION.to_owned(),
        language_id: profile.language_id,
        root_set_ids: profile.root_sets,
        known_bundle_digest: None,
    };
    let frame = client
        .call(ClientFrame::Dispatch {
            base: ClientFrameBase {
                schema_id: CLIENT_FRAME_SCHEMA_ID.to_owned(),
                schema_version: SCHEMA_VERSION.to_owned(),
                protocol_id: CLIENT_PROTOCOL_ID.to_owned(),
                protocol_version: CLIENT_PROTOCOL_VERSION.to_owned(),
                session_id: ClientSessionId::new("schema-bundle-uds").expect("session id"),
                workspace_identity: ClientWorkspaceIdentity::new(workspace_identity)
                    .expect("workspace identity"),
                trace_context: None,
            },
            request_id: ClientRequestId::new("schema-bundle-ready").expect("request id"),
            project_root: project_root.to_string_lossy().into_owned(),
            client_info: ClientInfo {
                name: "runtime-test".to_owned(),
                version: "1".to_owned(),
            },
            method: "asp.schema.bundle".to_owned(),
            params: serde_json::to_value(request).expect("request JSON"),
        })
        .await
        .expect("schema bundle response");
    let ClientFrame::Response {
        result: Some(result),
        error: None,
        ..
    } = frame
    else {
        panic!("schema bundle UDS route must return Ready")
    };
    let response: SchemaBundleResponse = serde_json::from_value(result).expect("typed response");
    response.validate().expect("valid response");
    assert!(matches!(response, SchemaBundleResponse::Ready { .. }));
    drop(client);
    shutdown.send(true).expect("shutdown UDS");
    server.await.expect("join UDS").expect("serve UDS");
}
