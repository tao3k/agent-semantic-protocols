use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};

use fs4::AsyncFileExt;
use tokio::net::{UnixListener, UnixSocket};

use super::model::{
    ENDPOINT_SCHEMA_ID, RuntimeServerEndpoint, SCHEMA_VERSION,
    runtime_server_transport_contract_digest,
};

const MAX_UNIX_SOCKET_PATH_BYTES: usize = 103;

unsafe extern "C" {
    fn getuid() -> u32;
}

pub struct RuntimeServerElection {
    _file: tokio::fs::File,
}

pub fn runtime_server_runtime_base(state_home: &Path) -> Result<PathBuf, String> {
    let canonical_state_home = std::fs::canonicalize(state_home).map_err(|error| {
        format!(
            "failed to canonicalize ASP State Home {} for Runtime Server identity: {error}",
            state_home.display()
        )
    })?;
    let mut identity = blake3::Hasher::new();
    identity.update(b"agent.semantic-protocols.runtime-server-state-home.v1\0");
    identity.update(canonical_state_home.as_os_str().as_bytes());
    let identity = identity.finalize().to_hex();
    Ok(PathBuf::from("/tmp")
        .join(format!("asp-runtime-server-{}", unsafe { getuid() }))
        .join(format!("state-{}", &identity[..24])))
}

pub fn runtime_server_endpoint_path(state_home: &Path) -> Result<PathBuf, String> {
    Ok(runtime_server_runtime_base(state_home)?.join("endpoint.v1.json"))
}

pub fn read_runtime_server_endpoint(
    state_home: &Path,
) -> Result<Option<RuntimeServerEndpoint>, String> {
    let endpoint_path = runtime_server_endpoint_path(state_home)?;
    let metadata = match std::fs::symlink_metadata(&endpoint_path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(format!(
                "failed to inspect Runtime Server endpoint {}: {error}",
                endpoint_path.display()
            ));
        }
    };
    if !metadata.file_type().is_file()
        || metadata.file_type().is_symlink()
        || metadata.uid() != unsafe { getuid() }
        || metadata.mode() & 0o777 != 0o600
    {
        return Err(format!(
            "Runtime Server endpoint is not a private, non-symlink current-UID file: {}",
            endpoint_path.display()
        ));
    }
    let bytes = match std::fs::read(&endpoint_path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(format!(
                "failed to read Runtime Server endpoint {}: {error}",
                endpoint_path.display()
            ));
        }
    };
    let endpoint: RuntimeServerEndpoint = serde_json::from_slice(&bytes).map_err(|error| {
        format!(
            "failed to decode Runtime Server endpoint {}: {error}",
            endpoint_path.display()
        )
    })?;
    endpoint.validate().map_err(|error| {
        format!(
            "invalid Runtime Server endpoint {}: {error}",
            endpoint_path.display()
        )
    })?;
    Ok(Some(endpoint))
}

/// Validates that endpoint transport paths are derived from this State Home
/// and from the advertised owner epoch, binding token, and artifact digest.
pub fn validate_runtime_server_endpoint_for_state_home(
    state_home: &Path,
    endpoint: &RuntimeServerEndpoint,
) -> Result<(), String> {
    endpoint.validate()?;
    let runtime_base = runtime_server_runtime_base(state_home)?;
    let digest = blake3::hash(
        format!(
            "{}\0{}\0{}",
            endpoint.owner_epoch, endpoint.binding_token, endpoint.runtime_artifact_digest
        )
        .as_bytes(),
    )
    .to_hex();
    let expected_socket = runtime_base.join(format!("r-{}.sock", &digest[..16]));
    let expected_data_socket = runtime_base.join(format!("r-{}.data.sock", &digest[..16]));
    let expected_status_memory = runtime_base.join("status.v1.memory");
    if Path::new(&endpoint.socket_path) != expected_socket
        || Path::new(&endpoint.data_plane_socket_path) != expected_data_socket
        || Path::new(&endpoint.status_memory_path) != expected_status_memory
    {
        return Err(format!(
            "Runtime Server endpoint State Home or binding identity mismatch: stateHome={} ownerEpoch={}",
            state_home.display(),
            endpoint.owner_epoch
        ));
    }
    Ok(())
}

