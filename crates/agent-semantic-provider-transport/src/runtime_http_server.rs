use std::{collections::BTreeMap, future::Future, path::PathBuf, pin::Pin, process::Stdio};

use bytes::Bytes;
use tokio::{
    io::{AsyncBufReadExt, BufReader, Lines},
    process::{Child, ChildStdout, Command},
};

use crate::{
    ProviderRuntimeContractReceipt, ProviderRuntimePeer, ProviderRuntimeRequestFrame,
    ProviderRuntimeResponseFrame, ProviderRuntimeResponseOutcome,
};

#[derive(Clone, Debug)]
pub struct ProviderHttpServerSpec {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    pub env: BTreeMap<String, String>,
    pub host: String,
    pub health_path: String,
    pub request_path: String,
    pub shutdown_path: String,
}

impl ProviderHttpServerSpec {
    pub fn new(program: impl Into<String>, cwd: PathBuf) -> Self {
        Self {
            program: program.into(),
            args: Vec::new(),
            cwd,
            env: BTreeMap::new(),
            host: "127.0.0.1".to_owned(),
            health_path: "/health".to_owned(),
            request_path: "/v1/provider-runtime".to_owned(),
            shutdown_path: "/shutdown".to_owned(),
        }
    }
}

pub struct ProviderHttpServerPeer {
    child: Child,
    stdout: Lines<BufReader<ChildStdout>>,
    stderr_task: tokio::task::JoinHandle<Result<u64, std::io::Error>>,
    client: reqwest::Client,
    base_url: String,
    health_path: String,
    request_path: String,
    shutdown_path: String,
    next_request_id: u64,
}

impl ProviderHttpServerPeer {
    pub async fn start(mut spec: ProviderHttpServerSpec) -> Result<Self, String> {
        spec.env
            .insert("ASP_PROVIDER_SERVER_HOST".to_owned(), spec.host.clone());
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
        let stderr_task = tokio::spawn(async move {
            let mut stderr = stderr;
            tokio::io::copy(&mut stderr, &mut tokio::io::sink()).await
        });
        let client = reqwest::Client::builder()
            .no_proxy()
            .http1_only()
            .pool_max_idle_per_host(1)
            .build()
            .map_err(|error| format!("construct provider HTTP client: {error}"))?;
        Ok(Self {
            child,
            stdout: BufReader::new(stdout).lines(),
            stderr_task,
            client,
            base_url: String::new(),
            health_path: spec.health_path,
            request_path: spec.request_path,
            shutdown_path: spec.shutdown_path,
            next_request_id: 0,
        })
    }

    async fn http_json(
        &mut self,
        method: &str,
        path: &str,
        body: Option<&[u8]>,
    ) -> Result<Vec<u8>, String> {
        let method = reqwest::Method::from_bytes(method.as_bytes())
            .map_err(|error| format!("construct provider HTTP method: {error}"))?;
        let url = format!("{}{}", self.base_url.trim_end_matches('/'), path);
        let mut request = self
            .client
            .request(method, url)
            .header(reqwest::header::ACCEPT, "application/json");
        if let Some(body) = body {
            request = request
                .header(reqwest::header::CONTENT_TYPE, "application/json")
                .body(body.to_vec());
        }
        let response = request
            .send()
            .await
            .map_err(|error| format!("send provider HTTP request: {error}"))?;
        let status = response.status();
        let response_body = response
            .bytes()
            .await
            .map_err(|error| format!("read provider HTTP response: {error}"))?;
        if !status.is_success() {
            return Err(format!("provider HTTP server returned status {status}"));
        }
        Ok(response_body.to_vec())
    }

    async fn contract_receipt(&mut self) -> Result<ProviderRuntimeContractReceipt, String> {
        let bootstrap = self
            .stdout
            .next_line()
            .await
            .map_err(|error| format!("read provider HTTP server bootstrap: {error}"))?
            .ok_or_else(|| "provider HTTP server exited before bootstrap".to_owned())?;
        let bootstrap: serde_json::Value = serde_json::from_str(&bootstrap)
            .map_err(|error| format!("decode provider HTTP server bootstrap: {error}"))?;
        if bootstrap
            .get("schemaId")
            .and_then(serde_json::Value::as_str)
            != Some("agent.semantic-protocols.provider-http-server-bootstrap")
            || bootstrap
                .get("schemaVersion")
                .and_then(serde_json::Value::as_str)
                != Some("1")
            || bootstrap
                .get("transport")
                .and_then(serde_json::Value::as_str)
                != Some("http-json-v1")
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
        self.base_url = endpoint_url.to_string();
        let health_path = self.health_path.clone();
        let response = self.http_json("GET", &health_path, None).await?;
        let receipt = serde_json::from_slice::<ProviderRuntimeContractReceipt>(&response)
            .map_err(|error| format!("decode provider HTTP server health: {error}"))?;
        receipt.validate()?;
        Ok(receipt)
    }

    async fn send_request(&mut self, operation: String, payload: Bytes) -> Result<Bytes, String> {
        self.next_request_id = self.next_request_id.saturating_add(1);
        let request_id = format!("provider-http-{}", self.next_request_id);
        let request = ProviderRuntimeRequestFrame::new(&request_id, operation, &payload)?;
        let request = serde_json::to_vec(&request)
            .map_err(|error| format!("encode provider HTTP server request: {error}"))?;
        let request_path = self.request_path.clone();
        let response = self
            .http_json("POST", &request_path, Some(&request))
            .await?;
        let response = serde_json::from_slice::<ProviderRuntimeResponseFrame>(&response)
            .map_err(|error| format!("decode provider HTTP server response: {error}"))?;
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

    async fn stop(&mut self) -> Result<(), String> {
        if self
            .child
            .try_wait()
            .map_err(|error| format!("inspect provider HTTP server: {error}"))?
            .is_none()
        {
            let shutdown_path = self.shutdown_path.clone();
            if self
                .http_json("POST", &shutdown_path, Some(b"{}"))
                .await
                .is_err()
            {
                self.child
                    .kill()
                    .await
                    .map_err(|error| format!("stop provider HTTP server: {error}"))?;
            }
        }
        self.child
            .wait()
            .await
            .map_err(|error| format!("reap provider HTTP server: {error}"))?;
        self.stderr_task.abort();
        Ok(())
    }
}

impl ProviderRuntimePeer for ProviderHttpServerPeer {
    fn handshake(
        &mut self,
    ) -> Pin<Box<dyn Future<Output = Result<ProviderRuntimeContractReceipt, String>> + Send + '_>>
    {
        Box::pin(self.contract_receipt())
    }

    fn request(
        &mut self,
        operation: String,
        payload: Bytes,
    ) -> Pin<Box<dyn Future<Output = Result<Bytes, String>> + Send + '_>> {
        Box::pin(self.send_request(operation, payload))
    }

    fn shutdown(&mut self) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send + '_>> {
        Box::pin(self.stop())
    }
}
