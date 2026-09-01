use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use agent_semantic_client_protocol::{
    CLIENT_CATALOG_SCHEMA_ID, CLIENT_FRAME_SCHEMA_ID, CLIENT_PROTOCOL_ID, CLIENT_PROTOCOL_VERSION,
    ClientCapabilities, ClientFrame, ClientFrameBase, ClientInfo, ClientOutcome,
    ClientProtocolCatalog, ClientRequestId, ClientSessionId, ClientTransport,
    ClientWorkspaceIdentity, SCHEMA_VERSION,
};
use agent_semantic_client_server::{
    AspClientCancelFuture, AspClientDispatchError, AspClientDispatchFuture,
    AspClientDispatchRequest, AspClientDispatcher, AspClientFrameService, AspClientGrpcTransport,
    CLIENT_FRAME_SESSION_CAPACITY, CLIENT_FRAME_SESSION_CONTROL_RESERVE, bind_asp_client_grpc_unix,
    serve_asp_client_grpc_unix,
};
use serde_json::json;

struct ExactQueryDispatcher {
    cancellation_by_request: Arc<Mutex<BTreeMap<String, Arc<tokio::sync::Notify>>>>,
}

impl AspClientDispatcher for ExactQueryDispatcher {
    fn dispatch(&self, request: AspClientDispatchRequest) -> AspClientDispatchFuture {
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
                    "reasonKind": "projection-missing",
                    "recommendedNext": {"action": "query-owner-or-admitted-scope"}
                })),
            })
        })
    }

    fn cancel(
        &self,
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
}

fn base() -> ClientFrameBase {
    ClientFrameBase {
        schema_id: CLIENT_FRAME_SCHEMA_ID.to_owned(),
        schema_version: SCHEMA_VERSION.to_owned(),
        protocol_id: CLIENT_PROTOCOL_ID.to_owned(),
        protocol_version: CLIENT_PROTOCOL_VERSION.to_owned(),
        session_id: ClientSessionId::new("grpc-exact-query-session").expect("session id"),
        workspace_identity: ClientWorkspaceIdentity::new("grpc-workspace")
            .expect("workspace identity"),
        trace_context: None,
    }
}

fn exact_query_frame(request_id: String) -> ClientFrame {
    ClientFrame::Dispatch {
        base: base(),
        request_id: ClientRequestId::new(request_id).expect("request id"),
        project_root: "/workspace".to_owned(),
        client_info: ClientInfo {
            name: "thin-cli".to_owned(),
            version: "1".to_owned(),
        },
        method: "rust.query".to_owned(),
        params: json!({
            "schemaId": "agent.semantic-protocols.asp-client-exact-query-request",
            "schemaVersion": "1",
            "selector": "rust://src/lib.rs#item/function/missing",
            "projection": "source"
        }),
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
        methods: Vec::new(),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn grpc_unix_exact_query_returns_typed_terminal() {
    let temporary = tempfile::tempdir().expect("temporary socket root");
    let socket_path = temporary.path().join("asp-client.grpc.sock");
    let listener = bind_asp_client_grpc_unix(&socket_path)
        .await
        .expect("bind public ASP Client Protocol socket");
    let service = Arc::new(AspClientFrameService::new(
        Arc::new(ExactQueryDispatcher {
            cancellation_by_request: Arc::new(Mutex::new(BTreeMap::new())),
        }),
        |_| Ok(catalog()),
    ));
    let (shutdown, shutdown_rx) = tokio::sync::watch::channel(false);
    let server = tokio::spawn(serve_asp_client_grpc_unix(listener, service, shutdown_rx));
    let client = AspClientGrpcTransport::connect_unix(&socket_path)
        .await
        .expect("connect public ASP Client Protocol stream");

    let initialized = client
        .call(ClientFrame::Initialize {
            base: base(),
            request_id: ClientRequestId::new("initialize").expect("request id"),
            project_root: "/workspace".to_owned(),
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
    assert_eq!(error["terminal"]["phase"], "resident-selector-read");

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

    let lazy_client = AspClientGrpcTransport::connect_unix(&socket_path)
        .await
        .expect("connect lazy ASP Client gRPC transport");
    let lazy_terminal = lazy_client
        .call(ClientFrame::Dispatch {
            base: base(),
            request_id: ClientRequestId::new("lazy-exact-query").expect("request id"),
            project_root: "/workspace".to_owned(),
            client_info: ClientInfo {
                name: "grpc-test".to_owned(),
                version: "1".to_owned(),
            },
            method: "rust.query".to_owned(),
            params: json!({"selector": "rust://crate#item/function/example"}),
        })
        .await
        .expect("lazy typed exact-query terminal");
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
        .begin_call(ClientFrame::Dispatch {
            base: base(),
            request_id: cancellation_request_id.clone(),
            project_root: "/workspace".to_owned(),
            client_info: ClientInfo {
                name: "grpc-test".to_owned(),
                version: "1".to_owned(),
            },
            method: "test.cancellation-probe".to_owned(),
            params: json!({}),
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
    let temporary = tempfile::tempdir().expect("temporary socket root");
    let socket_path = temporary.path().join("asp-client-backpressure.grpc.sock");
    let listener = bind_asp_client_grpc_unix(&socket_path)
        .await
        .expect("bind public ASP Client Protocol socket");
    let service = Arc::new(AspClientFrameService::new(
        Arc::new(ExactQueryDispatcher {
            cancellation_by_request: Arc::new(Mutex::new(BTreeMap::new())),
        }),
        |_| Ok(catalog()),
    ));
    let (shutdown, shutdown_rx) = tokio::sync::watch::channel(false);
    let server = tokio::spawn(serve_asp_client_grpc_unix(listener, service, shutdown_rx));
    let client = AspClientGrpcTransport::connect_unix(&socket_path)
        .await
        .expect("connect public ASP Client Protocol stream");
    client
        .call(ClientFrame::Initialize {
            base: base(),
            request_id: ClientRequestId::new("backpressure-initialize").expect("request id"),
            project_root: "/workspace".to_owned(),
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
            .begin_call(ClientFrame::Dispatch {
                base: base(),
                request_id: request_id.clone(),
                project_root: "/workspace".to_owned(),
                client_info: ClientInfo {
                    name: "grpc-test".to_owned(),
                    version: "1".to_owned(),
                },
                method: "test.cancellation-probe".to_owned(),
                params: json!({}),
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
async fn stalled_unix_handshake_returns_one_bounded_typed_failure() {
    let temporary = tempfile::tempdir().expect("temporary socket root");
    let socket_path = temporary.path().join("stalled.sock");
    let _listener = tokio::net::UnixListener::bind(&socket_path).expect("bind stalled listener");
    let started = tokio::time::Instant::now();

    let failure = tokio::time::timeout(
        std::time::Duration::from_secs(4),
        AspClientGrpcTransport::connect_unix(&socket_path),
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
