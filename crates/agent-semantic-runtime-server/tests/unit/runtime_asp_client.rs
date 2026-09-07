// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::sync::Arc;

use agent_semantic_client_protocol::ClientFrame;
use agent_semantic_client_protocol::ClientFrameBase;
use agent_semantic_client_protocol::ClientInfo;
use agent_semantic_client_protocol::ClientProjectId;
use agent_semantic_client_protocol::ClientRequestId;
use agent_semantic_client_protocol::ClientSessionId;
use agent_semantic_client_protocol::ClientWorkspaceIdentity;
use agent_semantic_client_protocol::SCHEMA_BUNDLE_REQUEST_SCHEMA_ID;
use agent_semantic_client_protocol::SchemaBundleRequest;
use agent_semantic_client_protocol::SchemaBundleResponse;
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
            "worktree:runtime-test",
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
    assert_eq!(terminal["details"]["generationState"], "failed");
    assert!(
        terminal["details"]["publicationError"]
            .as_str()
            .is_some_and(|error| error.contains("intentionally unavailable")),
        "fixture must preserve the deterministic generation-builder failure: {terminal}"
    );
    assert!(terminal.get("recommendedNext").is_none());
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
    let (generation_admission, project_id) =
        test_generation_admission(&project_root, &workspace_identity).await;
    let workspace_registry = Arc::new(
        agent_semantic_client_db::runtime_server_workspace::RuntimeServerWorkspaceRegistry::new(
            directory.path().join("workspace-generations"),
        )
        .expect("workspace registry"),
    );
    let service = agent_semantic_runtime_server::build_frame_service(
        schema_bundles,
        agent_session_registry,
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
    let base = ClientFrameBase {
        schema_id: CLIENT_FRAME_SCHEMA_ID.to_owned(),
        schema_version: SCHEMA_VERSION.to_owned(),
        protocol_id: CLIENT_PROTOCOL_ID.to_owned(),
        protocol_version: CLIENT_PROTOCOL_VERSION.to_owned(),
        session_id: ClientSessionId::new("session-initialize").expect("session id"),
        project_id: ClientProjectId::new(project_id).expect("project id"),
        workspace_id: ClientWorkspaceIdentity::new(workspace_identity).expect("workspace identity"),
        trace_context: None,
    };
    let initialize = service
        .handle_frame(ClientFrame::Initialize {
            base: base.clone(),
            request_id: ClientRequestId::new("initialize").expect("request id"),
            client_info: ClientInfo {
                name: "runtime-test".to_owned(),
                version: "1".to_owned(),
            },
            capabilities: serde_json::json!({}),
        })
        .await
        .expect("initialize frame")
        .expect("initialize response");
    let ClientFrame::Response {
        catalog: Some(catalog),
        ..
    } = initialize
    else {
        panic!("initialize must publish the exact session catalog")
    };
    let request = ClientFrame::Request {
        base: base.clone(),
        request_id: ClientRequestId::new(request_id).expect("request id"),
        catalog_generation: catalog.catalog_generation,
        workspace_generation: catalog.workspace_generation,
        method: method.to_owned(),
        params: params.clone(),
        client_timing_witness: None,
    };
    // This is a dispatcher contract test, not a transport test. Exercise the
    // frame service directly so semantic coverage does not require the test
    // process to bind an AF_UNIX listener that a caller sandbox may forbid.
    // The dedicated client-server integration suite owns real UDS/gRPC I/O.
    let response_frame = service
        .handle_frame(request)
        .await
        .expect("request frame")
        .expect("request response");

    if method == agent_semantic_client_protocol::WORKSPACE_SEARCH_PLAYBOOK_METHOD
        && params.get("fd").is_none()
    {
        let ClientFrame::Response {
            result: Some(result),
            error: None,
            ..
        } = response_frame
        else {
            panic!("contract query must bypass generation admission")
        };
        assert_eq!(result["result"], "provider-contract-failure");
        assert_eq!(result["reason"], "provider-not-registered");
        return;
    }

    if method.ends_with(".query") {
        assert_exact_query_not_ready_terminal(&response_frame, &params);
    }

    if method == agent_semantic_client_protocol::WORKSPACE_QUERY_PLAYBOOK_METHOD {
        let ClientFrame::Response {
            outcome: agent_semantic_client_protocol::ClientOutcome::Error,
            result: None,
            error: Some(error),
            ..
        } = response_frame
        else {
            panic!("Query Playbook hard cut must emit one typed failure response")
        };
        assert_eq!(
            error["reasonKind"],
            "query-playbook-runtime-binding-unavailable"
        );
        assert_eq!(
            error["terminal"]["failureStage"],
            "runtime-execution-binding-admission"
        );
        return;
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
async fn workspace_search_playbook_without_acquisition_still_requires_generation_admission() {
    warm_dispatch_without_resident_generation_returns_query_not_ready(
        "request-workspace-search-playbook-contract",
        agent_semantic_client_protocol::WORKSPACE_SEARCH_PLAYBOOK_METHOD,
        serde_json::json!({
            "schemaId": "agent.semantic-protocols.asp-client-workspace-search-playbook-request",
            "schemaVersion": "1",
            "languages": "rust"
        }),
    )
    .await;
}

#[tokio::test]
async fn search_dispatch_requires_a_committed_generation_admission() {
    warm_dispatch_without_resident_generation_returns_query_not_ready(
        "request-search",
        "rust.search",
        serde_json::json!({
            "schemaId": "agent.semantic-protocols.asp-client-search-request",
            "schemaVersion": "1",
            "intent": "conceptual",
            "query": "ready",
            "scope": "workspace",
            "coverage": "candidates",
            "maxOwners": 16,
            "deadlineMs": 250,
            "explain": "compact"
        }),
    )
    .await;
}

#[tokio::test]
async fn workspace_search_playbook_requires_a_committed_complete_generation() {
    warm_dispatch_without_resident_generation_returns_query_not_ready(
        "request-workspace-search-playbook",
        agent_semantic_client_protocol::WORKSPACE_SEARCH_PLAYBOOK_METHOD,
        serde_json::json!({
            "schemaId": "agent.semantic-protocols.asp-client-workspace-search-playbook-request",
            "schemaVersion": "1",
            "languages": "rust",
            "fd": [["-t", "f", "-e", "rs", "ready", "."]],
            "rg": [["-n", "ready", "."]],
            "tantivy": [["ready"]],
            "syntax": [{
                "producer": "rust",
                "argv": ["--treesitter-query", "((identifier) @symbol)"]
            }],
            "graph": [{
                "language": "gql",
                "argv": ["MATCH (a:Owner)-[:DEPENDS_ON]->(b:Owner) RETURN a, b"]
            }],
            "clauseOrder": [
                {"axis": "fd", "blockIndex": 0},
                {"axis": "rg", "blockIndex": 0},
                {"axis": "tantivy", "blockIndex": 0},
                {"axis": "syntax", "blockIndex": 0},
                {"axis": "graph", "blockIndex": 0}
            ]
        }),
    )
    .await;
}

#[tokio::test]
async fn query_playbook_never_falls_back_to_per_selector_exact_query() {
    warm_dispatch_without_resident_generation_returns_query_not_ready(
        "request-workspace-query-playbook",
        agent_semantic_client_protocol::WORKSPACE_QUERY_PLAYBOOK_METHOD,
        serde_json::json!({
            "schemaId": "agent.semantic-protocols.asp-client-workspace-query-playbook-request",
            "schemaVersion": "1",
            "selectors": [
                "org://docs/publication.org#item/heading/Publication",
                "rust://src/registry.rs#item/function/refresh_registry"
            ],
            "projection": "source"
        }),
    )
    .await;
}

#[tokio::test]
async fn workspace_syntax_query_requires_a_committed_complete_generation() {
    warm_dispatch_without_resident_generation_returns_query_not_ready(
        "request-workspace-syntax-query",
        agent_semantic_client_protocol::WORKSPACE_SYNTAX_QUERY_METHOD,
        serde_json::json!({
            "schemaId": "agent.semantic-protocols.asp-client-workspace-syntax-query-request",
            "schemaVersion": "1",
            "languages": "rust",
            "syntax": [{
                "producer": "rust",
                "argv": ["--treesitter-query", "((identifier) @symbol)"]
            }],
            "projection": "matches"
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
            "entryNodeIds": [],
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
                "intent": "conceptual",
                "query": "ready",
                "scope": "workspace",
                "coverage": "candidates",
                "maxOwners": 16,
                "deadlineMs": 250,
                "explain": "compact"
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
    let (generation_admission, project_id) =
        test_generation_admission(&project_root, &workspace_identity).await;
    let workspace_registry = Arc::new(
        agent_semantic_client_db::runtime_server_workspace::RuntimeServerWorkspaceRegistry::new(
            directory.path().join("workspace-generations"),
        )
        .expect("workspace registry"),
    );
    let service = agent_semantic_runtime_server::build_frame_service(
        agent_semantic_runtime_server::RuntimeSchemaBundleCatalog::load(&workspace_root)
            .await
            .expect("verified schema bundle catalog"),
        agent_session_registry,
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
        .expect("bind loopback endpoint");
    let endpoint = listener.local_addr().expect("loopback endpoint");
    let (shutdown, shutdown_receiver) = tokio::sync::watch::channel(false);
    let server = tokio::spawn(serve_asp_client_grpc_tcp(
        listener,
        service,
        shutdown_receiver,
    ));
    let client = AspClientGrpcTransport::connect_tcp(endpoint)
        .await
        .expect("connect loopback endpoint");
    let request = SchemaBundleRequest {
        schema_id: SCHEMA_BUNDLE_REQUEST_SCHEMA_ID.to_owned(),
        schema_version: SCHEMA_VERSION.to_owned(),
        language_id: profile.language_id,
        root_set_ids: profile.root_sets,
        known_bundle_digest: None,
    };
    let base = ClientFrameBase {
        schema_id: CLIENT_FRAME_SCHEMA_ID.to_owned(),
        schema_version: SCHEMA_VERSION.to_owned(),
        protocol_id: CLIENT_PROTOCOL_ID.to_owned(),
        protocol_version: CLIENT_PROTOCOL_VERSION.to_owned(),
        session_id: ClientSessionId::new("schema-bundle-uds").expect("session id"),
        project_id: ClientProjectId::new(project_id).expect("project id"),
        workspace_id: ClientWorkspaceIdentity::new(workspace_identity).expect("workspace identity"),
        trace_context: None,
    };
    let initialized = client
        .call(ClientFrame::Initialize {
            base: base.clone(),
            request_id: ClientRequestId::new("schema-bundle-initialize").expect("request id"),
            client_info: ClientInfo {
                name: "runtime-test".to_owned(),
                version: "1".to_owned(),
            },
            capabilities: serde_json::json!({}),
        })
        .await
        .expect("schema bundle session initialization");
    let ClientFrame::Response {
        catalog: Some(catalog),
        ..
    } = initialized
    else {
        panic!("initialize must return the exact Runtime catalog")
    };
    let frame = client
        .call(ClientFrame::Request {
            base,
            request_id: ClientRequestId::new("schema-bundle-ready").expect("request id"),
            catalog_generation: catalog.catalog_generation,
            workspace_generation: catalog.workspace_generation,
            method: "asp.schema.bundle".to_owned(),
            params: serde_json::to_value(request).expect("request JSON"),
            client_timing_witness: None,
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
