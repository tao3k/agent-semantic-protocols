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
use crate::capture::LimitedRead;
use crate::process_contract::{
    DEFAULT_PROVIDER_MEMORY_LIMIT_BYTES, ProviderProcessError, ProviderProcessFraming,
    ProviderProcessLimits, ProviderProcessReceipt, ProviderProcessSpec, StdinMode,
};

const EXECUTABLE_BUSY_SPAWN_RETRIES: usize = 5;
const EXECUTABLE_BUSY_SPAWN_RETRY_DELAY: Duration = Duration::from_millis(10);
const PROVIDER_MEMORY_OBSERVATION_GRACE: Duration = Duration::from_millis(250);
const PROVIDER_MEMORY_OBSERVATION_POLL_INTERVAL: Duration = Duration::from_millis(50);
const ASP_PROVIDER_TIMEOUT_MS_ENV: &str = "ASP_PROVIDER_TIMEOUT_MS";
const ASP_PROVIDER_MEMORY_LIMIT_BYTES_ENV: &str = "ASP_PROVIDER_MEMORY_LIMIT_BYTES";

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
            if bytes == 0 {
                return Err(format!(
                    "{ASP_PROVIDER_MEMORY_LIMIT_BYTES_ENV} must be greater than zero"
                ));
            }
            Some(bytes)
        }
        _ => Some(DEFAULT_PROVIDER_MEMORY_LIMIT_BYTES),
    };
    Ok(ProviderProcessLimits::new(
        timeout,
        None,
        None,
        memory_limit_bytes,
    ))
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

const PROVIDER_PROCESS_ADMISSION_POLL_INTERVAL: Duration = Duration::from_millis(2);

struct ProviderProcessAdmissionPermit {
    _slot: std::fs::File,
}

struct ProviderChild {
    child: Child,
    process_group_id: Option<i32>,
}

impl std::ops::Deref for ProviderChild {
    type Target = Child;

    fn deref(&self) -> &Self::Target {
        &self.child
    }
}

impl std::ops::DerefMut for ProviderChild {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.child
    }
}

impl Drop for ProviderChild {
    fn drop(&mut self) {
        kill_provider_process_group(self.process_group_id);
        let _ = self.child.start_kill();
    }
}

async fn acquire_provider_process_admission(
    limits: ProviderProcessLimits,
) -> Option<ProviderProcessAdmissionPermit> {
    let admission_root = provider_process_admission_root();
    if let Err(error) = std::fs::create_dir_all(&admission_root) {
        warn!(
            path = %admission_root.display(),
            %error,
            "failed to create provider process admission directory; continuing without admission"
        );
        return None;
    }

    loop {
        for slot in 0..provider_process_admission_slots(limits) {
            let slot_path = admission_root.join(format!("slot-{slot}.lock"));
            let file = match std::fs::OpenOptions::new()
                .create(true)
                .read(true)
                .write(true)
                .truncate(false)
                .open(&slot_path)
            {
                Ok(file) => file,
                Err(error) => {
                    warn!(
                        path = %slot_path.display(),
                        %error,
                        "failed to open provider process admission slot; continuing without admission"
                    );
                    return None;
                }
            };
            match fs2::FileExt::try_lock_exclusive(&file) {
                Ok(()) => return Some(ProviderProcessAdmissionPermit { _slot: file }),
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(error) => {
                    warn!(
                        path = %slot_path.display(),
                        %error,
                        "failed to lock provider process admission slot; continuing without admission"
                    );
                    return None;
                }
            }
        }
        tokio::time::sleep(PROVIDER_PROCESS_ADMISSION_POLL_INTERVAL).await;
    }
}

fn provider_process_admission_slots(limits: ProviderProcessLimits) -> usize {
    let cpu_slots = std::thread::available_parallelism()
        .map(std::num::NonZeroUsize::get)
        .unwrap_or(1)
        .saturating_sub(1)
        .max(1);
    provider_process_admission_slots_for(
        cpu_slots,
        provider_memory_budget_bytes(),
        limits.memory_limit_bytes(),
    )
}

