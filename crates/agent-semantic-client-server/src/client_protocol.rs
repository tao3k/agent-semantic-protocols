use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use agent_semantic_client_protocol::{
    ClientFrame, ClientFrameBase, ClientOutcome, ClientProtocolCatalog, ClientRequestId,
    ClientSessionId, ClientWorkspaceIdentity,
};
use agent_semantic_http_json::{HttpJsonRequest, HttpJsonResponse};
use serde_json::{Value, json};

pub type AspClientDispatchFuture =
    Pin<Box<dyn Future<Output = Result<Value, AspClientDispatchError>> + Send>>;
pub type AspClientCancelFuture = Pin<Box<dyn Future<Output = bool> + Send>>;

#[derive(Clone, Debug, PartialEq)]
pub struct AspClientDispatchRequest {
    pub session_id: ClientSessionId,
    pub workspace_identity: ClientWorkspaceIdentity,
    pub request_id: ClientRequestId,
    pub method: String,
    pub params: Value,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AspClientDispatchError {
    pub reason_kind: String,
    pub message: String,
}

pub trait AspClientDispatcher: Send + Sync + 'static {
    fn dispatch(&self, request: AspClientDispatchRequest) -> AspClientDispatchFuture;

    fn cancel(
        &self,
        workspace_identity: &ClientWorkspaceIdentity,
        session_id: &ClientSessionId,
        request_id: &ClientRequestId,
    ) -> AspClientCancelFuture;
}

/// One protocol session hosted by the ASP Client Server.  It is transport
/// neutral: HTTP/JSON and Runtime IPC decode the same `ClientFrame` and call
/// this owner, so lifecycle, generation, parameter, and isolation admission
/// cannot drift between bindings.
type CatalogResolution =
    std::pin::Pin<Box<dyn Future<Output = Result<ClientProtocolCatalog, String>> + Send>>;
type CatalogResolver = dyn Fn(String, String, String) -> CatalogResolution + Send + Sync + 'static;

/// Concurrent HTTP/JSON binding for the transport-neutral protocol owner.
/// Admission holds a per-session lock only for the synchronous state
/// transition; dispatch runs after the lock is released so cancellation and
/// unrelated requests cannot deadlock behind a long operation.
pub struct AspClientProtocolHttpService<D> {
    dispatcher: Arc<D>,
    resolve_catalog: Arc<CatalogResolver>,
}

async fn execute_admitted<D: AspClientDispatcher>(
    dispatcher: Arc<D>,
    catalog: Option<ClientProtocolCatalog>,
    frame: ClientFrame,
) -> Option<ClientFrame> {
    let base = frame.base().clone();
    match frame {
        ClientFrame::Initialize { request_id, .. } => Some(response(
            base,
            request_id,
            ClientOutcome::Ready,
            None,
            None,
            catalog,
        )),
        ClientFrame::Dispatch {
            request_id,
            method,
            params,
            ..
        }
        | ClientFrame::Request {
            request_id,
            method,
            params,
            ..
        } => {
            let dispatched = dispatcher
                .dispatch(AspClientDispatchRequest {
                    session_id: base.session_id.clone(),
                    workspace_identity: base.workspace_identity.clone(),
                    request_id: request_id.clone(),
                    method,
                    params,
                })
                .await;
            Some(match dispatched {
                Ok(result) => response(
                    base,
                    request_id,
                    ClientOutcome::Ready,
                    Some(result),
                    None,
                    None,
                ),
                Err(error) => {
                    let outcome = if error.reason_kind == "client-request-cancelled" {
                        ClientOutcome::Cancelled
                    } else {
                        ClientOutcome::Error
                    };
                    response(
                        base,
                        request_id,
                        outcome,
                        None,
                        Some(json!({
                            "reasonKind": error.reason_kind,
                            "message": error.message,
                        })),
                        None,
                    )
                }
            })
        }
        ClientFrame::Cancel { request_id, .. } => {
            let _ = dispatcher
                .cancel(&base.workspace_identity, &base.session_id, &request_id)
                .await;
            Some(response(
                base,
                request_id,
                ClientOutcome::Cancelled,
                None,
                None,
                None,
            ))
        }
        ClientFrame::Shutdown { request_id, .. } => Some(response(
            base,
            request_id,
            ClientOutcome::Ready,
            None,
            None,
            None,
        )),
        ClientFrame::Exit { .. } => None,
        ClientFrame::Response { request_id, .. } => Some(response(
            base,
            request_id,
            ClientOutcome::Error,
            None,
            Some(json!({
                "reasonKind": "client-response-direction-denied",
                "message": "clients cannot send response frames to ASP Server",
            })),
            None,
        )),
        ClientFrame::Event { .. } => None,
    }
}

