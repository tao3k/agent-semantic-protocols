use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};

use fs4::AsyncFileExt;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;

const SCHEMA_VERSION: &str = "1";
const ENDPOINT_SCHEMA_ID: &str = "agent.semantic-protocols.global-resident-endpoint.v1";
const REQUEST_SCHEMA_ID: &str = "agent.semantic-protocols.global-resident-control-request.v1";
const RECEIPT_SCHEMA_ID: &str = "agent.semantic-protocols.global-resident-control-receipt.v1";
const MAX_FRAME_BYTES: usize = 1024 * 1024;
const MAX_UNIX_SOCKET_PATH_BYTES: usize = 103;

unsafe extern "C" {
    fn getuid() -> u32;
}

pub struct GlobalResidentElection {
    _file: tokio::fs::File,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GlobalResidentEndpoint {
    pub schema_id: String,
    pub schema_version: String,
    pub transport_contract_digest: String,
    pub owner_epoch: u64,
    pub runtime_artifact_path: String,
    pub runtime_artifact_digest: String,
    pub binding_token: String,
    pub socket_path: String,
}

impl GlobalResidentEndpoint {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_id != ENDPOINT_SCHEMA_ID || self.schema_version != SCHEMA_VERSION {
            return Err("global resident endpoint schema identity mismatch".to_owned());
        }
        let expected = global_resident_transport_contract_digest();
        if self.transport_contract_digest != expected {
            return Err(format!(
                "global resident transport contract mismatch: expected={expected} actual={}",
                self.transport_contract_digest
            ));
        }
        if self.owner_epoch == 0
            || self.runtime_artifact_path.is_empty()
            || self.runtime_artifact_digest.is_empty()
            || self.binding_token.is_empty()
            || self.socket_path.is_empty()
        {
            return Err("global resident endpoint is incomplete".to_owned());
        }
        if !Path::new(&self.socket_path).is_absolute() {
            return Err("global resident socket path must be absolute".to_owned());
        }
        Ok(())
    }
}

impl GlobalResidentControlRequest {
    pub fn validate_for_endpoint(&self, endpoint: &GlobalResidentEndpoint) -> Result<(), String> {
        if self.schema_id != REQUEST_SCHEMA_ID || self.schema_version != SCHEMA_VERSION {
            return Err("global resident control request schema identity mismatch".to_owned());
        }
        if self.request_id.is_empty()
            || self.transport_contract_digest != endpoint.transport_contract_digest
            || self.owner_epoch != endpoint.owner_epoch
            || self.binding_token != endpoint.binding_token
        {
            return Err("global resident control request binding mismatch".to_owned());
        }
        Ok(())
    }
}

impl GlobalResidentControlReceipt {
    pub fn healthy(
        request_id: String,
        endpoint: &GlobalResidentEndpoint,
        workspace_entry_count: usize,
    ) -> Self {
        Self {
            schema_id: RECEIPT_SCHEMA_ID.to_owned(),
            schema_version: SCHEMA_VERSION.to_owned(),
            request_id,
            state: GlobalResidentState::Healthy,
            runtime_artifact_digest: endpoint.runtime_artifact_digest.clone(),
            transport_contract_digest: endpoint.transport_contract_digest.clone(),
            workspace_entry_count,
            reason: None,
        }
    }

    pub fn draining(
        request_id: String,
        endpoint: &GlobalResidentEndpoint,
        workspace_entry_count: usize,
    ) -> Self {
        Self {
            state: GlobalResidentState::Draining,
            ..Self::healthy(request_id, endpoint, workspace_entry_count)
        }
    }

