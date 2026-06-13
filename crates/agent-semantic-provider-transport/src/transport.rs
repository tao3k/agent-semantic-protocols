//! Tokio-backed process runner for ASP language providers.

use std::borrow::Cow;
use std::io::ErrorKind;
use std::process::{ExitStatus, Stdio};
use std::time::{Duration, Instant};

use bstr::BStr;
use bytes::Bytes;
use tokio::io::AsyncWriteExt;
use tokio::process::{Child, ChildStdin, Command};
use tokio::task::JoinHandle;
use tracing::{Instrument, debug, info_span, warn};

use crate::byte_text;
use crate::capture::{LimitedRead, ProviderOutputStream, capture_output_stream};
use crate::process_contract::{
    OutputMode, ProviderProcessError, ProviderProcessFraming, ProviderProcessLimits,
    ProviderProcessReceipt, ProviderProcessSpec, StdinMode,
};

/// Captured result from a provider process run.
#[derive(Debug)]
pub struct ProviderProcessOutput {
    /// Exit status reported by the provider process.
    pub status: ExitStatus,
    /// Captured stdout, possibly truncated by configured limits.
    pub stdout: Bytes,
    /// Captured stderr, possibly truncated by configured limits.
    pub stderr: Bytes,
    /// Structured receipt describing timing and truncation.
    pub receipt: ProviderProcessReceipt,
}

impl ProviderProcessOutput {
    /// Captured stdout as a `bstr` byte string.
    pub fn stdout_bstr(&self) -> &BStr {
        byte_text::as_bstr(self.stdout.as_ref())
    }

    /// Captured stderr as a `bstr` byte string.
    pub fn stderr_bstr(&self) -> &BStr {
        byte_text::as_bstr(self.stderr.as_ref())
    }

    /// Captured stdout rendered with lossy UTF-8 replacement.
    pub fn stdout_lossy(&self) -> Cow<'_, str> {
        byte_text::lossy(self.stdout.as_ref())
    }

    /// Captured stderr rendered with lossy UTF-8 replacement.
    pub fn stderr_lossy(&self) -> Cow<'_, str> {
        byte_text::lossy(self.stderr.as_ref())
    }
}

/// Run a provider process on a current-thread Tokio runtime.
pub fn run_provider_process(
    spec: ProviderProcessSpec,
) -> Result<ProviderProcessOutput, ProviderProcessError> {
    run_provider_process_with_framing(spec, ProviderProcessFraming::default())
}

/// Run a provider process with explicit stdout/stderr framing on a current-thread Tokio runtime.
pub fn run_provider_process_with_framing(
    spec: ProviderProcessSpec,
    framing: ProviderProcessFraming,
) -> Result<ProviderProcessOutput, ProviderProcessError> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .enable_time()
        .build()
        .map_err(|source| ProviderProcessError::Runtime { source })?;
    runtime.block_on(run_provider_process_async_with_framing(spec, framing))
}

/// Run a provider process asynchronously and capture stdout, stderr, and receipt data.
pub async fn run_provider_process_async(
    spec: ProviderProcessSpec,
) -> Result<ProviderProcessOutput, ProviderProcessError> {
    run_provider_process_async_with_framing(spec, ProviderProcessFraming::default()).await
}

/// Run a provider process asynchronously with explicit stdout/stderr framing.
pub async fn run_provider_process_async_with_framing(
    spec: ProviderProcessSpec,
    framing: ProviderProcessFraming,
) -> Result<ProviderProcessOutput, ProviderProcessError> {
    let timeout_ms = spec.limits.timeout.map(|timeout| timeout.as_millis());
    let span = info_span!(
        "provider_process",
        program = %spec.program,
        cwd = %spec.cwd.display(),
        args = spec.args.len(),
        timeout_ms = ?timeout_ms,
    );

    async move {
        let start = Instant::now();
        let stdin_mode = spec.stdin.clone();
        let stdout_mode = spec.stdout;
        let stderr_mode = spec.stderr;
        let limits = spec.limits;
        let mut child = spawn_provider_process(&spec, &stdin_mode)?;
        debug!("spawned provider process");
        let io_tasks = spawn_provider_io_tasks(
            &mut child,
            stdin_mode,
            stdout_mode,
            stderr_mode,
            limits,
            framing,
        )?;
        collect_provider_output(child, io_tasks, limits.timeout, start).await
    }
    .instrument(span)
    .await
}

