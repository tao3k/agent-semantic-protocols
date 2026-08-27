//! Runtime Server lifecycle receipt store owned by client-db composition.

use crate::{
    RuntimeServerActivationReadyReceipt, RuntimeServerDrainReceipt, RuntimeServerExitReceipt,
    RuntimeServerResidentTransactionReceipt, RuntimeServerSpawnReceipt,
    RuntimeServerSpawnReceiptRead,
};
use std::path::{Path, PathBuf};

const SERVER_DIR: &str = "runtime/server";
static READY_LISTENER_SEQUENCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
fn server_dir(home: &Path) -> PathBuf {
    if let Some(publication_dir) = std::env::var_os("ASP_RUNTIME_SERVER_PUBLICATION_DIR") {
        return PathBuf::from(publication_dir).join("lifecycle");
    }
    home.join(SERVER_DIR)
}
fn marker(home: &Path, name: &str) -> PathBuf {
    server_dir(home).join(name)
}

pub struct RuntimeServerActivationReadyListener {
    path: PathBuf,
    socket: tokio::net::UnixDatagram,
}

impl RuntimeServerActivationReadyListener {
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub async fn receive(&mut self) -> Result<RuntimeServerActivationReadyReceipt, String> {
        let mut bytes = vec![0_u8; 16 * 1024];
        let received = self
            .socket
            .recv(&mut bytes)
            .await
            .map_err(|error| format!("receive Runtime activation ready receipt: {error}"))?;
        serde_json::from_slice(&bytes[..received])
            .map_err(|error| format!("decode Runtime activation ready receipt: {error}"))
    }
}

impl Drop for RuntimeServerActivationReadyListener {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

pub async fn bind_activation_ready_listener(
    state_home: &Path,
    activation_generation: u64,
    publication_nonce: &str,
) -> Result<RuntimeServerActivationReadyListener, String> {
    use std::os::unix::ffi::OsStrExt as _;
    use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};

    let canonical_state_home = tokio::fs::canonicalize(state_home)
        .await
        .map_err(|error| format!("canonicalize Runtime activation State Home: {error}"))?;
    let expected_uid = tokio::fs::symlink_metadata(&canonical_state_home)
        .await
        .map_err(|error| format!("inspect Runtime activation State Home: {error}"))?
        .uid();
    let mut identity = blake3::Hasher::new();
    identity.update(b"agent.semantic-protocols.runtime-activation-ready.v1\0");
    identity.update(canonical_state_home.as_os_str().as_bytes());
    identity.update(&activation_generation.to_le_bytes());
    identity.update(publication_nonce.as_bytes());
    identity.update(&std::process::id().to_le_bytes());
    identity.update(
        &READY_LISTENER_SEQUENCE
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            .to_le_bytes(),
    );
    let digest = identity.finalize().to_hex();
    let runtime_base = crate::runtime_server_control::runtime_server_runtime_base(state_home)?;
    let uid = runtime_base
        .parent()
        .and_then(|parent| parent.file_name())
        .and_then(|name| name.to_str())
        .and_then(|name| name.strip_prefix("asp-runtime-server-"))
        .filter(|uid| !uid.is_empty() && uid.bytes().all(|byte| byte.is_ascii_digit()))
        .ok_or_else(|| "Runtime activation ready UID authority is invalid".to_owned())?;
    let uid_root = PathBuf::from("/tmp").join(format!("asp-ar-{uid}"));
    let root = uid_root.join(&digest[..20]);
    for directory in [&uid_root, &root] {
        match tokio::fs::create_dir(directory).await {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => {
                return Err(format!(
                    "create Runtime activation ready directory {}: {error}",
                    directory.display()
                ));
            }
        }
        let metadata = tokio::fs::symlink_metadata(directory)
            .await
            .map_err(|error| {
                format!(
                    "inspect Runtime activation ready directory {}: {error}",
                    directory.display()
                )
            })?;
        if !metadata.file_type().is_dir()
            || metadata.file_type().is_symlink()
            || metadata.uid() != expected_uid
        {
            return Err(format!(
                "Runtime activation ready directory is not owned by the State Home authority: {}",
                directory.display()
            ));
        }
        let mut permissions = metadata.permissions();
        permissions.set_mode(0o700);
        tokio::fs::set_permissions(directory, permissions)
            .await
            .map_err(|error| {
                format!(
                    "protect Runtime activation ready directory {}: {error}",
                    directory.display()
                )
            })?;
    }
    let path = root.join("r.sock");
    const MAX_PORTABLE_UNIX_SOCKET_PATH_BYTES: usize = 100;
    if path.as_os_str().as_bytes().len() > MAX_PORTABLE_UNIX_SOCKET_PATH_BYTES {
        return Err(format!(
            "Runtime activation ready socket exceeds portable sun_path budget: bytes={} path={}",
            path.as_os_str().as_bytes().len(),
            path.display()
        ));
    }
    match tokio::fs::remove_file(&path).await {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(format!(
                "remove stale Runtime activation ready socket {}: {error}",
                path.display()
            ));
        }
    }
    let socket = tokio::net::UnixDatagram::bind(&path).map_err(|error| {
        format!(
            "bind Runtime activation ready socket {}: {error}",
            path.display()
        )
    })?;
    Ok(RuntimeServerActivationReadyListener { path, socket })
}

