//! Bounded syscall-entry probe for otherwise unknown Bash readers.

use super::{ReaderProbeAccess, ReaderProbeObservation};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
#[cfg(target_os = "macos")]
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

// PreTool behavior observation has a bounded deadline and must not leave a
// process group or direct child behind.
const PROBE_TIMEOUT: Duration = Duration::from_millis(25);
#[cfg(target_os = "macos")]
static MATERIALIZATION_NONCE: AtomicU64 = AtomicU64::new(0);

/// Observe one parser-normalized command under the bounded Reader probe.
#[derive(Clone, Debug)]
pub struct ReaderProbeRequest {
    pub command_tokens: Vec<String>,
    pub subject: String,
}

pub fn observe(request: &ReaderProbeRequest) -> Option<ReaderProbeObservation> {
    Some(observe_one(
        request.command_tokens.as_slice(),
        request.subject.clone(),
    ))
}

fn observe_one(tokens: &[String], subject: String) -> ReaderProbeObservation {
    let started = Instant::now();
    let terminal =
        |access, terminal: &str, backend: &str, probe_process_launched| ReaderProbeObservation {
            subject: subject.clone(),
            access,
            backend: backend.to_owned(),
            terminal: terminal.to_owned(),
            elapsed_micros: u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX),
            probe_process_launched,
            cleanup_verified: terminal != "cleanup-failed",
        };

    #[cfg(not(target_os = "macos"))]
    {
        let _ = tokens;
        return terminal(
            ReaderProbeAccess::Unknown,
            "unsupported-platform",
            "unavailable",
            false,
        );
    }

    #[cfg(target_os = "macos")]
    {
        let Some(executable) = tokens.first() else {
            return terminal(
                ReaderProbeAccess::Unknown,
                "missing-executable",
                "dyld-open",
                false,
            );
        };
        if started.elapsed() >= PROBE_TIMEOUT {
            return terminal(
                ReaderProbeAccess::Unknown,
                "probe-timeout",
                "dyld-open",
                false,
            );
        }
        let Ok(executable) = which::which(executable) else {
            return terminal(
                ReaderProbeAccess::Unknown,
                "executable-unavailable",
                "dyld-open",
                false,
            );
        };
        if started.elapsed() >= PROBE_TIMEOUT {
            return terminal(
                ReaderProbeAccess::Unknown,
                "probe-timeout",
                "dyld-open",
                false,
            );
        }
        let Ok(probe_root) = materialize_probe_root() else {
            return terminal(
                ReaderProbeAccess::Unknown,
                "probe-root-failed",
                "dyld-open",
                false,
            );
        };
        if started.elapsed() >= PROBE_TIMEOUT {
            return terminal(
                ReaderProbeAccess::Unknown,
                "probe-timeout",
                "dyld-open",
                false,
            );
        }
        let Ok(interposer) = materialize_interposer(&probe_root) else {
            return terminal(
                ReaderProbeAccess::Unknown,
                "interposer-failed",
                "dyld-open",
                false,
            );
        };
        if started.elapsed() >= PROBE_TIMEOUT {
            return terminal(
                ReaderProbeAccess::Unknown,
                "probe-timeout",
                "dyld-open",
                false,
            );
        }
        let Ok(sentinel) = materialize_profile_sentinel(&probe_root, &subject) else {
            return terminal(
                ReaderProbeAccess::Unknown,
                "profile-sentinel-failed",
                "dyld-open",
                false,
            );
        };
        let sentinel_text = sentinel.to_string_lossy().into_owned();
        let argv = tokens
            .iter()
            .skip(1)
            .map(|token| {
                if token == &subject {
                    sentinel_text.clone()
                } else {
                    token.clone()
                }
            })
            .collect::<Vec<_>>();
        if !argv.iter().any(|token| token == &sentinel_text) {
            return terminal(
                ReaderProbeAccess::Unknown,
                "subject-not-replaced",
                "dyld-open",
                false,
            );
        }
        let Some((read_fd, write_fd)) = create_observation_pipe() else {
            return terminal(
                ReaderProbeAccess::Unknown,
                "pipe-failed",
                "dyld-open",
                false,
            );
        };
        if started.elapsed() >= PROBE_TIMEOUT {
            unsafe { libc::close(read_fd) };
            unsafe { libc::close(write_fd) };
            return terminal(
                ReaderProbeAccess::Unknown,
                "probe-timeout",
                "dyld-open",
                false,
            );
        }
        let child = spawn_probe(build_probe_command(
            &probe_root,
            &executable,
            &argv,
            &interposer,
            &sentinel_text,
            write_fd,
        ));
        let Ok(mut child) = child else {
            unsafe {
                libc::close(read_fd);
            }
            return terminal(
                ReaderProbeAccess::Unknown,
                "spawn-failed",
                "dyld-open",
                false,
            );
        };
        match wait_for_observation(read_fd, &mut child, started) {
            ProbeOutcome::Observed(flags) => terminal(
                super::classify_open_access_mode(flags),
                "open-entry-observed",
                "dyld-open",
                true,
            ),
            ProbeOutcome::Exited(code) => terminal(
                ReaderProbeAccess::Unknown,
                &format!("probe-exited-before-open:{}", code.map_or(-1, |code| code)),
                "dyld-open",
                true,
            ),
            ProbeOutcome::TimedOut => terminal(
                ReaderProbeAccess::Unknown,
                "probe-timeout",
                "dyld-open",
                true,
            ),
            ProbeOutcome::WaitFailed => terminal(
                ReaderProbeAccess::Unknown,
                "probe-wait-failed",
                "dyld-open",
                true,
            ),
            ProbeOutcome::CleanupFailed => terminal(
                ReaderProbeAccess::Unknown,
                "cleanup-failed",
                "dyld-open",
                true,
            ),
        }
    }
}

