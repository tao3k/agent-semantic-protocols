use tokio::{process::Child, task::JoinHandle};

use crate::{
    capture::{LimitedRead, ProviderOutputStream, capture_output_stream},
    process_contract::{
        OutputMode, ProviderProcessError, ProviderProcessFraming, ProviderProcessLimits, StdinMode,
    },
};

use super::write_stdin;

pub(super) struct ProviderIoTasks {
    pub(super) stdin: JoinHandle<Result<(), ProviderProcessError>>,
    pub(super) stdout: JoinHandle<Result<LimitedRead, ProviderProcessError>>,
    pub(super) stderr: JoinHandle<Result<LimitedRead, ProviderProcessError>>,
}

pub(super) fn spawn_provider_io_tasks(
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
            limits.max_stdout_bytes(),
            ProviderOutputStream::Stdout,
            stdout_mode,
            framing.stdout,
        )),
        stderr: tokio::spawn(capture_output_stream(
            stderr,
            limits.max_stderr_bytes(),
            ProviderOutputStream::Stderr,
            stderr_mode,
            framing.stderr,
        )),
    })
}
