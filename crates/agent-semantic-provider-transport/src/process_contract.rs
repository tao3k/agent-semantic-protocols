//! Public process execution contract for provider transport.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::io;
use std::path::PathBuf;
use std::time::Duration;

use bytes::Bytes;
use tokio::task::JoinError;

/// Complete command specification for one external provider process run.
#[derive(Debug, Clone)]
pub struct ProviderProcessSpec {
    /// Executable path or command name.
    pub program: String,
    /// Arguments passed to the provider executable.
    pub args: Vec<String>,
    /// Working directory used for provider execution.
    pub cwd: PathBuf,
    /// Environment variables injected into the provider process.
    pub env: BTreeMap<String, String>,
    /// Inherited environment variables removed before spawning the process.
    pub remove_env: BTreeSet<String>,
    /// Inherited environment-variable prefixes removed before spawning.
    pub remove_env_prefixes: BTreeSet<String>,
    /// Provider stdin handling mode.
    pub stdin: StdinMode,
    /// Provider stdout handling mode.
    pub stdout: OutputMode,
    /// Provider stderr handling mode.
    pub stderr: OutputMode,
    /// Runtime limits for captured provider output.
    pub limits: ProviderProcessLimits,
}

/// Stdin policy for an external provider process.
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum StdinMode {
    /// Inherit stdin from the current process.
    Inherit,
    /// Close stdin for the provider process.
    Closed,
    /// Write the provided bytes to provider stdin, then close it.
    Bytes(Bytes),
}

impl StdinMode {
    /// Build byte-mode stdin from any `Bytes`-compatible buffer.
    pub fn bytes(bytes: impl Into<Bytes>) -> Self {
        Self::Bytes(bytes.into())
    }
}

/// Output policy for an external provider process stream.
#[derive(Debug, Clone, Copy, Default, Eq, PartialEq)]
pub enum OutputMode {
    /// Capture the stream as bytes without writing it to the parent stream.
    #[default]
    Capture,
    /// Capture the stream as bytes and tee it to the matching parent stream.
    Tee,
}

/// Framing policy for an external provider process stream.
#[derive(Debug, Clone, Copy, Default, Eq, PartialEq)]
pub enum OutputFraming {
    /// Capture the exact byte stream.
    #[default]
    Bytes,
    /// Capture UTF-8 line frames normalized with `\n`.
    Lines,
    /// Capture big-endian u32 length-delimited frame payloads.
    LengthDelimited,
}

/// Framing policy for stdout and stderr.
#[derive(Debug, Clone, Copy, Default, Eq, PartialEq)]
pub struct ProviderProcessFraming {
    /// Framing used for provider stdout.
    pub stdout: OutputFraming,
    /// Framing used for provider stderr.
    pub stderr: OutputFraming,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
struct ProviderStdoutByteLimit(usize);

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
struct ProviderStderrByteLimit(usize);

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
struct ProviderMemoryByteLimit(u64);

/// Default resident-memory ceiling for one ASP provider process group.
pub const DEFAULT_PROVIDER_MEMORY_LIMIT_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const PROVIDER_MEMORY_LIMIT_BYTES_PER_EXECUTION_SLOT: u64 = 1024 * 1024 * 1024;
pub const MAX_ADAPTIVE_PROVIDER_MEMORY_LIMIT_BYTES: u64 = 16 * 1024 * 1024 * 1024;

pub fn adaptive_provider_memory_limit_bytes() -> u64 {
    let execution_slots = std::thread::available_parallelism()
        .map(|parallelism| parallelism.get() as u64)
        .unwrap_or(1);
    execution_slots
        .saturating_mul(PROVIDER_MEMORY_LIMIT_BYTES_PER_EXECUTION_SLOT)
        .clamp(
            DEFAULT_PROVIDER_MEMORY_LIMIT_BYTES,
            MAX_ADAPTIVE_PROVIDER_MEMORY_LIMIT_BYTES,
        )
}

/// Optional limits applied while running a provider process.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct ProviderProcessLimits {
    /// Maximum wall-clock runtime before timeout.
    timeout: Option<Duration>,
    /// Maximum stdout bytes retained in memory.
    max_stdout_bytes: Option<ProviderStdoutByteLimit>,
    /// Maximum stderr bytes retained in memory.
    max_stderr_bytes: Option<ProviderStderrByteLimit>,
    /// Maximum provider address-space bytes on supported platforms.
    memory_limit_bytes: Option<ProviderMemoryByteLimit>,
}