#[cfg(target_os = "macos")]
enum ProbeOutcome {
    Observed(i32),
    Exited(Option<i32>),
    TimedOut,
    WaitFailed,
    CleanupFailed,
}

#[cfg(target_os = "macos")]
fn build_probe_command(
    current_dir: &Path,
    executable: &Path,
    argv: &[String],
    interposer: &Path,
    sentinel: &str,
    write_fd: i32,
) -> Command {
    use std::os::fd::FromRawFd;

    let mut command = Command::new(executable);
    command
        .current_dir(current_dir)
        .args(argv)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("DYLD_INSERT_LIBRARIES", interposer)
        .env("ASP_READER_PROBE_TARGET", sentinel)
        .env("ASP_READER_PROBE_FD", "1")
        .stdin(Stdio::null())
        .stdout(unsafe { Stdio::from_raw_fd(write_fd) })
        .stderr(Stdio::null());
    command
}

#[cfg(target_os = "macos")]
fn spawn_probe(mut command: Command) -> std::io::Result<std::process::Child> {
    use std::os::unix::process::CommandExt;
    // Keep `Command` eligible for the platform's `posix_spawn` path. A
    // `pre_exec` closure forces a fork in a multithreaded Hook process and can
    // leave the child stalled before `exec`, which turns bounded concurrent
    // Reader observations into false `probe-timeout` terminals.
    command.process_group(0);
    command.spawn()
}

#[cfg(target_os = "macos")]
fn materialize_probe_root() -> Result<PathBuf, String> {
    let root =
        std::env::temp_dir().join(format!("asp-reader-probe-{}", unsafe { libc::geteuid() }));
    std::fs::create_dir_all(&root).map_err(|error| error.to_string())?;
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700))
        .map_err(|error| error.to_string())?;
    Ok(root)
}

#[cfg(target_os = "macos")]
fn materialize_interposer(root: &Path) -> Result<PathBuf, String> {
    use std::io::Write;

    let bytes = super::reader_probe_interposer_bytes();
    let expected_digest = blake3::hash(bytes);
    let digest = expected_digest.to_hex();
    let path = root.join(format!("interposer-{digest}.dylib"));
    if !path.try_exists().map_err(|error| error.to_string())? {
        let nonce = MATERIALIZATION_NONCE.fetch_add(1, Ordering::Relaxed);
        let candidate = root.join(format!(
            ".interposer-{}-{nonce}-{digest}",
            std::process::id()
        ));
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
            .map_err(|error| error.to_string())?;
        file.write_all(bytes).map_err(|error| error.to_string())?;
        file.sync_all().map_err(|error| error.to_string())?;
        drop(file);
        match std::fs::hard_link(&candidate, &path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => {
                let _ = std::fs::remove_file(&candidate);
                return Err(error.to_string());
            }
        }
        std::fs::remove_file(&candidate).map_err(|error| error.to_string())?;
    }
    let actual = std::fs::read(&path).map_err(|error| error.to_string())?;
    if blake3::hash(&actual) != expected_digest {
        return Err(format!(
            "Reader probe interposer digest mismatch at {}",
            path.display()
        ));
    }
    Ok(path)
}

