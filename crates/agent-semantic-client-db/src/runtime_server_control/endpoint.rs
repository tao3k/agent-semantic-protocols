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

pub fn runtime_server_runtime_base() -> PathBuf {
    PathBuf::from("/tmp").join(format!("asp-runtime-server-{}", unsafe { getuid() }))
}

pub fn runtime_server_endpoint_path(state_home: &Path) -> PathBuf {
    state_home
        .join("runtime")
        .join("server")
        .join("endpoint.v1.json")
}

pub fn read_runtime_server_endpoint(
    state_home: &Path,
) -> Result<Option<RuntimeServerEndpoint>, String> {
    let endpoint_path = runtime_server_endpoint_path(state_home);
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
    let temporary = endpoint_path.with_extension(format!("tmp-{}", endpoint.owner_epoch));
    if let Err(error) = tokio::fs::write(&temporary, bytes).await {
        let _ = tokio::fs::remove_file(&temporary).await;
        return Err(format!(
            "failed to write Runtime Server endpoint temporary file {}: {error}",
            temporary.display()
        ));
    }
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
    let socket = UnixSocket::new_stream()
        .map_err(|error| format!("failed to create Runtime Server socket: {error}"))?;
    socket.bind(socket_path).map_err(|error| {
        format!(
            "failed to bind Runtime Server socket {}: {error}",
            socket_path.display()
        )
    })?;
    socket
        .listen(runtime_server_listener_backlog())
        .map_err(|error| {
            format!(
                "failed to listen on Runtime Server socket {}: {error}",
                socket_path.display()
            )
        })
}

pub async fn acquire_runtime_server_election() -> Result<RuntimeServerElection, String> {
    let runtime_base = runtime_server_runtime_base();
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
    runtime_artifact_path: &Path,
    runtime_artifact_digest: &str,
    artifact_mode: &str,
    artifact_catalog_digest: &str,
    owner_epoch: u64,
    binding_token: &str,
) -> Result<RuntimeServerEndpoint, String> {
    let runtime_base = runtime_server_runtime_base();
    prepare_runtime_server_endpoint_in(
        &runtime_base,
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
    tokio::fs::create_dir_all(runtime_base)
        .await
        .map_err(|error| {
            format!(
                "failed to create Runtime Server directory {}: {error}",
                runtime_base.display()
            )
        })?;
    let mut permissions = tokio::fs::metadata(runtime_base)
        .await
        .map_err(|error| format!("failed to inspect Runtime Server directory: {error}"))?
        .permissions();
    permissions.set_mode(0o700);
    tokio::fs::set_permissions(runtime_base, permissions)
        .await
        .map_err(|error| format!("failed to protect Runtime Server directory: {error}"))?;
    let metadata = tokio::fs::metadata(runtime_base)
        .await
        .map_err(|error| format!("failed to inspect Runtime Server directory: {error}"))?;
    if metadata.uid() != unsafe { getuid() } || metadata.mode() & 0o777 != 0o700 {
        return Err("Runtime Server directory is not private to the current UID".to_owned());
    }
    let digest = blake3::hash(
        format!("{owner_epoch}\0{binding_token}\0{runtime_artifact_digest}").as_bytes(),
    )
    .to_hex();
    let socket_path = runtime_base.join(format!("r-{}.sock", &digest[..16]));
    let data_plane_socket_path = runtime_base.join(format!("r-{}.data.sock", &digest[..16]));
    let status_memory_path = runtime_base.join("status.v1.memory");
    let workspace_store_path = runtime_base.join("workspaces");
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