impl ProviderProcessLimits {
    pub fn with_workspace_build_memory_budget(self) -> Self {
        let adaptive_limit = adaptive_provider_memory_limit_bytes();
        let selected_limit = self.memory_limit_bytes().unwrap_or(0).max(adaptive_limit);
        self.with_memory_limit_bytes(Some(selected_limit))
    }
}

impl Default for ProviderProcessLimits {
    fn default() -> Self {
        Self::new(
            None,
            None,
            None,
            Some(adaptive_provider_memory_limit_bytes()),
        )
    }
}

impl ProviderProcessLimits {
    #[must_use]
    pub fn new(
        timeout: Option<Duration>,
        max_stdout_bytes: Option<usize>,
        max_stderr_bytes: Option<usize>,
        memory_limit_bytes: Option<u64>,
    ) -> Self {
        Self {
            timeout,
            max_stdout_bytes: max_stdout_bytes.map(ProviderStdoutByteLimit),
            max_stderr_bytes: max_stderr_bytes.map(ProviderStderrByteLimit),
            memory_limit_bytes: memory_limit_bytes.map(ProviderMemoryByteLimit),
        }
    }

    #[must_use]
    pub const fn timeout(&self) -> Option<Duration> {
        self.timeout
    }

    #[must_use]
    pub const fn max_stdout_bytes(&self) -> Option<usize> {
        match self.max_stdout_bytes {
            Some(value) => Some(value.0),
            None => None,
        }
    }

    #[must_use]
    pub const fn max_stderr_bytes(&self) -> Option<usize> {
        match self.max_stderr_bytes {
            Some(value) => Some(value.0),
            None => None,
        }
    }

    #[must_use]
    pub const fn memory_limit_bytes(&self) -> Option<u64> {
        match self.memory_limit_bytes {
            Some(value) => Some(value.0),
            None => None,
        }
    }

    /// Return these limits with a replaced wall-clock timeout.
    #[must_use]
    pub fn with_timeout(mut self, timeout: Option<Duration>) -> Self {
        self.timeout = timeout;
        self
    }

    /// Return these limits with a replaced stdout retention ceiling.
    #[must_use]
    pub fn with_max_stdout_bytes(mut self, max_stdout_bytes: Option<usize>) -> Self {
        self.max_stdout_bytes = max_stdout_bytes.map(ProviderStdoutByteLimit);
        self
    }

    /// Return these limits with a replaced stderr retention ceiling.
    #[must_use]
    pub fn with_max_stderr_bytes(mut self, max_stderr_bytes: Option<usize>) -> Self {
        self.max_stderr_bytes = max_stderr_bytes.map(ProviderStderrByteLimit);
        self
    }

    /// Return these limits with a replaced provider memory ceiling.
    #[must_use]
    pub fn with_memory_limit_bytes(mut self, memory_limit_bytes: Option<u64>) -> Self {
        self.memory_limit_bytes = memory_limit_bytes.map(ProviderMemoryByteLimit);
        self
    }
}

