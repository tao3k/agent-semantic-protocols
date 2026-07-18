//! Tokio-backed process runner for ASP language providers.

use std::borrow::Cow;
use std::env;
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

const EXECUTABLE_BUSY_SPAWN_RETRIES: usize = 5;
const EXECUTABLE_BUSY_SPAWN_RETRY_DELAY: Duration = Duration::from_millis(10);
const ASP_PROVIDER_TIMEOUT_MS_ENV: &str = "ASP_PROVIDER_TIMEOUT_MS";
const ASP_PROVIDER_MEMORY_LIMIT_BYTES_ENV: &str = "ASP_PROVIDER_MEMORY_LIMIT_BYTES";
const DEFAULT_PROVIDER_MEMORY_LIMIT_BYTES: u64 = 1024 * 1024 * 1024;

/// Resolve the optional facade timeout contract into provider process limits.
pub fn provider_process_limits_from_environment() -> Result<ProviderProcessLimits, String> {
    let timeout = env::var(ASP_PROVIDER_TIMEOUT_MS_ENV)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .map(|value| {
            value.parse::<u64>().map_err(|error| {
                format!(
                    "{ASP_PROVIDER_TIMEOUT_MS_ENV} must be an integer number of milliseconds: {error}"
                )
            })
        })
        .transpose()?
        .filter(|millis| *millis > 0)
        .map(Duration::from_millis);
    let memory_limit_bytes = match env::var(ASP_PROVIDER_MEMORY_LIMIT_BYTES_ENV) {
        Ok(value) if !value.trim().is_empty() => {
            let bytes = value.trim().parse::<u64>().map_err(|error| {
                format!(
                    "{ASP_PROVIDER_MEMORY_LIMIT_BYTES_ENV} must be an integer number of bytes: {error}"
                )
            })?;
            (bytes > 0).then_some(bytes)
        }
        _ => Some(DEFAULT_PROVIDER_MEMORY_LIMIT_BYTES),
    };
    Ok(ProviderProcessLimits {
        timeout,
        memory_limit_bytes,
        ..ProviderProcessLimits::default()
    })
}

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
        let mut child = spawn_provider_process(&spec, &stdin_mode).await?;
        debug!("spawned provider process");
        let io_tasks = spawn_provider_io_tasks(
            &mut child,
            stdin_mode,
            stdout_mode,
            stderr_mode,
            limits,
            framing,
        )?;
        collect_provider_output(child, io_tasks, limits, start).await
    }
    .instrument(span)
    .await
}

async fn spawn_provider_process(
    spec: &ProviderProcessSpec,
    stdin_mode: &StdinMode,
) -> Result<Child, ProviderProcessError> {
    for attempt in 0..=EXECUTABLE_BUSY_SPAWN_RETRIES {
        let mut command = provider_command(spec, stdin_mode);
        match command.spawn() {
            Ok(child) => return Ok(child),
            Err(source) => {
                if source.kind() == ErrorKind::ExecutableFileBusy
                    && attempt < EXECUTABLE_BUSY_SPAWN_RETRIES
                {
                    debug!(
                        attempt = attempt + 1,
                        program = %spec.program,
                        "provider executable was busy; retrying spawn"
                    );
                    tokio::time::sleep(EXECUTABLE_BUSY_SPAWN_RETRY_DELAY).await;
                    continue;
                }
                return Err(ProviderProcessError::Spawn {
                    program: spec.program.clone(),
                    source,
                });
            }
        }
    }
    unreachable!("bounded provider spawn retry loop must return")
}