pub async fn publish_activation_ready(
    path: &Path,
    receipt: &RuntimeServerActivationReadyReceipt,
) -> Result<(), String> {
    let bytes = serde_json::to_vec(receipt)
        .map_err(|error| format!("encode Runtime activation ready receipt: {error}"))?;
    let socket = tokio::net::UnixDatagram::unbound()
        .map_err(|error| format!("create Runtime activation ready sender: {error}"))?;
    socket
        .send_to(&bytes, path)
        .await
        .map_err(|error| format!("publish Runtime activation ready receipt: {error}"))?;
    Ok(())
}

pub fn spawn_receipt_digest(
    receipt: &RuntimeServerSpawnReceipt,
) -> Result<agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest, String> {
    let bytes = serde_json::to_vec(receipt)
        .map_err(|error| format!("encode Runtime owner-spawn receipt identity: {error}"))?;
    Ok(agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest::from_bytes(&bytes))
}

async fn atomic_empty(path: &Path) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "lifecycle marker has no parent".to_owned())?;
    tokio::fs::create_dir_all(parent)
        .await
        .map_err(|e| e.to_string())?;
    let staged = path.with_extension(format!(
        "stage-{}",
        agent_semantic_runtime::runtime_process_lifecycle::current_process_id()
    ));
    let file = tokio::fs::File::create(&staged)
        .await
        .map_err(|e| e.to_string())?;
    file.sync_all().await.map_err(|e| e.to_string())?;
    drop(file);
    tokio::fs::rename(staged, path)
        .await
        .map_err(|e| e.to_string())
}

