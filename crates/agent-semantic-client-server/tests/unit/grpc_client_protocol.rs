// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::Mutex;

use agent_semantic_client_protocol::ClientCapabilities;
use agent_semantic_client_protocol::ClientFrame;
use agent_semantic_client_protocol::ClientFrameBase;
use agent_semantic_client_protocol::ClientInfo;
use agent_semantic_client_protocol::ClientMethod;
use agent_semantic_client_protocol::ClientOutcome;
use agent_semantic_client_protocol::ClientProjectId;
use agent_semantic_client_protocol::ClientProtocolCatalog;
use agent_semantic_client_protocol::ClientRequestId;
use agent_semantic_client_protocol::ClientSessionId;
use agent_semantic_client_protocol::ClientTransport;
use agent_semantic_client_protocol::ClientWorkspaceIdentity;
use agent_semantic_client_protocol::RuntimeSearchClientTimingWitness;
use agent_semantic_client_protocol::protocol_identity::CLIENT_CATALOG_SCHEMA_ID;
use agent_semantic_client_protocol::protocol_identity::CLIENT_FRAME_SCHEMA_ID;
use agent_semantic_client_protocol::protocol_identity::CLIENT_PROTOCOL_ID;
use agent_semantic_client_protocol::protocol_identity::CLIENT_PROTOCOL_VERSION;
use agent_semantic_client_protocol::protocol_identity::SCHEMA_VERSION;
use agent_semantic_client_server::AspClientCancelFuture;
use agent_semantic_client_server::AspClientDispatchError;
use agent_semantic_client_server::AspClientDispatchFuture;
use agent_semantic_client_server::AspClientDispatchRequest;
use agent_semantic_client_server::AspClientDispatcher;
use agent_semantic_client_server::AspClientFrameService;
use agent_semantic_client_server::AspClientGrpcTransport;
use agent_semantic_client_server::AspClientResponseTelemetry;
use agent_semantic_client_server::CLIENT_FRAME_SESSION_CAPACITY;
use agent_semantic_client_server::bind_asp_client_grpc_tcp;
use agent_semantic_client_server::serve_asp_client_grpc_tcp;
use serde_json::json;

struct ExactQueryDispatcher {
    cancellation_by_request: Arc<Mutex<BTreeMap<String, Arc<tokio::sync::Notify>>>>,
    response_boundaries: Arc<Mutex<Vec<(String, String, bool)>>>,
}

impl AspClientDispatcher for ExactQueryDispatcher {
    fn dispatch(&self, request: AspClientDispatchRequest) -> AspClientDispatchFuture {
        if request.method == "test.large-response" {
            return Box::pin(async move {
                Ok(json!({
                    "schemaId": "agent.semantic-protocols.test-large-response",
                    "schemaVersion": "1",
                    "source": "x".repeat(4 * 1024 * 1024 + 64 * 1024),
                }))
            });
        }
        if request.method == "test.cancellation-probe" {
            let cancelled = Arc::new(tokio::sync::Notify::new());
            self.cancellation_by_request
                .lock()
                .expect("cancellation registry")
                .insert(
                    request.request_id.as_str().to_owned(),
                    Arc::clone(&cancelled),
                );
            return Box::pin(async move {
                cancelled.notified().await;
                Err(AspClientDispatchError {
                    reason_kind: "client-request-cancelled".to_owned(),
                    message: "client request was cancelled".to_owned(),
                    details: None,
                })
            });
        }
        Box::pin(async move {
            assert_eq!(request.method, "rust.query");
            Err(AspClientDispatchError {
                reason_kind: "projection-missing".to_owned(),
                message: "exact projection terminal: projection-missing".to_owned(),
                details: Some(json!({
                    "schemaId": "agent.semantic-protocols.asp-client-exact-query-failure",
                    "schemaVersion": "1",
                    "state": "failed",
                    "operationId": request.request_id.as_str(),
                    "languageId": "rust",
                    "providerId": "asp-rust",
                    "requestedSelector": "rust://src/lib.rs#item/function/missing",
                    "resolvedSelector": "rust://src/lib.rs#item/function/missing",
                    "projectionKind": "source",
                    "phase": "resident-selector-read",
                    "reasonKind": "projection-missing"
                })),
            })
        })
    }