/// Structured receipt for provider process execution.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ProviderProcessReceipt {
    /// Elapsed wall-clock duration.
    elapsed: Duration,
    /// Time spent waiting for a host-global provider execution slot.
    admission_wait: Duration,
    /// Provider exit status code when the process exited normally.
    status_code: Option<i32>,
    /// Whether the provider exit status was successful.
    status_success: bool,
    /// Full stdout byte count before truncation.
    stdout_bytes: usize,
    /// Full stderr byte count before truncation.
    stderr_bytes: usize,
    /// SHA-256 digest of full stdout bytes before truncation.
    stdout_sha256: Option<String>,
    /// SHA-256 digest of full stderr bytes before truncation.
    stderr_sha256: Option<String>,
    /// Whether stdout was truncated in the retained buffer.
    stdout_truncated: bool,
    /// Whether stderr was truncated in the retained buffer.
    stderr_truncated: bool,
    /// Whether the process exceeded its timeout.
    timed_out: bool,
    /// Whether the parent observed the provider above its memory ceiling.
    memory_limit_exceeded: bool,
    /// Unix signal that terminated the provider, when available.
    exit_signal: Option<i32>,
    /// Configured provider memory ceiling.
    memory_limit_bytes: Option<u64>,
    /// Whether the current platform applied the memory ceiling.
    memory_limit_enforced: bool,
    /// Whether ASP isolated and managed the provider as one process group.
    process_group_isolation_enforced: bool,
    /// Whether ASP terminated descendants left after the provider leader exited.
    descendant_cleanup_required: bool,
    /// Whether the provider failed through timeout, signal, or non-zero exit.
    abnormal_termination: bool,
    /// Stable termination classification for client receipts and diagnostics.
    termination_reason: String,
}

impl ProviderProcessReceipt {
    /// Return the provider exit status code when it exited normally.
    #[must_use]
    pub const fn status_code(&self) -> Option<i32> {
        self.status_code
    }

    /// Whether the provider exit status was successful.
    #[must_use]
    pub const fn status_success(&self) -> bool {
        self.status_success
    }

    #[must_use]
    pub const fn stdout_bytes(&self) -> usize {
        self.stdout_bytes
    }

    /// Return the full stderr byte count before truncation.
    #[must_use]
    pub const fn stderr_bytes(&self) -> usize {
        self.stderr_bytes
    }

    /// Return the SHA-256 digest of full stdout bytes.
    #[must_use]
    pub fn stdout_sha256(&self) -> Option<&str> {
        self.stdout_sha256.as_deref()
    }

    /// Return the SHA-256 digest of full stderr bytes.
    #[must_use]
    pub fn stderr_sha256(&self) -> Option<&str> {
        self.stderr_sha256.as_deref()
    }

    /// Whether the retained stdout buffer was truncated.
    #[must_use]
    pub const fn stdout_truncated(&self) -> bool {
        self.stdout_truncated
    }

    /// Whether the retained stderr buffer was truncated.
    #[must_use]
    pub const fn stderr_truncated(&self) -> bool {
        self.stderr_truncated
    }

    /// Whether the provider exceeded its timeout.
    #[must_use]
    pub const fn timed_out(&self) -> bool {
        self.timed_out
    }

    /// Return the terminating Unix signal, when available.
    #[must_use]
    pub const fn exit_signal(&self) -> Option<i32> {
        self.exit_signal
    }

    /// Return the configured provider memory ceiling.
    #[must_use]
    pub const fn memory_limit_bytes(&self) -> Option<u64> {
        self.memory_limit_bytes
    }

    /// Whether the current platform enforced the memory ceiling.
    #[must_use]
    pub const fn memory_limit_enforced(&self) -> bool {
        self.memory_limit_enforced
    }

    /// Whether the provider exceeded its memory ceiling.
    #[must_use]
    pub const fn memory_limit_exceeded(&self) -> bool {
        self.memory_limit_exceeded
    }

    /// Whether ASP isolated and managed the provider as one process group.
    #[must_use]
    pub const fn process_group_isolation_enforced(&self) -> bool {
        self.process_group_isolation_enforced
    }

