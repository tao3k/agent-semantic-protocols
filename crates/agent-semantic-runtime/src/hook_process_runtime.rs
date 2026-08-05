//! Tokio-owned process transport for bounded hook repair and replay.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Output, Stdio};
use std::time::Duration;

use tokio::io::AsyncWriteExt;

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

/// Execute one child under a Tokio deadline with cancellation-safe ownership.
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
    if let Some(mut child_stdin) = child.stdin.take() {
        child_stdin
            .write_all(request.stdin)
            .await
            .map_err(|error| format!("forward bounded hook process stdin: {error}"))?;
    }
    tokio::time::timeout(request.timeout, child.wait_with_output())
        .await
        .map_err(|_| {
            format!(
                "bounded hook process exceeded {} milliseconds: {}",
                request.timeout.as_millis(),
                request.executable.display()
            )
        })?
        .map_err(|error| format!("wait for bounded hook process: {error}"))
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