fn provider_process_admission_slots_for(
    cpu_slots: usize,
    memory_budget_bytes: Option<u64>,
    memory_limit_bytes: Option<u64>,
) -> usize {
    let memory_slots = memory_limit_bytes
        .filter(|limit| *limit > 0)
        .and_then(|limit| {
            memory_budget_bytes
                .and_then(|bytes| usize::try_from(bytes / limit).ok())
                .map(|slots| slots.max(1))
        })
        .unwrap_or(cpu_slots.max(1));
    cpu_slots.max(1).min(memory_slots).max(1)
}

#[cfg(unix)]
fn provider_memory_budget_bytes() -> Option<u64> {
    let pages = unsafe { libc::sysconf(libc::_SC_PHYS_PAGES) };
    let page_size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
    (pages > 0 && page_size > 0)
        .then(|| {
            u64::try_from(pages)
                .ok()?
                .checked_mul(u64::try_from(page_size).ok()?)?
                .checked_div(2)
        })
        .flatten()
}

#[cfg(not(unix))]
fn provider_memory_budget_bytes() -> Option<u64> {
    None
}

fn provider_process_admission_root() -> std::path::PathBuf {
    let mut root = std::env::temp_dir();
    #[cfg(unix)]
    root.push(format!(
        "agent-semantic-provider-admission-v1-{}",
        // SAFETY: geteuid has no preconditions and does not dereference memory.
        unsafe { libc::geteuid() }
    ));
    #[cfg(not(unix))]
    root.push("agent-semantic-provider-admission-v1");
    root
}

/// Run a provider process asynchronously with explicit stdout/stderr framing.
pub async fn run_provider_process_async_with_framing(
    spec: ProviderProcessSpec,
    framing: ProviderProcessFraming,
) -> Result<ProviderProcessOutput, ProviderProcessError> {
    let timeout_ms = spec.limits.timeout().map(|timeout| timeout.as_millis());
    let span = info_span!(
        "provider_process",
        program = %spec.program,
        cwd = %spec.cwd.display(),
        args = spec.args.len(),
        timeout_ms = ?timeout_ms,
    );

    async move {
        let admission_started = Instant::now();
        let _admission_permit = acquire_provider_process_admission(spec.limits).await;
        let admission_wait = admission_started.elapsed();
        let admission_wait_ms = admission_wait.as_millis();
        debug!(admission_wait_ms, "admitted provider process");
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
        collect_provider_output(child, io_tasks, limits, start, admission_wait).await
    }
    .instrument(span)
    .await
}

async fn spawn_provider_process(
    spec: &ProviderProcessSpec,
    stdin_mode: &StdinMode,
) -> Result<ProviderChild, ProviderProcessError> {
    for attempt in 0..=EXECUTABLE_BUSY_SPAWN_RETRIES {
        let mut command = provider_command(spec, stdin_mode);
        match command.spawn() {
            Ok(child) => {
                let process_group_id = provider_process_group_id(&child);
                return Ok(ProviderChild {
                    child,
                    process_group_id,
                });
            }
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
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    configure_provider_process(&mut command, spec.limits.memory_limit_bytes());
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
                if libc::getrlimit(libc::RLIMIT_AS, limit.as_mut_ptr()) != 0 {
                    return Err(std::io::Error::last_os_error());
                }
                let mut limit = limit.assume_init();
                let requested = memory_limit_bytes as libc::rlim_t;
                limit.rlim_cur = if limit.rlim_max == libc::RLIM_INFINITY {
                    requested
                } else {
                    requested.min(limit.rlim_max)
                };
                if libc::setrlimit(libc::RLIMIT_AS, &limit) != 0 {
                    return Err(std::io::Error::last_os_error());
                }
            }
            Ok(())
        });
    }
}

#[cfg(not(unix))]
fn configure_provider_process(_command: &mut Command, _memory_limit_bytes: Option<u64>) {}

#[path = "transport_io.rs"]
mod io_tasks;
use io_tasks::{ProviderIoTasks, spawn_provider_io_tasks};