fn spawn_provider_process(
    spec: &ProviderProcessSpec,
    stdin_mode: &StdinMode,
) -> Result<Child, ProviderProcessError> {
    let mut command = Command::new(&spec.program);
    command
        .args(&spec.args)
        .current_dir(&spec.cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (key, value) in &spec.env {
        command.env(key, value);
    }
    match stdin_mode {
        StdinMode::Inherit => {
            command.stdin(Stdio::inherit());
        }
        StdinMode::Closed => {
            command.stdin(Stdio::null());
        }
        StdinMode::Bytes(_) => {
            command.stdin(Stdio::piped());
        }
    }

    command
        .spawn()
        .map_err(|source| ProviderProcessError::Spawn {
            program: spec.program.clone(),
            source,
        })
}

struct ProviderIoTasks {
    stdin: JoinHandle<Result<(), ProviderProcessError>>,
    stdout: JoinHandle<Result<LimitedRead, ProviderProcessError>>,
    stderr: JoinHandle<Result<LimitedRead, ProviderProcessError>>,
}

fn spawn_provider_io_tasks(
    child: &mut Child,
    stdin_mode: StdinMode,
    stdout_mode: OutputMode,
    stderr_mode: OutputMode,
    limits: ProviderProcessLimits,
    framing: ProviderProcessFraming,
) -> Result<ProviderIoTasks, ProviderProcessError> {
    let stdout = child
        .stdout
        .take()
        .ok_or(ProviderProcessError::CaptureStdout)?;
    let stderr = child
        .stderr
        .take()
        .ok_or(ProviderProcessError::CaptureStderr)?;
    let stdin = child.stdin.take();

    Ok(ProviderIoTasks {
        stdin: tokio::spawn(write_stdin(stdin, stdin_mode)),
        stdout: tokio::spawn(capture_output_stream(
            stdout,
            limits.max_stdout_bytes,
            ProviderOutputStream::Stdout,
            stdout_mode,
            framing.stdout,
        )),
        stderr: tokio::spawn(capture_output_stream(
            stderr,
            limits.max_stderr_bytes,
            ProviderOutputStream::Stderr,
            stderr_mode,
            framing.stderr,
        )),
    })
}

async fn collect_provider_output(
    mut child: Child,
    tasks: ProviderIoTasks,
    timeout: Option<Duration>,
    start: Instant,
) -> Result<ProviderProcessOutput, ProviderProcessError> {
    let ProviderIoTasks {
        stdin: stdin_task,
        stdout: stdout_task,
        stderr: stderr_task,
    } = tasks;

    let status = if let Some(timeout) = timeout {
        tokio::select! {
            result = child.wait() => {
                result.map_err(|source| ProviderProcessError::Wait { source })?
            }
            _ = tokio::time::sleep(timeout) => {
                warn!(
                    timeout_ms = timeout.as_millis(),
                    "provider process timed out; requesting kill"
                );
                let _ = child.start_kill();
                let _ = child.wait().await;
                let _ = join_transport_task(stdin_task, "stdin").await;
                let (stdout, stderr) =
                    join_readers_after_timeout(stdout_task, stderr_task).await;
                return Err(ProviderProcessError::Timeout {
                    timeout,
                    receipt: provider_process_receipt(start, None, stdout, stderr, true),
                });
            }
        }
    } else {
        child
            .wait()
            .await
            .map_err(|source| ProviderProcessError::Wait { source })?
    };

    debug!(
        status = ?status.code(),
        elapsed_ms = start.elapsed().as_millis(),
        "provider process exited"
    );
    join_transport_task(stdin_task, "stdin").await?;
    let stdout = join_transport_task(stdout_task, "stdout").await?;
    let stderr = join_transport_task(stderr_task, "stderr").await?;
    Ok(provider_process_output(
        start, status, stdout, stderr, false,
    ))
}

fn provider_process_output(
    start: Instant,
    status: ExitStatus,
    stdout: LimitedRead,
    stderr: LimitedRead,
    timed_out: bool,
) -> ProviderProcessOutput {
    let receipt = provider_process_receipt(
        start,
        Some(status),
        stdout.clone(),
        stderr.clone(),
        timed_out,
    );
    ProviderProcessOutput {
        status,
        receipt,
        stdout: stdout.bytes,
        stderr: stderr.bytes,
    }
}

fn provider_process_receipt(
    start: Instant,
    status: Option<ExitStatus>,
    stdout: LimitedRead,
    stderr: LimitedRead,
    timed_out: bool,
) -> ProviderProcessReceipt {
    ProviderProcessReceipt {
        elapsed: start.elapsed(),
        status_code: status.and_then(|status| status.code()),
        status_success: status.is_some_and(|status| status.success()),
        stdout_bytes: stdout.total_bytes,
        stderr_bytes: stderr.total_bytes,
        stdout_sha256: stdout.sha256,
        stderr_sha256: stderr.sha256,
        stdout_truncated: stdout.truncated,
        stderr_truncated: stderr.truncated,
        timed_out,
    }
}

async fn write_stdin(
    stdin: Option<ChildStdin>,
    stdin_mode: StdinMode,
) -> Result<(), ProviderProcessError> {
    if let StdinMode::Bytes(bytes) = stdin_mode {
        let mut stdin = stdin.ok_or(ProviderProcessError::CaptureStdin)?;
        if let Err(source) = stdin.write_all(&bytes).await {
            if source.kind() == ErrorKind::BrokenPipe {
                return Ok(());
            }
            return Err(ProviderProcessError::StdinWrite { source });
        }
        if let Err(source) = stdin.shutdown().await {
            if source.kind() == ErrorKind::BrokenPipe {
                return Ok(());
            }
            return Err(ProviderProcessError::StdinClose { source });
        }
    }
    Ok(())
}

async fn join_transport_task<T>(
    task: JoinHandle<Result<T, ProviderProcessError>>,
    task_name: &'static str,
) -> Result<T, ProviderProcessError> {
    task.await.map_err(|source| ProviderProcessError::Join {
        task: task_name,
        source,
    })?
}

async fn join_readers_after_timeout(
    stdout_task: JoinHandle<Result<LimitedRead, ProviderProcessError>>,
    stderr_task: JoinHandle<Result<LimitedRead, ProviderProcessError>>,
) -> (LimitedRead, LimitedRead) {
    let stdout = match join_transport_task(stdout_task, "stdout").await {
        Ok(stdout) => stdout,
        Err(_) => LimitedRead::empty(),
    };
    let stderr = match join_transport_task(stderr_task, "stderr").await {
        Ok(stderr) => stderr,
        Err(_) => LimitedRead::empty(),
    };
    (stdout, stderr)
}

#[cfg(test)]
#[path = "../tests/unit/transport.rs"]
mod transport_tests;
