// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! HTTP JSON client, server, and Tokio runtime implementation.

use std::future::Future;
use std::sync::Arc;

use bytes::Bytes;
use http_body_util::BodyExt;
use http_body_util::Full;
use hyper::Request;
use hyper::Response;
use hyper::StatusCode;
use hyper::body::Incoming;
use hyper::client::conn::http1 as client_http1;
use hyper::server::conn::http1 as server_http1;
use hyper::service::service_fn;
use hyper_util::rt::TokioExecutor;
use hyper_util::rt::TokioIo;
use tokio::net::TcpListener;
use tokio::sync::watch;
use tokio::task::JoinSet;
use tokio_stream::StreamExt;
use tokio_stream::wrappers::TcpListenerStream;

/// One decoded HTTP JSON request at the transport boundary.
#[derive(Debug)]
pub struct HttpJsonRequest {
    pub method: String,
    pub path: String,
    pub body: Bytes,
}

/// One encoded HTTP JSON response at the transport boundary.
#[derive(Debug)]
pub struct HttpJsonResponse {
    pub status: u16,
    pub body: Bytes,
}

/// POST one bounded JSON payload to the loopback ASP Client HTTP endpoint.
pub async fn post_json(
    endpoint: &str,
    path: &str,
    body: Bytes,
) -> Result<HttpJsonResponse, String> {
    let uri: hyper::Uri = endpoint
        .parse()
        .map_err(|error| format!("parse HTTP JSON endpoint: {error}"))?;
    if uri.scheme_str() != Some("http") || !matches!(uri.host(), Some("127.0.0.1" | "localhost")) {
        return Err("HTTP JSON client requires a loopback http endpoint".to_owned());
    }
    let authority = uri
        .authority()
        .ok_or_else(|| "HTTP JSON endpoint has no authority".to_owned())?;
    let stream = tokio::net::TcpStream::connect(authority.as_str())
        .await
        .map_err(|error| format!("connect HTTP JSON endpoint: {error}"))?;
    stream
        .set_nodelay(true)
        .map_err(|error| format!("enable HTTP JSON TCP_NODELAY: {error}"))?;
    let io = TokioIo::new(stream);
    let (mut sender, connection) = client_http1::handshake(io)
        .await
        .map_err(|error| format!("handshake HTTP JSON endpoint: {error}"))?;
    let connection_task = tokio::spawn(async move {
        connection
            .await
            .map_err(|error| format!("HTTP JSON client connection: {error}"))
    });
    let request = Request::post(path)
        .header("content-type", "application/json")
        .body(Full::new(body))
        .map_err(|error| format!("build HTTP JSON request: {error}"))?;
    let response = sender
        .send_request(request)
        .await
        .map_err(|error| format!("send HTTP JSON request: {error}"))?;
    let status = response.status().as_u16();
    let body = response
        .into_body()
        .collect()
        .await
        .map_err(|error| format!("read HTTP JSON response: {error}"))?
        .to_bytes();
    connection_task
        .await
        .map_err(|error| format!("join HTTP JSON client connection: {error}"))??;
    Ok(HttpJsonResponse { status, body })
}

impl HttpJsonResponse {
    /// Encode a dynamic JSON API boundary as an HTTP response.
    pub fn json(status: u16, value: &serde_json::Value) -> Result<Self, String> {
        serde_json::to_vec(value)
            .map(Bytes::from)
            .map(|body| Self { status, body })
            .map_err(|error| format!("encode HTTP JSON response: {error}"))
    }
}

/// Serve HTTP/1 JSON requests until shutdown, tracking every connection task.
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
    let mut listener = TcpListenerStream::new(listener);

    loop {
        tokio::select! {
            changed = shutdown.changed() => {
                if changed.is_err() || *shutdown.borrow() {
                    break;
                }
            }
            accepted = listener.next() => {
                let Some(accepted) = accepted else {
                    break;
                };
                let stream = accepted
                    .map_err(|error| format!("accept HTTP JSON connection: {error}"))?;
                stream
                    .set_nodelay(true)
                    .map_err(|error| format!("enable HTTP JSON TCP_NODELAY: {error}"))?;
                let handler = Arc::clone(&handler);
                let mut connection_shutdown = shutdown.clone();
                connections.spawn(async move {
                    let service = service_fn(move |request| {
                        serve_request(request, Arc::clone(&handler))
                    });
                    let connection = server_http1::Builder::new()
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

/// Serve the ASP northbound data plane using HTTP/2 prior knowledge.
/// Generic provider HTTP remains owned by `serve_http_json` (HTTP/1).
pub async fn serve_http_json_h2<H, F>(
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
    let mut listener = TcpListenerStream::new(listener);
    loop {
        tokio::select! {
            changed = shutdown.changed() => {
                if changed.is_err() || *shutdown.borrow() { break; }
            }
            accepted = listener.next() => {
                let Some(accepted) = accepted else { break; };
                let stream = accepted.map_err(|error| format!("accept HTTP/2 JSON connection: {error}"))?;
                stream
                    .set_nodelay(true)
                    .map_err(|error| format!("enable HTTP/2 JSON TCP_NODELAY: {error}"))?;
                let handler = Arc::clone(&handler);
                let mut connection_shutdown = shutdown.clone();
                connections.spawn(async move {
                    let service = service_fn(move |request| serve_request(request, Arc::clone(&handler)));
                    let connection = hyper::server::conn::http2::Builder::new(TokioExecutor::new())
                        .serve_connection(TokioIo::new(stream), service);
                    tokio::pin!(connection);
                    tokio::select! {
                        result = &mut connection => result.map_err(|error| format!("serve HTTP/2 JSON connection: {error}")),
                        _ = connection_shutdown.changed() => {
                            connection.as_mut().graceful_shutdown();
                            connection.await.map_err(|error| format!("drain HTTP/2 JSON connection: {error}"))
                        }
                    }
                });
            }
        }
    }
    while let Some(result) = connections.join_next().await {
        result.map_err(|error| format!("join HTTP/2 JSON connection: {error}"))??;
    }
    Ok(())
}

/// Run an HTTP JSON server future on its owned multi-thread Tokio runtime.
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
