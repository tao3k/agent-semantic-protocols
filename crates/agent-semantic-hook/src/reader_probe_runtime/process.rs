//! Bounded child-process lifecycle for permission-differential Reader probes.

use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::process::Stdio;
use std::time::Instant;

use super::filesystem::ensure_secure_directory;

pub(super) enum ProbeOutcome {
    Exited(std::process::ExitStatus),
    TimedOut,
    WaitFailed,
    CleanupFailed,
}

pub(super) fn replace_subject_operand(
    tokens: &[String],
    subject: &str,
    sentinel: &str,
) -> Vec<String> {
    tokens
        .iter()
        .skip(1)
        .map(|token| {
            if token == subject {
                sentinel.to_owned()
            } else {
                token.clone()
            }
        })
        .collect()
}

fn build_permission_probe_command(executable: &Path, argv: &[String]) -> Command {
    let mut command = Command::new(executable);
    command
        .args(argv)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    command
}

fn spawn_probe(mut command: Command) -> std::io::Result<std::process::Child> {
    use std::os::unix::process::CommandExt as _;
    command.process_group(0);
    command.spawn()
}

pub(super) fn materialize_probe_root() -> Result<PathBuf, String> {
    let root =
        std::env::temp_dir().join(format!("asp-reader-probe-{}", unsafe { libc::geteuid() }));
    ensure_secure_directory(&root)?;
    Ok(root)
}

pub(super) struct ProfileSentinels {
    pub(super) readable: PathBuf,
    pub(super) denied: PathBuf,
}

pub(super) fn materialize_profile_sentinels(
    root: &Path,
    subject: &str,
) -> Result<ProfileSentinels, String> {
    let extension = Path::new(subject)
        .extension()
        .and_then(|extension| extension.to_str())
        .filter(|extension| !extension.is_empty())
        .unwrap_or("source");
    let readable = root.join(format!("readable-sentinel.{extension}"));
    let denied = root.join(format!("denied-sentinel.{extension}"));
    materialize_profile_sentinel(&readable, 0o400)?;
    materialize_profile_sentinel(&denied, 0o000)?;
    Ok(ProfileSentinels { readable, denied })
}

fn materialize_profile_sentinel(path: &Path, mode: u32) -> Result<(), String> {
    use std::os::unix::fs::MetadataExt as _;
    use std::os::unix::fs::OpenOptionsExt as _;
    use std::os::unix::fs::PermissionsExt as _;
    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(mode)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(path)
    {
        Ok(file) => file.sync_all().map_err(|error| error.to_string())?,
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(error.to_string()),
    }
    let metadata = std::fs::symlink_metadata(path).map_err(|error| error.to_string())?;
    if !metadata.file_type().is_file()
        || metadata.file_type().is_symlink()
        || metadata.uid() != unsafe { libc::geteuid() }
        || metadata.len() != 0
    {
        return Err(format!(
            "Reader probe profile sentinel is not an empty regular file: {}",
            path.display()
        ));
    }
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))
        .map_err(|error| error.to_string())?;
    Ok(())
}

pub(super) fn run_permission_candidate(
    _current_dir: &Path,
    executable: &Path,
    argv: &[String],
    deadline: Instant,
) -> ProbeOutcome {
    let Ok(child) = spawn_probe(build_permission_probe_command(executable, argv)) else {
        return ProbeOutcome::WaitFailed;
    };
    wait_for_process_exit(child, deadline)
}

fn wait_for_process_exit(mut child: std::process::Child, deadline: Instant) -> ProbeOutcome {
    let child_id = child.id();
    let (terminal_tx, terminal_rx) = std::sync::mpsc::sync_channel(1);
    let waiter = std::thread::spawn(move || {
        let terminal = child.wait();
        let _ = terminal_tx.send(terminal);
    });
    match terminal_rx.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
        Ok(Ok(status)) => {
            let joined = waiter.join().is_ok();
            if joined && cleanup_process_group(child_id) {
                ProbeOutcome::Exited(status)
            } else {
                ProbeOutcome::CleanupFailed
            }
        }
        Ok(Err(_)) => {
            let _ = waiter.join();
            ProbeOutcome::WaitFailed
        }
        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
            let killed = terminate_process_tree(child_id);
            let reaped = terminal_rx.recv().is_ok();
            let joined = waiter.join().is_ok();
            if killed && reaped && joined && cleanup_process_group(child_id) {
                ProbeOutcome::TimedOut
            } else {
                ProbeOutcome::CleanupFailed
            }
        }
        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
            let _ = terminate_process_tree(child_id);
            let _ = waiter.join();
            ProbeOutcome::WaitFailed
        }
    }
}

fn terminate_process_tree(child_id: u32) -> bool {
    let Ok(pid) = i32::try_from(child_id) else {
        return false;
    };
    if unsafe { libc::kill(-pid, libc::SIGKILL) } == 0 {
        return true;
    }
    if std::io::Error::last_os_error().raw_os_error() != Some(libc::ESRCH) {
        return false;
    }
    (unsafe { libc::kill(pid, libc::SIGKILL) }) == 0
        || std::io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH)
}

fn cleanup_process_group(child_id: u32) -> bool {
    let Ok(pid) = i32::try_from(child_id) else {
        return false;
    };
    if unsafe { libc::kill(-pid, 0) } != 0 {
        return std::io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH);
    }
    (unsafe { libc::kill(-pid, libc::SIGKILL) }) == 0
}

pub(super) fn exit_signature(status: &std::process::ExitStatus) -> (Option<i32>, Option<i32>) {
    use std::os::unix::process::ExitStatusExt as _;
    (status.code(), status.signal())
}