    /// Whether descendants remained after the one-shot provider leader exited.
    #[must_use]
    pub const fn descendant_cleanup_required(&self) -> bool {
        self.descendant_cleanup_required
    }

    /// Whether the provider terminated abnormally.
    #[must_use]
    pub const fn abnormal_termination(&self) -> bool {
        self.abnormal_termination
    }

    /// Return the stable termination classification.
    #[must_use]
    pub fn termination_reason(&self) -> &str {
        &self.termination_reason
    }

    /// Return the elapsed wall-clock duration.
    #[must_use]
    pub const fn elapsed(&self) -> Duration {
        self.elapsed
    }

    /// Return the time spent waiting for provider-process admission.
    #[must_use]
    pub const fn admission_wait(&self) -> Duration {
        self.admission_wait
    }
}

pub(crate) struct ProviderProcessReceiptInput {
    pub(crate) elapsed: Duration,
    pub(crate) admission_wait: Duration,
    pub(crate) status_code: Option<i32>,
    pub(crate) status_success: bool,
    pub(crate) stdout_bytes: usize,
    pub(crate) stderr_bytes: usize,
    pub(crate) stdout_sha256: Option<String>,
    pub(crate) stderr_sha256: Option<String>,
    pub(crate) stdout_truncated: bool,
    pub(crate) stderr_truncated: bool,
    pub(crate) timed_out: bool,
    pub(crate) memory_limit_exceeded: bool,
    pub(crate) exit_signal: Option<i32>,
    pub(crate) memory_limit_bytes: Option<u64>,
    pub(crate) memory_limit_enforced: bool,
    pub(crate) process_group_isolation_enforced: bool,
    pub(crate) descendant_cleanup_required: bool,
    pub(crate) abnormal_termination: bool,
    pub(crate) termination_reason: String,
}

impl ProviderProcessReceipt {
    pub(crate) fn from_input(input: ProviderProcessReceiptInput) -> Self {
        Self {
            elapsed: input.elapsed,
            admission_wait: input.admission_wait,
            status_code: input.status_code,
            status_success: input.status_success,
            stdout_bytes: input.stdout_bytes,
            stderr_bytes: input.stderr_bytes,
            stdout_sha256: input.stdout_sha256,
            stderr_sha256: input.stderr_sha256,
            stdout_truncated: input.stdout_truncated,
            stderr_truncated: input.stderr_truncated,
            timed_out: input.timed_out,
            memory_limit_exceeded: input.memory_limit_exceeded,
            exit_signal: input.exit_signal,
            memory_limit_bytes: input.memory_limit_bytes,
            memory_limit_enforced: input.memory_limit_enforced,
            process_group_isolation_enforced: input.process_group_isolation_enforced,
            descendant_cleanup_required: input.descendant_cleanup_required,
            abnormal_termination: input.abnormal_termination,
            termination_reason: input.termination_reason,
        }
    }
}

