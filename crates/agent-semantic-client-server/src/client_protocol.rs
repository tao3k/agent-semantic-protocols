// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use agent_semantic_client_protocol::ClientFrame;
use agent_semantic_client_protocol::ClientFrameBase;
use agent_semantic_client_protocol::ClientOutcome;
use agent_semantic_client_protocol::ClientProjectId;
use agent_semantic_client_protocol::ClientProtocolCatalog;
use agent_semantic_client_protocol::ClientRequestId;
use agent_semantic_client_protocol::ClientSessionId;
use agent_semantic_client_protocol::ClientWorkspaceIdentity;
use serde_json::Value;
use serde_json::json;

pub type AspClientDispatchFuture =
    Pin<Box<dyn Future<Output = Result<Value, AspClientDispatchError>> + Send>>;
pub type AspClientCancelFuture = Pin<Box<dyn Future<Output = bool> + Send>>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AspClientResponseTelemetry {
    pub project_id: ClientProjectId,
    pub workspace_id: ClientWorkspaceIdentity,
    pub session_id: ClientSessionId,
    pub request_id: ClientRequestId,
    pub outcome: ClientOutcome,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AspClientDispatchRequest {
    pub project_id: ClientProjectId,
    pub session_id: ClientSessionId,
    pub workspace_id: ClientWorkspaceIdentity,
    pub request_id: ClientRequestId,
    pub method: String,
    pub params: Value,
    pub client_timing_witness:
        Option<agent_semantic_client_protocol::RuntimeSearchClientTimingWitness>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AspClientDispatchError {
    pub reason_kind: String,
    pub message: String,
    pub details: Option<Value>,
}

pub trait AspClientDispatcher: Send + Sync + 'static {
    fn dispatch(&self, request: AspClientDispatchRequest) -> AspClientDispatchFuture;

    fn cancel(
        &self,
        project_id: &ClientProjectId,
        workspace_id: &ClientWorkspaceIdentity,
        session_id: &ClientSessionId,
        request_id: &ClientRequestId,
    ) -> AspClientCancelFuture;

    fn response_serialized(&self, _response: &AspClientResponseTelemetry, _elapsed_micros: u64) {}

    fn terminal_egressed(
        &self,
        _response: &AspClientResponseTelemetry,
        _elapsed_micros: u64,
        _delivered: bool,
    ) {
    }
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
pub struct AspClientFrameService<D> {
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
        ClientFrame::Request {
            request_id,
            method,
            params,
            client_timing_witness,
            ..
        } => {
            if let Some(witness) = client_timing_witness.as_ref()
                && let Err(error) =
                    witness.admit_for_request(base.session_id.as_str(), request_id.as_str())
            {
                return Some(response(
                    base,
                    request_id,
                    ClientOutcome::Error,
                    None,
                    Some(json!({
                        "reasonKind": error.reason_kind(),
                        "message": "client timing witness does not match the admitted ClientFrame"
                    })),
                    None,
                ));
            }
            let dispatched = dispatcher
                .dispatch(AspClientDispatchRequest {
                    project_id: base.project_id.clone(),
                    session_id: base.session_id.clone(),
                    workspace_id: base.workspace_id.clone(),
                    request_id: request_id.clone(),
                    method,
                    params,
                    client_timing_witness,
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
                    let mut error_payload = json!({
                        "reasonKind": error.reason_kind,
                        "message": error.message,
                    });
                    if let Some(details) = error.details {
                        error_payload
                            .as_object_mut()
                            .expect("ASP Client dispatch error payload must be an object")
                            .insert("terminal".to_owned(), details);
                    }
                    response(base, request_id, outcome, None, Some(error_payload), None)
                }
            })
        }
        ClientFrame::Cancel { request_id, .. } => {
            let _ = dispatcher
                .cancel(
                    &base.project_id,
                    &base.workspace_id,
                    &base.session_id,
                    &request_id,
                )
                .await;
            // The correlated in-flight dispatch owns the exactly-one terminal.
            // Returning a second response here races that terminal and can hide
            // a still-running server task behind an apparently successful
            // cancellation receipt.
            None
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

impl<D: AspClientDispatcher> AspClientFrameService<D> {
    pub fn new(
        dispatcher: Arc<D>,
        resolve_catalog: impl Fn(&str) -> Result<ClientProtocolCatalog, String> + Send + Sync + 'static,
    ) -> Self {
        Self::new_async(dispatcher, move |_project_id, workspace_id, _session_id| {
            let result = resolve_catalog(&workspace_id);
            async move { result }
        })
    }

    pub fn new_async<F, Fut>(dispatcher: Arc<D>, resolve_catalog: F) -> Self
    where
        F: Fn(String, String, String) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<ClientProtocolCatalog, String>> + Send + 'static,
    {
        Self {
            dispatcher,
            resolve_catalog: Arc::new(move |project_id, workspace_id, session_id| {
                Box::pin(resolve_catalog(project_id, workspace_id, session_id))
            }),
        }
    }

    /// Apply the shared ASP Client Protocol admission and dispatch semantics to
    /// one typed frame, independent of its HTTP or gRPC transport.
    pub async fn handle_frame(&self, frame: ClientFrame) -> Result<Option<ClientFrame>, String> {
        let base = frame.base().clone();
        let catalog = if matches!(frame, ClientFrame::Initialize { .. }) {
            Some(
                (self.resolve_catalog)(
                    base.project_id.as_str().to_owned(),
                    base.workspace_id.as_str().to_owned(),
                    base.session_id.as_str().to_owned(),
                )
                .await?,
            )
        } else {
            None
        };
        Ok(execute_admitted(Arc::clone(&self.dispatcher), catalog, frame).await)
    }

    pub fn response_serialized(&self, response: &AspClientResponseTelemetry, elapsed_micros: u64) {
        self.dispatcher
            .response_serialized(response, elapsed_micros);
    }

    pub fn terminal_egressed(
        &self,
        response: &AspClientResponseTelemetry,
        elapsed_micros: u64,
        delivered: bool,
    ) {
        self.dispatcher
            .terminal_egressed(response, elapsed_micros, delivered);
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
