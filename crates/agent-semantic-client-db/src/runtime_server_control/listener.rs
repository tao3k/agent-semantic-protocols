use libc::getuid;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::Path;
use tokio::net::TcpListener;

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

pub async fn bind_runtime_server_listener() -> Result<TcpListener, String> {
    TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
        .await
        .map_err(|error| format!("failed to bind Runtime Server loopback listener: {error}"))
}

pub async fn bind_runtime_server_listener_at(
    endpoint: &super::RuntimeServerLoopbackEndpoint,
) -> Result<TcpListener, String> {
    endpoint.validate()?;
    TcpListener::bind(endpoint.socket_addr())
        .await
        .map_err(|error| format!("failed to bind published Runtime Server endpoint: {error}"))
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
