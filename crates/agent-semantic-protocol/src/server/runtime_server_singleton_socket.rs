use std::env;
use std::io;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use tokio::net::UnixDatagram;
use tokio::runtime::Handle;
use tokio::time::timeout;

const SINGLETON_IO_DEADLINE: Duration = Duration::from_millis(500);

pub(super) enum SingletonSocketElection {
    Acquired(SingletonSocketGuard),
    ResidentExists,
}

pub(super) struct SingletonSocketGuard {
    path: PathBuf,
    inode: u64,
    socket: Option<UnixDatagram>,
    released: bool,
}

pub(super) async fn resident_exists(state_home: &Path) -> Result<bool, String> {
    let path = singleton_path(state_home)?;
    match timeout(SINGLETON_IO_DEADLINE, tokio::fs::metadata(&path)).await {
        Ok(Ok(_)) => probe_live(&path).await,
        Ok(Err(error)) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Ok(Err(error)) => Err(format!(
            "failed to inspect Runtime Server singleton socket {}: {error}",
            path.display()
        )),
        Err(_) => Err(format!(
            "timed out inspecting Runtime Server singleton socket {}; refusing stale recovery",
            path.display()
        )),
    }
}

pub(super) async fn acquire(state_home: &Path) -> Result<SingletonSocketElection, String> {
    require_canonical_global_owner(state_home).await?;
    let path = singleton_path(state_home)?;
    let parent = path.parent().ok_or_else(|| {
        format!(
            "Runtime Server singleton socket has no parent: {}",
            path.display()
        )
    })?;

    timeout(SINGLETON_IO_DEADLINE, tokio::fs::create_dir_all(parent))
        .await
        .map_err(|_| {
            format!(
                "timed out creating Runtime Server singleton directory {}; refusing ownership",
                parent.display()
            )
        })?
        .map_err(|error| {
            format!(
                "failed to create Runtime Server singleton directory {}: {error}",
                parent.display()
            )
        })?;

    match UnixDatagram::bind(&path) {
        Ok(socket) => guard(path, socket)
            .await
            .map(SingletonSocketElection::Acquired),
        Err(error) if error.kind() == io::ErrorKind::AddrInUse => {
            if resident_exists(state_home).await? {
                return Ok(SingletonSocketElection::ResidentExists);
            }

            timeout(SINGLETON_IO_DEADLINE, tokio::fs::remove_file(&path))
                .await
                .map_err(|_| {
                    format!(
                        "timed out removing stale Runtime Server singleton socket {}; refusing ownership",
                        path.display()
                    )
                })?
                .map_err(|remove_error| {
                    format!(
                        "failed to remove stale Runtime Server singleton socket {}: {remove_error}",
                        path.display()
                    )
                })?;

            let socket = UnixDatagram::bind(&path).map_err(|bind_error| {
                format!(
                    "failed to bind Runtime Server singleton socket {} after stale recovery: {bind_error}",
                    path.display()
                )
            })?;
            guard(path, socket)
                .await
                .map(SingletonSocketElection::Acquired)
        }
        Err(error) => Err(format!(
            "failed to bind Runtime Server singleton socket {}: {error}",
            path.display()
        )),
    }
}

impl SingletonSocketGuard {
    pub(super) async fn release(mut self) -> Result<(), String> {
        self.released = true;
        drop(self.socket.take());
        remove_if_owned(&self.path, self.inode).await
    }
}

impl Drop for SingletonSocketGuard {
    fn drop(&mut self) {
        if self.released {
            return;
        }

        if let Ok(runtime) = Handle::try_current() {
            self.released = true;
            let guard = SingletonSocketGuard {
                path: self.path.clone(),
                inode: self.inode,
                socket: self.socket.take(),
                // Ownership has already crossed the Drop scheduling boundary.
                // If runtime teardown cancels this future before its first poll,
                // the nested guard must fail closed instead of spawning again.
                released: true,
            };
            runtime.spawn(async move {
                let _ = guard.release().await;
            });
        }
    }
}

fn singleton_path(state_home: &Path) -> Result<PathBuf, String> {
    Ok(
        agent_semantic_client_db::runtime_server_runtime_base(state_home)?
            .join("runtime-server-singleton.sock"),
    )
}