pub async fn publish_runtime_server_endpoint(
    endpoint_path: &Path,
    endpoint: &RuntimeServerEndpoint,
) -> Result<(), String> {
    let parent = endpoint_path
        .parent()
        .ok_or_else(|| "Runtime Server endpoint path has no parent".to_owned())?;
    tokio::fs::create_dir_all(parent).await.map_err(|error| {
        format!(
            "failed to create Runtime Server endpoint directory {}: {error}",
            parent.display()
        )
    })?;
    let bytes = serde_json::to_vec(endpoint)
        .map_err(|error| format!("failed to encode Runtime Server endpoint: {error}"))?;
    let temporary_identity =
        blake3::hash(format!("{}\0{}", endpoint.owner_epoch, endpoint.binding_token).as_bytes())
            .to_hex();
    let temporary = endpoint_path.with_extension(format!(
        "tmp-{}-{}",
        endpoint.owner_epoch,
        &temporary_identity[..16]
    ));
    let mut temporary_file = tokio::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .await
        .map_err(|error| {
            format!(
                "failed to create exclusive Runtime Server endpoint temporary file for {}: {error}",
                endpoint_path.display()
            )
        })?;
    let mut permissions = temporary_file
        .metadata()
        .await
        .map_err(|error| {
            format!("failed to inspect Runtime Server endpoint temporary file: {error}")
        })?
        .permissions();
    permissions.set_mode(0o600);
    temporary_file
        .set_permissions(permissions)
        .await
        .map_err(|error| {
            format!("failed to protect Runtime Server endpoint temporary file: {error}")
        })?;
    if let Err(error) = tokio::io::AsyncWriteExt::write_all(&mut temporary_file, &bytes).await {
        drop(temporary_file);
        let _ = tokio::fs::remove_file(&temporary).await;
        return Err(format!(
            "failed to write Runtime Server endpoint temporary file for {}: {error}",
            endpoint_path.display()
        ));
    }
    if let Err(error) = temporary_file.sync_all().await {
        drop(temporary_file);
        let _ = tokio::fs::remove_file(&temporary).await;
        return Err(format!(
            "failed to sync Runtime Server endpoint temporary file: {error}"
        ));
    }
    drop(temporary_file);
    if let Err(error) = tokio::fs::rename(&temporary, endpoint_path).await {
        let _ = tokio::fs::remove_file(&temporary).await;
        return Err(format!(
            "failed to publish Runtime Server endpoint {}: {error}",
            endpoint_path.display()
        ));
    }
    Ok(())
}

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

