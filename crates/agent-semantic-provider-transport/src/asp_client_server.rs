use std::{collections::BTreeMap, future::Future, path::PathBuf, pin::Pin, process::Stdio};

use bytes::Bytes;
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, BufReader, Lines},
    process::{Child, ChildStdout, Command},
};

use crate::{
    ProviderRuntimeContractReceipt, ProviderRuntimePeer, ProviderRuntimeRequestFrame,
    ProviderRuntimeResponseFrame, ProviderRuntimeResponseOutcome,
};

#[derive(Clone, Debug)]
pub struct AspClientServerSpec {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    pub env: BTreeMap<String, String>,
    pub host: String,
    pub health_path: String,
    pub request_path: String,
    pub shutdown_path: String,
}

impl AspClientServerSpec {
    pub fn new(program: impl Into<String>, cwd: PathBuf) -> Self {
        Self {
            program: program.into(),
            args: Vec::new(),
            cwd,
            env: BTreeMap::new(),
            host: "127.0.0.1:0".to_owned(),
            health_path: "/health".to_owned(),
            request_path: "/v1/provider-runtime".to_owned(),
            shutdown_path: "/shutdown".to_owned(),
        }
    }
}

pub struct AspClientServerPeer {
    state: tokio::sync::Mutex<AspClientServerState>,
    launch_program: String,
    launch_args: Vec<String>,
    health_path: String,
    request_path: String,
    shutdown_path: String,
    next_request_id: std::sync::atomic::AtomicU64,
}

struct AspClientServerState {
    child: Child,
    stdout: Lines<BufReader<ChildStdout>>,
    stderr_task: tokio::task::JoinHandle<Result<u64, std::io::Error>>,
    stderr_capture: std::sync::Arc<std::sync::Mutex<Vec<u8>>>,
    http: Option<AspClientServerHttpClient>,
}

const MAX_BOOTSTRAP_STDERR_BYTES: usize = 16 * 1024;
const DEFAULT_PROVIDER_HTTP_REQUEST_TIMEOUT: std::time::Duration =
    std::time::Duration::from_secs(30);

#[derive(Clone)]
struct AspClientServerHttpClient {
    client: reqwest::Client,
    base_url: reqwest::Url,
}

impl AspClientServerHttpClient {
    fn new(base_url: reqwest::Url) -> Result<Self, String> {
        Self::new_with_timeout(base_url, DEFAULT_PROVIDER_HTTP_REQUEST_TIMEOUT)
    }

    fn new_with_timeout(
        base_url: reqwest::Url,
        request_timeout: std::time::Duration,
    ) -> Result<Self, String> {
        let client = reqwest::Client::builder()
            .no_proxy()
            .http1_only()
            .pool_max_idle_per_host(1)
            .timeout(request_timeout)
            .build()
            .map_err(|error| format!("construct ASP Client Server HTTP client: {error}"))?;
        Ok(Self { client, base_url })
    }

    async fn json(&self, method: &str, path: &str, body: Option<&[u8]>) -> Result<Vec<u8>, String> {
        let method = reqwest::Method::from_bytes(method.as_bytes())
            .map_err(|error| format!("construct ASP Client Server HTTP method: {error}"))?;
        let url = self
            .base_url
            .join(path.trim_start_matches('/'))
            .map_err(|error| format!("construct ASP Client Server URL: {error}"))?;
        let mut request = self
            .client
            .request(method, url)
            .header(reqwest::header::ACCEPT, "application/json");
        if let Some(body) = body {
            request = request
                .header(reqwest::header::CONTENT_TYPE, "application/json")
                .body(body.to_vec());
        }
        let response = request.send().await.map_err(|error| {
            if error.is_timeout() {
                "ASP Client Server request deadline exceeded".to_owned()
            } else {
                format!("send ASP Client Server request: {error}")
            }
        })?;
        let status = response.status();
        let response_body = response
            .bytes()
            .await
            .map_err(|error| format!("read ASP Client Server response: {error}"))?;
        if !status.is_success() {
            return Err(format!("ASP Client Server returned status {status}"));
        }
        Ok(response_body.to_vec())
    }
}

