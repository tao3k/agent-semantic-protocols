use std::collections::BTreeMap;
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::process::Stdio;

use bytes::Bytes;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};

use crate::{
    ProviderRuntimeContractReceipt, ProviderRuntimePeer, ProviderRuntimeRequestFrame,
    ProviderRuntimeResponseFrame, ProviderRuntimeResponseOutcome,
};

const DEFAULT_MAX_FRAME_BYTES: usize = 16 * 1024 * 1024;

#[derive(Clone, Debug)]
pub struct ProviderRuntimeProcessSpec {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    pub env: BTreeMap<String, String>,
    pub max_frame_bytes: usize,
}

impl ProviderRuntimeProcessSpec {
    pub fn new(program: impl Into<String>, cwd: PathBuf) -> Self {
        Self {
            program: program.into(),
            args: Vec::new(),
            cwd,
            env: BTreeMap::new(),
            max_frame_bytes: DEFAULT_MAX_FRAME_BYTES,
        }
    }
}

pub struct ProviderRuntimeProcessPeer {
    child: Child,
    stdin: ChildStdin,
    stdout: ChildStdout,
    stderr_task: tokio::task::JoinHandle<Result<u64, std::io::Error>>,
    max_frame_bytes: usize,
    next_request_id: u64,
}

impl ProviderRuntimeProcessPeer {
    pub async fn start(spec: ProviderRuntimeProcessSpec) -> Result<Self, String> {
        let mut command = Command::new(&spec.program);
        command
            .args(&spec.args)
            .current_dir(&spec.cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        for (key, value) in &spec.env {
            command.env(key, value);
        }
        let mut child = command.spawn().map_err(|error| {
            format!(
                "failed to start resident provider runtime `{}`: {error}",
                spec.program
            )
        })?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| "resident provider runtime stdin is unavailable".to_owned())?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "resident provider runtime stdout is unavailable".to_owned())?;
        let mut stderr = child
            .stderr
            .take()
            .ok_or_else(|| "resident provider runtime stderr is unavailable".to_owned())?;
        let stderr_task =
            tokio::spawn(async move { tokio::io::copy(&mut stderr, &mut tokio::io::sink()).await });
        Ok(Self {
            child,
            stdin,
            stdout,
            stderr_task,
            max_frame_bytes: spec.max_frame_bytes.max(1),
            next_request_id: 1,
        })
    }

    async fn read_frame(&mut self) -> Result<Vec<u8>, String> {
        let length = self
            .stdout
            .read_u32()
            .await
            .map_err(|error| format!("read resident provider frame length: {error}"))?
            as usize;
        if length == 0 || length > self.max_frame_bytes {
            return Err(format!(
                "resident provider frame length is outside contract: length={length} max={}",
                self.max_frame_bytes
            ));
        }
        let mut frame = vec![0_u8; length];
        self.stdout
            .read_exact(&mut frame)
            .await
            .map_err(|error| format!("read resident provider frame payload: {error}"))?;
        Ok(frame)
    }

    async fn write_frame(&mut self, frame: &[u8]) -> Result<(), String> {
        if frame.is_empty() || frame.len() > self.max_frame_bytes {
            return Err(format!(
                "resident provider request frame is outside contract: length={} max={}",
                frame.len(),
                self.max_frame_bytes
            ));
        }
        self.stdin
            .write_u32(
                frame
                    .len()
                    .try_into()
                    .map_err(|_| "resident provider request frame length exceeds u32".to_owned())?,
            )
            .await
            .map_err(|error| format!("write resident provider frame length: {error}"))?;
        self.stdin
            .write_all(frame)
            .await
            .map_err(|error| format!("write resident provider frame payload: {error}"))?;
        self.stdin
            .flush()
            .await
            .map_err(|error| format!("flush resident provider frame: {error}"))
    }
}

impl ProviderRuntimePeer for ProviderRuntimeProcessPeer {
    fn handshake(
        &mut self,
    ) -> Pin<Box<dyn Future<Output = Result<ProviderRuntimeContractReceipt, String>> + Send + '_>>
    {
        Box::pin(async move {
            let frame = self.read_frame().await?;
            let receipt: ProviderRuntimeContractReceipt = serde_json::from_slice(&frame)
                .map_err(|error| format!("decode resident provider handshake: {error}"))?;
            receipt.validate()?;
            Ok(receipt)
        })
    }

    fn request(
        &mut self,
        operation: String,
        payload: Bytes,
    ) -> Pin<Box<dyn Future<Output = Result<Bytes, String>> + Send + '_>> {
        Box::pin(async move {
            let request_id = format!("provider-runtime-request-{}", self.next_request_id);
            self.next_request_id = self.next_request_id.saturating_add(1);
            let request = ProviderRuntimeRequestFrame::new(&request_id, operation, &payload)?;
            let encoded = serde_json::to_vec(&request)
                .map_err(|error| format!("encode resident provider request: {error}"))?;
            self.write_frame(&encoded).await?;

            let response: ProviderRuntimeResponseFrame =
                serde_json::from_slice(&self.read_frame().await?)
                    .map_err(|error| format!("decode resident provider response: {error}"))?;
            response.validate()?;
            if response.request_id != request_id {
                return Err(format!(
                    "resident provider response requestId drift: expected={request_id} actual={}",
                    response.request_id
                ));
            }
            match response.outcome {
                ProviderRuntimeResponseOutcome::Ready => response
                    .payload_bytes()?
                    .map(Bytes::from)
                    .ok_or_else(|| "validated provider response omitted payload".to_owned()),
                ProviderRuntimeResponseOutcome::Error => Err(response
                    .error
                    .expect("validated provider runtime error response")),
            }
        })
    }

    fn shutdown(&mut self) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send + '_>> {
        Box::pin(async move {
            let _ = self.stdin.shutdown().await;
            if self
                .child
                .try_wait()
                .map_err(|error| {
                    format!("inspect resident provider runtime during shutdown: {error}")
                })?
                .is_none()
            {
                self.child
                    .kill()
                    .await
                    .map_err(|error| format!("kill resident provider runtime: {error}"))?;
            }
            self.child
                .wait()
                .await
                .map_err(|error| format!("reap resident provider runtime: {error}"))?;
            self.stderr_task.abort();
            Ok(())
        })
    }
}