    fn cancel(
        &self,
        _: &ClientProjectId,
        _: &ClientWorkspaceIdentity,
        _: &ClientSessionId,
        request_id: &ClientRequestId,
    ) -> AspClientCancelFuture {
        let cancelled = self
            .cancellation_by_request
            .lock()
            .expect("cancellation registry")
            .get(request_id.as_str())
            .cloned();
        Box::pin(async move {
            if let Some(cancelled) = cancelled {
                cancelled.notify_one();
                true
            } else {
                false
            }
        })
    }

    fn response_serialized(&self, response: &AspClientResponseTelemetry, _elapsed_micros: u64) {
        if response.request_id.as_str() != "exact-query" {
            return;
        }
        self.response_boundaries
            .lock()
            .expect("response boundary registry")
            .push((
                response.request_id.as_str().to_owned(),
                "schema-validate-serialize".to_owned(),
                true,
            ));
    }

    fn terminal_egressed(
        &self,
        response: &AspClientResponseTelemetry,
        _elapsed_micros: u64,
        delivered: bool,
    ) {
        if response.request_id.as_str() != "exact-query" {
            return;
        }
        self.response_boundaries
            .lock()
            .expect("response boundary registry")
            .push((
                response.request_id.as_str().to_owned(),
                "terminal-egress".to_owned(),
                delivered,
            ));
    }
}

fn base() -> ClientFrameBase {
    ClientFrameBase {
        schema_id: CLIENT_FRAME_SCHEMA_ID.to_owned(),
        schema_version: SCHEMA_VERSION.to_owned(),
        protocol_id: CLIENT_PROTOCOL_ID.to_owned(),
        protocol_version: CLIENT_PROTOCOL_VERSION.to_owned(),
        session_id: ClientSessionId::new("grpc-exact-query-session").expect("session id"),
        project_id: ClientProjectId::new("repo-grpc-project").expect("project id"),
        workspace_id: ClientWorkspaceIdentity::new("workspace-grpc-workspace")
            .expect("workspace identity"),
        trace_context: None,
    }
}

fn exact_query_frame(request_id: String) -> ClientFrame {
    ClientFrame::Request {
        base: base(),
        request_id: ClientRequestId::new(request_id).expect("request id"),
        catalog_generation: format!("sha256:{}", "a".repeat(64)),
        workspace_generation: format!("blake3-256:{}", "b".repeat(64)),
        method: "rust.query".to_owned(),
        params: json!({}),
        client_timing_witness: None,
    }
}

fn large_response_frame() -> ClientFrame {
    ClientFrame::Request {
        base: base(),
        request_id: ClientRequestId::new("large-response").expect("request id"),
        catalog_generation: format!("sha256:{}", "a".repeat(64)),
        workspace_generation: format!("blake3-256:{}", "b".repeat(64)),
        method: "test.large-response".to_owned(),
        params: json!({}),
        client_timing_witness: None,
    }
}

fn catalog() -> ClientProtocolCatalog {
    ClientProtocolCatalog {
        schema_id: CLIENT_CATALOG_SCHEMA_ID.to_owned(),
        schema_version: SCHEMA_VERSION.to_owned(),
        protocol_id: CLIENT_PROTOCOL_ID.to_owned(),
        protocol_version: CLIENT_PROTOCOL_VERSION.to_owned(),
        catalog_generation: format!("sha256:{}", "a".repeat(64)),
        workspace_generation: format!("blake3-256:{}", "b".repeat(64)),
        transports: vec![ClientTransport::RuntimeIpc],
        capabilities: ClientCapabilities {
            request_cancellation: true,
            events: true,
            streaming: true,
            trace_context: true,
        },
        methods: [
            "rust.query",
            "test.large-response",
            "test.cancellation-probe",
        ]
        .into_iter()
        .map(|method| ClientMethod {
            method: method.to_owned(),
            route_id: agent_semantic_client_protocol::ClientRouteId::new(method)
                .expect("test route id"),
            request_schema_id: agent_semantic_client_protocol::ClientSchemaId::new(format!(
                "agent.semantic-protocols.test.{method}.request"
            ))
            .expect("test request schema id"),
            response_schema_id: agent_semantic_client_protocol::ClientSchemaId::new(format!(
                "agent.semantic-protocols.test.{method}.response"
            ))
            .expect("test response schema id"),
            error_schema_ids: Vec::new(),
            parameters: Vec::new(),
            cancellable: true,
            streaming: false,
        })
        .collect(),
    }
}