/// Transport-level failure while running an external provider process.
#[derive(Debug)]
pub enum ProviderProcessError {
    /// The Tokio-owned provider supervisor was closed before this request was admitted.
    AdmissionClosed,
    /// The Tokio-owned provider supervisor cancelled this request during shutdown.
    Cancelled,
    /// The current-thread runtime used by the blocking adapter could not start.
    Runtime { source: io::Error },
    /// The provider process could not be spawned.
    Spawn { program: String, source: io::Error },
    /// Tokio did not expose a stdout pipe after spawn.
    CaptureStdout,
    /// Tokio did not expose a stderr pipe after spawn.
    CaptureStderr,
    /// Tokio did not expose a stdin pipe for byte-mode stdin.
    CaptureStdin,
    /// Writing the configured stdin payload failed.
    StdinWrite { source: io::Error },
    /// Closing provider stdin failed after writing the payload.
    StdinClose { source: io::Error },
    /// Reading provider stdout failed.
    StdoutRead { source: io::Error },
    /// Reading provider stderr failed.
    StderrRead { source: io::Error },
    /// Writing provider stdout to the parent stream failed in tee mode.
    StdoutTeeWrite { source: io::Error },
    /// Writing provider stderr to the parent stream failed in tee mode.
    StderrTeeWrite { source: io::Error },
    /// Waiting for the provider process failed.
    Wait { source: io::Error },
    /// A transport helper task failed before returning its result.
    Join {
        /// Helper task name.
        task: &'static str,
        /// Tokio join error.
        source: JoinError,
    },
    /// The provider exceeded its configured timeout and was killed.
    Timeout {
        /// Configured timeout.
        timeout: Duration,
        /// Partial receipt built from retained output.
        receipt: Box<ProviderProcessReceipt>,
    },
    /// The provider exceeded its configured memory ceiling and was killed.
    MemoryLimit {
        /// Configured byte ceiling.
        limit_bytes: u64,
        /// Partial receipt built from retained output.
        receipt: Box<ProviderProcessReceipt>,
    },
}

impl fmt::Display for ProviderProcessError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AdmissionClosed => write!(formatter, "provider process supervisor is closed"),
            Self::Cancelled => write!(formatter, "provider process execution was cancelled"),
            Self::Runtime { source } => {
                write!(
                    formatter,
                    "failed to create provider transport runtime: {source}"
                )
            }
            Self::Spawn { program, source } => {
                write!(
                    formatter,
                    "failed to spawn provider process `{program}`: {source}"
                )
            }
            Self::CaptureStdout => write!(formatter, "failed to capture provider stdout"),
            Self::CaptureStderr => write!(formatter, "failed to capture provider stderr"),
            Self::CaptureStdin => write!(formatter, "failed to open provider stdin"),
            Self::StdinWrite { source } => {
                write!(formatter, "failed to write provider stdin: {source}")
            }
            Self::StdinClose { source } => {
                write!(formatter, "failed to close provider stdin: {source}")
            }
            Self::StdoutRead { source } => {
                write!(formatter, "failed to read provider stdout: {source}")
            }
            Self::StderrRead { source } => {
                write!(formatter, "failed to read provider stderr: {source}")
            }
            Self::StdoutTeeWrite { source } => {
                write!(formatter, "failed to tee provider stdout: {source}")
            }
            Self::StderrTeeWrite { source } => {
                write!(formatter, "failed to tee provider stderr: {source}")
            }
            Self::Wait { source } => {
                write!(formatter, "failed to wait for provider process: {source}")
            }
            Self::Join { task, source } => {
                write!(formatter, "provider transport {task} task failed: {source}")
            }
            Self::Timeout { timeout, receipt } => write!(
                formatter,
                "provider process timed out after {timeout:?}; stdoutBytes={} stderrBytes={}",
                receipt.stdout_bytes, receipt.stderr_bytes
            ),
            Self::MemoryLimit {
                limit_bytes,
                receipt,
            } => write!(
                formatter,
                "provider process exceeded memory limit {limit_bytes} bytes; stdoutBytes={} stderrBytes={}",
                receipt.stdout_bytes, receipt.stderr_bytes
            ),
        }
    }
}

impl Error for ProviderProcessError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Runtime { source }
            | Self::Spawn { source, .. }
            | Self::StdinWrite { source }
            | Self::StdinClose { source }
            | Self::StdoutRead { source }
            | Self::StderrRead { source }
            | Self::StdoutTeeWrite { source }
            | Self::StderrTeeWrite { source }
            | Self::Wait { source } => Some(source),
            Self::Join { source, .. } => Some(source),
            Self::CaptureStdout
            | Self::CaptureStderr
            | Self::CaptureStdin
            | Self::AdmissionClosed
            | Self::Cancelled
            | Self::Timeout { .. }
            | Self::MemoryLimit { .. } => None,
        }
    }
}
