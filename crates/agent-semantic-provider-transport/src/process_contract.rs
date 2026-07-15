//! Public process execution contract for provider transport.

use std::collections::BTreeMap;
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

/// Optional limits applied while running a provider process.
#[derive(Debug, Clone, Copy, Default, Eq, PartialEq)]
pub struct ProviderProcessLimits {
    /// Maximum wall-clock runtime before timeout.
    pub timeout: Option<Duration>,
    /// Maximum stdout bytes retained in memory.
    pub max_stdout_bytes: Option<usize>,
    /// Maximum stderr bytes retained in memory.
    pub max_stderr_bytes: Option<usize>,
    /// Maximum provider address-space bytes on supported platforms.
    pub memory_limit_bytes: Option<u64>,
}

/// Structured receipt for provider process execution.
#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub struct ProviderProcessReceipt {
    /// Elapsed wall-clock duration.
    pub elapsed: Duration,
    /// Provider exit status code when the process exited normally.
    pub status_code: Option<i32>,
    /// Whether the provider exit status was successful.
    pub status_success: bool,
    /// Full stdout byte count before truncation.
    pub stdout_bytes: usize,
    /// Full stderr byte count before truncation.
    pub stderr_bytes: usize,
    /// SHA-256 digest of full stdout bytes before truncation.
    pub stdout_sha256: Option<String>,
    /// SHA-256 digest of full stderr bytes before truncation.
    pub stderr_sha256: Option<String>,
    /// Whether stdout was truncated in the retained buffer.
    pub stdout_truncated: bool,
    /// Whether stderr was truncated in the retained buffer.
    pub stderr_truncated: bool,
    /// Whether the process exceeded its timeout.
    pub timed_out: bool,
    /// Whether the parent observed the provider above its memory ceiling.
    pub memory_limit_exceeded: bool,
    /// Unix signal that terminated the provider, when available.
    pub exit_signal: Option<i32>,
    /// Configured provider memory ceiling.
    pub memory_limit_bytes: Option<u64>,
    /// Whether the current platform applied the memory ceiling.
    pub memory_limit_enforced: bool,
    /// Whether the provider failed through timeout, signal, or non-zero exit.
    pub abnormal_termination: bool,
    /// Stable termination classification for client receipts and diagnostics.
    pub termination_reason: String,
}

/// Transport-level failure while running an external provider process.
#[derive(Debug)]
pub enum ProviderProcessError {
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
        receipt: ProviderProcessReceipt,
    },
    /// The provider exceeded its configured memory ceiling and was killed.
    MemoryLimit {
        /// Configured byte ceiling.
        limit_bytes: u64,
        /// Partial receipt built from retained output.
        receipt: ProviderProcessReceipt,
    },
}

impl fmt::Display for ProviderProcessError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
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
            | Self::Timeout { .. }
            | Self::MemoryLimit { .. } => None,
        }
    }
}
