// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use tokio::process::Child;
use tokio::task::JoinHandle;

use crate::capture::LimitedRead;
use crate::capture::ProviderOutputStream;
use crate::capture::capture_output_stream;
use crate::process_contract::OutputMode;
use crate::process_contract::ProviderProcessError;
use crate::process_contract::ProviderProcessFraming;
use crate::process_contract::ProviderProcessLimits;
use crate::process_contract::StdinMode;

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