async fn collect_provider_output(
    mut child: ProviderChild,
    tasks: ProviderIoTasks,
    limits: ProviderProcessLimits,
    start: Instant,
    admission_wait: Duration,
) -> Result<ProviderProcessOutput, ProviderProcessError> {
    let ProviderIoTasks {
        stdin: stdin_task,
        stdout: stdout_task,
        stderr: stderr_task,
    } = tasks;
    let child_pid = child.id();
    debug!(
        child_pid,
        memory_limit_bytes = limits.memory_limit_bytes(),
        "provider memory gate armed"
    );

    let status = if let Some(timeout) = limits.timeout() {
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
                    receipt: Box::new(provider_process_receipt(
                        start, admission_wait, None, stdout, stderr, true, false, false, limits,
                    )),
                });
                }
            }
            observation = provider_memory_limit_exceeded(child_pid, limits.memory_limit_bytes()) => {
                let limit_bytes = limits.memory_limit_bytes().expect("memory monitor requires limit");
                warn!(
                    limit_bytes,
                    observed_resident_bytes = observation.resident_bytes,
                    observed_process_count = observation.process_count,
                    termination_action = "kill-process-group",
                    "provider process exceeded memory limit; requesting kill"
                );
                terminate_provider_process(&mut child).await;
                let _ = join_transport_task(stdin_task, "stdin").await;
                let (stdout, stderr) = join_readers_after_timeout(stdout_task, stderr_task).await;
                return Err(ProviderProcessError::MemoryLimit {
                    limit_bytes,
                    receipt: Box::new(provider_process_receipt(
                        start, admission_wait, None, stdout, stderr, false, true, false, limits,
                    )),
                });
            }
        }
    } else {
        tokio::select! {
            biased;
            result = child.wait() => {
                result.map_err(|source| ProviderProcessError::Wait { source })?
            }
            observation = provider_memory_limit_exceeded(child_pid, limits.memory_limit_bytes()) => {
                let limit_bytes = limits.memory_limit_bytes().expect("memory monitor requires limit");
                warn!(
                    limit_bytes,
                    observed_resident_bytes = observation.resident_bytes,
                    observed_process_count = observation.process_count,
                    termination_action = "kill-process-group",
                    "provider process exceeded memory limit; requesting kill"
                );
                terminate_provider_process(&mut child).await;
                let _ = join_transport_task(stdin_task, "stdin").await;
                let (stdout, stderr) = join_readers_after_timeout(stdout_task, stderr_task).await;
                return Err(ProviderProcessError::MemoryLimit {
                    limit_bytes,
                    receipt: Box::new(provider_process_receipt(
                        start, admission_wait, None, stdout, stderr, false, true, false, limits,
                    )),
                });
            }
        }
    };

    // Provider invocations are not resident. Once the leader exits, any
    // remaining descendant belongs to this invocation and must be terminated
    // before inherited output pipes can keep collection alive indefinitely.
    let descendant_cleanup_required = kill_provider_process_group(child.process_group_id);
    if descendant_cleanup_required {
        warn!(
            process_group_id = child.process_group_id,
            termination_action = "kill-process-group",
            descendant_cleanup_required = true,
            "one-shot provider leader exited with live descendants; terminated orphan process group"
        );
    }

    debug!(
        status = ?status.code(),
        elapsed_ms = start.elapsed().as_millis(),
        "provider process exited"
    );
    join_transport_task(stdin_task, "stdin").await?;
    let stdout = join_transport_task(stdout_task, "stdout").await?;
    let stderr = join_transport_task(stderr_task, "stderr").await?;
    Ok(provider_process_output(
        start,
        admission_wait,
        status,
        stdout,
        stderr,
        false,
        false,
        descendant_cleanup_required,
        limits,
    ))
}

async fn terminate_provider_process(child: &mut ProviderChild) {
    kill_provider_process_group(child.process_group_id);
    let _ = child.start_kill();
    let _ = child.wait().await;
}

#[cfg(unix)]
fn provider_process_group_id(child: &Child) -> Option<i32> {
    child.id().and_then(|pid| i32::try_from(pid).ok())
}

#[cfg(not(unix))]
fn provider_process_group_id(_child: &Child) -> Option<i32> {
    None
}

#[cfg(unix)]
fn kill_provider_process_group(process_group_id: Option<i32>) -> bool {
    let Some(process_group_id) = process_group_id else {
        return false;
    };
    let group_exists = unsafe { libc::kill(-process_group_id, 0) == 0 };
    if group_exists {
        unsafe {
            libc::kill(-process_group_id, libc::SIGKILL);
        }
    }
    group_exists
}

#[cfg(not(unix))]
fn kill_provider_process_group(_process_group_id: Option<i32>) -> bool {
    false
}

