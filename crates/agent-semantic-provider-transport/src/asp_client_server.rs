// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::collections::BTreeMap;
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::process::Stdio;

use bytes::Bytes;
use tokio::io::AsyncBufReadExt;
use tokio::io::AsyncReadExt;
use tokio::io::BufReader;
use tokio::io::Lines;
use tokio::process::ChildStdout;
use tokio::process::Command;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio::sync::watch;

use crate::ProviderRuntimeContractReceipt;
use crate::ProviderRuntimePeer;
use crate::ProviderRuntimeRequestFrame;
use crate::ProviderRuntimeResponseFrame;
use crate::ProviderRuntimeResponseOutcome;

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
    bootstrap_stdout: tokio::sync::Mutex<Option<Lines<BufReader<ChildStdout>>>>,
    http: tokio::sync::RwLock<Option<AspClientServerHttpClient>>,
    lifecycle: watch::Receiver<AspClientServerChildLifecycle>,
    lifecycle_control: mpsc::Sender<AspClientServerChildControl>,
    launch_program: String,
    launch_args: Vec<String>,
    health_path: String,
    request_path: String,
    request_stream_path: String,
    shutdown_path: String,
    next_request_id: std::sync::atomic::AtomicU64,
}

#[derive(Clone, Debug)]
enum AspClientServerChildLifecycle {
    Running,
    Exited { status: String, stderr: String },
    Failed(String),
}

enum AspClientServerChildControl {
    Stop {
        force: bool,
        response: oneshot::Sender<Result<(), String>>,
    },
}

const MAX_BOOTSTRAP_STDERR_BYTES: usize = 16 * 1024;
const DEFAULT_PROVIDER_HTTP_LIFECYCLE_DEADLINE: std::time::Duration =
    std::time::Duration::from_secs(2);
/// Provider HTTP servers must admit this wire-frame size without changing
/// language-local server limits.  Corpus-sized operations are expressed as a
/// sequence of bounded frames instead of one unbounded request body.
const MAX_PROVIDER_HTTP_REQUEST_FRAME_BYTES: usize = 896 * 1024;
const PROVIDER_HTTP_REQUEST_STREAM_CHUNK_BYTES: usize = 128 * 1024;
#[derive(Clone)]
struct AspClientServerHttpClient {
    client: reqwest::Client,
    base_url: reqwest::Url,
}

