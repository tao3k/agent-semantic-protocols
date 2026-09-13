// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::collections::BTreeMap;
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::process::Stdio;

use bytes::Bytes;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::process::ChildStdin;
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
    state: tokio::sync::Mutex<ProviderRuntimeProcessState>,
    lifecycle: watch::Receiver<ProviderRuntimeProcessLifecycle>,
    lifecycle_control: mpsc::Sender<ProviderRuntimeProcessControl>,
}

struct ProviderRuntimeProcessState {
    stdin: Option<ChildStdin>,
    stdout: ChildStdout,
    max_frame_bytes: usize,
    next_request_id: u64,
}

#[derive(Clone, Debug)]
enum ProviderRuntimeProcessLifecycle {
    Running,
    Exited,
    Failed(String),
}

enum ProviderRuntimeProcessControl {
    Stop {
        force: bool,
        response: oneshot::Sender<Result<(), String>>,
    },
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
        let (lifecycle_control, mut lifecycle_commands) = mpsc::channel(1);
        let (lifecycle_writer, lifecycle) =
            watch::channel(ProviderRuntimeProcessLifecycle::Running);
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
                    Some(ProviderRuntimeProcessControl::Stop { force, response }) => {
                        ChildOutcome::Stop { force, response }
                    }
                    None => ChildOutcome::Exited(child.wait().await),
                },
            };
            let (status, stop_response) = match outcome {
                ChildOutcome::Exited(status) => (status, None),
                ChildOutcome::Stop { force, response } => {
                    if force && let Err(error) = child.kill().await {
                        let reason = format!(
                            "reasonKind=provider-runtime-process-kill-failed phase=shutdown error={error}"
                        );
                        lifecycle_writer
                            .send_replace(ProviderRuntimeProcessLifecycle::Failed(reason.clone()));
                        let _ = response.send(Err(reason));
                        return;
                    }
                    (child.wait().await, Some(response))
                }
            };
            let stderr_result = stderr_task.await;
            let terminal = match status {
                Ok(status) => {
                    tracing::info!(
                        target: "asp.client_server",
                        phase = "child-lifecycle",
                        reason_kind = "provider-runtime-process-exited",
                        status = %status,
                    );
                    ProviderRuntimeProcessLifecycle::Exited
                }
                Err(error) => {
                    let reason = format!(
                        "reasonKind=provider-runtime-process-wait-failed phase=child-lifecycle error={error}"
                    );
                    tracing::error!(
                        target: "asp.client_server",
                        phase = "child-lifecycle",
                        reason_kind = "provider-runtime-process-wait-failed",
                        error = %error,
                    );
                    ProviderRuntimeProcessLifecycle::Failed(reason)
                }
            };
            let stop_result = match (&terminal, stderr_result) {
                (ProviderRuntimeProcessLifecycle::Failed(reason), _) => Err(reason.clone()),
                (_, Err(error)) => Err(format!(
                    "reasonKind=provider-runtime-process-stderr-task-failed phase=child-lifecycle error={error}"
                )),
                (_, Ok(Err(error))) => Err(format!(
                    "reasonKind=provider-runtime-process-stderr-read-failed phase=child-lifecycle error={error}"
                )),
                (_, Ok(Ok(_))) => Ok(()),
            };
            lifecycle_writer.send_replace(terminal);
            if let Some(response) = stop_response {
                let _ = response.send(stop_result);
            }
        });
        Ok(Self {
            state: tokio::sync::Mutex::new(ProviderRuntimeProcessState {
                stdin: Some(stdin),
                stdout,
                max_frame_bytes: spec.max_frame_bytes.max(1),
                next_request_id: 1,
            }),
            lifecycle,
            lifecycle_control,
        })
    }

    async fn read_frame(state: &mut ProviderRuntimeProcessState) -> Result<Vec<u8>, String> {
        let length = state
            .stdout
            .read_u32()
            .await
            .map_err(|error| format!("read resident provider frame length: {error}"))?
            as usize;
        if length == 0 || length > state.max_frame_bytes {
            return Err(format!(
                "resident provider frame length is outside contract: length={length} max={}",
                state.max_frame_bytes
            ));
        }
        let mut frame = vec![0_u8; length];
        state
            .stdout
            .read_exact(&mut frame)
            .await
            .map_err(|error| format!("read resident provider frame payload: {error}"))?;
        Ok(frame)
    }

    async fn write_frame(
        state: &mut ProviderRuntimeProcessState,
        frame: &[u8],
    ) -> Result<(), String> {
        if frame.is_empty() || frame.len() > state.max_frame_bytes {
            return Err(format!(
                "resident provider request frame is outside contract: length={} max={}",
                frame.len(),
                state.max_frame_bytes
            ));
        }
        let stdin = state
            .stdin
            .as_mut()
            .ok_or_else(|| "resident provider runtime stdin is closed".to_owned())?;
        stdin
            .write_u32(
                frame
                    .len()
                    .try_into()
                    .map_err(|_| "resident provider request frame length exceeds u32".to_owned())?,
            )
            .await
            .map_err(|error| format!("write resident provider frame length: {error}"))?;
        stdin
            .write_all(frame)
            .await
            .map_err(|error| format!("write resident provider frame payload: {error}"))?;
        stdin
            .flush()
            .await
            .map_err(|error| format!("flush resident provider frame: {error}"))
    }

    async fn wait_lifecycle_terminal(&self) -> Result<ProviderRuntimeProcessLifecycle, String> {
        let mut lifecycle = self.lifecycle.clone();
        loop {
            let current = lifecycle.borrow().clone();
            match current {
                ProviderRuntimeProcessLifecycle::Running => {
                    lifecycle.changed().await.map_err(|_| {
                        "reasonKind=provider-runtime-process-lifecycle-closed phase=child-lifecycle"
                            .to_owned()
                    })?;
                }
                terminal => return Ok(terminal),
            }
        }
    }
}