impl AspClientServerPeer {
    pub async fn start(mut spec: AspClientServerSpec) -> Result<Self, String> {
        spec.env
            .insert("ASP_CLIENT_SERVER_HOST".to_owned(), spec.host.clone());
        let mut command = Command::new(&spec.program);
        command
            .args(&spec.args)
            .current_dir(&spec.cwd)
            .envs(&spec.env)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        let mut child = command
            .spawn()
            .map_err(|error| format!("start provider HTTP server: {error}"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "provider HTTP server stdout is unavailable".to_owned())?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| "provider HTTP server stderr is unavailable".to_owned())?;
        let stderr_capture = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let stderr_capture_writer = std::sync::Arc::clone(&stderr_capture);
        let stderr_task = tokio::spawn(async move {
            let mut stderr = stderr;
            let mut buffer = [0_u8; 4096];
            let mut total = 0_u64;
            loop {
                let read = stderr.read(&mut buffer).await?;
                if read == 0 {
                    return Ok(total);
                }
                total = total.saturating_add(read as u64);
                let mut capture = stderr_capture_writer
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                let remaining = MAX_BOOTSTRAP_STDERR_BYTES.saturating_sub(capture.len());
                capture.extend_from_slice(&buffer[..read.min(remaining)]);
            }
        });
        Ok(Self {
            state: tokio::sync::Mutex::new(AspClientServerState {
                child,
                stdout: BufReader::new(stdout).lines(),
                stderr_task,
                stderr_capture,
                http: None,
            }),
            launch_program: spec.program,
            launch_args: spec.args,
            health_path: spec.health_path,
            request_path: spec.request_path,
            shutdown_path: spec.shutdown_path,
            next_request_id: std::sync::atomic::AtomicU64::new(0),
        })
    }

    async fn http_json(
        &self,
        method: &str,
        path: &str,
        body: Option<&[u8]>,
    ) -> Result<Vec<u8>, String> {
        let http = self
            .state
            .lock()
            .await
            .http
            .clone()
            .ok_or_else(|| "ASP Client Server HTTP client is not ready".to_owned())?;
        http.json(method, path, body).await
    }