async fn prepare_private_runtime_directory(directory: &Path) -> Result<(), String> {
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

pub async fn acquire_runtime_server_election(
    state_home: &Path,
) -> Result<RuntimeServerElection, String> {
    let runtime_base = runtime_server_runtime_base(state_home)?;
    tokio::fs::create_dir_all(&runtime_base)
        .await
        .map_err(|error| {
            format!(
                "failed to create Runtime Server directory {}: {error}",
                runtime_base.display()
            )
        })?;
    let lock_path = runtime_base.join("runtime-server.owner.lock");
    let file = tokio::fs::OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .open(&lock_path)
        .await
        .map_err(|error| {
            format!(
                "failed to open Runtime Server election lock {}: {error}",
                lock_path.display()
            )
        })?;
    file.try_lock().map_err(|error| {
        format!(
            "Runtime Server election is already held or unavailable at {}: {error}",
            lock_path.display()
        )
    })?;
    Ok(RuntimeServerElection { _file: file })
}

pub async fn prepare_runtime_server_endpoint(
    state_home: &Path,
    runtime_artifact_path: &Path,
    runtime_artifact_digest: &str,
    artifact_mode: &str,
    artifact_catalog_digest: &str,
    owner_epoch: u64,
    binding_token: &str,
) -> Result<RuntimeServerEndpoint, String> {
    let runtime_base = runtime_server_runtime_base(state_home)?;
    let workspace_store_path = runtime_base.join("workspaces");
    prepare_runtime_server_endpoint_in_with_workspace_store(
        &runtime_base,
        &workspace_store_path,
        runtime_artifact_path,
        runtime_artifact_digest,
        artifact_mode,
        artifact_catalog_digest,
        owner_epoch,
        binding_token,
    )
    .await
}

pub async fn prepare_runtime_server_endpoint_with_workspace_store(
    state_home: &Path,
    workspace_store_path: &Path,
    runtime_artifact_path: &Path,
    runtime_artifact_digest: &str,
    artifact_mode: &str,
    artifact_catalog_digest: &str,
    owner_epoch: u64,
    binding_token: &str,
) -> Result<RuntimeServerEndpoint, String> {
    prepare_runtime_server_endpoint_in_with_workspace_store(
        &runtime_server_runtime_base(state_home)?,
        workspace_store_path,
        runtime_artifact_path,
        runtime_artifact_digest,
        artifact_mode,
        artifact_catalog_digest,
        owner_epoch,
        binding_token,
    )
    .await
}

pub async fn prepare_runtime_server_endpoint_in(
    runtime_base: &Path,
    runtime_artifact_path: &Path,
    runtime_artifact_digest: &str,
    artifact_mode: &str,
    artifact_catalog_digest: &str,
    owner_epoch: u64,
    binding_token: &str,
) -> Result<RuntimeServerEndpoint, String> {
    let workspace_store_path = runtime_base.join("workspaces");
    prepare_runtime_server_endpoint_in_with_workspace_store(
        runtime_base,
        &workspace_store_path,
        runtime_artifact_path,
        runtime_artifact_digest,
        artifact_mode,
        artifact_catalog_digest,
        owner_epoch,
        binding_token,
    )
    .await
}

async fn prepare_runtime_server_endpoint_in_with_workspace_store(
    runtime_base: &Path,
    workspace_store_path: &Path,
    runtime_artifact_path: &Path,
    runtime_artifact_digest: &str,
    artifact_mode: &str,
    artifact_catalog_digest: &str,
    owner_epoch: u64,
    binding_token: &str,
) -> Result<RuntimeServerEndpoint, String> {
    let uid_root = runtime_base
        .parent()
        .ok_or_else(|| "Runtime Server directory has no UID root".to_owned())?;
    let canonical_uid_root_name = format!("asp-runtime-server-{}", unsafe { getuid() });
    if uid_root.file_name().and_then(|name| name.to_str()) == Some(&canonical_uid_root_name) {
        prepare_private_runtime_directory(uid_root).await?;
    }
    prepare_private_runtime_directory(runtime_base).await?;
    let digest = blake3::hash(
        format!("{owner_epoch}\0{binding_token}\0{runtime_artifact_digest}").as_bytes(),
    )
    .to_hex();
    let socket_path = runtime_base.join(format!("r-{}.sock", &digest[..16]));
    let data_plane_socket_path = runtime_base.join(format!("r-{}.data.sock", &digest[..16]));
    let status_memory_path = runtime_base.join("status.v1.memory");
    if socket_path.as_os_str().as_bytes().len() > MAX_UNIX_SOCKET_PATH_BYTES {
        return Err(format!(
            "Runtime Server socket path exceeds Unix sun_path budget: {}",
            socket_path.display()
        ));
    }
    if data_plane_socket_path.as_os_str().as_bytes().len() > MAX_UNIX_SOCKET_PATH_BYTES {
        return Err(format!(
            "Runtime Server data-plane socket path exceeds Unix sun_path budget: {}",
            data_plane_socket_path.display()
        ));
    }
    Ok(RuntimeServerEndpoint {
        schema_id: ENDPOINT_SCHEMA_ID.to_owned(),
        schema_version: SCHEMA_VERSION.to_owned(),
        transport_contract_digest: runtime_server_transport_contract_digest(),
        owner_epoch,
        runtime_artifact_path: runtime_artifact_path.to_string_lossy().into_owned(),
        runtime_artifact_digest: runtime_artifact_digest.to_owned(),
        artifact_mode: artifact_mode.to_owned(),
        artifact_catalog_digest: artifact_catalog_digest.to_owned(),
        binding_token: binding_token.to_owned(),
        socket_path: socket_path.to_string_lossy().into_owned(),
        data_plane_socket_path: data_plane_socket_path.to_string_lossy().into_owned(),
        workspace_store_path: workspace_store_path.to_string_lossy().into_owned(),
        status_memory_path: status_memory_path.to_string_lossy().into_owned(),
    })
}
