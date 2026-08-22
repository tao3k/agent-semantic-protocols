use libc::getuid;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::Path;
use tokio::net::{UnixListener, UnixSocket};

pub fn runtime_server_listener_backlog() -> u32 {
    crate::runtime_concurrency::RuntimeConcurrencyPlan::current()
        .reader_limit()
        .saturating_mul(64) as u32
}

pub fn runtime_server_connection_pool_size() -> usize {
    crate::runtime_concurrency::RuntimeConcurrencyPlan::current().reader_limit()
}

pub fn runtime_server_connection_pool_capacity() -> usize {
    runtime_server_connection_pool_size()
        .saturating_mul(4)
        .clamp(8, 128)
}

pub fn bind_runtime_server_listener(socket_path: &Path) -> Result<UnixListener, String> {
    validate_private_runtime_directory(
        socket_path
            .parent()
            .ok_or_else(|| "Runtime Server socket path has no parent".to_owned())?,
    )?;
    let socket = UnixSocket::new_stream()
        .map_err(|error| format!("failed to create Runtime Server socket: {error}"))?;
    socket.bind(socket_path).map_err(|error| {
        format!(
            "failed to bind Runtime Server socket {}: {error}",
            socket_path.display()
        )
    })?;
    let mut permissions = std::fs::symlink_metadata(socket_path)
        .map_err(|error| format!("failed to inspect bound Runtime Server socket: {error}"))?
        .permissions();
    permissions.set_mode(0o600);
    std::fs::set_permissions(socket_path, permissions)
        .map_err(|error| format!("failed to protect Runtime Server socket: {error}"))?;
    let metadata = std::fs::symlink_metadata(socket_path)
        .map_err(|error| format!("failed to verify bound Runtime Server socket: {error}"))?;
    if !std::os::unix::fs::FileTypeExt::is_socket(&metadata.file_type())
        || metadata.uid() != unsafe { getuid() }
        || metadata.mode() & 0o777 != 0o600
    {
        return Err("Runtime Server socket is not a private current-UID Unix socket".to_owned());
    }
    socket
        .listen(runtime_server_listener_backlog())
        .map_err(|error| {
            format!(
                "failed to listen on Runtime Server socket {}: {error}",
                socket_path.display()
            )
        })
}

fn validate_private_runtime_directory(path: &Path) -> Result<(), String> {
    let metadata = std::fs::symlink_metadata(path).map_err(|error| {
        format!(
            "failed to inspect Runtime Server private directory {}: {error}",
            path.display()
        )
    })?;
    if !metadata.file_type().is_dir()
        || metadata.file_type().is_symlink()
        || metadata.uid() != unsafe { getuid() }
        || metadata.mode() & 0o777 != 0o700
    {
        return Err(format!(
            "Runtime Server directory is not a private, non-symlink current-UID directory: {}",
            path.display()
        ));
    }
    Ok(())
}

pub(super) async fn prepare_private_runtime_directory(directory: &Path) -> Result<(), String> {
    match tokio::fs::create_dir(directory).await {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(error) => {
            return Err(format!(
                "failed to create Runtime Server directory {}: {error}",
                directory.display()
            ));
        }
    }
    let metadata = tokio::fs::symlink_metadata(directory)
        .await
        .map_err(|error| format!("failed to inspect Runtime Server directory: {error}"))?;
    if !metadata.file_type().is_dir()
        || metadata.file_type().is_symlink()
        || metadata.uid() != unsafe { getuid() }
    {
        return Err(format!(
            "Runtime Server directory is not a non-symlink current-UID directory: {}",
            directory.display()
        ));
    }
    let mut permissions = metadata.permissions();
    permissions.set_mode(0o700);
    tokio::fs::set_permissions(directory, permissions)
        .await
        .map_err(|error| format!("failed to protect Runtime Server directory: {error}"))?;
    validate_private_runtime_directory(directory)
}

/// Authenticates a connected Unix peer against the effective ASP owner UID.
/// Path ownership is discovery authority only; the kernel credential is the
/// connection authority and cannot be forged by replacing an endpoint file.
pub fn validate_runtime_server_peer_fd(fd: std::os::fd::RawFd) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        let mut credential: libc::ucred = unsafe { std::mem::zeroed() };
        let mut length = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
        let result = unsafe {
            libc::getsockopt(
                fd,
                libc::SOL_SOCKET,
                libc::SO_PEERCRED,
                (&mut credential as *mut libc::ucred).cast(),
                &mut length,
            )
        };
        if result != 0 {
            return Err(format!(
                "failed to authenticate Runtime Server Unix peer: {}",
                std::io::Error::last_os_error()
            ));
        }
        if credential.uid != unsafe { libc::geteuid() } {
            return Err("Runtime Server Unix peer UID mismatch".to_owned());
        }
        return Ok(());
    }
    #[cfg(any(
        target_os = "macos",
        target_os = "freebsd",
        target_os = "dragonfly",
        target_os = "netbsd",
        target_os = "openbsd"
    ))]
    {
        let mut effective_uid: libc::uid_t = 0;
        let mut effective_gid: libc::gid_t = 0;
        let result = unsafe { libc::getpeereid(fd, &mut effective_uid, &mut effective_gid) };
        if result != 0 {
            return Err(format!(
                "failed to authenticate Runtime Server Unix peer: {}",
                std::io::Error::last_os_error()
            ));
        }
        if effective_uid != unsafe { libc::geteuid() } {
            return Err("Runtime Server Unix peer UID mismatch".to_owned());
        }
        return Ok(());
    }
    #[cfg(not(any(
        target_os = "linux",
        target_os = "macos",
        target_os = "freebsd",
        target_os = "dragonfly",
        target_os = "netbsd",
        target_os = "openbsd"
    )))]
    {
        let _ = fd;
        Err("Runtime Server Unix peer authentication is unsupported on this platform".to_owned())
    }
}