    async fn contract_receipt(&self) -> Result<ProviderRuntimeContractReceipt, String> {
        let mut state = self.state.lock().await;
        let bootstrap = match state
            .stdout
            .next_line()
            .await
            .map_err(|error| format!("read provider HTTP server bootstrap: {error}"))?
        {
            Some(bootstrap) => bootstrap,
            None => {
                let status = state.child.wait().await.map_err(|error| {
                    format!("reap provider HTTP server after bootstrap EOF: {error}")
                })?;
                let _ = tokio::time::timeout(
                    std::time::Duration::from_millis(100),
                    &mut state.stderr_task,
                )
                .await;
                let stderr = state
                    .stderr_capture
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                let stderr = String::from_utf8_lossy(&stderr);
                return Err(format!(
                    "provider HTTP server exited before bootstrap: status={status} program={} args={:?} stderr={stderr:?}",
                    self.launch_program, self.launch_args
                ));
            }
        };
        let bootstrap: serde_json::Value = serde_json::from_str(&bootstrap)
            .map_err(|error| format!("decode provider HTTP server bootstrap: {error}"))?;
        if bootstrap
            .get("schemaId")
            .and_then(serde_json::Value::as_str)
            != Some("agent.semantic-protocols.asp-client-server-bootstrap")
            || bootstrap
                .get("schemaVersion")
                .and_then(serde_json::Value::as_str)
                != Some("1")
            || bootstrap
                .get("transport")
                .and_then(serde_json::Value::as_str)
                != Some("http-json")
            || bootstrap.get("state").and_then(serde_json::Value::as_str) != Some("ready")
        {
            return Err("provider HTTP server bootstrap is not ready".to_owned());
        }
        let endpoint = bootstrap
            .get("endpoint")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| "provider HTTP server bootstrap endpoint is absent".to_owned())?;
        let endpoint_url = reqwest::Url::parse(endpoint)
            .map_err(|error| format!("decode provider HTTP server endpoint: {error}"))?;
        if endpoint_url.scheme() != "http"
            || endpoint_url.host_str() != Some("127.0.0.1")
            || endpoint_url.port().is_none()
            || endpoint_url.path() != "/"
            || endpoint_url.query().is_some()
            || endpoint_url.fragment().is_some()
        {
            return Err("provider HTTP server bootstrap endpoint is invalid".to_owned());
        }
        state.http = Some(AspClientServerHttpClient::new(endpoint_url)?);
        drop(state);
        let health_path = self.health_path.clone();
        let response = self.http_json("GET", &health_path, None).await?;
        let receipt = serde_json::from_slice::<ProviderRuntimeContractReceipt>(&response)
            .map_err(|error| format!("decode provider HTTP server health: {error}"))?;
        receipt.validate()?;
        Ok(receipt)
    }

    async fn send_request(&self, operation: String, payload: Bytes) -> Result<Bytes, String> {
        let request_id = self
            .next_request_id
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            .saturating_add(1);
        let request_id = format!("provider-http-{request_id}");
    let request = ProviderRuntimeRequestFrame::new(&request_id, operation, &payload)?;
    let request = serde_json::to_vec(&request)
        .map_err(|error| format!("encode provider HTTP server request: {error}"))?;
    let request_prefix = String::from_utf8_lossy(&request)
        .chars()
        .take(4096)
        .collect::<String>();
        let request_path = self.request_path.clone();
        let response = self
            .http_json("POST", &request_path, Some(&request))
            .await?;
    let response = serde_json::from_slice::<ProviderRuntimeResponseFrame>(&response).map_err(
        |error| {
            let prefix = String::from_utf8_lossy(&response)
                .chars()
                .take(256)
                .collect::<String>();
            format!(
                "decode provider HTTP server response: {error}; bytes={}; prefix={prefix:?}; requestPrefix={request_prefix:?}",
                response.len(),
            )
        },
    )?;
        response.validate()?;
        if response.request_id != request_id {
            return Err("provider HTTP server response requestId drift".to_owned());
        }
        match response.outcome {
            ProviderRuntimeResponseOutcome::Ready => {
                Ok(Bytes::from(response.payload_bytes()?.ok_or_else(|| {
                    "provider HTTP server ready payload is absent".to_owned()
                })?))
            }
            ProviderRuntimeResponseOutcome::Error => Err(response
                .error
                .unwrap_or_else(|| "provider HTTP server returned an error".to_owned())),
        }
    }

    async fn stop(&self) -> Result<(), String> {
        let running = self
            .state
            .lock()
            .await
            .child
            .try_wait()
            .map_err(|error| format!("inspect provider HTTP server: {error}"))?
            .is_none();
        if running {
            let shutdown_path = self.shutdown_path.clone();
            if self
                .http_json("POST", &shutdown_path, Some(b"{}"))
                .await
                .is_err()
            {
                self.state
                    .lock()
                    .await
                    .child
                    .kill()
                    .await
                    .map_err(|error| format!("stop provider HTTP server: {error}"))?;
            }
        }
        let mut state = self.state.lock().await;
        state
            .child
            .wait()
            .await
            .map_err(|error| format!("reap provider HTTP server: {error}"))?;
        state.stderr_task.abort();
        Ok(())
    }
}

impl ProviderRuntimePeer for AspClientServerPeer {
    fn handshake(
        &self,
    ) -> Pin<Box<dyn Future<Output = Result<ProviderRuntimeContractReceipt, String>> + Send + '_>>
    {
        Box::pin(self.contract_receipt())
    }

    fn request(
        &self,
        operation: String,
        payload: Bytes,
    ) -> Pin<Box<dyn Future<Output = Result<Bytes, String>> + Send + '_>> {
        Box::pin(self.send_request(operation, payload))
    }

    fn shutdown(&self) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send + '_>> {
        Box::pin(self.stop())
    }
}

#[cfg(test)]
#[path = "../tests/unit/asp_client_server.rs"]
mod tests;