pub async fn create_run_intent(home: &Path) -> Result<(), String> {
    atomic_empty(&marker(home, "run-intent.v1")).await
}
pub async fn remove_run_intent(home: &Path) -> Result<(), String> {
    remove_if_present(&marker(home, "run-intent.v1")).await
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeServerOperatorStopReceipt {
    pub schema_id: String,
    pub schema_version: String,
    pub state: String,
    pub stopped_through_activation_generation: u64,
}

pub async fn mark_operator_stopped(home: &Path) -> Result<(), String> {
    let _mutation_guard =
        agent_semantic_artifacts::runtime_artifact_retention::RuntimeArtifactMutationGuard::try_acquire(
            &home.join("runtime/artifacts"),
        )?;
    let path = marker(home, "operator-stop.v1.json");
    let parent = path
        .parent()
        .ok_or_else(|| "operator marker has no parent".to_owned())?;
    tokio::fs::create_dir_all(parent)
        .await
        .map_err(|e| e.to_string())?;
    let staged = path.with_extension(format!(
        "stage-{}",
        agent_semantic_runtime::runtime_process_lifecycle::current_process_id()
    ));
    let bytes = serde_json::to_vec(&RuntimeServerOperatorStopReceipt {
        schema_id: "agent.semantic-protocols.runtime-server-operator-stop".to_owned(),
        schema_version: "1".to_owned(),
        state: "stopped".to_owned(),
        stopped_through_activation_generation:
            agent_semantic_artifacts::runtime_artifact_publication::current_runtime_artifact_activation_generation(home)
                .await?,
    })
    .map_err(|e| e.to_string())?;
    tokio::fs::write(&staged, bytes)
        .await
        .map_err(|e| e.to_string())?;
    tokio::fs::rename(staged, path)
        .await
        .map_err(|e| e.to_string())
}

pub async fn clear_operator_stopped(home: &Path) -> Result<(), String> {
    let _mutation_guard =
        agent_semantic_artifacts::runtime_artifact_retention::RuntimeArtifactMutationGuard::try_acquire(
            &home.join("runtime/artifacts"),
        )?;
    remove_if_present(&marker(home, "operator-stop.v1.json")).await
}
pub async fn operator_stopped(home: &Path) -> Result<bool, String> {
    Ok(read_operator_stop_receipt(home).await?.is_some())
}

pub async fn read_operator_stop_receipt(
    home: &Path,
) -> Result<Option<RuntimeServerOperatorStopReceipt>, String> {
    read_operator_stop_receipt_locked(home).await
}

async fn read_operator_stop_receipt_locked(
    home: &Path,
) -> Result<Option<RuntimeServerOperatorStopReceipt>, String> {
    let path = marker(home, "operator-stop.v1.json");
    let bytes = match tokio::fs::read(&path).await {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.to_string()),
    };
    let value = serde_json::from_slice::<serde_json::Value>(&bytes)
        .map_err(|error| format!("invalid Runtime Server operator-stop receipt: {error}"))?;
    let object = value
        .as_object()
        .ok_or_else(|| "invalid Runtime Server operator-stop receipt shape".to_owned())?;
    if object.get("schemaId").and_then(serde_json::Value::as_str)
        != Some("agent.semantic-protocols.runtime-server-operator-stop")
        || object.get("state").and_then(serde_json::Value::as_str) != Some("stopped")
    {
        return Err("invalid Runtime Server operator-stop receipt identity".to_owned());
    }
    match object
        .get("schemaVersion")
        .and_then(serde_json::Value::as_str)
    {
        Some("1") if object.contains_key("stoppedThroughActivationGeneration") => {}
        Some("1") => {
            return Err(
                "Runtime Server operator-stop v1 receipt is missing stoppedThroughActivationGeneration"
                    .to_owned(),
            );
        }
        Some("2") => {
            return Err(
                "unreleased Runtime Server operator-stop schemaVersion=2 is not current".to_owned(),
            );
        }
        Some(version) => {
            return Err(format!(
                "unsupported Runtime Server operator-stop schemaVersion={version}"
            ));
        }
        None => {
            return Err("Runtime Server operator-stop receipt is missing schemaVersion".to_owned());
        }
    }
    let receipt = serde_json::from_value::<RuntimeServerOperatorStopReceipt>(value)
        .map_err(|error| format!("invalid Runtime Server operator-stop receipt: {error}"))?;
    if receipt.schema_id != "agent.semantic-protocols.runtime-server-operator-stop"
        || receipt.schema_version != "1"
        || receipt.state != "stopped"
    {
        return Err("invalid Runtime Server operator-stop receipt identity".to_owned());
    }
    Ok(Some(receipt))
}

pub async fn admit_activation_after_operator_stop(
    home: &Path,
    activation_generation: u64,
) -> Result<bool, String> {
    if !tokio::fs::try_exists(marker(home, "operator-stop.v1.json"))
        .await
        .map_err(|error| error.to_string())?
    {
        return Ok(true);
    }
    let _mutation_guard =
        agent_semantic_artifacts::runtime_artifact_retention::RuntimeArtifactMutationGuard::try_acquire(
            &home.join("runtime/artifacts"),
        )?;
    let Some(receipt) = read_operator_stop_receipt_locked(home).await? else {
        return Ok(true);
    };
    if activation_generation <= receipt.stopped_through_activation_generation {
        return Ok(false);
    }
    remove_if_present(&marker(home, "operator-stop.v1.json")).await?;
    Ok(true)
}

fn owner_receipt(home: &Path) -> PathBuf {
    marker(home, "owner-spawn.v1.json")
}
pub async fn read_owner_receipt(home: &Path) -> Result<Option<RuntimeServerSpawnReceipt>, String> {
    match read_owner_receipt_state(home).await? {
        Some(RuntimeServerSpawnReceiptRead::Current(receipt)) => Ok(Some(receipt)),
        Some(RuntimeServerSpawnReceiptRead::Stale(stale)) => Err(serde_json::json!({
            "schemaId": "agent.semantic-protocols.runtime-authority-observation",
            "schemaVersion": "1",
            "state": "stale",
            "reasonKind": "runtime-authority-stale",
            "observedSchemaId": stale.schema_id,
            "observedSchemaVersion": stale.schema_version,
            "observationReasonKind": stale.reason_kind,
            "recommendedNext": "publish-validated-runtime-activation",
        })
        .to_string()),
        None => Ok(None),
    }
}