async fn require_canonical_global_owner(state_home: &Path) -> Result<(), String> {
    let canonical_binary = state_home.join("runtime").join("bin").join("asp");
    match timeout(
        SINGLETON_IO_DEADLINE,
        tokio::fs::metadata(&canonical_binary),
    )
    .await
    {
        Ok(Err(error)) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Ok(Err(error)) => {
            return Err(format!(
                "failed to inspect canonical Runtime Server binary {}: {error}",
                canonical_binary.display()
            ));
        }
        Err(_) => {
            return Err(format!(
                "timed out inspecting canonical Runtime Server binary {}; refusing ownership",
                canonical_binary.display()
            ));
        }
        Ok(Ok(_)) => {}
    }

    let current_exe = env::current_exe()
        .map_err(|error| format!("failed to resolve current Runtime Server executable: {error}"))?;
    let canonical_current = timeout(SINGLETON_IO_DEADLINE, tokio::fs::canonicalize(&current_exe))
        .await
        .map_err(|_| {
            format!(
                "timed out canonicalizing Runtime Server executable {}; refusing ownership",
                current_exe.display()
            )
        })?
        .map_err(|error| {
            format!(
                "failed to canonicalize Runtime Server executable {}: {error}",
                current_exe.display()
            )
        })?;
    let canonical_owner = timeout(
        SINGLETON_IO_DEADLINE,
        tokio::fs::canonicalize(&canonical_binary),
    )
    .await
    .map_err(|_| {
        format!(
            "timed out canonicalizing installed Runtime Server binary {}; refusing ownership",
            canonical_binary.display()
        )
    })?
    .map_err(|error| {
        format!(
            "failed to canonicalize installed Runtime Server binary {}: {error}",
            canonical_binary.display()
        )
    })?;

    if canonical_current != canonical_owner {
        return Err(format!(
            "refusing non-canonical Runtime Server owner {}; installed state home {} is owned by {}; debug and test daemons must use an isolated ASP_STATE_HOME",
            canonical_current.display(),
            state_home.display(),
            canonical_owner.display()
        ));
    }

    Ok(())
}

async fn probe_live(path: &Path) -> Result<bool, String> {
    let socket = UnixDatagram::unbound().map_err(|error| {
        format!(
            "failed to create Runtime Server singleton probe for {}: {error}",
            path.display()
        )
    })?;
    match timeout(SINGLETON_IO_DEADLINE, async { socket.connect(path) }).await {
        Ok(Ok(())) => Ok(true),
        Ok(Err(error))
            if matches!(
                error.kind(),
                io::ErrorKind::ConnectionRefused | io::ErrorKind::NotFound
            ) =>
        {
            Ok(false)
        }
        Ok(Err(error)) => Err(format!(
            "failed to probe Runtime Server singleton socket {}: {error}",
            path.display()
        )),
        Err(_) => Err(format!(
            "timed out probing Runtime Server singleton socket {}; refusing stale recovery",
            path.display()
        )),
    }
}

async fn guard(path: PathBuf, socket: UnixDatagram) -> Result<SingletonSocketGuard, String> {
    let metadata = timeout(SINGLETON_IO_DEADLINE, tokio::fs::metadata(&path))
        .await
        .map_err(|_| {
            format!(
                "timed out reading Runtime Server singleton socket metadata {}; refusing ownership",
                path.display()
            )
        })?
        .map_err(|error| {
            format!(
                "failed to read Runtime Server singleton socket metadata {}: {error}",
                path.display()
            )
        })?;
    Ok(SingletonSocketGuard {
        path,
        inode: metadata.ino(),
        socket: Some(socket),
        released: false,
    })
}

async fn remove_if_owned(path: &Path, inode: u64) -> Result<(), String> {
    let metadata = match timeout(SINGLETON_IO_DEADLINE, tokio::fs::metadata(path)).await {
        Ok(Ok(metadata)) => metadata,
        Ok(Err(error)) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Ok(Err(error)) => {
            return Err(format!(
                "failed to inspect Runtime Server singleton socket {} during release: {error}",
                path.display()
            ));
        }
        Err(_) => {
            return Err(format!(
                "timed out inspecting Runtime Server singleton socket {} during release",
                path.display()
            ));
        }
    };
    if metadata.ino() != inode {
        return Ok(());
    }

    timeout(SINGLETON_IO_DEADLINE, tokio::fs::remove_file(path))
        .await
        .map_err(|_| {
            format!(
                "timed out removing Runtime Server singleton socket {} during release",
                path.display()
            )
        })?
        .or_else(|error| {
            if error.kind() == io::ErrorKind::NotFound {
                Ok(())
            } else {
                Err(error)
            }
        })
        .map_err(|error| {
            format!(
                "failed to remove Runtime Server singleton socket {} during release: {error}",
                path.display()
            )
        })
}
