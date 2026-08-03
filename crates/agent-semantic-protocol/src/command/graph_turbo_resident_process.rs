use serde_json::Value;
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

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
    receipts: Receiver<Result<Value, String>>,
    reader: Option<JoinHandle<()>>,
    stderr: Arc<Mutex<String>>,
    stderr_reader: Option<JoinHandle<()>>,
    request_timeout: Duration,
    closed: bool,
}

impl GraphTurboResidentProcess {
    pub fn spawn(spec: GraphTurboResidentLaunchSpec) -> Result<Self, String> {
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
            .stderr(Stdio::piped());
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
        let stderr_reader = thread::spawn(move || {
            let mut output = String::new();
            let _ = BufReader::new(stderr).read_to_string(&mut output);
            if let Ok(mut destination) = stderr_writer.lock() {
                *destination = output;
            }
        });
        let (sender, receipts) = mpsc::channel();
        let reader = thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let receipt = line
                    .map_err(|error| {
                        format!("failed to read resident Graph Turbo receipt: {error}")
                    })
                    .and_then(|line| {
                        serde_json::from_str(&line).map_err(|error| {
                            format!("resident Graph Turbo emitted invalid JSON: {error}")
                        })
                    });
                if sender.send(receipt).is_err() {
                    return;
                }
            }
            let _ = sender.send(Err(
                "resident Graph Turbo process closed stdout before a receipt".to_string(),
            ));
        });
        Ok(Self {
            child,
            stdin,
            receipts,
            reader: Some(reader),
            stderr: stderr_output,
            stderr_reader: Some(stderr_reader),
            request_timeout: spec.request_timeout,
            closed: false,
        })
    }

    pub fn request(&mut self, message: &Value) -> Result<Value, String> {
        if self.closed {
            return Err("resident Graph Turbo process is already closed".to_string());
        }
        let request_id = message
            .get("requestId")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| "resident Graph Turbo requestId must be non-empty".to_string())?;
        serde_json::to_writer(&mut self.stdin, message)
            .map_err(|error| format!("failed to encode resident Graph Turbo request: {error}"))?;
        self.stdin
            .write_all(b"\n")
            .and_then(|()| self.stdin.flush())
            .map_err(|error| format!("failed to write resident Graph Turbo request: {error}"))?;
        let receipt = self
            .receipts
            .recv_timeout(self.request_timeout)
            .map_err(|error| format!("resident Graph Turbo receipt timeout: {error}"))?;
        let receipt = receipt.map_err(|error| {
            thread::sleep(Duration::from_millis(10));
            let stderr = self
                .stderr
                .lock()
                .map(|output| output.trim().to_string())
                .unwrap_or_else(|_| "<stderr lock poisoned>".to_string());
            if stderr.is_empty() {
                error
            } else {
                format!("{error}; stderr: {stderr}")
            }
        })?;
        if receipt.get("requestId").and_then(Value::as_str) != Some(request_id) {
            return Err("resident Graph Turbo receipt requestId mismatch".to_string());
        }
        Ok(receipt)
    }

    pub fn shutdown(&mut self, message: &Value) -> Result<Value, String> {
        let receipt = self.request(message)?;
        if receipt.get("status").and_then(Value::as_str) != Some("shutdown-accepted") {
            return Err("resident Graph Turbo rejected shutdown".to_string());
        }
        self.closed = true;
        self.child.wait().map_err(|error| {
            format!("failed to wait for resident Graph Turbo shutdown: {error}")
        })?;
        if let Some(reader) = self.reader.take() {
            reader
                .join()
                .map_err(|_| "resident Graph Turbo receipt reader panicked".to_string())?;
        }
        if let Some(reader) = self.stderr_reader.take() {
            reader
                .join()
                .map_err(|_| "resident Graph Turbo stderr reader panicked".to_string())?;
        }
        Ok(receipt)
    }

    pub fn process_id(&self) -> u32 {
        self.child.id()
    }
}

impl Drop for GraphTurboResidentProcess {
    fn drop(&mut self) {
        if !self.closed {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
        if let Some(reader) = self.stderr_reader.take() {
            let _ = reader.join();
        }
    }
}
