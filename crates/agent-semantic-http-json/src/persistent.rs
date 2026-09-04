//! Persistent HTTP/2 JSON client connection.

use bytes::Bytes;
use http_body_util::BodyExt;
use http_body_util::Full;
use hyper::Request;
use hyper::client::conn::http2;
use hyper_util::rt::TokioExecutor;
use hyper_util::rt::TokioIo;
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio::task::JoinHandle;

/// Reusable HTTP/2 prior-knowledge connection to a loopback endpoint.
pub struct HttpJsonConnection {
    sender: Arc<http2::SendRequest<Full<Bytes>>>,
    driver: Option<JoinHandle<Result<(), String>>>,
}

impl HttpJsonConnection {
    pub async fn connect(endpoint: &str) -> Result<Self, String> {
        let uri: hyper::Uri = endpoint
            .parse()
            .map_err(|e| format!("parse endpoint: {e}"))?;
        if uri.scheme_str() != Some("http")
            || !matches!(uri.host(), Some("127.0.0.1" | "localhost"))
        {
            return Err("persistent HTTP JSON requires loopback http endpoint".to_owned());
        }
        let authority = uri
            .authority()
            .ok_or_else(|| "endpoint has no authority".to_owned())?;
        let stream = TcpStream::connect(authority.as_str())
            .await
            .map_err(|e| e.to_string())?;
        stream
            .set_nodelay(true)
            .map_err(|e| format!("enable HTTP/2 TCP_NODELAY: {e}"))?;
        let (sender, connection) = http2::handshake(TokioExecutor::new(), TokioIo::new(stream))
            .await
            .map_err(|e| format!("HTTP/2 prior-knowledge handshake: {e}"))?;
        let driver = tokio::spawn(async move {
            connection
                .await
                .map_err(|e| format!("HTTP/2 connection: {e}"))
        });
        Ok(Self {
            sender: Arc::new(sender),
            driver: Some(driver),
        })
    }

    pub async fn post_json(&self, path: &str, body: Bytes) -> Result<(u16, Bytes), String> {
        let request = Request::post(path)
            .header("content-type", "application/json")
            .body(Full::new(body))
            .map_err(|e| e.to_string())?;
        let mut sender = (*self.sender).clone();
        let response = sender
            .send_request(request)
            .await
            .map_err(|e| e.to_string())?;
        let status = response.status().as_u16();
        let body = response
            .into_body()
            .collect()
            .await
            .map_err(|e| e.to_string())?
            .to_bytes();
        Ok((status, body))
    }

    pub async fn close(mut self) -> Result<(), String> {
        drop(self.sender);
        if let Some(driver) = self.driver.take() {
            driver.await.map_err(|e| e.to_string())??;
        }
        Ok(())
    }
}
