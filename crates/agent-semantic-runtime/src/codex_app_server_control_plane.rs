//! Tokio-owned Codex app-server authority for the multi-agent v2 control plane.

use std::ffi::OsString;

use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::{Mutex, mpsc, oneshot};
use tokio::task::JoinHandle;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CodexHostThreadState {
    Active,
    Idle,
    Stopped,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CodexHostThread {
    pub thread_id: String,
    pub parent_thread_id: String,
    pub agent_path: String,
    pub agent_nickname: Option<String>,
    pub agent_role: String,
    pub state: CodexHostThreadState,
}

enum AuthorityRequest {
    ListDirectChildren {
        root_session_id: String,
        response: oneshot::Sender<Result<Vec<CodexHostThread>, String>>,
    },
    Shutdown {
        response: oneshot::Sender<Result<(), String>>,
    },
}

/// One Runtime-owned actor and one lazily-created Codex app-server process.
///
/// All JSON-RPC requests and notifications are consumed by the actor.  This
/// prevents concurrent callers from spawning duplicate app-server processes or
/// competing for stdout, and gives Runtime shutdown one joinable owner.
pub struct CodexAppServerControlPlane {
    sender: mpsc::Sender<AuthorityRequest>,
    task: Mutex<Option<JoinHandle<()>>>,
}

impl CodexAppServerControlPlane {
    pub fn start() -> Self {
        let (sender, receiver) = mpsc::channel(32);
        let task = tokio::spawn(authority_loop(receiver));
        Self {
            sender,
            task: Mutex::new(Some(task)),
        }
    }

    pub async fn list_direct_children(
        &self,
        root_session_id: &str,
    ) -> Result<Vec<CodexHostThread>, String> {
        if root_session_id.trim().is_empty() {
            return Err("Codex host authority requires a root session id".to_owned());
        }
        let (response, receipt) = oneshot::channel();
        self.sender
            .send(AuthorityRequest::ListDirectChildren {
                root_session_id: root_session_id.to_owned(),
                response,
            })
            .await
            .map_err(|_| "Codex host authority task is unavailable".to_owned())?;
        receipt
            .await
            .map_err(|_| "Codex host authority task dropped its receipt".to_owned())?
    }

    pub async fn shutdown(&self) -> Result<(), String> {
        let task = self.task.lock().await.take();
        let Some(task) = task else {
            return Ok(());
        };
        let (response, receipt) = oneshot::channel();
        let send_result = self
            .sender
            .send(AuthorityRequest::Shutdown { response })
            .await;
        let actor_result = if send_result.is_ok() {
            receipt.await.unwrap_or_else(|_| {
                Err("Codex host authority task dropped its shutdown receipt".to_owned())
            })
        } else {
            Ok(())
        };
        task.await
            .map_err(|error| format!("Codex host authority task failed to join: {error}"))?;
        actor_result
    }
}

async fn authority_loop(mut receiver: mpsc::Receiver<AuthorityRequest>) {
    let mut process = None;
    while let Some(request) = receiver.recv().await {
        match request {
            AuthorityRequest::ListDirectChildren {
                root_session_id,
                response,
            } => {
                let result = list_with_one_reconnect(&mut process, &root_session_id).await;
                let _ = response.send(result);
            }
            AuthorityRequest::Shutdown { response } => {
                let result = shutdown_process(&mut process).await;
                let _ = response.send(result);
                return;
            }
        }
    }
    let _ = shutdown_process(&mut process).await;
}

async fn list_with_one_reconnect(
    process: &mut Option<CodexAppServerProcess>,
    root_session_id: &str,
) -> Result<Vec<CodexHostThread>, String> {
    for attempt in 0..2 {
        if process.is_none() {
            *process = Some(CodexAppServerProcess::spawn().await?);
        }
        let result = process
            .as_mut()
            .expect("process initialized")
            .list_direct_children(root_session_id)
            .await;
        match result {
            Ok(children) => return Ok(children),
            Err(error) if attempt == 0 => {
                let _ = shutdown_process(process).await;
                if error.contains("invalid Codex host authority") {
                    return Err(error);
                }
            }
            Err(error) => return Err(error),
        }
    }
    Err("Codex host authority reconnect exhausted".to_owned())
}

async fn shutdown_process(process: &mut Option<CodexAppServerProcess>) -> Result<(), String> {
    let Some(mut process) = process.take() else {
        return Ok(());
    };
    if process.child.id().is_some() {
        process
            .child
            .kill()
            .await
            .map_err(|error| format!("failed to stop Codex app-server authority: {error}"))?;
    }
    process
        .child
        .wait()
        .await
        .map_err(|error| format!("failed to join Codex app-server authority: {error}"))?;
    Ok(())
}

struct CodexAppServerProcess {
    child: Child,
    stdin: ChildStdin,
    lines: Lines<BufReader<ChildStdout>>,
    next_request_id: u64,
}

impl CodexAppServerProcess {
    async fn spawn() -> Result<Self, String> {
        let binary = std::env::var_os("ASP_CODEX_BIN").unwrap_or_else(|| OsString::from("codex"));
        let mut child = Command::new(binary)
            .args(["app-server", "--stdio"])
            .kill_on_drop(true)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map_err(|error| format!("failed to start Codex app-server authority: {error}"))?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| "Codex app-server authority has no stdin".to_owned())?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "Codex app-server authority has no stdout".to_owned())?;
        let mut process = Self {
            child,
            stdin,
            lines: BufReader::new(stdout).lines(),
            next_request_id: 1,
        };
        process.initialize().await?;
        Ok(process)
    }

    async fn initialize(&mut self) -> Result<(), String> {
        self.request(
            "initialize",
            serde_json::json!({
                "clientInfo": {
                    "name": "asp_runtime_control_plane",
                    "title": "ASP Runtime Control Plane",
                    "version": env!("CARGO_PKG_VERSION"),
                },
                "capabilities": { "experimentalApi": true },
            }),
        )
        .await?;
        self.notify("initialized", serde_json::json!({})).await
    }

    async fn list_direct_children(
        &mut self,
        root_session_id: &str,
    ) -> Result<Vec<CodexHostThread>, String> {
        let response = self
            .request(
                "thread/list",
                serde_json::json!({
                    "parentThreadId": root_session_id,
                    "limit": 100,
                    "sortKey": "updated_at",
                    "sortDirection": "desc",
                }),
            )
            .await?;
        let rows = response
            .pointer("/result/data")
            .and_then(Value::as_array)
            .ok_or_else(|| "invalid Codex host authority thread/list response".to_owned())?;
        rows.iter()
            .filter(|row| {
                row.get("parentThreadId").and_then(Value::as_str) == Some(root_session_id)
            })
            .map(parse_thread)
            .collect()
    }

    async fn request(&mut self, method: &str, params: Value) -> Result<Value, String> {
        let request_id = self.next_request_id;
        self.next_request_id = self.next_request_id.saturating_add(1);
        self.write(&serde_json::json!({
            "method": method,
            "id": request_id,
            "params": params,
        }))
        .await?;
        while let Some(line) = self
            .lines
            .next_line()
            .await
            .map_err(|error| format!("failed reading Codex host authority: {error}"))?
        {
            let value = serde_json::from_str::<Value>(&line)
                .map_err(|error| format!("invalid Codex host authority JSON: {error}"))?;
            if value.get("id").and_then(Value::as_u64) != Some(request_id) {
                continue;
            }
            if let Some(error) = value.get("error") {
                return Err(format!("Codex host authority {method} failed: {error}"));
            }
            return Ok(value);
        }
        Err("Codex host authority stdout closed".to_owned())
    }

    async fn notify(&mut self, method: &str, params: Value) -> Result<(), String> {
        self.write(&serde_json::json!({ "method": method, "params": params }))
            .await
    }

    async fn write(&mut self, value: &Value) -> Result<(), String> {
        self.stdin
            .write_all(value.to_string().as_bytes())
            .await
            .map_err(|error| format!("failed writing Codex host authority: {error}"))?;
        self.stdin
            .write_all(b"\n")
            .await
            .map_err(|error| format!("failed delimiting Codex host authority: {error}"))?;
        self.stdin
            .flush()
            .await
            .map_err(|error| format!("failed flushing Codex host authority: {error}"))
    }
}

fn parse_thread(value: &Value) -> Result<CodexHostThread, String> {
    let required = |pointer: &str, label: &str| {
        value
            .pointer(pointer)
            .and_then(Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| format!("invalid Codex host authority thread: missing {label}"))
    };
    let state = match required("/status/type", "status.type")?.as_str() {
        "active" => CodexHostThreadState::Active,
        "idle" => CodexHostThreadState::Idle,
        "notLoaded" => CodexHostThreadState::Stopped,
        status => {
            return Err(format!(
                "invalid Codex host authority thread status `{status}`"
            ));
        }
    };
    Ok(CodexHostThread {
        thread_id: required("/id", "id")?,
        parent_thread_id: required("/parentThreadId", "parentThreadId")?,
        agent_path: required("/source/subAgent/thread_spawn/agent_path", "agent_path")?,
        agent_nickname: value
            .pointer("/source/subAgent/thread_spawn/agent_nickname")
            .and_then(Value::as_str)
            .map(str::to_owned),
        agent_role: required("/source/subAgent/thread_spawn/agent_role", "agent_role")?,
        state,
    })
}
