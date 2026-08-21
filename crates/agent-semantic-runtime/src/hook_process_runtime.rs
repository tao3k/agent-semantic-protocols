//! Tokio-owned process transport for bounded hook repair and replay.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Output, Stdio};
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// One bounded child-process request owned by the hook process runtime.
pub struct HookProcessRequest<'a> {
    /// Executable selected by the caller's artifact authority.
    pub executable: &'a Path,
    /// Exact argument vector forwarded to the executable.
    pub args: &'a [OsString],
    /// Bytes forwarded to the child's standard input.
    pub stdin: &'a [u8],
    /// Maximum duration before the child is killed on drop.
    pub timeout: Duration,
    /// Optional current directory for an installer process.
    pub current_dir: Option<&'a Path>,
    /// Optional environment binding used to fence recursive repair.
    pub environment: Option<(&'a str, &'a str)>,
    /// Discard stdout when the child is an installer rather than a hook replay.
    pub discard_stdout: bool,
}

/// Execute one child under a Tokio deadline, killing and reaping it on timeout.
pub async fn run_hook_process(request: HookProcessRequest<'_>) -> Result<Output, String> {
    let mut command = tokio::process::Command::new(request.executable);
    command
        .args(request.args)
        .stdin(Stdio::piped())
        .stdout(if request.discard_stdout {
            Stdio::null()
        } else {
            Stdio::piped()
        })
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    if let Some(current_dir) = request.current_dir {
        command.current_dir(current_dir);
    }
    if let Some((name, value)) = request.environment {
        command.env(name, value);
    }
    let mut child = command.spawn().map_err(|error| {
        format!(
            "start bounded hook process {}: {error}",
            request.executable.display()
        )
    })?;
    let stdout = child.stdout.take().map(drain_hook_process_pipe);
    let stderr = child.stderr.take().map(drain_hook_process_pipe);
    if let Some(mut child_stdin) = child.stdin.take() {
        if let Err(error) = child_stdin.write_all(request.stdin).await {
            kill_reap_and_drain_hook_process(&mut child, stdout, stderr).await?;
            return Err(format!("forward bounded hook process stdin: {error}"));
        }
    }
    let status = match tokio::time::timeout(request.timeout, child.wait()).await {
        Ok(status) => status.map_err(|error| format!("wait for bounded hook process: {error}"))?,
        Err(_) => {
            kill_reap_and_drain_hook_process(&mut child, stdout, stderr).await?;
            return Err(format!(
                "bounded hook process exceeded {} milliseconds and was killed/reaped: {}",
                request.timeout.as_millis(),
                request.executable.display()
            ));
        }
    };
    let (stdout, stderr) = collect_hook_process_pipes(stdout, stderr).await?;
    Ok(Output {
        status,
        stdout,
        stderr,
    })
}

async fn kill_reap_and_drain_hook_process(
    child: &mut tokio::process::Child,
    stdout: Option<tokio::task::JoinHandle<Result<Vec<u8>, String>>>,
    stderr: Option<tokio::task::JoinHandle<Result<Vec<u8>, String>>>,
) -> Result<(), String> {
    child
        .start_kill()
        .map_err(|error| format!("kill timed-out hook process: {error}"))?;
    child
        .wait()
        .await
        .map_err(|error| format!("reap timed-out hook process: {error}"))?;
    drain_hook_process_pipes(stdout, stderr).await;
    Ok(())
}

fn drain_hook_process_pipe<R>(mut pipe: R) -> tokio::task::JoinHandle<Result<Vec<u8>, String>>
where
    R: tokio::io::AsyncRead + Unpin + Send + 'static,
{
    tokio::spawn(async move {
        let mut bytes = Vec::new();
        pipe.read_to_end(&mut bytes)
            .await
            .map_err(|error| format!("drain bounded hook process output: {error}"))?;
        Ok(bytes)
    })
}

async fn drain_hook_process_pipes(
    stdout: Option<tokio::task::JoinHandle<Result<Vec<u8>, String>>>,
    stderr: Option<tokio::task::JoinHandle<Result<Vec<u8>, String>>>,
) -> (Vec<u8>, Vec<u8>) {
    async fn drain(pipe: Option<tokio::task::JoinHandle<Result<Vec<u8>, String>>>) -> Vec<u8> {
        let Some(pipe) = pipe else {
            return Vec::new();
        };
        pipe.await.ok().and_then(Result::ok).unwrap_or_default()
    }
    tokio::join!(drain(stdout), drain(stderr))
}

async fn collect_hook_process_pipes(
    stdout: Option<tokio::task::JoinHandle<Result<Vec<u8>, String>>>,
    stderr: Option<tokio::task::JoinHandle<Result<Vec<u8>, String>>>,
) -> Result<(Vec<u8>, Vec<u8>), String> {
    async fn collect(
        pipe: Option<tokio::task::JoinHandle<Result<Vec<u8>, String>>>,
    ) -> Result<Vec<u8>, String> {
        let Some(pipe) = pipe else {
            return Ok(Vec::new());
        };
        pipe.await
            .map_err(|error| format!("join bounded hook process output drain: {error}"))?
    }
    tokio::try_join!(collect(stdout), collect(stderr))
}

/// Construct the argument vector for the canonical development installer.
#[must_use]
pub fn hook_development_installer_args(
    root: &Path,
    justfile: &Path,
    runtime_bin: &Path,
) -> Vec<OsString> {
    vec![
        OsString::from("exec"),
        root.as_os_str().to_owned(),
        OsString::from("just"),
        OsString::from("--justfile"),
        justfile.as_os_str().to_owned(),
        OsString::from("agent-tools-install-protocol"),
        runtime_bin.as_os_str().to_owned(),
    ]
}

/// Canonical executable for development installer delegation.
#[must_use]
pub fn hook_development_installer_executable() -> PathBuf {
    PathBuf::from("direnv")
}
