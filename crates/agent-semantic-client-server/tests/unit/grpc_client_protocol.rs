use std::sync::Arc;

use agent_semantic_client_protocol::{
    CLIENT_CATALOG_SCHEMA_ID, CLIENT_FRAME_SCHEMA_ID, CLIENT_PROTOCOL_ID, CLIENT_PROTOCOL_VERSION,
    ClientCapabilities, ClientFrame, ClientFrameBase, ClientInfo, ClientOutcome,
    ClientProtocolCatalog, ClientRequestId, ClientSessionId, ClientTransport,
    ClientWorkspaceIdentity, SCHEMA_VERSION,
};
use agent_semantic_client_server::{
    AspClientCancelFuture, AspClientDispatchError, AspClientDispatchFuture,
    AspClientDispatchRequest, AspClientDispatcher, AspClientFrameService, AspClientGrpcTransport,
    bind_asp_client_grpc_unix, serve_asp_client_grpc_unix, serve_asp_client_http_json,
};
use serde_json::json;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};

struct ExactQueryDispatcher;

impl AspClientDispatcher for ExactQueryDispatcher {
    fn dispatch(&self, request: AspClientDispatchRequest) -> AspClientDispatchFuture {
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
        _: &ClientRequestId,
    ) -> AspClientCancelFuture {
        Box::pin(async { true })
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

#[tokio::test]
async fn grpc_unix_exact_query_returns_typed_terminal() {
    let temporary = tempfile::tempdir().expect("temporary socket root");
    let socket_path = temporary.path().join("asp-client.grpc.sock");
    let listener = bind_asp_client_grpc_unix(&socket_path)
        .await
        .expect("bind public ASP Client Protocol socket");
    let service = Arc::new(AspClientFrameService::new(
        Arc::new(ExactQueryDispatcher),
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
        .call(ClientFrame::Dispatch {
            base: base(),
            request_id: ClientRequestId::new("exact-query").expect("request id"),
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
        })
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

    let cancelled = client
        .call(ClientFrame::Cancel {
            base: base(),
            request_id: ClientRequestId::new("cancel-request").expect("request id"),
        })
        .await
        .expect("typed cancellation terminal");
    let ClientFrame::Response {
        request_id,
        outcome: ClientOutcome::Cancelled,
        result: None,
        error: None,
        ..
    } = cancelled
    else {
        panic!("expected typed cancellation terminal");
    };
    assert_eq!(request_id.as_str(), "cancel-request");

    drop(lazy_client);
    drop(client);
    shutdown.send(true).expect("signal server shutdown");
    server
        .await
        .expect("join gRPC server")
        .expect("gRPC server");
}

#[tokio::test]
async fn http_json_initialize_uses_the_same_typed_frame_service() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind public ASP Client Protocol HTTP endpoint");
    let address = listener.local_addr().expect("HTTP endpoint address");
    let service = Arc::new(AspClientFrameService::new(
        Arc::new(ExactQueryDispatcher),
        |_| Ok(catalog()),
    ));
    let (shutdown, shutdown_rx) = tokio::sync::watch::channel(false);
    let server = tokio::spawn(serve_asp_client_http_json(listener, service, shutdown_rx));

    let request = ClientFrame::Initialize {
        base: base(),
        request_id: ClientRequestId::new("http-initialize").expect("request id"),
        project_root: "/workspace".to_owned(),
        client_info: ClientInfo {
            name: "http-test".to_owned(),
            version: "1".to_owned(),
        },
        capabilities: json!({"requestCancellation": true}),
    };
    let body = serde_json::to_vec(&request).expect("encode HTTP frame");
    let mut client = TcpStream::connect(address)
        .await
        .expect("connect HTTP JSON endpoint");
    client
        .write_all(
            format!(
                "POST /protocol/frame HTTP/1.1\r\nHost: {address}\r\nContent-Type: application/json\r\nConnection: close\r\nContent-Length: {}\r\n\r\n",
                body.len()
            )
            .as_bytes(),
        )
        .await
        .expect("write HTTP JSON headers");
    client.write_all(&body).await.expect("write HTTP JSON body");
    let mut wire = Vec::new();
    client
        .read_to_end(&mut wire)
        .await
        .expect("read HTTP JSON response");
    let separator = wire
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .expect("HTTP response headers");
    let headers = String::from_utf8_lossy(&wire[..separator]);
    assert!(
        headers.starts_with("HTTP/1.1 200"),
        "unexpected response: {headers}"
    );
    let frame: ClientFrame =
        serde_json::from_slice(&wire[separator + 4..]).expect("decode HTTP frame");
    assert!(matches!(
        frame,
        ClientFrame::Response {
            request_id,
            outcome: ClientOutcome::Ready,
            catalog: Some(_),
            ..
        } if request_id.as_str() == "http-initialize"
    ));

    shutdown.send(true).expect("signal HTTP server shutdown");
    server
        .await
        .expect("join HTTP server")
        .expect("HTTP server");
}