#[cfg(target_os = "macos")]
fn materialize_profile_sentinel(root: &Path, subject: &str) -> Result<PathBuf, String> {
    let extension = Path::new(subject)
        .extension()
        .and_then(|extension| extension.to_str())
        .filter(|extension| !extension.is_empty())
        .unwrap_or("source");
    let path = root.join(format!("profile.{extension}"));
    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
    {
        Ok(file) => file.sync_all().map_err(|error| error.to_string())?,
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(error.to_string()),
    }
    let metadata = std::fs::symlink_metadata(&path).map_err(|error| error.to_string())?;
    if !metadata.file_type().is_file() || metadata.len() != 0 {
        return Err(format!(
            "Reader probe profile sentinel is not an empty regular file: {}",
            path.display()
        ));
    }
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o444))
        .map_err(|error| error.to_string())?;
    Ok(path)
}

#[cfg(target_os = "macos")]
fn create_observation_pipe() -> Option<(i32, i32)> {
    let mut fds = [-1; 2];
    if unsafe { libc::pipe(fds.as_mut_ptr()) } != 0 {
        return None;
    }
    unsafe {
        libc::fcntl(fds[0], libc::F_SETFL, libc::O_NONBLOCK);
    }
    Some((fds[0], fds[1]))
}

#[cfg(target_os = "macos")]
fn wait_for_observation(
    read_fd: i32,
    child: &mut std::process::Child,
    started: Instant,
) -> ProbeOutcome {
    let mut bytes = [0_u8; std::mem::size_of::<i32>()];
    let mut observed = 0_usize;
    let mut exited = None;
    let mut timed_out = false;
    loop {
        let read = unsafe {
            libc::read(
                read_fd,
                bytes[observed..].as_mut_ptr().cast(),
                bytes.len() - observed,
            )
        };
        if read > 0 {
            let Ok(read) = usize::try_from(read) else {
                unsafe { libc::close(read_fd) };
                return ProbeOutcome::WaitFailed;
            };
            observed += read;
            if observed == bytes.len() {
                break;
            }
        }
        match child.try_wait() {
            Ok(Some(status)) => {
                exited = Some(status.code());
                break;
            }
            Ok(None) => {}
            Err(_) => {
                unsafe { libc::close(read_fd) };
                return ProbeOutcome::WaitFailed;
            }
        }
        if started.elapsed() >= PROBE_TIMEOUT {
            timed_out = true;
            break;
        }
        let remaining = PROBE_TIMEOUT.saturating_sub(started.elapsed());
        if remaining.is_zero() {
            timed_out = true;
            break;
        }
        let timeout_millis = remaining.as_millis().clamp(1, i32::MAX as u128) as i32;
        let mut descriptor = libc::pollfd {
            fd: read_fd,
            events: libc::POLLIN | libc::POLLHUP | libc::POLLERR,
            revents: 0,
        };
        // SAFETY: descriptor refers to the owned observation pipe and remains valid
        // until the unified cleanup block closes it below.
        let poll_result = unsafe { libc::poll(&mut descriptor, 1, timeout_millis) };
        if poll_result < 0 {
            let error = std::io::Error::last_os_error();
            if error.kind() == std::io::ErrorKind::Interrupted {
                continue;
            }
            break;
        }
    }
    unsafe { libc::close(read_fd) };
    let mut cleanup_verified = true;
    if exited.is_none() && child.try_wait().ok().flatten().is_none() {
        if let Ok(pid) = i32::try_from(child.id()) {
            cleanup_verified &= unsafe { libc::kill(-pid, libc::SIGKILL) } == 0
                || std::io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH);
        }
        // `process_group(0)` is the descendant cleanup authority, while the
        // direct kill is a mandatory fallback if the platform did not publish
        // the process group before the deadline. Always reap the direct child.
        cleanup_verified &= child.kill().is_ok() || child.try_wait().ok().flatten().is_some();
        cleanup_verified &= child.wait().is_ok();
    }
    if let Ok(pid) = i32::try_from(child.id()) {
        cleanup_verified &= unsafe { libc::kill(-pid, 0) } != 0
            && std::io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH);
    }
    if !cleanup_verified {
        return ProbeOutcome::CleanupFailed;
    }
    if observed == bytes.len() {
        ProbeOutcome::Observed(i32::from_ne_bytes(bytes))
    } else if let Some(code) = exited {
        ProbeOutcome::Exited(code)
    } else if timed_out {
        ProbeOutcome::TimedOut
    } else {
        ProbeOutcome::WaitFailed
    }
}

#[cfg(test)]
#[path = "../tests/unit/reader_probe_runtime.rs"]
mod tests;
