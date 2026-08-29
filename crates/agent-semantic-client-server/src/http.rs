//! HTTP/JSON adapter for the transport-neutral ASP ClientFrame owner.

use std::sync::Arc;

use agent_semantic_client_protocol::ClientFrame;
use agent_semantic_http_json::{HttpJsonRequest, HttpJsonResponse, serve_http_json};
use bytes::Bytes;
use tokio::{net::TcpListener, sync::watch};

use crate::{AspClientDispatcher, AspClientFrameService};

/// The only HTTP path that carries a public ASP ClientFrame.
pub const ASP_CLIENT_HTTP_FRAME_PATH: &str = "/protocol/frame";

/// Serve HTTP/JSON frames through the same `AspClientFrameService` used by
/// gRPC.  HTTP contributes framing only; it has no method or route catalog of
/// its own.
pub async fn serve_asp_client_http_json<D: AspClientDispatcher>(
    listener: TcpListener,
    service: Arc<AspClientFrameService<D>>,
    shutdown: watch::Receiver<bool>,
) -> Result<(), String> {
    serve_http_json(listener, shutdown, move |request: HttpJsonRequest| {
        let service = Arc::clone(&service);
        async move { handle_http_frame(service, request).await }
    })
    .await
}

async fn handle_http_frame<D: AspClientDispatcher>(
    service: Arc<AspClientFrameService<D>>,
    request: HttpJsonRequest,
) -> Result<HttpJsonResponse, String> {
    if request.method != "POST" || request.path != ASP_CLIENT_HTTP_FRAME_PATH {
        return HttpJsonResponse::json(
            404,
            &serde_json::json!({
                "error": "ASP Client HTTP JSON exposes only POST /protocol/frame"
            }),
        );
    }
    let frame: ClientFrame = match serde_json::from_slice(&request.body) {
        Ok(frame) => frame,
        Err(error) => {
            return HttpJsonResponse::json(
                400,
                &serde_json::json!({
                    "error": format!("decode ASP ClientFrame HTTP JSON body: {error}")
                }),
            );
        }
    };
    let response = match service.handle_frame(frame).await {
        Ok(response) => response,
        Err(error) => {
            return HttpJsonResponse::json(412, &serde_json::json!({"error": error}));
        }
    };
    match response {
        Some(response) => {
            let body = serde_json::to_vec(&response)
                .map(Bytes::from)
                .map_err(|error| format!("encode ASP ClientFrame HTTP JSON response: {error}"))?;
            Ok(HttpJsonResponse { status: 200, body })
        }
        None => Ok(HttpJsonResponse {
            status: 204,
            body: Bytes::new(),
        }),
    }
}
