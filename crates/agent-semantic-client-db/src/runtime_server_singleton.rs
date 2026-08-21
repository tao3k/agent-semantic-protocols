//! Runtime Server singleton ownership primitive.

use std::path::{Path, PathBuf};
use tokio::net::UnixDatagram;

pub enum RuntimeServerSingletonElection { Acquired(RuntimeServerSingletonGuard), ResidentExists }
pub struct RuntimeServerSingletonGuard { pub(crate) path: PathBuf, socket: Option<UnixDatagram> }

impl Drop for RuntimeServerSingletonGuard {
    fn drop(&mut self) {
        self.socket.take();
        let _ = std::fs::remove_file(&self.path);
    }
}

pub async fn acquire(state_home: &Path) -> Result<Option<RuntimeServerSingletonGuard>, String> {
    let path = state_home.join("runtime/server/asp-singleton.sock");
    if let Some(parent) = path.parent() { tokio::fs::create_dir_all(parent).await.map_err(|e| e.to_string())?; }
    match UnixDatagram::bind(&path) {
        Ok(socket) => Ok(Some(RuntimeServerSingletonGuard { path, socket: Some(socket) })),
        Err(error) if error.kind() == std::io::ErrorKind::AddrInUse => Ok(None),
        Err(error) => Err(error.to_string()),
    }
}

pub async fn acquire_election(state_home: &Path) -> Result<RuntimeServerSingletonElection, String> {
    Ok(match acquire(state_home).await? { Some(guard) => RuntimeServerSingletonElection::Acquired(guard), None => RuntimeServerSingletonElection::ResidentExists })
}

impl RuntimeServerSingletonGuard {
    pub async fn release(mut self) -> Result<(), String> { self.socket.take(); match tokio::fs::remove_file(&self.path).await { Ok(()) => Ok(()), Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()), Err(e) => Err(e.to_string()) } }
}
