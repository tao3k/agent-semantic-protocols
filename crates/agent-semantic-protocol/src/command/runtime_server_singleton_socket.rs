use std::fs;
use std::io;
use std::os::unix::fs::MetadataExt;
use std::os::unix::net::UnixDatagram;
use std::path::{Path, PathBuf};

const SINGLETON_SOCKET_NAME: &str = "runtime-server-singleton.sock";

pub(super) enum AcquireOutcome {
    Acquired(SingletonSocketGuard),
    ResidentExists,
    Stale,
}

pub(super) struct SingletonSocketGuard {
    path: PathBuf,
    inode: u64,
    _socket: UnixDatagram,
}

pub(super) fn acquire(state_home: &Path) -> Result<AcquireOutcome, String> {
    let server_root = state_home.join("runtime").join("server");
    fs::create_dir_all(&server_root).map_err(|error| {
        format!(
            "failed to create Runtime Server singleton directory {}: {error}",
            server_root.display()
        )
    })?;
    let path = server_root.join(SINGLETON_SOCKET_NAME);
    match UnixDatagram::bind(&path) {
        Ok(socket) => guard(path, socket).map(AcquireOutcome::Acquired),
        Err(error) if error.kind() == io::ErrorKind::AddrInUse => probe(&path),
        Err(error) => Err(format!(
            "failed to bind Runtime Server singleton socket {}: {error}",
            path.display()
        )),
    }
}

/// Recover a stale pathname only while the caller owns the Runtime Server election.
pub(super) fn acquire_with_stale_recovery_under_election(
    state_home: &Path,
) -> Result<AcquireOutcome, String> {
    match acquire(state_home)? {
        AcquireOutcome::Stale => {
            let path = state_home
                .join("runtime")
                .join("server")
                .join(SINGLETON_SOCKET_NAME);
            match fs::remove_file(&path) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => {
                    return Err(format!(
                        "failed to remove stale Runtime Server singleton socket {}: {error}",
                        path.display()
                    ));
                }
            }
            match UnixDatagram::bind(&path) {
                Ok(socket) => guard(path, socket).map(AcquireOutcome::Acquired),
                Err(error) if error.kind() == io::ErrorKind::AddrInUse => probe(&path),
                Err(error) => Err(format!(
                    "failed to bind Runtime Server singleton socket after stale recovery {}: {error}",
                    path.display()
                )),
            }
        }
        outcome => Ok(outcome),
    }
}

fn probe(path: &Path) -> Result<AcquireOutcome, String> {
    let probe = UnixDatagram::unbound().map_err(|error| {
        format!(
            "failed to create Runtime Server singleton probe for {}: {error}",
            path.display()
        )
    })?;
    match probe.connect(path) {
        Ok(()) => Ok(AcquireOutcome::ResidentExists),
        Err(error)
            if matches!(
                error.kind(),
                io::ErrorKind::ConnectionRefused | io::ErrorKind::NotFound
            ) => Ok(AcquireOutcome::Stale),
        Err(error) => Err(format!(
            "failed to probe Runtime Server singleton socket {}: {error}",
            path.display()
        )),
    }
}

fn guard(path: PathBuf, socket: UnixDatagram) -> Result<SingletonSocketGuard, String> {
    let inode = fs::metadata(&path)
        .map_err(|error| {
            format!(
                "failed to inspect Runtime Server singleton socket {}: {error}",
                path.display()
            )
        })?
        .ino();
    Ok(SingletonSocketGuard {
        path,
        inode,
        _socket: socket,
    })
}

impl Drop for SingletonSocketGuard {
    fn drop(&mut self) {
        let owns_current_path = fs::metadata(&self.path)
            .map(|metadata| metadata.ino() == self.inode)
            .unwrap_or(false);
        if owns_current_path {
            let _ = fs::remove_file(&self.path);
        }
    }
}
