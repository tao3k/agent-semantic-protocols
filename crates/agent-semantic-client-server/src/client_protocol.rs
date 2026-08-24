use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use agent_semantic_client_protocol::{
    ClientFrame, ClientFrameBase, ClientOutcome, ClientProtocolCatalog, ClientRequestId,
    ClientSession, ClientSessionId, ClientWorkspaceIdentity,
};
use agent_semantic_http_json::{HttpJsonRequest, HttpJsonResponse};
use serde_json::{Value, json};

pub type AspClientDispatchFuture =
    Pin<Box<dyn Future<Output = Result<Value, AspClientDispatchError>> + Send>>;

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
    ) -> bool;
}

/// One protocol session hosted by the ASP Client Server.  It is transport
/// neutral: HTTP/JSON and Runtime IPC decode the same `ClientFrame` and call
/// this owner, so lifecycle, generation, parameter, and isolation admission
/// cannot drift between bindings.
pub struct AspClientProtocolSession<D> {
    catalog: ClientProtocolCatalog,
    session: ClientSession,
    dispatcher: Arc<D>,
}

type CatalogResolution =
    std::pin::Pin<Box<dyn Future<Output = Result<ClientProtocolCatalog, String>> + Send>>;
type CatalogResolver = dyn Fn(String, String) -> CatalogResolution + Send + Sync + 'static;

/// Concurrent HTTP/JSON binding for the transport-neutral protocol owner.
/// Admission holds a per-session lock only for the synchronous state
/// transition; dispatch runs after the lock is released so cancellation and
/// unrelated requests cannot deadlock behind a long operation.
pub struct AspClientProtocolHttpService<D> {
    dispatcher: Arc<D>,
    resolve_catalog: Arc<CatalogResolver>,
    sessions: tokio::sync::RwLock<
        HashMap<
            (ClientWorkspaceIdentity, ClientSessionId),
            Arc<tokio::sync::Mutex<AspClientProtocolSession<D>>>,
        >,
    >,
}

impl<D: AspClientDispatcher> AspClientProtocolSession<D> {
    pub fn new(catalog: ClientProtocolCatalog, dispatcher: Arc<D>) -> Result<Self, String> {
        catalog
            .validate()
            .map_err(|error| format!("{}: {}", error.reason_kind, error.message))?;
        Ok(Self {
            catalog,
            session: ClientSession::default(),
            dispatcher,
        })
    }

    pub async fn handle(&mut self, frame: ClientFrame) -> Option<ClientFrame> {
        let base = frame.base().clone();
        let request_id = correlated_request_id(&frame).cloned();
        if let Err(error) = self.admit(&frame) {
            return request_id.map(|request_id| {
                admission_error_response(base, request_id, error.reason_kind, error.message)
            });
        }
        execute_admitted(Arc::clone(&self.dispatcher), self.catalog.clone(), frame).await
    }

    fn admit(
        &mut self,
        frame: &ClientFrame,
    ) -> Result<(), agent_semantic_client_protocol::ClientAdmissionError> {
        self.session.admit(frame, &self.catalog).map(|_| ())
    }
}

async fn execute_admitted<D: AspClientDispatcher>(
    dispatcher: Arc<D>,
    catalog: ClientProtocolCatalog,
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
            Some(catalog),
        )),
        ClientFrame::Request {
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
            let _ = dispatcher.cancel(&base.workspace_identity, &base.session_id, &request_id);
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
        Self::new_async(dispatcher, move |workspace_identity, _project_root| {
            let result = resolve_catalog(&workspace_identity);
            async move { result }
        })
    }

    pub fn new_async<F, Fut>(dispatcher: Arc<D>, resolve_catalog: F) -> Self
    where
        F: Fn(String, String) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<ClientProtocolCatalog, String>> + Send + 'static,
    {
        Self {
            dispatcher,
            resolve_catalog: Arc::new(move |workspace_identity, project_root| {
                Box::pin(resolve_catalog(workspace_identity, project_root))
            }),
            sessions: tokio::sync::RwLock::new(HashMap::new()),
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
        let key = (base.workspace_identity.clone(), base.session_id.clone());
        let session = if let ClientFrame::Initialize { project_root, .. } = &frame {
            if let Some(session) = self.sessions.read().await.get(&key).cloned() {
                session
            } else {
                let catalog = match (self.resolve_catalog)(
                    base.workspace_identity.as_str().to_owned(),
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
                };
                let candidate = Arc::new(tokio::sync::Mutex::new(AspClientProtocolSession::new(
                    catalog,
                    Arc::clone(&self.dispatcher),
                )?));
                Arc::clone(
                    self.sessions
                        .write()
                        .await
                        .entry(key.clone())
                        .or_insert(candidate),
                )
            }
        } else {
            let Some(session) = self.sessions.read().await.get(&key).cloned() else {
                return HttpJsonResponse::json(
                    409,
                    &json!({
                        "reasonKind": "client-session-not-initialized",
                        "message": "initialize the workspace-scoped client session first",
                    }),
                );
            };
            session
        };

        let (admission_error, dispatcher, catalog) = {
            let mut session = session.lock().await;
            let error = session.admit(&frame).err();
            (
                error,
                Arc::clone(&session.dispatcher),
                session.catalog.clone(),
            )
        };
        let response_frame = match admission_error {
            Some(error) => correlated_request_id(&frame).map(|request_id| {
                admission_error_response(base, request_id.clone(), error.reason_kind, error.message)
            }),
            None => execute_admitted(dispatcher, catalog, frame).await,
        };
        match response_frame {
            Some(response_frame) => HttpJsonResponse::json(
                200,
                &serde_json::to_value(response_frame)
                    .map_err(|error| format!("encode ASP Client Protocol frame: {error}"))?,
            ),
            None => {
                self.sessions.write().await.remove(&key);
                HttpJsonResponse::json(200, &json!({"state": "exited"}))
            }
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

fn admission_error_response(
    base: ClientFrameBase,
    request_id: ClientRequestId,
    reason_kind: impl Into<String>,
    message: impl Into<String>,
) -> ClientFrame {
    response(
        base,
        request_id,
        ClientOutcome::Error,
        None,
        Some(json!({
            "reasonKind": reason_kind.into(),
            "message": message.into(),
        })),
        None,
    )
}

fn correlated_request_id(frame: &ClientFrame) -> Option<&ClientRequestId> {
    match frame {
        ClientFrame::Initialize { request_id, .. }
        | ClientFrame::Request { request_id, .. }
        | ClientFrame::Cancel { request_id, .. }
        | ClientFrame::Shutdown { request_id, .. }
        | ClientFrame::Response { request_id, .. } => Some(request_id),
        ClientFrame::Exit { .. } | ClientFrame::Event { .. } => None,
    }
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