impl ProviderRuntimePeer for ProviderRuntimeProcessPeer {
    fn handshake(
        &self,
    ) -> Pin<Box<dyn Future<Output = Result<ProviderRuntimeContractReceipt, String>> + Send + '_>>
    {
        Box::pin(async move {
            let mut state = self.state.lock().await;
            let frame = Self::read_frame(&mut state).await?;
            let receipt: ProviderRuntimeContractReceipt = serde_json::from_slice(&frame)
                .map_err(|error| format!("decode resident provider handshake: {error}"))?;
            receipt.validate()?;
            Ok(receipt)
        })
    }

    fn request(
        &self,
        operation: String,
        payload: Bytes,
    ) -> Pin<Box<dyn Future<Output = Result<Bytes, String>> + Send + '_>> {
        Box::pin(async move {
            let mut state = self.state.lock().await;
            let request_id = format!("provider-runtime-request-{}", state.next_request_id);
            state.next_request_id = state.next_request_id.saturating_add(1);
            let request = ProviderRuntimeRequestFrame::new(&request_id, operation, &payload)?;
            let encoded = serde_json::to_vec(&request)
                .map_err(|error| format!("encode resident provider request: {error}"))?;
            Self::write_frame(&mut state, &encoded).await?;

            let response: ProviderRuntimeResponseFrame =
                serde_json::from_slice(&Self::read_frame(&mut state).await?)
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

    fn shutdown(&self) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send + '_>> {
        Box::pin(async move {
            if !matches!(
                self.lifecycle.borrow().clone(),
                ProviderRuntimeProcessLifecycle::Running
            ) {
                return Ok(());
            }
            let stdin = self.state.lock().await.stdin.take();
            if let Some(mut stdin) = stdin {
                stdin
                    .shutdown()
                    .await
                    .map_err(|error| format!("close resident provider runtime stdin: {error}"))?;
            }
            let (response, stopped) = oneshot::channel();
            if self
                .lifecycle_control
                .send(ProviderRuntimeProcessControl::Stop {
                    force: false,
                    response,
                })
                .await
                .is_err()
            {
                return self.wait_lifecycle_terminal().await.map(|_| ());
            }
            stopped.await.map_err(|_| {
                "reasonKind=provider-runtime-process-stop-receipt-dropped phase=shutdown".to_owned()
            })?
        })
    }

    fn wait_terminated(&self) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send + '_>> {
        Box::pin(async move {
            match self.wait_lifecycle_terminal().await? {
                ProviderRuntimeProcessLifecycle::Exited => Ok(()),
                ProviderRuntimeProcessLifecycle::Failed(reason) => Err(reason),
                ProviderRuntimeProcessLifecycle::Running => unreachable!(),
            }
        })
    }
}
