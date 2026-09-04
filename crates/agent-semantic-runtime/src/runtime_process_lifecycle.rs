//! Tokio-owned process lifecycle primitives.

use std::path::Path;
use std::path::PathBuf;

#[cfg(target_os = "macos")]
#[link(name = "proc")]
unsafe extern "C" {
    fn proc_pidpath(pid: libc::c_int, buffer: *mut libc::c_void, buffer_size: u32) -> libc::c_int;
}

pub struct RuntimeProcessLaunchSpec {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub current_dir: Option<PathBuf>,
    pub environment: Vec<(String, String)>,
    pub stderr: PathBuf,
}

pub struct RuntimeProcessLaunchReceipt {
    pub process_id: u32,
}
pub fn current_process_id() -> u32 {
    std::process::id()
}

pub async fn launch_detached(
    spec: RuntimeProcessLaunchSpec,
) -> Result<RuntimeProcessLaunchReceipt, String> {
    let handle = launch_monitored(spec).await?;
    let process_id = handle.process_id();
    drop(handle);
    Ok(RuntimeProcessLaunchReceipt { process_id })
}

pub async fn canonicalize(path: impl Into<PathBuf>) -> std::io::Result<PathBuf> {
    tokio::fs::canonicalize(path.into()).await
}

pub async fn write(path: impl Into<PathBuf>, bytes: impl Into<Vec<u8>>) -> std::io::Result<()> {
    tokio::fs::write(path.into(), bytes.into()).await
}

pub async fn process_id_is_alive(process_id: u32) -> bool {
    tokio::task::spawn_blocking(move || {
        let Ok(process_id) = i32::try_from(process_id) else {
            return false;
        };
        // SAFETY: signal 0 only probes process existence/permission.
        (unsafe { libc::kill(process_id, 0) == 0 })
            || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
    })
    .await
    .unwrap_or(false)
}

pub async fn process_executable_matches(
    process_id: u32,
    expected: impl Into<PathBuf>,
) -> Result<bool, String> {
    let expected = expected.into();
    tokio::task::spawn_blocking(move || {
        let actual = process_executable_path(process_id)?;
        let actual = std::fs::canonicalize(actual).map_err(|e| e.to_string())?;
        let expected = std::fs::canonicalize(Path::new(&expected)).map_err(|e| e.to_string())?;
        Ok(actual == expected)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(target_os = "linux")]
fn process_executable_path(process_id: u32) -> Result<PathBuf, String> {
    std::fs::read_link(format!("/proc/{process_id}/exe")).map_err(|error| error.to_string())
}

#[cfg(target_os = "macos")]
fn process_executable_path(process_id: u32) -> Result<PathBuf, String> {
    use std::ffi::CStr;
    use std::os::unix::ffi::OsStrExt;

    const PROC_PIDPATHINFO_MAXSIZE: usize = 4096;
    let mut buffer = vec![0_u8; PROC_PIDPATHINFO_MAXSIZE];
    let result = unsafe {
        proc_pidpath(
            process_id as libc::c_int,
            buffer.as_mut_ptr().cast(),
            buffer.len() as u32,
        )
    };
    if result <= 0 {
        return Err(std::io::Error::last_os_error().to_string());
    }
    let path = CStr::from_bytes_until_nul(&buffer).map_err(|error| error.to_string())?;
    Ok(PathBuf::from(std::ffi::OsStr::from_bytes(path.to_bytes())))
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn process_executable_path(_process_id: u32) -> Result<PathBuf, String> {
    Err("Runtime process executable identity is unsupported on this platform".to_owned())
}

pub async fn terminate(process_id: u32) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        let pid = i32::try_from(process_id).map_err(|e| e.to_string())?;
        // SAFETY: caller has already validated ownership; this is the bounded lifecycle signal.
        let result = unsafe { libc::kill(pid, libc::SIGTERM) };
        classify_kill_result(result, std::io::Error::last_os_error())
    })
    .await
    .map_err(|e| e.to_string())?
}

pub async fn force_terminate(process_id: u32) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        let pid = i32::try_from(process_id).map_err(|e| e.to_string())?;
        // SAFETY: caller has validated the owned process identity.
        let result = unsafe { libc::kill(pid, libc::SIGKILL) };
        classify_kill_result(result, std::io::Error::last_os_error())
    })
    .await
    .map_err(|e| e.to_string())?
}

fn classify_kill_result(result: i32, error: std::io::Error) -> Result<(), String> {
    if result == 0 || error.raw_os_error() == Some(libc::ESRCH) {
        Ok(())
    } else {
        Err(error.to_string())
    }
}

#[cfg(test)]
#[path = "../tests/unit/runtime_process_lifecycle_signal.rs"]
mod tests;
pub struct RuntimeProcessLaunchHandle {
    process_id: u32,
    child: tokio::process::Child,
}

impl RuntimeProcessLaunchHandle {
    pub fn process_id(&self) -> u32 {
        self.process_id
    }

    pub async fn wait(&mut self) -> Result<std::process::ExitStatus, String> {
        self.child
            .wait()
            .await
            .map_err(|error| format!("wait for Runtime process {}: {error}", self.process_id))
    }

    pub fn try_wait(&mut self) -> Result<Option<std::process::ExitStatus>, String> {
        self.child
            .try_wait()
            .map_err(|error| format!("inspect Runtime process {}: {error}", self.process_id))
    }
}

pub async fn launch_monitored(
    spec: RuntimeProcessLaunchSpec,
) -> Result<RuntimeProcessLaunchHandle, String> {
    let stderr = tokio::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&spec.stderr)
        .await
        .map_err(|error| {
            format!(
                "open Runtime process stderr {}: {error}",
                spec.stderr.display()
            )
        })?;
    let mut command = tokio::process::Command::new(&spec.program);
    let stderr = stderr.into_std().await;
    command.args(&spec.args);
    if let Some(current_dir) = spec.current_dir {
        command.current_dir(current_dir);
    }
    for (key, value) in spec.environment {
        command.env(key, value);
    }
    command
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::from(stderr));
    #[cfg(unix)]
    {
        command.process_group(0);
    }
    let child = command
        .spawn()
        .map_err(|error| format!("spawn Runtime process: {error}"))?;
    let process_id = child
        .id()
        .ok_or_else(|| "spawned Runtime process has no pid".to_owned())?;
    Ok(RuntimeProcessLaunchHandle { process_id, child })
}