pub async fn read_owner_receipt_state(
    home: &Path,
) -> Result<Option<RuntimeServerSpawnReceiptRead>, String> {
    match tokio::fs::read(owner_receipt(home)).await {
        Ok(bytes) => {
            crate::runtime_server_owner_receipt::decode_runtime_server_spawn_receipt(&bytes)
                .map(Some)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}
pub async fn write_owner_receipt(
    home: &Path,
    receipt: &RuntimeServerSpawnReceipt,
) -> Result<(), String> {
    let path = owner_receipt(home);
    let parent = path
        .parent()
        .ok_or_else(|| "owner receipt has no parent".to_owned())?;
    tokio::fs::create_dir_all(parent)
        .await
        .map_err(|e| e.to_string())?;
    let staged = path.with_extension(format!(
        "stage-{}",
        agent_semantic_runtime::runtime_process_lifecycle::current_process_id()
    ));
    tokio::fs::write(
        &staged,
        serde_json::to_vec(receipt).map_err(|e| e.to_string())?,
    )
    .await
    .map_err(|e| e.to_string())?;
    tokio::fs::rename(staged, path)
        .await
        .map_err(|e| e.to_string())
}
pub async fn remove_owner_receipt(home: &Path) -> Result<(), String> {
    remove_if_present(&owner_receipt(home)).await
}

fn exit_receipt(home: &Path) -> PathBuf {
    marker(home, "daemon-exit.v1.json")
}
fn drain_receipt(home: &Path) -> PathBuf {
    marker(home, "daemon-drain.v1.json")
}
pub async fn publish_drain(home: &Path, receipt: RuntimeServerDrainReceipt) -> Result<(), String> {
    let path = drain_receipt(home);
    let parent = path
        .parent()
        .ok_or_else(|| "drain receipt has no parent".to_owned())?;
    tokio::fs::create_dir_all(parent)
        .await
        .map_err(|e| e.to_string())?;
    let staged = path.with_extension(format!(
        "stage-{}",
        agent_semantic_runtime::runtime_process_lifecycle::current_process_id()
    ));
    tokio::fs::write(
        &staged,
        serde_json::to_vec(&receipt).map_err(|e| e.to_string())?,
    )
    .await
    .map_err(|e| e.to_string())?;
    tokio::fs::rename(staged, path)
        .await
        .map_err(|e| e.to_string())
}

pub async fn read_latest_drain(home: &Path) -> Result<Option<RuntimeServerDrainReceipt>, String> {
    match tokio::fs::read(drain_receipt(home)).await {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(|error| format!("invalid Runtime Server drain receipt: {error}")),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.to_string()),
    }
}

pub async fn observe_resident_transaction(
    home: &Path,
) -> Result<RuntimeServerResidentTransactionReceipt, String> {
    let spawn = read_owner_receipt(home)
        .await?
        .ok_or_else(|| "Runtime resident transaction requires an owner-spawn receipt".to_owned())?;
    let applied = agent_semantic_artifacts::runtime_artifact_publication::
        read_applied_runtime_artifact_activation_event(home)
        .await?
        .ok_or_else(|| "Runtime resident transaction requires an applied activation".to_owned())?;
    let endpoint = crate::runtime_server_control::read_runtime_server_supervisor_endpoint(home)
        .await?
        .ok_or_else(|| "Runtime resident transaction requires a published endpoint".to_owned())?;
    endpoint.validate_service_reachability().await?;

    if spawn.activation_generation != applied.activation_generation
        || spawn.launcher_artifact_digest != applied.artifact_digest
        || spawn.launcher_artifact_path != applied.artifact_path.display().to_string()
    {
        return Err(
            "Runtime resident transaction launcher and applied activation identities differ"
                .to_owned(),
        );
    }
    if spawn.process_id != endpoint.owner_process_id
        || spawn.launcher_artifact_path != endpoint.runtime_artifact_path
    {
        return Err(
            "Runtime resident transaction launcher and endpoint owner identities differ".to_owned(),
        );
    }
    let endpoint_binary_content_digest =
        agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest::parse(
            &endpoint.binary_content_digest,
        )
        .map_err(|error| format!("invalid Runtime endpoint binary content digest: {error}"))?;
    if endpoint_binary_content_digest != applied.artifact_digest {
        return Err(
            "Runtime resident transaction applied and endpoint artifact digests differ".to_owned(),
        );
    }
    if spawn.previous_serving_digest != applied.previous_artifact_digest {
        return Err("Runtime resident transaction previous serving identities differ".to_owned());
    }

    let previous_drain_state = match spawn.previous_owner_epoch {
        Some(previous_owner_epoch) => {
            let drain = read_latest_drain(home).await?.ok_or_else(|| {
                format!(
                    "Runtime resident transaction is missing drain receipt for previous owner epoch {previous_owner_epoch}"
                )
            })?;
            if drain.owner_epoch != previous_owner_epoch || !drain.clean_drain {
                return Err(format!(
                    "Runtime resident transaction previous owner did not drain cleanly: expectedEpoch={previous_owner_epoch} observedEpoch={} cleanDrain={}",
                    drain.owner_epoch, drain.clean_drain
                ));
            }
            "clean"
        }
        None if spawn.previous_serving_digest.is_some() => "not-running",
        None => "not-required",
    };

    Ok(RuntimeServerResidentTransactionReceipt {
        schema_id: "agent.semantic-protocols.runtime-server-resident-transaction-receipt"
            .to_owned(),
        schema_version: "1".to_owned(),
        state: "ready".to_owned(),
        activation_generation: spawn.activation_generation,
        launcher_artifact_path: spawn.launcher_artifact_path,
        launcher_artifact_digest: spawn.launcher_artifact_digest,
        spawn_argv: spawn.spawn_argv,
        applied_artifact_digest: applied.artifact_digest,
        applied_activation_generation: applied.activation_generation,
        endpoint_owner_epoch: endpoint.owner_epoch,
        endpoint_binary_content_digest,
        endpoint_runtime_generation_digest: endpoint.runtime_generation_digest,
        control_endpoint: endpoint.socket_path,
        data_endpoint: endpoint.data_plane_socket_path,
        provider_endpoint: endpoint.provider_plane_socket_path,
        previous_serving_digest: applied.previous_artifact_digest,
        previous_owner_epoch: spawn.previous_owner_epoch,
        previous_drain_state: previous_drain_state.to_owned(),
    })
}
pub async fn publish_with_errors(
    home: &Path,
    owner_epoch: u64,
    clean_drain: bool,
    errors: Vec<String>,
) -> Result<(), String> {
    let receipt = RuntimeServerExitReceipt {
        schema_id: "agent.semantic-protocols.runtime-server-daemon-exit".to_owned(),
        schema_version: "1".to_owned(),
        owner_epoch,
        clean_drain,
        errors,
    };
    let path = exit_receipt(home);
    let parent = path
        .parent()
        .ok_or_else(|| "exit receipt has no parent".to_owned())?;
    tokio::fs::create_dir_all(parent)
        .await
        .map_err(|e| e.to_string())?;
    let staged = path.with_extension(format!(
        "stage-{}",
        agent_semantic_runtime::runtime_process_lifecycle::current_process_id()
    ));
    tokio::fs::write(
        &staged,
        serde_json::to_vec(&receipt).map_err(|e| e.to_string())?,
    )
    .await
    .map_err(|e| e.to_string())?;
    tokio::fs::rename(staged, path)
        .await
        .map_err(|e| e.to_string())
}
pub async fn read_latest_owner_exit(
    home: &Path,
) -> Result<Option<RuntimeServerExitReceipt>, String> {
    match tokio::fs::read(exit_receipt(home)).await {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(|e| e.to_string()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

/// Read the fixed exit marker only when it belongs to the currently expected owner.
/// A stale marker is evidence, not a terminal result for a new owner.
pub async fn read_owner_exit_for(
    home: &Path,
    expected_owner_epoch: u64,
) -> Result<Option<RuntimeServerExitReceipt>, String> {
    match read_latest_owner_exit(home).await? {
        Some(receipt) if receipt.owner_epoch == expected_owner_epoch => Ok(Some(receipt)),
        Some(receipt) => {
            eprintln!(
                "{}",
                serde_json::json!({
                    "schemaId": "agent.semantic-protocols.runtime-server-daemon-exit-stale",
                    "schemaVersion": "1",
                    "state": "ignored",
                    "expectedOwnerEpoch": expected_owner_epoch,
                    "observedOwnerEpoch": receipt.owner_epoch,
                    "reasonKind": "stale-owner-exit-receipt",
                })
            );
            Ok(None)
        }
        None => Ok(None),
    }
}
pub async fn remove_stale(home: &Path) -> Result<(), String> {
    remove_if_present(&exit_receipt(home)).await
}
pub async fn await_owner_exit(
    home: &Path,
    owner_epoch: u64,
) -> Result<RuntimeServerExitReceipt, String> {
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        if let Some(receipt) = read_owner_exit_for(home, owner_epoch).await? {
            return Ok(receipt);
        }
        if tokio::time::Instant::now() >= deadline {
            return Err("Runtime Server owner exit receipt timeout".to_owned());
        }
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
}

async fn remove_if_present(path: &Path) -> Result<(), String> {
    match tokio::fs::remove_file(path).await {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}
