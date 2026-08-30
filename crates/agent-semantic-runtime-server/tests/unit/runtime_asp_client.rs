use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use agent_semantic_client_protocol::{
    CLIENT_FRAME_SCHEMA_ID, CLIENT_PROTOCOL_ID, CLIENT_PROTOCOL_VERSION, ClientFrame,
    ClientFrameBase, ClientInfo, ClientRequestId, ClientSessionId, ClientWorkspaceIdentity,
    SCHEMA_BUNDLE_REQUEST_SCHEMA_ID,
    SCHEMA_VERSION, SchemaBundleRequest, SchemaBundleResponse,
};
use agent_semantic_client_server::{
    AspClientGrpcTransport, bind_asp_client_grpc_unix, serve_asp_client_grpc_unix,
};
use agent_semantic_schema_manager::SchemaManager;
use tokio::sync::Notify;

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

fn assert_exact_admission_terminal(
    response_frame: &ClientFrame,
    params: &serde_json::Value,
    expected_reason_kind: &str,
    expected_admission_state: &str,
    expected_recommended_action: &str,
) {
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
    assert_eq!(error["reasonKind"], expected_reason_kind);
    assert_eq!(
        terminal["schemaId"],
        "agent.semantic-protocols.asp-client-exact-query-failure"
    );
    assert_eq!(terminal["phase"], "workspace-generation-admission");
    assert_eq!(terminal["generationDigest"], serde_json::Value::Null);
    assert_eq!(terminal["rootDigest"], serde_json::Value::Null);
    assert_eq!(terminal["requestedSelector"], params["selector"]);
    assert_eq!(
        terminal["details"]["admissionState"],
        expected_admission_state
    );
    assert_eq!(terminal["details"]["attempt"], 1);
    assert!(
        terminal["details"]["candidateGenerationDigest"]
            .as_str()
            .is_some_and(|digest| {
                digest.starts_with("blake3-256:") && digest.len() == "blake3-256:".len() + 64
            }),
        "admission terminal must carry its bound candidate identity: {terminal}"
    );
    assert_eq!(terminal["details"]["failureStage"], serde_json::Value::Null);
    assert_eq!(
        terminal["recommendedNext"]["action"],
        expected_recommended_action
    );
    assert_eq!(
        terminal["elapsedMicros"],
        terminal["residentReadElapsedMicros"].as_u64().unwrap()
            + terminal["serviceElapsedMicros"].as_u64().unwrap()
    );
    assert!(
        terminal["serviceElapsedMicros"].as_u64().unwrap() < 1_000,
        "admission state must terminalize within the sub-millisecond service gate: {terminal}"
    );
}

async fn dispatch_without_initialize_admits_the_pinned_provider_candidate(
    request_id: &str,
    method: &str,
    params: serde_json::Value,
    language_id: &str,
    provider_id: &str,
) {
    let directory = tempfile::tempdir().expect("temporary workspace");
    let project_root = directory.path().join("project");
    std::fs::create_dir_all(&project_root).expect("create project root");
    std::fs::write(project_root.join("lib.rs"), "pub fn ready() {}\n")
        .expect("write source fixture");
    let workspace_identity =
        agent_semantic_client_db::AgentSessionRegistry::workspace_id(&project_root)
            .expect("workspace identity");
    let builds = Arc::new(AtomicUsize::new(0));
    let builder_started = Arc::new(Notify::new());
    let observed = Arc::new(std::sync::Mutex::new(None));
    let admission = Arc::new(
        agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationAdmission::new(
            Arc::new({
                let builds = Arc::clone(&builds);
                let builder_started = Arc::clone(&builder_started);
                let observed = Arc::clone(&observed);
                move |_, _, candidate, _, _, provider_target, _| {
                    builder_started.notify_one();
                    builds.fetch_add(1, Ordering::AcqRel);
                    *observed.lock().expect("generation observation lock") =
                        Some((candidate.candidate_generation.digest, provider_target));
                    Box::pin(std::future::pending())
                }
            }),
        ),
    );
    let registry = Arc::new(
        agent_semantic_client_db::runtime_server_workspace::RuntimeServerWorkspaceRegistry::new(
            directory.path().join("runtime"),
        )
        .expect("workspace registry"),
    );
    let (runtime_search_service, _runtime_search_requests) =
        agent_semantic_client_db::runtime_search_service::runtime_search_service_channel();
    let telemetry = agent_semantic_client_db::runtime_telemetry_bus::RuntimeTelemetryBus::new();
    let schema_bundles = agent_semantic_runtime_server::RuntimeSchemaBundleCatalog::load(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."),
    )
    .await
    .expect("verified schema bundle catalog");
    let service = agent_semantic_runtime_server::build_frame_service(
        schema_bundles,
        runtime_search_service,
        registry,
        digest('a'),
        Arc::from(registered_language_provider_pairs()),
        admission,
        agent_semantic_runtime_server::query_generation::RuntimeQueryGenerationAuthority::new(),
        telemetry.sender,
    )
    .expect("frame service");
    let client_socket_dir = tempfile::tempdir().expect("client socket directory");
    let client_socket_path = client_socket_dir.path().join("asp-client.sock");
    let listener = bind_asp_client_grpc_unix(&client_socket_path)
        .await
        .expect("bind ASP Client gRPC socket");
    let (client_shutdown, client_shutdown_receiver) = tokio::sync::watch::channel(false);
    let client_server = tokio::spawn(serve_asp_client_grpc_unix(
        listener,
        service,
        client_shutdown_receiver,
    ));
    let client = AspClientGrpcTransport::connect_unix(&client_socket_path)
        .await
        .expect("connect ASP Client gRPC transport");
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
    let response = client.call(request).await.expect("dispatch response");

    let response_frame = response;

    if method.ends_with(".query") {
        assert_exact_admission_terminal(
            &response_frame,
            &params,
            "runtime-generation-queued",
            "Queued",
            "observe-runtime-dispatch",
        );
    }

    assert!(
        matches!(response_frame, ClientFrame::Response { error: Some(_), .. }),
        "generation failure must be a typed client response: {response_frame:?}"
    );
    assert_eq!(builds.load(Ordering::Acquire), 1);
    let (candidate_digest, provider_target) = observed
        .lock()
        .expect("generation observation lock")
        .clone()
        .expect("dispatch generation observation");
    let normalized_candidate_digest = candidate_digest.replacen("blake3:", "blake3-256:", 1);
    assert!(normalized_candidate_digest.starts_with("blake3-256:"));
    assert_eq!(normalized_candidate_digest.len(), "blake3-256:".len() + 64);
    assert_eq!(
        provider_target,
        Some(
            agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationProviderTarget {
                language_id: language_id.to_owned(),
                provider_id: Some(provider_id.to_owned()),
            }
        )
    );
    drop(client);
    client_shutdown.send(true).expect("shutdown gRPC service");
    client_server
        .await
        .expect("join gRPC service")
        .expect("serve gRPC service");
}

