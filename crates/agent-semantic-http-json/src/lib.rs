#![deny(dead_code)]

//! Shared Tokio/Hyper HTTP JSON host used by ASP protocol bindings.

use std::{future::Future, sync::Arc};

use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::{
    Request, Response, StatusCode, body::Incoming, server::conn::http1, service::service_fn,
};
use hyper_util::rt::TokioIo;
use tokio::{net::TcpListener, sync::watch, task::JoinSet};

#[derive(Debug)]
pub struct HttpJsonRequest {
    pub method: String,
    pub path: String,
    pub body: Bytes,
}

#[derive(Debug)]
pub struct HttpJsonResponse {
    pub status: u16,
    pub body: Bytes,
}

impl HttpJsonResponse {
    pub fn json(status: u16, value: &serde_json::Value) -> Result<Self, String> {
        serde_json::to_vec(value)
            .map(Bytes::from)
            .map(|body| Self { status, body })
            .map_err(|error| format!("encode HTTP JSON response: {error}"))
    }
}

pub async fn serve_http_json<H, F>(
    listener: TcpListener,
    mut shutdown: watch::Receiver<bool>,
    handler: H,
) -> Result<(), String>
where
    H: Fn(HttpJsonRequest) -> F + Send + Sync + 'static,
    F: Future<Output = Result<HttpJsonResponse, String>> + Send + 'static,
{
    let handler = Arc::new(handler);
    let mut connections = JoinSet::new();

    loop {
        tokio::select! {
            changed = shutdown.changed() => {
                if changed.is_err() || *shutdown.borrow() {
                    break;
                }
            }
            accepted = listener.accept() => {
                let (stream, _) = accepted
                    .map_err(|error| format!("accept HTTP JSON connection: {error}"))?;
                let handler = Arc::clone(&handler);
                let mut connection_shutdown = shutdown.clone();
                connections.spawn(async move {
                    let service = service_fn(move |request| {
                        serve_request(request, Arc::clone(&handler))
                    });
                    let connection = http1::Builder::new()
                        .serve_connection(TokioIo::new(stream), service);
                    tokio::pin!(connection);
                    tokio::select! {
                        result = &mut connection => result
                            .map_err(|error| format!("serve HTTP JSON connection: {error}")),
                        _ = connection_shutdown.changed() => {
                            connection.as_mut().graceful_shutdown();
                            connection.await.map_err(|error| {
                                format!("drain HTTP JSON connection: {error}")
                            })
                        }
                    }
                });
            }
        }
    }

    while let Some(result) = connections.join_next().await {
        result.map_err(|error| format!("join HTTP JSON connection: {error}"))??;
    }
    Ok(())
}

pub fn run_http_json<F>(server: F) -> Result<(), String>
where
    F: Future<Output = Result<(), String>> + Send + 'static,
{
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("build HTTP JSON Tokio runtime: {error}"))?
        .block_on(server)
}

async fn serve_request<H, F>(
    request: Request<Incoming>,
    handler: Arc<H>,
) -> Result<Response<Full<Bytes>>, std::convert::Infallible>
where
    H: Fn(HttpJsonRequest) -> F + Send + Sync + 'static,
    F: Future<Output = Result<HttpJsonResponse, String>> + Send + 'static,
{
    let method = request.method().as_str().to_owned();
    let path = request.uri().path().to_owned();
    const MAX_HTTP_JSON_BODY_BYTES: usize = 1024 * 1024;
    let body = match http_body_util::Limited::new(request.into_body(), MAX_HTTP_JSON_BODY_BYTES)
        .collect()
        .await
    {
        Ok(body) => body.to_bytes(),
        Err(error) => {
            return Ok(json_error(
                StatusCode::BAD_REQUEST,
                format!("read HTTP JSON request body: {error}"),
            ));
        }
    };
    let response = match handler(HttpJsonRequest { method, path, body }).await {
        Ok(response) => response,
        Err(error) => return Ok(json_error(StatusCode::INTERNAL_SERVER_ERROR, error)),
    };
    Ok(Response::builder()
        .status(response.status)
        .header(hyper::header::CONTENT_TYPE, "application/json")
        .body(Full::new(response.body))
        .unwrap_or_else(|error| {
            json_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("build HTTP JSON response: {error}"),
            )
        }))
}

fn json_error(status: StatusCode, error: String) -> Response<Full<Bytes>> {
    let body = serde_json::to_vec(&serde_json::json!({ "error": error }))
        .map(Bytes::from)
        .unwrap_or_else(|_| Bytes::from_static(br#"{"error":"response-encoding-failed"}"#));
    Response::builder()
        .status(status)
        .header(hyper::header::CONTENT_TYPE, "application/json")
        .body(Full::new(body))
        .expect("static HTTP JSON error response")
}
