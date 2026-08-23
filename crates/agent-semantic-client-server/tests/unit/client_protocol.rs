use std::sync::{Arc, Mutex};

use agent_semantic_client_protocol::{
    CLIENT_CATALOG_SCHEMA_ID, CLIENT_FRAME_SCHEMA_ID, CLIENT_PROTOCOL_ID, CLIENT_PROTOCOL_VERSION,
    ClientCapabilities, ClientFrame, ClientFrameBase, ClientInfo, ClientMethod, ClientOutcome,
    ClientParameter, ClientParameterCardinality, ClientParameterSource, ClientParameterType,
    ClientProtocolCatalog, ClientTransport, SCHEMA_VERSION,
};
use agent_semantic_client_server::{
    AspClientDispatchFuture, AspClientDispatchRequest, AspClientDispatcher,
    AspClientProtocolHttpService, AspClientProtocolSession, HttpJsonRequest,
};
use serde_json::json;

struct Dispatcher {
    requests: Mutex<Vec<AspClientDispatchRequest>>,
}

impl AspClientDispatcher for Dispatcher {
    fn dispatch(&self, request: AspClientDispatchRequest) -> AspClientDispatchFuture {
        self.requests.lock().expect("requests").push(request);
        Box::pin(async { Ok(json!({"state": "ready"})) })
    }

    fn cancel(&self, _: &str, _: &str, _: &str) -> bool {
        true
    }
}

fn base() -> ClientFrameBase {
    ClientFrameBase {
        schema_id: CLIENT_FRAME_SCHEMA_ID.to_owned(),
        schema_version: SCHEMA_VERSION.to_owned(),
        protocol_id: CLIENT_PROTOCOL_ID.to_owned(),
        protocol_version: CLIENT_PROTOCOL_VERSION.to_owned(),
        session_id: "session".to_owned(),
        workspace_identity: "workspace".to_owned(),
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
async fn initialize_returns_catalog_and_request_dispatches() {
    let dispatcher = Arc::new(Dispatcher {
        requests: Mutex::new(Vec::new()),
    });
    let mut session =
        AspClientProtocolSession::new(catalog(), Arc::clone(&dispatcher)).expect("session");
    let initialized = session
        .handle(ClientFrame::Initialize {
            base: base(),
            request_id: "initialize".to_owned(),
            client_info: ClientInfo {
                name: "test".to_owned(),
                version: "1".to_owned(),
            },
            capabilities: json!({}),
        })
        .await
        .expect("initialize response");
    assert!(matches!(
        initialized,
        ClientFrame::Response {
            outcome: ClientOutcome::Ready,
            catalog: Some(_),
            ..
        }
    ));

    let catalog = catalog();
    let result = session
        .handle(ClientFrame::Request {
            base: base(),
            request_id: "request".to_owned(),
            catalog_generation: catalog.catalog_generation,
            workspace_generation: catalog.workspace_generation,
            method: "rust.search".to_owned(),
            params: json!({"query": "owner"}),
        })
        .await
        .expect("request response");
    assert!(matches!(
        result,
        ClientFrame::Response {
            outcome: ClientOutcome::Ready,
            ..
        }
    ));
    assert_eq!(dispatcher.requests.lock().expect("requests").len(), 1);
}

#[tokio::test]
async fn http_binding_uses_the_same_workspace_scoped_session_owner() {
    let dispatcher = Arc::new(Dispatcher {
        requests: Mutex::new(Vec::new()),
    });
    let service = AspClientProtocolHttpService::new(Arc::clone(&dispatcher), |_| Ok(catalog()));
    let frame = ClientFrame::Initialize {
        base: base(),
        request_id: "initialize-http".to_owned(),
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
