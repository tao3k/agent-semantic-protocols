use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::Path;

use fs4::AsyncFileExt;

use super::endpoint_identity::{
    runtime_server_endpoint_path, runtime_server_endpoint_path_async,
    runtime_server_runtime_base_async,
};
use super::model::{
    ENDPOINT_SCHEMA_ID, RuntimeServerEndpoint, RuntimeServerEndpointOwnerBinding, SCHEMA_VERSION,
    runtime_server_transport_contract_digest,
};
use agent_semantic_runtime::runtime_artifact_catalog::RuntimeBinaryIdentity;

const MAX_UNIX_SOCKET_PATH_BYTES: usize = 103;

unsafe extern "C" {
    fn getuid() -> u32;
}

pub struct RuntimeServerElection {
    _file: tokio::fs::File,
}

/// Serializes supervisor handoffs without contending with the daemon's
/// lifetime-long owner election.
pub struct RuntimeServerSupervisorTransaction {
    _file: tokio::fs::File,
}

pub enum RuntimeServerElectionAttempt {
    Acquired(RuntimeServerElection),
    Contended,
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

pub async fn read_runtime_server_supervisor_endpoint(
    state_home: &Path,
) -> Result<Option<RuntimeServerEndpoint>, String> {
    let endpoint_path = runtime_server_endpoint_path_async(state_home).await?;
    let metadata = match tokio::fs::symlink_metadata(&endpoint_path).await {
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
    let bytes = tokio::fs::read(&endpoint_path).await.map_err(|error| {
        format!(
            "failed to read Runtime Server endpoint {}: {error}",
            endpoint_path.display()
        )
    })?;
    let endpoint: RuntimeServerEndpoint = serde_json::from_slice(&bytes).map_err(|error| {
        format!(
            "failed to decode Runtime Server endpoint {}: {error}",
            endpoint_path.display()
        )
    })?;
    endpoint.validate_supervisor_control()?;
    super::endpoint_validation::validate_runtime_server_endpoint_for_state_home(
        state_home, &endpoint,
    )?;
    Ok(Some(endpoint))
}

/// Reads only the stable lifecycle owner envelope from the canonical endpoint.
///
/// This is the fail-closed recovery path when the service endpoint cannot be
/// decoded. Callers may use it to bind a live process to its owner receipt and
/// retire that exact owner, but must never use it for data-plane admission.
pub async fn read_runtime_server_endpoint_owner_binding(
    state_home: &Path,
) -> Result<Option<RuntimeServerEndpointOwnerBinding>, String> {
    let endpoint_path = runtime_server_endpoint_path_async(state_home).await?;
    let metadata = match tokio::fs::symlink_metadata(&endpoint_path).await {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(format!(
                "failed to inspect Runtime Server endpoint owner binding {}: {error}",
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
            "Runtime Server endpoint owner binding is not a private, non-symlink current-UID file: {}",
            endpoint_path.display()
        ));
    }
    let bytes = match tokio::fs::read(&endpoint_path).await {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(format!(
                "failed to read Runtime Server endpoint owner binding {}: {error}",
                endpoint_path.display()
            ));
        }
    };
    let binding: RuntimeServerEndpointOwnerBinding =
        serde_json::from_slice(&bytes).map_err(|error| {
            format!(
                "failed to decode Runtime Server endpoint owner binding {}: {error}",
                endpoint_path.display()
            )
        })?;
    binding.validate()?;
    Ok(Some(binding))
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

pub async fn acquire_runtime_server_election(
    state_home: &Path,
) -> Result<RuntimeServerElection, String> {
    match try_acquire_runtime_server_election(state_home).await? {
        RuntimeServerElectionAttempt::Acquired(election) => Ok(election),
        RuntimeServerElectionAttempt::Contended => {
            Err("Runtime Server election is already held".to_owned())
        }
    }
}

pub async fn try_acquire_runtime_server_election(
    state_home: &Path,
) -> Result<RuntimeServerElectionAttempt, String> {
    let (file, lock_path) = open_runtime_server_election_file(state_home).await?;
    match file.try_lock() {
        Ok(()) => Ok(RuntimeServerElectionAttempt::Acquired(
            RuntimeServerElection { _file: file },
        )),
        Err(fs4::TryLockError::WouldBlock) => Ok(RuntimeServerElectionAttempt::Contended),
        Err(fs4::TryLockError::Error(error)) => Err(format!(
            "failed to acquire Runtime Server election at {}: {error}",
            lock_path.display()
        )),
    }
}

async fn open_runtime_server_election_file(
    state_home: &Path,
) -> Result<(tokio::fs::File, std::path::PathBuf), String> {
    let runtime_base = runtime_server_runtime_base_async(state_home).await?;
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
    Ok((file, lock_path))
}

#[tracing::instrument(name = "asp.runtime.server_election.wait", skip_all)]
pub async fn wait_for_runtime_server_election(
    state_home: &Path,
) -> Result<RuntimeServerElection, String> {
    let started_at = std::time::Instant::now();
    let (file, lock_path) = open_runtime_server_election_file(state_home).await?;
    let file = tokio::task::spawn_blocking(move || {
        file.lock().map_err(|error| {
            format!(
                "failed to await Runtime Server election at {}: {error}",
                lock_path.display()
            )
        })?;
        Ok::<_, String>(file)
    })
    .await
    .map_err(|error| format!("Runtime Server election task failed: {error}"))??;
    tracing::info!(
        elapsed_micros = u64::try_from(started_at.elapsed().as_micros()).unwrap_or(u64::MAX),
        "Runtime Server election acquired"
    );
    Ok(RuntimeServerElection { _file: file })
}

pub async fn acquire_runtime_server_supervisor_transaction(
    state_home: &Path,
) -> Result<RuntimeServerSupervisorTransaction, String> {
    let runtime_base = runtime_server_runtime_base_async(state_home).await?;
    tokio::fs::create_dir_all(&runtime_base)
        .await
        .map_err(|error| format!("failed to create Runtime Server directory: {error}"))?;
    let lock_path = runtime_base.join("runtime-server.supervisor.lock");
    let file = tokio::fs::OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .open(&lock_path)
        .await
        .map_err(|error| {
            format!(
                "failed to open Runtime Server supervisor transaction {}: {error}",
                lock_path.display()
            )
        })?;
    let file = tokio::task::spawn_blocking(move || {
        file.lock().map_err(|error| {
            format!(
                "failed to acquire Runtime Server supervisor transaction {}: {error}",
                lock_path.display()
            )
        })?;
        Ok::<_, String>(file)
    })
    .await
    .map_err(|error| format!("Runtime Server supervisor transaction task failed: {error}"))??;
    Ok(RuntimeServerSupervisorTransaction { _file: file })
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
    let runtime_base = runtime_server_runtime_base_async(state_home).await?;
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
    let runtime_base = runtime_server_runtime_base_async(state_home).await?;
    prepare_runtime_server_endpoint_in_with_workspace_store(
        &runtime_base,
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

pub async fn prepare_runtime_server_endpoint_with_workspace_store_and_identity(
    state_home: &Path,
    workspace_store_path: &Path,
    runtime_artifact_path: &Path,
    runtime_binary_identity: &RuntimeBinaryIdentity,
    artifact_mode: &str,
    artifact_catalog_digest: &str,
    owner_epoch: u64,
    binding_token: &str,
    client_http_endpoint: &str,
) -> Result<RuntimeServerEndpoint, String> {
    let runtime_base = runtime_server_runtime_base_async(state_home).await?;
    let mut endpoint = prepare_runtime_server_endpoint_in_with_workspace_store_and_identity(
        &runtime_base,
        workspace_store_path,
        runtime_artifact_path,
        runtime_binary_identity,
        artifact_mode,
        artifact_catalog_digest,
        owner_epoch,
        binding_token,
    )
    .await?;
    endpoint.client_http_endpoint = client_http_endpoint.to_owned();
    Ok(endpoint)
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
    prepare_runtime_server_endpoint_in_with_workspace_store_and_identity(
        runtime_base,
        workspace_store_path,
        runtime_artifact_path,
        &RuntimeBinaryIdentity::Content {
            value: runtime_artifact_digest.to_owned(),
            algorithm: "blake3-256".to_owned(),
        },
        artifact_mode,
        artifact_catalog_digest,
        owner_epoch,
        binding_token,
    )
    .await
}

async fn prepare_runtime_server_endpoint_in_with_workspace_store_and_identity(
    runtime_base: &Path,
    workspace_store_path: &Path,
    runtime_artifact_path: &Path,
    runtime_binary_identity: &RuntimeBinaryIdentity,
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
        super::listener::prepare_private_runtime_directory(uid_root).await?;
    }
    super::listener::prepare_private_runtime_directory(runtime_base).await?;
    let digest = blake3::hash(
        format!(
            "{owner_epoch}\0{binding_token}\0{}",
            runtime_binary_identity.value()
        )
        .as_bytes(),
    )
    .to_hex();
    let socket_path = runtime_base.join(format!("r-{}.sock", &digest[..16]));
    let data_plane_socket_path = runtime_base.join(format!("r-{}.data.sock", &digest[..16]));
    let provider_plane_socket_path = super::provider_endpoint::provider_plane_socket_path(
        &runtime_base,
        &digest,
        MAX_UNIX_SOCKET_PATH_BYTES,
    )?;
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
        owner_process_id: agent_semantic_runtime::runtime_process_lifecycle::current_process_id(),
        runtime_artifact_path: runtime_artifact_path.to_string_lossy().into_owned(),
        runtime_binary_identity: runtime_binary_identity.clone(),
        monitor_capability: true,
        observed_runtime_binary_identity: runtime_binary_identity.clone(),
        artifact_mode: artifact_mode.to_owned(),
        artifact_catalog_digest: artifact_catalog_digest.to_owned(),
        binding_token: binding_token.to_owned(),
        socket_path: socket_path.to_string_lossy().into_owned(),
        data_plane_socket_path: data_plane_socket_path.to_string_lossy().into_owned(),
        provider_plane_socket_path: provider_plane_socket_path.to_string_lossy().into_owned(),
        client_http_endpoint: "http://127.0.0.1:1".to_owned(),
        workspace_store_path: workspace_store_path.to_string_lossy().into_owned(),
        status_memory_path: status_memory_path.to_string_lossy().into_owned(),
    })
}