#[tokio::test]
async fn client_timing_witness_must_match_the_admitted_frame_identity() {
    let service = AspClientFrameService::new(
        Arc::new(ExactQueryDispatcher {
            cancellation_by_request: Arc::new(Mutex::new(BTreeMap::new())),
            response_boundaries: Arc::new(Mutex::new(Vec::new())),
        }),
        |_| Ok(catalog()),
    );
    service
        .handle_frame(ClientFrame::Initialize {
            base: base(),
            request_id: ClientRequestId::new("initialize-timing").expect("request id"),
            client_info: ClientInfo {
                name: "timing-test".into(),
                version: "1".into(),
            },
            capabilities: json!({}),
        })
        .await
        .expect("initialize terminal");
    let mut request = exact_query_frame("timed-request".into());
    let ClientFrame::Request {
        client_timing_witness,
        ..
    } = &mut request
    else {
        unreachable!()
    };
    *client_timing_witness = Some(
        RuntimeSearchClientTimingWitness::new(
            "grpc-exact-query-session",
            "foreign-request",
            [1, 2, 3],
        )
        .expect("well-formed but foreign witness"),
    );

    let response = service
        .handle_frame(request)
        .await
        .expect("typed mismatch terminal");
    let Some(ClientFrame::Response {
        outcome: ClientOutcome::Error,
        error: Some(error),
        ..
    }) = response
    else {
        panic!("foreign timing witness must fail before dispatch")
    };
    assert_eq!(
        error["reasonKind"],
        "runtime-search-client-timing-identity-mismatch"
    );
}

