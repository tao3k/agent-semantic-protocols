use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;

use agent_semantic_client_protocol::{
    CLIENT_CATALOG_SCHEMA_ID, CLIENT_FRAME_SCHEMA_ID, CLIENT_PROTOCOL_ID, CLIENT_PROTOCOL_VERSION,
    ClientCapabilities, ClientFrame, ClientFrameBase, ClientInfo, ClientMethod, ClientOutcome,
    ClientParameter, ClientParameterCardinality, ClientParameterSource, ClientParameterType,
    ClientProtocolCatalog, ClientRequestId, ClientSessionId, ClientTransport,
    ClientWorkspaceIdentity, SCHEMA_VERSION,
};
use agent_semantic_client_server::{
    AspClientCancelFuture, AspClientDispatchFuture, AspClientDispatchRequest, AspClientDispatcher,
    AspClientProtocolHttpService, HttpJsonRequest,
};
use serde_json::json;
use tokio::sync::Notify;

struct Dispatcher {
    requests: Mutex<Vec<AspClientDispatchRequest>>,
}

impl AspClientDispatcher for Dispatcher {
    fn dispatch(&self, request: AspClientDispatchRequest) -> AspClientDispatchFuture {
        self.requests.lock().expect("requests").push(request);
        Box::pin(async { Ok(json!({"state": "ready"})) })
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

struct SlowDispatcher {
    entered: Arc<Notify>,
    release: Arc<Notify>,
    cancelled: Arc<AtomicBool>,
}

impl AspClientDispatcher for SlowDispatcher {
    fn dispatch(&self, _: AspClientDispatchRequest) -> AspClientDispatchFuture {
        let entered = Arc::clone(&self.entered);
        let release = Arc::clone(&self.release);
        let cancelled = Arc::clone(&self.cancelled);
        Box::pin(async move {
            entered.notify_one();
            release.notified().await;
            if cancelled.load(Ordering::Acquire) {
                Err(agent_semantic_client_server::AspClientDispatchError {
                    reason_kind: "client-request-cancelled".to_owned(),
                    message: "cancelled by client".to_owned(),
                })
            } else {
                Ok(json!({"state": "ready"}))
            }
        })
    }

    fn cancel(
        &self,
        _: &ClientWorkspaceIdentity,
        _: &ClientSessionId,
        _: &ClientRequestId,
    ) -> AspClientCancelFuture {
        self.cancelled.store(true, Ordering::Release);
        self.release.notify_waiters();
        Box::pin(async { true })
    }
}

fn base() -> ClientFrameBase {
    ClientFrameBase {
        schema_id: CLIENT_FRAME_SCHEMA_ID.to_owned(),
        schema_version: SCHEMA_VERSION.to_owned(),
        protocol_id: CLIENT_PROTOCOL_ID.to_owned(),
        protocol_version: CLIENT_PROTOCOL_VERSION.to_owned(),
        session_id: ClientSessionId::new("session").expect("session id"),
        workspace_identity: ClientWorkspaceIdentity::new("workspace").expect("workspace id"),
        trace_context: None,
    }
}

fn request_id(value: &str) -> ClientRequestId {
    ClientRequestId::new(value).expect("request id")
}

fn catalog() -> ClientProtocolCatalog {
    ClientProtocolCatalog {
        schema_id: CLIENT_CATALOG_SCHEMA_ID.to_owned(),
        schema_version: SCHEMA_VERSION.to_owned(),
        protocol_id: CLIENT_PROTOCOL_ID.to_owned(),
        protocol_version: CLIENT_PROTOCOL_VERSION.to_owned(),
        catalog_generation: format!("sha256:{}", "a".repeat(64)),
        workspace_generation: format!("blake3-256:{}", "b".repeat(64)),
        transports: vec![ClientTransport::HttpJson],
        capabilities: ClientCapabilities {
            request_cancellation: true,
            events: true,
            streaming: false,
            trace_context: true,
        },
        methods: vec![ClientMethod {
            method: "rust.search".to_owned(),
            route_id: "rust.search".to_owned(),
            request_schema_id: CLIENT_FRAME_SCHEMA_ID.to_owned(),
            response_schema_id: "agent.semantic-protocols.search-packet".to_owned(),
            error_schema_ids: vec!["agent.semantic-protocols.route-failure".to_owned()],
            parameters: vec![ClientParameter {
                name: "query".to_owned(),
                value_type: ClientParameterType::String,
                cardinality: ClientParameterCardinality::Required,
                source: ClientParameterSource::Request,
            }],
            cancellable: true,
            streaming: false,
        }],
    }
}

#[tokio::test]
async fn http_binding_uses_the_same_workspace_scoped_session_owner() {
    let dispatcher = Arc::new(Dispatcher {
        requests: Mutex::new(Vec::new()),
    });
    let service = AspClientProtocolHttpService::new(Arc::clone(&dispatcher), |_| Ok(catalog()));
    let frame = ClientFrame::Initialize {
        base: base(),
        request_id: request_id("initialize-http"),
        project_root: "/workspace".to_owned(),
        client_info: ClientInfo {
            name: "http-test".to_owned(),
            version: "1".to_owned(),
        },
        capabilities: json!({}),
    };
    let response = service
        .handle(HttpJsonRequest {
            method: "POST".to_owned(),
            path: "/protocol/frame".to_owned(),
            body: serde_json::to_vec(&frame).expect("encode frame").into(),
        })
        .await
        .expect("HTTP response");
    assert_eq!(response.status, 200);
    let response: ClientFrame = serde_json::from_slice(&response.body).expect("response frame");
    assert!(matches!(
        response,
        ClientFrame::Response {
            outcome: ClientOutcome::Ready,
            catalog: Some(_),
            ..
        }
    ));
}

#[tokio::test]
async fn http_cancellation_releases_session_admission_before_dispatch() {
    let dispatcher = Arc::new(SlowDispatcher {
        entered: Arc::new(Notify::new()),
        release: Arc::new(Notify::new()),
        cancelled: Arc::new(AtomicBool::new(false)),
    });
    let service = Arc::new(AspClientProtocolHttpService::new(
        Arc::clone(&dispatcher),
        |_| Ok(catalog()),
    ));
    let initialize = ClientFrame::Initialize {
        base: base(),
        request_id: request_id("initialize-cancel"),
        project_root: "/workspace".to_owned(),
        client_info: ClientInfo {
            name: "cancel-test".to_owned(),
            version: "1".to_owned(),
        },
        capabilities: json!({}),
    };
    service
        .handle(HttpJsonRequest {
            method: "POST".to_owned(),
            path: "/protocol/frame".to_owned(),
            body: serde_json::to_vec(&initialize)
                .expect("encode initialize")
                .into(),
        })
        .await
        .expect("initialize response");

    let protocol_catalog = catalog();
    let request = ClientFrame::Request {
        base: base(),
        request_id: request_id("slow-request"),
        catalog_generation: protocol_catalog.catalog_generation,
        workspace_generation: protocol_catalog.workspace_generation,
        method: "rust.search".to_owned(),
        params: json!({"query": "owner"}),
    };
    let request_service = Arc::clone(&service);
    let request_task = tokio::spawn(async move {
        request_service
            .handle(HttpJsonRequest {
                method: "POST".to_owned(),
                path: "/protocol/frame".to_owned(),
                body: serde_json::to_vec(&request).expect("encode request").into(),
            })
            .await
            .expect("request response")
    });
    dispatcher.entered.notified().await;

    let cancel = ClientFrame::Cancel {
        base: base(),
        request_id: request_id("slow-request"),
    };
    let cancel_response = tokio::time::timeout(
        Duration::from_millis(100),
        service.handle(HttpJsonRequest {
            method: "POST".to_owned(),
            path: "/protocol/frame".to_owned(),
            body: serde_json::to_vec(&cancel).expect("encode cancel").into(),
        }),
    )
    .await
    .expect("cancel must not wait for dispatch")
    .expect("cancel response");
    let cancel_frame: ClientFrame =
        serde_json::from_slice(&cancel_response.body).expect("cancel frame");
    assert!(matches!(
        cancel_frame,
        ClientFrame::Response {
            outcome: ClientOutcome::Cancelled,
            ..
        }
    ));

    let request_response = request_task.await.expect("request task");
    let request_frame: ClientFrame =
        serde_json::from_slice(&request_response.body).expect("request frame");
    assert!(matches!(
        request_frame,
        ClientFrame::Response {
            outcome: ClientOutcome::Cancelled,
            ..
        }
    ));
}
#[tokio::test]
async fn http_dispatch_lazily_admits_without_initialize() {
    let dispatcher = Arc::new(Dispatcher {
        requests: Mutex::new(Vec::new()),
    });
    let service = AspClientProtocolHttpService::new(Arc::clone(&dispatcher), |_| Ok(catalog()));
    let frame = ClientFrame::Dispatch {
        base: base(),
        request_id: request_id("dispatch-without-initialize"),
        project_root: "/workspace".to_owned(),
        client_info: ClientInfo {
            name: "asp-client-test".to_owned(),
            version: "1".to_owned(),
        },
        method: "rust.search".to_owned(),
        params: json!({"operation": "pipe", "query": "ready"}),
    };

    let started = tokio::time::Instant::now();
    let response = service
        .handle(HttpJsonRequest {
            method: "POST".to_owned(),
            path: "/protocol/frame".to_owned(),
            body: serde_json::to_vec(&frame).expect("encode frame").into(),
        })
        .await
        .expect("HTTP response");
    let elapsed = started.elapsed();

    assert_eq!(response.status, 200);
    assert!(
        elapsed < std::time::Duration::from_millis(1),
        "typed ASP Client dispatch exceeded 1ms: {elapsed:?}"
    );
    let response: ClientFrame = serde_json::from_slice(&response.body).expect("response frame");
    assert!(matches!(
        response,
        ClientFrame::Response {
            outcome: ClientOutcome::Ready,
            catalog: None,
            ..
        }
    ));
    let requests = dispatcher.requests.lock().expect("requests");
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "rust.search");
}