impl<D: AspClientDispatcher> AspClientProtocolHttpService<D> {
    pub fn new(
        dispatcher: Arc<D>,
        resolve_catalog: impl Fn(&str) -> Result<ClientProtocolCatalog, String> + Send + Sync + 'static,
    ) -> Self {
        Self::new_async(
            dispatcher,
            move |workspace_identity, _session_id, _project_root| {
                let result = resolve_catalog(&workspace_identity);
                async move { result }
            },
        )
    }

    pub fn new_async<F, Fut>(dispatcher: Arc<D>, resolve_catalog: F) -> Self
    where
        F: Fn(String, String, String) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<ClientProtocolCatalog, String>> + Send + 'static,
    {
        Self {
            dispatcher,
            resolve_catalog: Arc::new(move |workspace_identity, session_id, project_root| {
                Box::pin(resolve_catalog(
                    workspace_identity,
                    session_id,
                    project_root,
                ))
            }),
        }
    }

    pub async fn handle(&self, request: HttpJsonRequest) -> Result<HttpJsonResponse, String> {
        if request.method == "GET" && request.path == "/health" {
            return HttpJsonResponse::json(
                200,
                &json!({
                    "protocolId": agent_semantic_client_protocol::CLIENT_PROTOCOL_ID,
                    "protocolVersion": agent_semantic_client_protocol::CLIENT_PROTOCOL_VERSION,
                    "state": "ready",
                }),
            );
        }
        if request.method != "POST" || request.path != "/protocol/frame" {
            return HttpJsonResponse::json(
                404,
                &json!({
                    "reasonKind": "client-http-route-missing",
                    "message": "ASP Client Protocol uses POST /protocol/frame",
                }),
            );
        }
        let frame: ClientFrame = match serde_json::from_slice(&request.body) {
            Ok(frame) => frame,
            Err(error) => {
                return HttpJsonResponse::json(
                    400,
                    &json!({
                        "reasonKind": "client-frame-decode-failed",
                        "message": error.to_string(),
                    }),
                );
            }
        };
        let base = frame.base().clone();

        let catalog = if let ClientFrame::Initialize { project_root, .. }
        | ClientFrame::Dispatch { project_root, .. } = &frame
        {
            Some(
                match (self.resolve_catalog)(
                    base.workspace_identity.as_str().to_owned(),
                    base.session_id.as_str().to_owned(),
                    project_root.clone(),
                )
                .await
                {
                    Ok(catalog) => catalog,
                    Err(message) => {
                        return HttpJsonResponse::json(
                            409,
                            &json!({
                                "reasonKind": "client-workspace-catalog-unavailable",
                                "message": message,
                            }),
                        );
                    }
                },
            )
        } else {
            None
        };
        let response_frame = execute_admitted(Arc::clone(&self.dispatcher), catalog, frame).await;
        match response_frame {
            Some(response_frame) => HttpJsonResponse::json(
                200,
                &serde_json::to_value(response_frame)
                    .map_err(|error| format!("encode ASP Client Protocol frame: {error}"))?,
            ),
            None => HttpJsonResponse::json(200, &json!({"state": "exited"})),
        }
    }
}

pub async fn serve_asp_client_protocol_http<D: AspClientDispatcher>(
    listener: tokio::net::TcpListener,
    shutdown: tokio::sync::watch::Receiver<bool>,
    service: Arc<AspClientProtocolHttpService<D>>,
) -> Result<(), String> {
    agent_semantic_http_json::serve_http_json_h2(listener, shutdown, move |request| {
        let service = Arc::clone(&service);
        async move { service.handle(request).await }
    })
    .await
}

fn response(
    base: ClientFrameBase,
    request_id: ClientRequestId,
    outcome: ClientOutcome,
    result: Option<Value>,
    error: Option<Value>,
    catalog: Option<ClientProtocolCatalog>,
) -> ClientFrame {
    ClientFrame::Response {
        base,
        request_id,
        outcome,
        result,
        error,
        catalog,
    }
}
