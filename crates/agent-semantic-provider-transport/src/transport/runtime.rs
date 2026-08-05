//! Tokio-backed process runner for ASP language providers.

use std::borrow::Cow;
use std::env;
use std::io::ErrorKind;
use std::process::{ExitStatus, Stdio};
use std::time::{Duration, Instant};

use bstr::BStr;
use bytes::Bytes;
use tokio::process::{Child, Command};
use tracing::{Instrument, debug, info_span, warn};

use crate::byte_text;
use crate::process_contract::{
    DEFAULT_PROVIDER_MEMORY_LIMIT_BYTES, ProviderProcessError, ProviderProcessFraming,
    ProviderProcessLimits, ProviderProcessReceipt, ProviderProcessSpec, StdinMode,
};

const EXECUTABLE_BUSY_SPAWN_RETRIES: usize = 5;
const EXECUTABLE_BUSY_SPAWN_RETRY_DELAY: Duration = Duration::from_millis(10);
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

pub(super) struct ProviderChild {
    pub(super) child: Child,
    pub(super) process_group_id: Option<i32>,
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

pub(super) fn provider_process_admission_slots_for(
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

pub(super) async fn spawn_provider_process(
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

use super::{
    collect_provider_output, kill_provider_process_group, provider_process_group_id,
    spawn_provider_io_tasks,
};