fn provider_process_output(
    start: Instant,
    admission_wait: Duration,
    status: ExitStatus,
    stdout: LimitedRead,
    stderr: LimitedRead,
    timed_out: bool,
    memory_limit_exceeded: bool,
    descendant_cleanup_required: bool,
    limits: ProviderProcessLimits,
) -> ProviderProcessOutput {
    let receipt = provider_process_receipt(
        start,
        admission_wait,
        Some(status),
        stdout.clone(),
        stderr.clone(),
        timed_out,
        memory_limit_exceeded,
        descendant_cleanup_required,
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
    admission_wait: Duration,
    status: Option<ExitStatus>,
    stdout: LimitedRead,
    stderr: LimitedRead,
    timed_out: bool,
    memory_limit_exceeded: bool,
    descendant_cleanup_required: bool,
    limits: ProviderProcessLimits,
) -> ProviderProcessReceipt {
    let exit_signal = provider_exit_signal(status.as_ref());
    let status_success = status.is_some_and(|status| status.success());
    let memory_limit_enforced = cfg!(unix) && limits.memory_limit_bytes().is_some();
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
    ProviderProcessReceipt::from_input(crate::process_contract::ProviderProcessReceiptInput {
        elapsed: start.elapsed(),
        admission_wait,
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
        memory_limit_bytes: limits.memory_limit_bytes(),
        memory_limit_enforced,
        process_group_isolation_enforced: cfg!(unix),
        descendant_cleanup_required,
        abnormal_termination: timed_out || memory_limit_exceeded || !status_success,
        termination_reason: termination_reason.to_string(),
    })
}

#[derive(Clone, Copy, Debug)]
struct ProviderMemoryObservation {
    resident_bytes: u64,
    process_count: usize,
}

#[cfg(target_os = "macos")]
async fn provider_memory_limit_exceeded(
    pid: Option<u32>,
    limit: Option<u64>,
) -> ProviderMemoryObservation {
    let (Some(pid), Some(limit)) = (pid, limit) else {
        return std::future::pending::<ProviderMemoryObservation>().await;
    };
    tokio::time::sleep(PROVIDER_MEMORY_OBSERVATION_GRACE).await;
    loop {
        if let Some(observation) = macos_process_group_memory(pid)
            && observation.resident_bytes > limit
        {
            return observation;
        }
        tokio::time::sleep(PROVIDER_MEMORY_OBSERVATION_POLL_INTERVAL).await;
    }
}

#[cfg(not(target_os = "macos"))]
async fn provider_memory_limit_exceeded(
    _pid: Option<u32>,
    _limit: Option<u64>,
) -> ProviderMemoryObservation {
    std::future::pending::<ProviderMemoryObservation>().await
}

#[cfg(target_os = "macos")]
fn macos_process_group_memory(process_group_id: u32) -> Option<ProviderMemoryObservation> {
    unsafe extern "C" {
        fn proc_listallpids(buffer: *mut libc::c_void, buffersize: i32) -> i32;
    }
    let pid_count = unsafe { proc_listallpids(std::ptr::null_mut(), 0) };
    let capacity = usize::try_from(pid_count).ok()?.saturating_add(64);
    let mut pids = vec![0_i32; capacity];
    let buffer_bytes = i32::try_from(pids.len().checked_mul(std::mem::size_of::<i32>())?).ok()?;
    let listed = unsafe { proc_listallpids(pids.as_mut_ptr().cast(), buffer_bytes) };
    let listed = usize::try_from(listed).ok()?.min(pids.len());
    let process_group_id = i32::try_from(process_group_id).ok()?;
    let mut resident_bytes = 0_u64;
    let mut process_count = 0_usize;
    for pid in pids.into_iter().take(listed).filter(|pid| *pid > 0) {
        if unsafe { libc::getpgid(pid) } != process_group_id {
            continue;
        }
        let Some(process_resident_bytes) = macos_resident_bytes(pid as u32) else {
            continue;
        };
        resident_bytes = resident_bytes.saturating_add(process_resident_bytes);
        process_count += 1;
    }
    (process_count > 0).then_some(ProviderMemoryObservation {
        resident_bytes,
        process_count,
    })
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

#[cfg(unix)]
fn memory_limit_failure_signal(signal: i32) -> bool {
    matches!(
        signal,
        libc::SIGKILL | libc::SIGSEGV | libc::SIGABRT | libc::SIGBUS
    )
}

#[cfg(not(unix))]
const fn memory_limit_failure_signal(_: i32) -> bool {
    false
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