fn provider_command(spec: &ProviderProcessSpec, stdin_mode: &StdinMode) -> Command {
    let mut command = Command::new(&spec.program);
    command
        .args(&spec.args)
        .current_dir(&spec.cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    configure_provider_process(&mut command, spec.limits.memory_limit_bytes);
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
}

#[cfg(unix)]
fn configure_provider_process(command: &mut Command, memory_limit_bytes: Option<u64>) {
    use std::os::unix::process::CommandExt;

    unsafe {
        #[cfg(target_os = "macos")]
        let _ = memory_limit_bytes;
        command.as_std_mut().pre_exec(move || {
            if libc::setpgid(0, 0) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            #[cfg(not(target_os = "macos"))]
            if let Some(memory_limit_bytes) = memory_limit_bytes {
                let mut limit = std::mem::MaybeUninit::<libc::rlimit>::uninit();
                if libc::getrlimit(provider_memory_resource(), limit.as_mut_ptr()) != 0 {
                    return Err(std::io::Error::last_os_error());
                }
                let mut limit = limit.assume_init();
                let requested = memory_limit_bytes as libc::rlim_t;
                limit.rlim_cur = if limit.rlim_max == libc::RLIM_INFINITY {
                    requested
                } else {
                    requested.min(limit.rlim_max)
                };
                if libc::setrlimit(provider_memory_resource(), &limit) != 0 {
                    return Err(std::io::Error::last_os_error());
                }
            }
            Ok(())
        });
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
const fn provider_memory_resource() -> libc::c_int {
    libc::RLIMIT_AS
}

#[cfg(not(unix))]
fn configure_provider_process(_command: &mut Command, _memory_limit_bytes: Option<u64>) {}

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
    limits: ProviderProcessLimits,
    start: Instant,
) -> Result<ProviderProcessOutput, ProviderProcessError> {
    let ProviderIoTasks {
        stdin: stdin_task,
        stdout: stdout_task,
        stderr: stderr_task,
    } = tasks;
    let child_pid = child.id();

    let status = if let Some(timeout) = limits.timeout {
        tokio::select! {
            biased;
            result = child.wait() => {
                result.map_err(|source| ProviderProcessError::Wait { source })?
            }
            _ = tokio::time::sleep(timeout) => {
                if let Some(status) = child
                    .try_wait()
                    .map_err(|source| ProviderProcessError::Wait { source })?
                {
                    status
                } else {
                warn!(
                    timeout_ms = timeout.as_millis(),
                    "provider process timed out; requesting kill"
                );
                terminate_provider_process(&mut child).await;
                let _ = join_transport_task(stdin_task, "stdin").await;
                let (stdout, stderr) =
                    join_readers_after_timeout(stdout_task, stderr_task).await;
                return Err(ProviderProcessError::Timeout {
                    timeout,
                    receipt: provider_process_receipt(start, None, stdout, stderr, true, false, limits),
                });
                }
            }
            _ = provider_memory_limit_exceeded(child_pid, limits.memory_limit_bytes) => {
                let limit_bytes = limits.memory_limit_bytes.expect("memory monitor requires limit");
                warn!(limit_bytes, "provider process exceeded memory limit; requesting kill");
                terminate_provider_process(&mut child).await;
                let _ = join_transport_task(stdin_task, "stdin").await;
                let (stdout, stderr) = join_readers_after_timeout(stdout_task, stderr_task).await;
                return Err(ProviderProcessError::MemoryLimit {
                    limit_bytes,
                    receipt: provider_process_receipt(start, None, stdout, stderr, false, true, limits),
                });
            }
        }
    } else {
        tokio::select! {
            biased;
            result = child.wait() => {
                result.map_err(|source| ProviderProcessError::Wait { source })?
            }
            _ = provider_memory_limit_exceeded(child_pid, limits.memory_limit_bytes) => {
                let limit_bytes = limits.memory_limit_bytes.expect("memory monitor requires limit");
                terminate_provider_process(&mut child).await;
                let _ = join_transport_task(stdin_task, "stdin").await;
                let (stdout, stderr) = join_readers_after_timeout(stdout_task, stderr_task).await;
                return Err(ProviderProcessError::MemoryLimit {
                    limit_bytes,
                    receipt: provider_process_receipt(start, None, stdout, stderr, false, true, limits),
                });
            }
        }
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
        start, status, stdout, stderr, false, false, limits,
    ))
}

async fn terminate_provider_process(child: &mut Child) {
    kill_provider_process_group(child);
    let _ = child.start_kill();
    let _ = child.wait().await;
}

#[cfg(unix)]
fn kill_provider_process_group(child: &Child) {
    let Some(pid) = child.id() else {
        return;
    };
    let Ok(process_group_id) = i32::try_from(pid) else {
        return;
    };
    unsafe {
        libc::kill(-process_group_id, libc::SIGKILL);
    }
}

#[cfg(not(unix))]
fn kill_provider_process_group(_child: &Child) {}

fn provider_process_output(
    start: Instant,
    status: ExitStatus,
    stdout: LimitedRead,
    stderr: LimitedRead,
    timed_out: bool,
    memory_limit_exceeded: bool,
    limits: ProviderProcessLimits,
) -> ProviderProcessOutput {
    let receipt = provider_process_receipt(
        start,
        Some(status),
        stdout.clone(),
        stderr.clone(),
        timed_out,
        memory_limit_exceeded,
        limits,
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
    memory_limit_exceeded: bool,
    limits: ProviderProcessLimits,
) -> ProviderProcessReceipt {
    let exit_signal = provider_exit_signal(status.as_ref());
    let status_success = status.is_some_and(|status| status.success());
    let memory_limit_enforced = cfg!(unix) && limits.memory_limit_bytes.is_some();
    let memory_limit_suspected =
        memory_limit_enforced && exit_signal.is_some_and(memory_limit_failure_signal);
    let termination_reason = if memory_limit_exceeded {
        "memory-limit-exceeded"
    } else if timed_out {
        "timeout"
    } else if memory_limit_suspected {
        "memory-limit-suspected"
    } else if exit_signal.is_some() {
        "signal"
    } else if status_success {
        "success"
    } else {
        "exit-code"
    };
    ProviderProcessReceipt {
        elapsed: start.elapsed(),
        status_code: status.and_then(|status| status.code()),
        status_success,
        stdout_bytes: stdout.total_bytes,
        stderr_bytes: stderr.total_bytes,
        stdout_sha256: stdout.sha256,
        stderr_sha256: stderr.sha256,
        stdout_truncated: stdout.truncated,
        stderr_truncated: stderr.truncated,
        timed_out,
        memory_limit_exceeded,
        exit_signal,
        memory_limit_bytes: limits.memory_limit_bytes,
        memory_limit_enforced,
        abnormal_termination: timed_out || memory_limit_exceeded || !status_success,
        termination_reason: termination_reason.to_string(),
    }
}

#[cfg(target_os = "macos")]
async fn provider_memory_limit_exceeded(pid: Option<u32>, limit: Option<u64>) {
    let (Some(pid), Some(limit)) = (pid, limit) else {
        std::future::pending::<()>().await;
        return;
    };
    loop {
        tokio::time::sleep(Duration::from_millis(25)).await;
        if macos_resident_bytes(pid).is_some_and(|resident| resident > limit) {
            return;
        }
    }
}

#[cfg(not(target_os = "macos"))]
async fn provider_memory_limit_exceeded(_pid: Option<u32>, _limit: Option<u64>) {
    std::future::pending::<()>().await;
}

#[cfg(target_os = "macos")]
fn macos_resident_bytes(pid: u32) -> Option<u64> {
    #[repr(C)]
    #[derive(Default)]
    struct ProcTaskInfo {
        virtual_size: u64,
        resident_size: u64,
        total_user: u64,
        total_system: u64,
        threads_user: u64,
        threads_system: u64,
        policy: i32,
        faults: i32,
        pageins: i32,
        cow_faults: i32,
        messages_sent: i32,
        messages_received: i32,
        syscalls_mach: i32,
        syscalls_unix: i32,
        csw: i32,
        threadnum: i32,
        numrunning: i32,
        priority: i32,
    }
    unsafe extern "C" {
        fn proc_pidinfo(
            pid: i32,
            flavor: i32,
            arg: u64,
            buffer: *mut libc::c_void,
            buffersize: i32,
        ) -> i32;
    }
    const PROC_PIDTASKINFO: i32 = 4;
    let mut info = ProcTaskInfo::default();
    let size = std::mem::size_of::<ProcTaskInfo>() as i32;
    let read = unsafe {
        proc_pidinfo(
            pid as i32,
            PROC_PIDTASKINFO,
            0,
            (&mut info as *mut ProcTaskInfo).cast(),
            size,
        )
    };
    (read == size).then_some(info.resident_size)
}

#[cfg(unix)]
fn provider_exit_signal(status: Option<&ExitStatus>) -> Option<i32> {
    use std::os::unix::process::ExitStatusExt;
    status.and_then(ExitStatusExt::signal)
}

#[cfg(not(unix))]
fn provider_exit_signal(_status: Option<&ExitStatus>) -> Option<i32> {
    None
}

fn memory_limit_failure_signal(signal: i32) -> bool {
    matches!(
        signal,
        libc::SIGKILL | libc::SIGSEGV | libc::SIGABRT | libc::SIGBUS
    )
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
#[path = "../tests/unit/transport/mod.rs"]
mod transport_tests;