impl AspClientServerHttpClient {
    fn new(base_url: reqwest::Url) -> Result<Self, String> {
        let client = reqwest::Client::builder()
            .no_proxy()
            .http1_only()
            .pool_max_idle_per_host(1)
            // Do not install a whole-request timeout here: projection batches
            // inherit cancellation from the generation owner. Health and
            // shutdown remain bounded explicitly below.
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
            let body_prefix =
                String::from_utf8_lossy(&response_body[..response_body.len().min(4096)]);
            return Err(format!(
                "reasonKind=asp-client-server-http-status status={status} bodyPrefix={body_prefix:?}"
            ));
        }
        Ok(response_body.to_vec())
    }

    async fn json_with_deadline(
        &self,
        method: &str,
        path: &str,
        body: Option<&[u8]>,
        deadline: std::time::Duration,
    ) -> Result<Vec<u8>, String> {
        tokio::time::timeout(deadline, self.json(method, path, body))
            .await
            .map_err(|_| "ASP Client Server lifecycle deadline exceeded".to_owned())?
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
        let stderr_task: tokio::task::JoinHandle<Result<u64, std::io::Error>> =
            tokio::spawn(async move {
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
        let (lifecycle_control, mut lifecycle_commands) = mpsc::channel(1);
        let (lifecycle_writer, lifecycle) = watch::channel(AspClientServerChildLifecycle::Running);
        let launch_program = spec.program.clone();
        let launch_args = spec.args.clone();
        tokio::spawn(async move {
            enum ChildOutcome {
                Exited(Result<std::process::ExitStatus, std::io::Error>),
                Stop {
                    force: bool,
                    response: oneshot::Sender<Result<(), String>>,
                },
            }

            let outcome = tokio::select! {
                status = child.wait() => ChildOutcome::Exited(status),
                command = lifecycle_commands.recv() => match command {
                    Some(AspClientServerChildControl::Stop { force, response }) => {
                        ChildOutcome::Stop { force, response }
                    }
                    None => ChildOutcome::Exited(child.wait().await),
                },
            };
            let (status, stop_response) = match outcome {
                ChildOutcome::Exited(status) => (status, None),
                ChildOutcome::Stop { force, response } => {
                    if force {
                        if let Err(error) = child.kill().await {
                            let reason = format!(
                                "reasonKind=asp-client-server-child-kill-failed phase=shutdown error={error}"
                            );
                            lifecycle_writer.send_replace(AspClientServerChildLifecycle::Failed(
                                reason.clone(),
                            ));
                            tracing::error!(
                                target: "asp.client_server",
                                phase = "shutdown",
                                reason_kind = "asp-client-server-child-kill-failed",
                                error = %error,
                            );
                            let _ = response.send(Err(reason));
                            return;
                        }
                    }
                    (child.wait().await, Some(response))
                }
            };
            let stderr_result = stderr_task.await;
            let stderr = String::from_utf8_lossy(
                &stderr_capture
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner),
            )
            .into_owned();
            let terminal = match status {
                Ok(status) => {
                    tracing::info!(
                        target: "asp.client_server",
                        phase = "child-lifecycle",
                        reason_kind = "asp-client-server-child-exited",
                        status = %status,
                        program = %launch_program,
                        args = ?launch_args,
                    );
                    AspClientServerChildLifecycle::Exited {
                        status: status.to_string(),
                        stderr,
                    }
                }
                Err(error) => {
                    let reason = format!(
                        "reasonKind=asp-client-server-child-wait-failed phase=child-lifecycle error={error}"
                    );
                    tracing::error!(
                        target: "asp.client_server",
                        phase = "child-lifecycle",
                        reason_kind = "asp-client-server-child-wait-failed",
                        error = %error,
                    );
                    AspClientServerChildLifecycle::Failed(reason)
                }
            };
            let stop_result = match (&terminal, stderr_result) {
                (AspClientServerChildLifecycle::Failed(reason), _) => Err(reason.clone()),
                (_, Err(error)) => Err(format!(
                    "reasonKind=asp-client-server-stderr-task-failed phase=child-lifecycle error={error}"
                )),
                (_, Ok(Err(error))) => Err(format!(
                    "reasonKind=asp-client-server-stderr-read-failed phase=child-lifecycle error={error}"
                )),
                (_, Ok(Ok(_))) => Ok(()),
            };
            lifecycle_writer.send_replace(terminal);
            if let Some(response) = stop_response {
                let _ = response.send(stop_result);
            }
        });
        Ok(Self {
            bootstrap_stdout: tokio::sync::Mutex::new(Some(BufReader::new(stdout).lines())),
            http: tokio::sync::RwLock::new(None),
            lifecycle,
            lifecycle_control,
            launch_program: spec.program,
            launch_args: spec.args,
            health_path: spec.health_path,
            request_path: spec.request_path,
            request_stream_path: "/v1/provider-runtime-stream".to_owned(),
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
            .http
            .read()
            .await
            .clone()
            .ok_or_else(|| "ASP Client Server HTTP client is not ready".to_owned())?;
        http.json(method, path, body).await
    }

    async fn http_json_with_lifecycle_deadline(
        &self,
        method: &str,
        path: &str,
        body: Option<&[u8]>,
    ) -> Result<Vec<u8>, String> {
        let http = self
            .http
            .read()
            .await
            .clone()
            .ok_or_else(|| "ASP Client Server HTTP client is not ready".to_owned())?;
        http.json_with_deadline(method, path, body, DEFAULT_PROVIDER_HTTP_LIFECYCLE_DEADLINE)
            .await
    }

    async fn contract_receipt(&self) -> Result<ProviderRuntimeContractReceipt, String> {
        let mut stdout = self
            .bootstrap_stdout
            .lock()
            .await
            .take()
            .ok_or_else(|| "provider HTTP server bootstrap was already consumed".to_owned())?;
        let bootstrap = match stdout
            .next_line()
            .await
            .map_err(|error| format!("read provider HTTP server bootstrap: {error}"))?
        {
            Some(bootstrap) => bootstrap,
            None => {
                let terminal = self.wait_child_terminated().await;
                let (status, stderr) = match terminal {
                    Ok(AspClientServerChildLifecycle::Exited { status, stderr }) => {
                        (status, stderr)
                    }
                    Ok(AspClientServerChildLifecycle::Failed(reason)) | Err(reason) => {
                        return Err(reason);
                    }
                    Ok(AspClientServerChildLifecycle::Running) => unreachable!(),
                };
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
        self.http
            .write()
            .await
            .replace(AspClientServerHttpClient::new(endpoint_url)?);
        let health_path = self.health_path.clone();
        let response = self
            .http_json_with_lifecycle_deadline("GET", &health_path, None)
            .await?;
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
        let request = ProviderRuntimeRequestFrame::new(&request_id, operation.as_str(), &payload)?;
        let request = serde_json::to_vec(&request)
            .map_err(|error| format!("encode provider HTTP server request: {error}"))?;
        let request_prefix = String::from_utf8_lossy(&request)
            .chars()
            .take(4096)
            .collect::<String>();
        let response = if request.len() <= MAX_PROVIDER_HTTP_REQUEST_FRAME_BYTES {
            let request_path = self.request_path.clone();
            self.http_json("POST", &request_path, Some(&request))
                .await?
        } else {
            self.send_streamed_request(&request_id, &request).await?
        };
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

    async fn send_streamed_request(
        &self,
        stream_id: &str,
        request: &[u8],
    ) -> Result<Vec<u8>, String> {
        let request = std::str::from_utf8(request)
            .map_err(|error| format!("provider runtime request frame is not UTF-8: {error}"))?;
        let chunks = utf8_chunks(request, PROVIDER_HTTP_REQUEST_STREAM_CHUNK_BYTES);
        if chunks.len() > 1024 {
            return Err(format!(
                "reasonKind=provider-runtime-request-stream-too-large frameCount={} maxFrameCount=1024",
                chunks.len()
            ));
        }
        let stream_path = self.request_stream_path.clone();
        let mut final_response = None;
        for (frame_index, request_chunk) in chunks.iter().enumerate() {
            let frame = serde_json::json!({
                "schemaId": "agent.semantic-protocols.provider-runtime-request-stream-frame",
                "schemaVersion": "1",
                "streamId": stream_id,
                "frameIndex": frame_index,
                "frameCount": chunks.len(),
                "requestChunk": request_chunk,
            });
            let frame = serde_json::to_vec(&frame)
                .map_err(|error| format!("encode provider request stream frame: {error}"))?;
            if frame.len() > MAX_PROVIDER_HTTP_REQUEST_FRAME_BYTES {
                return Err("reasonKind=provider-runtime-request-stream-frame-too-large".to_owned());
            }
            let response = self.http_json("POST", &stream_path, Some(&frame)).await?;
            if frame_index + 1 == chunks.len() {
                final_response = Some(response);
            } else {
                let ack: serde_json::Value = serde_json::from_slice(&response)
                    .map_err(|error| format!("decode provider request stream ack: {error}"))?;
                if ack.get("schemaId").and_then(serde_json::Value::as_str)
                    != Some("agent.semantic-protocols.provider-runtime-request-stream-ack")
                    || ack.get("schemaVersion").and_then(serde_json::Value::as_str) != Some("1")
                    || ack.get("streamId").and_then(serde_json::Value::as_str) != Some(stream_id)
                    || ack.get("frameIndex").and_then(serde_json::Value::as_u64)
                        != Some(frame_index as u64)
                    || ack.get("state").and_then(serde_json::Value::as_str) != Some("accepted")
                {
                    return Err("provider request stream acknowledgement drift".to_owned());
                }
            }
        }
        final_response.ok_or_else(|| "provider request stream emitted no frames".to_owned())
    }

    async fn wait_child_terminated(&self) -> Result<AspClientServerChildLifecycle, String> {
        let mut lifecycle = self.lifecycle.clone();
        loop {
            let current = lifecycle.borrow().clone();
            match current {
                AspClientServerChildLifecycle::Running => {
                    lifecycle.changed().await.map_err(|_| {
                        "reasonKind=asp-client-server-child-lifecycle-closed phase=child-lifecycle"
                            .to_owned()
                    })?
                }
                terminal => return Ok(terminal),
            }
        }
    }

    async fn stop(&self) -> Result<(), String> {
        if !matches!(
            self.lifecycle.borrow().clone(),
            AspClientServerChildLifecycle::Running
        ) {
            return Ok(());
        }
        let shutdown_path = self.shutdown_path.clone();
        let force = self
            .http_json_with_lifecycle_deadline("POST", &shutdown_path, Some(b"{}"))
            .await
            .is_err();
        let (response, stopped) = oneshot::channel();
        if self
            .lifecycle_control
            .send(AspClientServerChildControl::Stop { force, response })
            .await
            .is_err()
        {
            return self.wait_child_terminated().await.map(|_| ());
        }
        stopped.await.map_err(|_| {
            "reasonKind=asp-client-server-stop-receipt-dropped phase=shutdown".to_owned()
        })?
    }
}

fn utf8_chunks(value: &str, max_bytes: usize) -> Vec<&str> {
    let mut chunks = Vec::new();
    let mut start = 0;
    while start < value.len() {
        let mut end = (start + max_bytes).min(value.len());
        while !value.is_char_boundary(end) {
            end -= 1;
        }
        chunks.push(&value[start..end]);
        start = end;
    }
    chunks
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

    fn wait_terminated(&self) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send + '_>> {
        Box::pin(async move {
            match self.wait_child_terminated().await? {
                AspClientServerChildLifecycle::Exited { .. } => Ok(()),
                AspClientServerChildLifecycle::Failed(reason) => Err(reason),
                AspClientServerChildLifecycle::Running => unreachable!(),
            }
        })
    }
}

#[cfg(test)]
#[path = "../tests/unit/asp_client_server.rs"]
mod tests;
