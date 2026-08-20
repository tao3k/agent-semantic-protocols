use std::{future::Future, sync::Arc};

use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::{
    Request, Response, StatusCode, body::Incoming, server::conn::http1, service::service_fn,
};
use hyper_util::rt::TokioIo;
use tokio::{net::TcpListener, sync::watch, task::JoinSet};

#[derive(Debug)]
pub struct AspClientServerRequest {
    pub method: String,
    pub path: String,
    pub body: Bytes,
}

#[derive(Debug)]
pub struct AspClientServerResponse {
    pub status: u16,
    pub body: Bytes,
}

impl AspClientServerResponse {
    pub fn json(status: u16, value: &serde_json::Value) -> Result<Self, String> {
        serde_json::to_vec(value)
            .map(Bytes::from)
            .map(|body| Self { status, body })
            .map_err(|error| format!("encode asp-client-server JSON response: {error}"))
    }
}

pub async fn serve_asp_client_server<H, F>(
    listener: TcpListener,
    mut shutdown: watch::Receiver<bool>,
    handler: H,
) -> Result<(), String>
where
    H: Fn(AspClientServerRequest) -> F + Send + Sync + 'static,
    F: Future<Output = Result<AspClientServerResponse, String>> + Send + 'static,
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
                    .map_err(|error| format!("accept asp-client-server connection: {error}"))?;
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
                            .map_err(|error| format!("serve asp-client-server connection: {error}")),
                        _ = connection_shutdown.changed() => {
                            connection.as_mut().graceful_shutdown();
                            connection.await.map_err(|error| {
                                format!("drain asp-client-server connection: {error}")
                            })
                        }
                    }
                });
            }
        }
    }

    while let Some(result) = connections.join_next().await {
        result.map_err(|error| format!("join asp-client-server connection: {error}"))??;
    }
    Ok(())
}

pub fn run_asp_client_server<F>(server: F) -> Result<(), String>
where
    F: Future<Output = Result<(), String>> + Send + 'static,
{
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("build asp-client-server Tokio runtime: {error}"))?
        .block_on(server)
}

async fn serve_request<H, F>(
    request: Request<Incoming>,
    handler: Arc<H>,
) -> Result<Response<Full<Bytes>>, std::convert::Infallible>
where
    H: Fn(AspClientServerRequest) -> F + Send + Sync + 'static,
    F: Future<Output = Result<AspClientServerResponse, String>> + Send + 'static,
{
    let method = request.method().as_str().to_owned();
    let path = request.uri().path().to_owned();
    let body = match request.into_body().collect().await {
        Ok(body) => body.to_bytes(),
        Err(error) => {
            return Ok(json_error(
                StatusCode::BAD_REQUEST,
                format!("read asp-client-server request body: {error}"),
            ));
        }
    };
    let response = match handler(AspClientServerRequest { method, path, body }).await {
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
                format!("build asp-client-server response: {error}"),
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
        .expect("static asp-client-server error response")
}
