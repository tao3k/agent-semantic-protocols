use serde_json::Value;
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::Mutex;

const STDERR_RECEIPT_LIMIT: usize = 64 * 1024;

#[derive(Clone, Debug)]
pub struct GraphTurboResidentLaunchSpec {
    pub program: PathBuf,
    pub args: Vec<OsString>,
    pub cwd: PathBuf,
    pub env: BTreeMap<OsString, OsString>,
    pub execution_command_digest: String,
    pub request_timeout: Duration,
}

pub fn admit_candidate_rank_receipt(receipt: Value) -> Result<Value, String> {
    if receipt.get("status").and_then(Value::as_str) != Some("rank-completed")
        || receipt.get("authority").and_then(Value::as_str) != Some("candidate")
    {
        return Err(
            "Graph Turbo resident receipt crossed the candidate-only authority boundary".to_owned(),
        );
    }
    Ok(receipt)
}

pub struct GraphTurboResidentProcess {
    child: Child,
    stdin: ChildStdin,
    receipts: Lines<BufReader<ChildStdout>>,
    stderr: Arc<Mutex<String>>,
    stderr_reader: Option<tokio::task::JoinHandle<()>>,
    request_timeout: Duration,
    closed: bool,
}

impl GraphTurboResidentProcess {
    pub async fn spawn(spec: GraphTurboResidentLaunchSpec) -> Result<Self, String> {
        if spec.execution_command_digest.is_empty() {
            return Err("Graph Turbo execution command digest must not be empty".to_string());
        }
        let mut command = Command::new(&spec.program);
        command
            .args(&spec.args)
            .current_dir(&spec.cwd)
            .envs(&spec.env)
            .env(
                "ASP_PROVIDER_EXECUTION_COMMAND_DIGEST",
                &spec.execution_command_digest,
            )
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            command.as_std_mut().process_group(0);
        }
        let mut child = command.spawn().map_err(|error| {
            format!(
                "failed to spawn resident Graph Turbo process `{}`: {error}",
                spec.program.display()
            )
        })?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| "resident Graph Turbo stdin was not piped".to_string())?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "resident Graph Turbo stdout was not piped".to_string())?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| "resident Graph Turbo stderr was not piped".to_string())?;
        let stderr_output = Arc::new(Mutex::new(String::new()));
        let stderr_writer = Arc::clone(&stderr_output);
        let stderr_reader = tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let mut destination = stderr_writer.lock().await;
                destination.push_str(&line);
                destination.push('\n');
                if destination.len() > STDERR_RECEIPT_LIMIT {
                    let overflow = destination.len() - STDERR_RECEIPT_LIMIT;
                    destination.drain(..overflow);
                }
            }
        });
        Ok(Self {
            child,
            stdin,
            receipts: BufReader::new(stdout).lines(),
            stderr: stderr_output,
            stderr_reader: Some(stderr_reader),
            request_timeout: spec.request_timeout,
            closed: false,
        })
    }

    pub async fn request(&mut self, message: &Value) -> Result<Value, String> {
        if self.closed {
            return Err("resident Graph Turbo process is already closed".to_string());
        }
        let request_id = message
            .get("requestId")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| "resident Graph Turbo requestId must be non-empty".to_string())?;
        let encoded = serde_json::to_vec(message)
            .map_err(|error| format!("failed to encode resident Graph Turbo request: {error}"))?;
        self.stdin
            .write_all(&encoded)
            .await
            .map_err(|error| format!("failed to write resident Graph Turbo request: {error}"))?;
        self.stdin
            .write_all(b"\n")
            .await
            .map_err(|error| format!("failed to write resident Graph Turbo request: {error}"))?;
        self.stdin
            .flush()
            .await
            .map_err(|error| format!("failed to flush resident Graph Turbo request: {error}"))?;
        let line = tokio::time::timeout(self.request_timeout, self.receipts.next_line())
            .await
            .map_err(|_| {
                format!(
                    "resident Graph Turbo receipt exceeded {:?}",
                    self.request_timeout
                )
            })?
            .map_err(|error| format!("failed to read resident Graph Turbo receipt: {error}"))?;
        let line = match line {
            Some(line) => line,
            None => {
                let stderr = self.stderr.lock().await.trim().to_owned();
                return Err(if stderr.is_empty() {
                    "resident Graph Turbo process closed stdout before a receipt".to_owned()
                } else {
                    format!(
                        "resident Graph Turbo process closed stdout before a receipt; stderr: {stderr}"
                    )
                });
            }
        };
        let receipt = serde_json::from_str::<Value>(&line)
            .map_err(|error| format!("resident Graph Turbo emitted invalid JSON: {error}"))?;
        if receipt.get("requestId").and_then(Value::as_str) != Some(request_id) {
            return Err("resident Graph Turbo receipt requestId mismatch".to_string());
        }
        Ok(receipt)
    }

    pub async fn shutdown(&mut self, message: &Value) -> Result<Value, String> {
        let receipt = self.request(message).await?;
        if receipt.get("status").and_then(Value::as_str) != Some("shutdown-accepted") {
            return Err("resident Graph Turbo rejected shutdown".to_string());
        }
        self.closed = true;
        tokio::time::timeout(self.request_timeout, self.child.wait())
            .await
            .map_err(|_| "resident Graph Turbo child did not exit after typed shutdown".to_owned())?
            .map_err(|error| {
                format!("failed to wait for resident Graph Turbo shutdown: {error}")
            })?;
        if let Some(reader) = self.stderr_reader.take() {
            reader
                .await
                .map_err(|error| format!("resident Graph Turbo stderr reader failed: {error}"))?;
        }
        Ok(receipt)
    }

    pub fn process_id(&self) -> u32 {
        self.child
            .id()
            .expect("resident Graph Turbo child is live while its process owner exists")
    }
}

impl Drop for GraphTurboResidentProcess {
    fn drop(&mut self) {
        if !self.closed {
            let _ = self.child.start_kill();
        }
        if let Some(reader) = self.stderr_reader.take() {
            reader.abort();
        }
    }
}