#[tokio::test]
async fn search_dispatch_admits_the_pinned_provider_candidate() {
    dispatch_without_initialize_admits_the_pinned_provider_candidate(
        "request-search",
        "rust.search",
        serde_json::json!({
            "schemaId": "agent.semantic-protocols.asp-client-search-request",
            "schemaVersion": "1",
            "operation": "pipe"
        }),
        "rust",
        "asp-rust",
    )
    .await;
}

#[tokio::test]
async fn exact_query_dispatch_admits_the_pinned_provider_candidate() {
    dispatch_without_initialize_admits_the_pinned_provider_candidate(
        "request-query",
        "rust.query",
        serde_json::json!({
            "schemaId": "agent.semantic-protocols.asp-client-exact-query-request",
            "schemaVersion": "1",
            "selector": "rust://src/lib.rs#item/function/ready",
            "projection": "source"
        }),
        "rust",
        "asp-rust",
    )
    .await;
}

#[tokio::test]
async fn registered_language_search_routes_share_runtime_admission() {
    for (language_id, provider_id) in registered_language_provider_pairs() {
        dispatch_without_initialize_admits_the_pinned_provider_candidate(
            &format!("request-{language_id}-search"),
            &format!("{language_id}.search"),
            serde_json::json!({
                "schemaId": "agent.semantic-protocols.asp-client-search-request",
                "schemaVersion": "1",
                "operation": "pipe"
            }),
            &language_id,
            &provider_id,
        )
        .await;
    }
}

#[tokio::test]
async fn registered_language_exact_query_routes_share_typed_runtime_terminal() {
    for (language_id, provider_id) in registered_language_provider_pairs() {
        dispatch_without_initialize_admits_the_pinned_provider_candidate(
            &format!("request-{language_id}-query"),
            &format!("{language_id}.query"),
            serde_json::json!({
                "schemaId": "agent.semantic-protocols.asp-client-exact-query-request",
                "schemaVersion": "1",
                "selector": format!("{language_id}://source#item/function/missing"),
                "projection": "source"
            }),
            &language_id,
            &provider_id,
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
    let builds = Arc::new(AtomicUsize::new(0));
    let admission = Arc::new(
        agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationAdmission::new(
            Arc::new({
                let builds = Arc::clone(&builds);
                move |_, _, _, _, _, _, _| {
                    builds.fetch_add(1, Ordering::AcqRel);
                    Box::pin(std::future::pending())
                }
            }),
        ),
    );
    let registry = Arc::new(
        agent_semantic_client_db::runtime_server_workspace::RuntimeServerWorkspaceRegistry::new(
            directory.path().join("runtime"),
        )
        .expect("workspace registry"),
    );
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
    let service = agent_semantic_runtime_server::build_frame_service(
        agent_semantic_runtime_server::RuntimeSchemaBundleCatalog::load(&workspace_root)
            .await
            .expect("verified schema bundle catalog"),
        runtime_search_service,
        registry,
        digest('a'),
        Arc::from(registered_language_provider_pairs()),
        admission,
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
    assert_eq!(builds.load(Ordering::Acquire), 0);

    drop(client);
    shutdown.send(true).expect("shutdown UDS");
    server.await.expect("join UDS").expect("serve UDS");
}
