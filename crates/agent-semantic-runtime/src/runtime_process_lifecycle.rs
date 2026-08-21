//! Tokio-owned process lifecycle primitives.

use std::path::{Path, PathBuf};

pub struct RuntimeProcessLaunchSpec {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub current_dir: Option<PathBuf>,
    pub environment: Vec<(String, String)>,
    pub stderr: PathBuf,
}

pub struct RuntimeProcessLaunchReceipt { pub process_id: u32 }
pub fn current_process_id() -> u32 { std::process::id() }

pub async fn launch_detached(spec: RuntimeProcessLaunchSpec) -> Result<RuntimeProcessLaunchReceipt, String> {
    let stderr = tokio::fs::OpenOptions::new().create(true).append(true).open(&spec.stderr).await.map_err(|e| e.to_string())?;
    let mut command = tokio::process::Command::new(&spec.program);
    let stderr = stderr.into_std().await;
    command.args(&spec.args);
    if let Some(current_dir) = spec.current_dir { command.current_dir(current_dir); }
    for (key, value) in spec.environment { command.env(key, value); }
    command.stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::from(stderr));
    #[cfg(unix)]
    {
        command.process_group(0);
    }
    let child = command.spawn().map_err(|e| format!("spawn runtime process: {e}"))?;
    Ok(RuntimeProcessLaunchReceipt { process_id: child.id().ok_or_else(|| "spawned process has no pid".to_owned())? })
}

pub async fn canonicalize(path: impl Into<PathBuf>) -> std::io::Result<PathBuf> {
    tokio::fs::canonicalize(path.into()).await
}

pub async fn write(path: impl Into<PathBuf>, bytes: impl Into<Vec<u8>>) -> std::io::Result<()> {
    tokio::fs::write(path.into(), bytes.into()).await
}

pub async fn process_id_is_alive(process_id: u32) -> bool {
    tokio::task::spawn_blocking(move || {
        let Ok(process_id) = i32::try_from(process_id) else { return false; };
        // SAFETY: signal 0 only probes process existence/permission.
        (unsafe { libc::kill(process_id, 0) == 0 })
            || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
    }).await.unwrap_or(false)
}

pub async fn process_executable_matches(process_id: u32, expected: impl Into<PathBuf>) -> Result<bool, String> {
    let expected = expected.into();
    tokio::task::spawn_blocking(move || {
        let actual = std::fs::canonicalize(format!("/proc/{process_id}/exe")).map_err(|e| e.to_string())?;
        let expected = std::fs::canonicalize(Path::new(&expected)).map_err(|e| e.to_string())?;
        Ok(actual == expected)
    }).await.map_err(|e| e.to_string())?
}

pub async fn terminate(process_id: u32) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        let pid = i32::try_from(process_id).map_err(|e| e.to_string())?;
        // SAFETY: caller has already validated ownership; this is the bounded lifecycle signal.
        let result = unsafe { libc::kill(pid, libc::SIGTERM) };
        if result == 0 { Ok(()) } else { Err(std::io::Error::last_os_error().to_string()) }
    }).await.map_err(|e| e.to_string())?
}

pub async fn force_terminate(process_id: u32) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        let pid = i32::try_from(process_id).map_err(|e| e.to_string())?;
        // SAFETY: caller has validated the owned process identity.
        if unsafe { libc::kill(pid, libc::SIGKILL) } == 0 { Ok(()) } else { Err(std::io::Error::last_os_error().to_string()) }
    }).await.map_err(|e| e.to_string())?
}