    pub fn starting(request_id: String, runtime_artifact_digest: String, reason: String) -> Self {
        Self {
            schema_id: RECEIPT_SCHEMA_ID.to_owned(),
            schema_version: SCHEMA_VERSION.to_owned(),
            request_id,
            state: GlobalResidentState::Starting,
            runtime_artifact_digest,
            transport_contract_digest: global_resident_transport_contract_digest(),
            workspace_entry_count: 0,
            reason: Some(reason),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GlobalResidentOperation {
    Status,
    Reconcile,
    Restart,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GlobalResidentControlRequest {
    pub schema_id: String,
    pub schema_version: String,
    pub operation: GlobalResidentOperation,
    pub request_id: String,
    pub transport_contract_digest: String,
    pub owner_epoch: u64,
    pub binding_token: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GlobalResidentState {
    Healthy,
    Starting,
    Draining,
    Degraded,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GlobalResidentControlReceipt {
    pub schema_id: String,
    pub schema_version: String,
    pub request_id: String,
    pub state: GlobalResidentState,
    pub runtime_artifact_digest: String,
    pub transport_contract_digest: String,
    pub workspace_entry_count: usize,
    pub reason: Option<String>,
}

pub fn global_resident_runtime_base() -> PathBuf {
    PathBuf::from("/tmp").join(format!("asp-global-{}", unsafe { getuid() }))
}

pub fn global_resident_endpoint_path(state_home: &Path) -> PathBuf {
    state_home
        .join("runtime")
        .join("state")
        .join("global-resident-endpoint.v1.json")
}

pub async fn acquire_global_resident_election() -> Result<GlobalResidentElection, String> {
    let runtime_base = global_resident_runtime_base();
    tokio::fs::create_dir_all(&runtime_base)
        .await
        .map_err(|error| {
            format!(
                "failed to create global resident runtime directory {}: {error}",
                runtime_base.display()
            )
        })?;
    let lock_path = runtime_base.join("global-resident.owner.lock");
    let file = tokio::fs::OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .open(&lock_path)
        .await
        .map_err(|error| {
            format!(
                "failed to open global resident election lock {}: {error}",
                lock_path.display()
            )
        })?;
    file.try_lock().map_err(|error| {
        format!(
            "global resident election is already held or unavailable at {}: {error}",
            lock_path.display()
        )
    })?;
    Ok(GlobalResidentElection { _file: file })
}

pub async fn prepare_global_resident_endpoint(
    runtime_artifact_path: &Path,
    runtime_artifact_digest: &str,
    owner_epoch: u64,
    binding_token: &str,
) -> Result<GlobalResidentEndpoint, String> {
    let runtime_base = global_resident_runtime_base();
    tokio::fs::create_dir_all(&runtime_base)
        .await
        .map_err(|error| {
            format!(
                "failed to create global resident runtime directory {}: {error}",
                runtime_base.display()
            )
        })?;
    let mut permissions = tokio::fs::metadata(&runtime_base)
        .await
        .map_err(|error| format!("failed to inspect global resident runtime directory: {error}"))?
        .permissions();
    permissions.set_mode(0o700);
    tokio::fs::set_permissions(&runtime_base, permissions)
        .await
        .map_err(|error| format!("failed to protect global resident runtime directory: {error}"))?;
    let metadata = tokio::fs::metadata(&runtime_base)
        .await
        .map_err(|error| format!("failed to inspect global resident runtime directory: {error}"))?;
    if metadata.uid() != unsafe { getuid() } || metadata.mode() & 0o777 != 0o700 {
        return Err(
            "global resident runtime directory is not private to the current UID".to_owned(),
        );
    }
    let digest = blake3::hash(
        format!("{owner_epoch}\0{binding_token}\0{runtime_artifact_digest}").as_bytes(),
    )
    .to_hex();
    let socket_path = runtime_base.join(format!("r-{}.sock", &digest[..16]));
    if socket_path.as_os_str().as_bytes().len() > MAX_UNIX_SOCKET_PATH_BYTES {
        return Err(format!(
            "global resident socket path exceeds Unix sun_path budget: {}",
            socket_path.display()
        ));
    }
    Ok(GlobalResidentEndpoint {
        schema_id: ENDPOINT_SCHEMA_ID.to_owned(),
        schema_version: SCHEMA_VERSION.to_owned(),
        transport_contract_digest: global_resident_transport_contract_digest(),
        owner_epoch,
        runtime_artifact_path: runtime_artifact_path.to_string_lossy().into_owned(),
        runtime_artifact_digest: runtime_artifact_digest.to_owned(),
        binding_token: binding_token.to_owned(),
        socket_path: socket_path.to_string_lossy().into_owned(),
    })
}

pub async fn call_global_resident(
    endpoint: &GlobalResidentEndpoint,
    operation: GlobalResidentOperation,
    request_id: String,
) -> Result<GlobalResidentControlReceipt, String> {
    endpoint.validate()?;
    let mut stream = UnixStream::connect(&endpoint.socket_path)
        .await
        .map_err(|error| format!("failed to connect global resident endpoint: {error}"))?;
    let request = GlobalResidentControlRequest {
        schema_id: REQUEST_SCHEMA_ID.to_owned(),
        schema_version: SCHEMA_VERSION.to_owned(),
        operation,
        request_id: request_id.clone(),
        transport_contract_digest: endpoint.transport_contract_digest.clone(),
        owner_epoch: endpoint.owner_epoch,
        binding_token: endpoint.binding_token.clone(),
    };
    write_frame(&mut stream, &request).await?;
    let receipt: GlobalResidentControlReceipt = read_frame(&mut stream).await?;
    if receipt.schema_id != RECEIPT_SCHEMA_ID
        || receipt.schema_version != SCHEMA_VERSION
        || receipt.request_id != request_id
        || receipt.transport_contract_digest != endpoint.transport_contract_digest
    {
        return Err("global resident control receipt identity mismatch".to_owned());
    }
    Ok(receipt)
}

pub fn global_resident_transport_contract_digest() -> String {
    format!(
        "blake3-256:{}",
        blake3::hash(include_bytes!(
            "../../../schemas/global-resident-control.v1.schema.json"
        ))
        .to_hex()
    )
}

pub async fn read_global_resident_request(
    stream: &mut UnixStream,
) -> Result<GlobalResidentControlRequest, String> {
    read_frame(stream).await
}

pub async fn write_global_resident_receipt(
    stream: &mut UnixStream,
    receipt: &GlobalResidentControlReceipt,
) -> Result<(), String> {
    write_frame(stream, receipt).await
}

async fn write_frame<T: Serialize>(stream: &mut UnixStream, value: &T) -> Result<(), String> {
    let bytes = serde_json::to_vec(value)
        .map_err(|error| format!("failed to encode global resident frame: {error}"))?;
    if bytes.len() > MAX_FRAME_BYTES {
        return Err("global resident frame exceeds size limit".to_owned());
    }
    stream
        .write_u32(
            bytes
                .len()
                .try_into()
                .map_err(|_| "frame length overflow")?,
        )
        .await
        .map_err(|error| format!("failed to write global resident frame length: {error}"))?;
    stream
        .write_all(&bytes)
        .await
        .map_err(|error| format!("failed to write global resident frame: {error}"))
}

async fn read_frame<T: for<'de> Deserialize<'de>>(stream: &mut UnixStream) -> Result<T, String> {
    let length = stream
        .read_u32()
        .await
        .map_err(|error| format!("failed to read global resident frame length: {error}"))?
        as usize;
    if length > MAX_FRAME_BYTES {
        return Err("global resident frame exceeds size limit".to_owned());
    }
    let mut bytes = vec![0; length];
    stream
        .read_exact(&mut bytes)
        .await
        .map_err(|error| format!("failed to read global resident frame: {error}"))?;
    serde_json::from_slice(&bytes)
        .map_err(|error| format!("failed to decode global resident frame: {error}"))
}