#[tokio::test]
async fn grpc_transport_reports_serialization_before_terminal_egress() {
    let listener = bind_asp_client_grpc_tcp()
        .await
        .expect("bind response-boundary endpoint");
    let endpoint = listener.local_addr().expect("response-boundary endpoint");
    let response_boundaries = Arc::new(Mutex::new(Vec::new()));
    let service = Arc::new(AspClientFrameService::new(
        Arc::new(ExactQueryDispatcher {
            cancellation_by_request: Arc::new(Mutex::new(BTreeMap::new())),
            response_boundaries: Arc::clone(&response_boundaries),
        }),
        |_| Ok(catalog()),
    ));
    let (shutdown, shutdown_rx) = tokio::sync::watch::channel(false);
    let server = tokio::spawn(serve_asp_client_grpc_tcp(listener, service, shutdown_rx));
    let client = AspClientGrpcTransport::connect_tcp(endpoint)
        .await
        .expect("connect response-boundary session");
    client
        .call(ClientFrame::Initialize {
            base: base(),
            request_id: ClientRequestId::new("initialize-boundary").expect("request id"),
            client_info: ClientInfo {
                name: "boundary-test".into(),
                version: "1".into(),
            },
            capabilities: json!({}),
        })
        .await
        .expect("initialize response-boundary session");

    let _ = client
        .call(exact_query_frame("exact-query".to_owned()))
        .await
        .expect("receive exact-query terminal");
    assert_eq!(
        *response_boundaries.lock().expect("response boundaries"),
        vec![
            (
                "exact-query".to_owned(),
                "schema-validate-serialize".to_owned(),
                true,
            ),
            ("exact-query".to_owned(), "terminal-egress".to_owned(), true,),
        ]
    );

    drop(client);
    shutdown.send(true).expect("signal server shutdown");
    server
        .await
        .expect("join response-boundary server")
        .expect("response-boundary server");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn grpc_loopback_exact_query_returns_typed_terminal() {
    let listener = bind_asp_client_grpc_tcp()
        .await
        .expect("bind public ASP Client Protocol endpoint");
    let endpoint = listener.local_addr().expect("client endpoint");
    let response_boundaries = Arc::new(Mutex::new(Vec::new()));
    let service = Arc::new(AspClientFrameService::new(
        Arc::new(ExactQueryDispatcher {
            cancellation_by_request: Arc::new(Mutex::new(BTreeMap::new())),
            response_boundaries: Arc::clone(&response_boundaries),
        }),
        |_| Ok(catalog()),
    ));
    let (shutdown, shutdown_rx) = tokio::sync::watch::channel(false);
    let server = tokio::spawn(serve_asp_client_grpc_tcp(listener, service, shutdown_rx));
    let client = AspClientGrpcTransport::connect_tcp(endpoint)
        .await
        .expect("connect public ASP Client Protocol stream");

    let initialized = client
        .call(ClientFrame::Initialize {
            base: base(),
            request_id: ClientRequestId::new("initialize").expect("request id"),
            client_info: ClientInfo {
                name: "thin-cli".to_owned(),
                version: "1".to_owned(),
            },
            capabilities: json!({"requestCancellation": true}),
        })
        .await
        .expect("initialize typed session");
    assert!(matches!(
        initialized,
        ClientFrame::Response {
            outcome: ClientOutcome::Ready,
            catalog: Some(_),
            ..
        }
    ));

    let terminal = client
        .call(exact_query_frame("exact-query".to_owned()))
        .await
        .expect("typed exact-query terminal");
    let ClientFrame::Response {
        request_id,
        outcome: ClientOutcome::Error,
        result: None,
        error: Some(error),
        ..
    } = terminal
    else {
        panic!("expected typed exact-query failure terminal");
    };
    assert_eq!(request_id.as_str(), "exact-query");
    assert_eq!(error["reasonKind"], "projection-missing");
    assert_eq!(
        error["terminal"]["schemaId"],
        "agent.semantic-protocols.asp-client-exact-query-failure"
    );
    assert_eq!(
        response_boundaries
            .lock()
            .expect("response boundaries")
            .iter()
            .filter(|(request_id, _, _)| request_id == "exact-query")
            .cloned()
            .collect::<Vec<_>>(),
        vec![
            (
                "exact-query".to_owned(),
                "schema-validate-serialize".to_owned(),
                true,
            ),
            ("exact-query".to_owned(), "terminal-egress".to_owned(), true,),
        ]
    );
    assert_eq!(error["terminal"]["phase"], "resident-selector-read");

    let large_terminal = client
        .call(large_response_frame())
        .await
        .expect("partitioned response terminal");
    let ClientFrame::Response {
        outcome: ClientOutcome::Ready,
        result: Some(result),
        ..
    } = large_terminal
    else {
        panic!("expected typed large response terminal");
    };
    assert!(result["source"].as_str().expect("source").len() > 4 * 1024 * 1024);

    let mut sequential_nanos = Vec::with_capacity(10);
    for index in 0..10 {
        let started = std::time::Instant::now();
        let response = client
            .call(exact_query_frame(format!("warm-sequential-{index}")))
            .await
            .expect("warm typed terminal");
        sequential_nanos.push(started.elapsed().as_nanos());
        assert!(matches!(
            response,
            ClientFrame::Response {
                outcome: ClientOutcome::Error,
                error: Some(_),
                ..
            }
        ));
    }
    sequential_nanos.sort_unstable();
    let sequential_p95_nanos = sequential_nanos[9];

    let mut concurrent = tokio::task::JoinSet::new();
    for index in 0..32 {
        let client = client.clone();
        concurrent.spawn(async move {
            let started = std::time::Instant::now();
            let response = client
                .call(exact_query_frame(format!("warm-concurrent-{index}")))
                .await?;
            Ok::<_, String>((started.elapsed().as_nanos(), response))
        });
    }
    let mut concurrent_nanos = Vec::with_capacity(32);
    while let Some(result) = concurrent.join_next().await {
        let (elapsed_nanos, response) = result
            .expect("join concurrent request")
            .expect("concurrent typed terminal");
        concurrent_nanos.push(elapsed_nanos);
        assert!(matches!(
            response,
            ClientFrame::Response {
                outcome: ClientOutcome::Error,
                error: Some(_),
                ..
            }
        ));
    }
    concurrent_nanos.sort_unstable();
    let concurrent_p95_nanos = concurrent_nanos[30];
    let latency_state = if sequential_p95_nanos < 1_000_000 && concurrent_p95_nanos < 1_000_000 {
        "passed"
    } else {
        "failed"
    };
    println!(
        "{{\"schemaId\":\"agent.semantic-protocols.grpc-warm-latency-receipt\",\"schemaVersion\":\"1\",\"state\":\"{latency_state}\",\"thresholdNanos\":1000000,\"sequentialCount\":10,\"sequentialTerminalCount\":10,\"sequentialP95Nanos\":{sequential_p95_nanos},\"concurrentCount\":32,\"concurrentTerminalCount\":32,\"concurrentP95Nanos\":{concurrent_p95_nanos}}}"
    );
    assert!(
        sequential_p95_nanos < 1_000_000,
        "warm sequential gRPC p95 must remain below 1ms: {sequential_p95_nanos}ns"
    );
    assert!(
        concurrent_p95_nanos < 1_000_000,
        "warm 32-concurrent gRPC p95 must remain below 1ms: {concurrent_p95_nanos}ns"
    );

    let lazy_client = AspClientGrpcTransport::connect_tcp(endpoint)
        .await
        .expect("connect lazy ASP Client gRPC transport");
    let lazy_terminal = lazy_client
        .call(exact_query_frame("request-before-initialize".to_owned()))
        .await
        .expect("pre-initialize request has one typed terminal");
    assert!(matches!(
        lazy_terminal,
        ClientFrame::Response {
            outcome: ClientOutcome::Error,
            result: None,
            error: Some(_),
            ..
        }
    ));

    let cancellation_request_id = ClientRequestId::new("cancel-request").expect("request id");
    let pending = client
        .begin_call(ClientFrame::Request {
            base: base(),
            request_id: cancellation_request_id.clone(),
            catalog_generation: format!("sha256:{}", "a".repeat(64)),
            workspace_generation: format!("blake3-256:{}", "b".repeat(64)),
            method: "test.cancellation-probe".to_owned(),
            params: json!({}),
            client_timing_witness: None,
        })
        .await
        .expect("begin cancellable request");
    client
        .cancel_pending(base(), cancellation_request_id)
        .await
        .expect("send correlated cancellation");
    let cancelled = pending.wait().await.expect("typed cancellation terminal");
    let ClientFrame::Response {
        request_id,
        outcome: ClientOutcome::Cancelled,
        result: None,
        error: Some(error),
        ..
    } = cancelled
    else {
        panic!("expected typed cancellation terminal");
    };
    assert_eq!(request_id.as_str(), "cancel-request");
    assert_eq!(error["reasonKind"], "client-request-cancelled");

    drop(lazy_client);
    drop(client);
    shutdown.send(true).expect("signal server shutdown");
    server
        .await
        .expect("join gRPC server")
        .expect("gRPC server");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn full_session_rejects_one_excess_data_call_and_preserves_cancellation_lane() {
    let listener = bind_asp_client_grpc_tcp()
        .await
        .expect("bind public ASP Client Protocol endpoint");
    let endpoint = listener.local_addr().expect("client endpoint");
    let service = Arc::new(AspClientFrameService::new(
        Arc::new(ExactQueryDispatcher {
            cancellation_by_request: Arc::new(Mutex::new(BTreeMap::new())),
            response_boundaries: Arc::new(Mutex::new(Vec::new())),
        }),
        |_| Ok(catalog()),
    ));
    let (shutdown, shutdown_rx) = tokio::sync::watch::channel(false);
    let server = tokio::spawn(serve_asp_client_grpc_tcp(listener, service, shutdown_rx));
    let client = AspClientGrpcTransport::connect_tcp(endpoint)
        .await
        .expect("connect public ASP Client Protocol stream");
    client
        .call(ClientFrame::Initialize {
            base: base(),
            request_id: ClientRequestId::new("backpressure-initialize").expect("request id"),
            client_info: ClientInfo {
                name: "grpc-test".to_owned(),
                version: "1".to_owned(),
            },
            capabilities: json!({"requestCancellation": true}),
        })
        .await
        .expect("initialize typed session");

    let held_count = CLIENT_FRAME_SESSION_CAPACITY;
    let mut held = Vec::with_capacity(held_count);
    for index in 0..held_count {
        let request_id =
            ClientRequestId::new(format!("backpressure-held-{index}")).expect("request id");
        let pending = client
            .begin_call(ClientFrame::Request {
                base: base(),
                request_id: request_id.clone(),
                catalog_generation: format!("sha256:{}", "a".repeat(64)),
                workspace_generation: format!("blake3-256:{}", "b".repeat(64)),
                method: "test.cancellation-probe".to_owned(),
                params: json!({}),
                client_timing_witness: None,
            })
            .await
            .expect("admit bounded data call");
        held.push((request_id, pending));
    }
    let rejected = match client
        .begin_call(exact_query_frame("backpressure-rejected".to_owned()))
        .await
    {
        Ok(_) => panic!("full session admitted an excess data call"),
        Err(error) => error,
    };
    assert!(rejected.contains("reasonKind=client-session-backpressure"));
    assert!(rejected.contains("capacity=32"));
    assert!(rejected.contains("pending=32"));
    assert!(rejected.contains("reservedControlSlots=1"));
    assert!(rejected.contains("retryAdmitted=false"));

    for (request_id, _) in &held {
        client
            .cancel_pending(base(), request_id.clone())
            .await
            .expect("reserved control lane admits cancellation");
    }
    for (request_id, pending) in held {
        let terminal = pending.wait().await.expect("typed cancellation terminal");
        let ClientFrame::Response {
            request_id: terminal_request_id,
            outcome: ClientOutcome::Cancelled,
            result: None,
            error: Some(error),
            ..
        } = terminal
        else {
            panic!("unexpected cancellation terminal for {request_id:?}");
        };
        assert_eq!(terminal_request_id, request_id);
        assert_eq!(
            error.get("reasonKind").and_then(serde_json::Value::as_str),
            Some("client-request-cancelled")
        );
    }
    assert_eq!(client.pending_call_count(), 0);

    drop(client);
    shutdown.send(true).expect("signal server shutdown");
    server
        .await
        .expect("join gRPC server")
        .expect("gRPC server");
}

#[tokio::test]
async fn stalled_loopback_handshake_returns_one_bounded_typed_failure() {
    let _listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("bind stalled listener");
    let endpoint = _listener.local_addr().expect("stalled endpoint");
    let started = tokio::time::Instant::now();

    let failure = tokio::time::timeout(
        std::time::Duration::from_secs(4),
        AspClientGrpcTransport::connect_tcp(endpoint),
    )
    .await
    .expect("transport owns a shorter connect deadline")
    .err()
    .expect("stalled handshake must fail");

    assert!(
        failure.contains("reasonKind=runtime-client-connect-deadline-exceeded")
            || failure.contains("reasonKind=runtime-client-session-deadline-exceeded"),
        "unexpected terminal: {failure}"
    );
    assert!(failure.contains("retryAdmitted=false"));
    assert!(started.elapsed() < std::time::Duration::from_secs(4));
}
