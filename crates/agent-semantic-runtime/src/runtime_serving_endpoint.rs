//! Read-only, content-proven Runtime serving endpoint resolution.
//!
//! This module is deliberately below the DB/daemon plane: a normal client may
//! read immutable lifecycle publications, but it never opens Turso, starts a
//! Runtime, or probes a socket while resolving the publication.

use std::path::Path;
use std::path::PathBuf;

use agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest;
use agent_semantic_artifacts::runtime_artifact_activation::RuntimeArtifactActivationEvent;
use agent_semantic_artifacts::runtime_artifact_activation::read_applied_runtime_artifact_activation_event;
use agent_semantic_artifacts::runtime_artifact_catalog::RuntimeBinaryIdentity;
use serde::Deserialize;

const ENDPOINT_SCHEMA_ID: &str = "agent.semantic-protocols.runtime-server-endpoint";
const OWNER_SCHEMA_ID: &str = "agent.semantic-protocols.runtime-server-owner-spawn.v1";
const SCHEMA_VERSION: &str = "1";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeServingEndpoint {
    pub data_socket_addr: std::net::SocketAddr,
    pub publication_nonce: String,
    pub artifact_digest: Blake3ContentDigest,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct OwnerSpawnReceipt {
    schema_id: String,
    schema_version: String,
    process_id: u32,
    nonce: String,
    state_home: String,
    publication_nonce: String,
    launcher_artifact_path: String,
    launcher_artifact_digest: Blake3ContentDigest,
    spawn_argv: Vec<String>,
    previous_serving_digest: Option<Blake3ContentDigest>,
    previous_owner_epoch: Option<u64>,
}

#[derive(Clone, Copy, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
enum LoopbackTransport {
    LoopbackTcp,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LoopbackEndpoint {
    transport: LoopbackTransport,
    address: std::net::IpAddr,
    port: u16,
}

impl LoopbackEndpoint {
    fn socket_addr(&self) -> Result<std::net::SocketAddr, String> {
        if self.transport != LoopbackTransport::LoopbackTcp
            || !self.address.is_loopback()
            || self.port == 0
        {
            return Err(
                "Runtime serving endpoint is not a nonzero loopback TCP binding".to_owned(),
            );
        }
        Ok(std::net::SocketAddr::new(self.address, self.port))
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct EndpointReceipt {
    schema_id: String,
    schema_version: String,
    binary_content_digest: Blake3ContentDigest,
    runtime_generation_digest: String,
    schema_digest: String,
    transport_contract_digest: String,
    owner_epoch: u64,
    owner_process_id: u32,
    runtime_artifact_path: String,
    runtime_binary_identity: RuntimeBinaryIdentity,
    monitor_capability: bool,
    observed_runtime_binary_identity: RuntimeBinaryIdentity,
    artifact_mode: String,
    artifact_catalog_digest: String,
    binding_token: String,
    control_endpoint: LoopbackEndpoint,
    data_endpoint: LoopbackEndpoint,
    provider_endpoint: LoopbackEndpoint,
    workspace_store_path: String,
    status_memory_path: String,
}

fn server_dir(state_home: &Path) -> PathBuf {
    std::env::var_os("ASP_RUNTIME_SERVER_PUBLICATION_DIR")
        .map(PathBuf::from)
        .map(|path| path.join("lifecycle"))
        .unwrap_or_else(|| state_home.join("runtime/server"))
}

fn endpoint_path(state_home: &Path) -> Result<PathBuf, String> {
    if let Some(directory) = std::env::var_os("ASP_RUNTIME_SERVER_PUBLICATION_DIR") {
        return Ok(PathBuf::from(directory).join("endpoint.json"));
    }
    let resident = state_home.join("runtime/resident/current/endpoint.json");
    if std::fs::symlink_metadata(&resident).is_ok() {
        return Ok(resident);
    }
    let canonical = std::fs::canonicalize(state_home).map_err(|error| {
        format!(
            "canonicalize ASP State Home {}: {error}",
            state_home.display()
        )
    })?;
    #[cfg(unix)]
    use std::os::unix::ffi::OsStrExt as _;
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"agent.semantic-protocols.runtime-server-state-home.v1\0");
    #[cfg(unix)]
    hasher.update(canonical.as_os_str().as_bytes());
    #[cfg(not(unix))]
    hasher.update(canonical.to_string_lossy().as_bytes());
    let identity = hasher.finalize().to_hex();
    #[cfg(unix)]
    let uid = unsafe { libc::getuid() };
    #[cfg(not(unix))]
    let uid = 0;
    Ok(std::env::temp_dir()
        .join(format!("asp-runtime-server-{uid}"))
        .join(format!("state-{}", &identity[..24]))
        .join("endpoint.v1.json"))
}

/// Resolves a serving endpoint from the same immutable owner, activation, and
/// endpoint facts used by the daemon. This performs no connection attempt.
pub async fn resolve_runtime_serving_endpoint(
    state_home: &Path,
) -> Result<RuntimeServingEndpoint, String> {
    let owner_path = server_dir(state_home).join("owner-spawn.v1.json");
    let owner_bytes = tokio::fs::read(&owner_path).await.map_err(|error| {
        format!(
            "Runtime serving endpoint requires owner-spawn receipt {}: {error}",
            owner_path.display()
        )
    })?;
    let owner: OwnerSpawnReceipt = serde_json::from_slice(&owner_bytes)
        .map_err(|error| format!("decode Runtime owner-spawn receipt: {error}"))?;
    if owner.schema_id != OWNER_SCHEMA_ID
        || owner.schema_version != SCHEMA_VERSION
        || owner.process_id == 0
        || owner.nonce.is_empty()
        || owner.state_home != state_home.display().to_string()
        || owner.publication_nonce.is_empty()
        || !Path::new(&owner.launcher_artifact_path).is_absolute()
        || owner.spawn_argv.is_empty()
    {
        return Err(
            "reasonKind=runtime-authority-stale Runtime owner-spawn receipt is not current"
                .to_owned(),
        );
    }
    let applied = read_applied_runtime_artifact_activation_event(state_home)
        .await?
        .ok_or_else(|| "Runtime serving endpoint requires an applied activation".to_owned())?;
    let endpoint_path = endpoint_path(state_home)?;
    let endpoint_bytes = tokio::fs::read(&endpoint_path).await.map_err(|error| {
        format!(
            "Runtime serving endpoint requires endpoint {}: {error}",
            endpoint_path.display()
        )
    })?;
    let endpoint: EndpointReceipt = serde_json::from_slice(&endpoint_bytes)
        .map_err(|error| format!("decode Runtime serving endpoint: {error}"))?;
    validate_content_binding(&owner, &applied, &endpoint)?;
    Ok(RuntimeServingEndpoint {
        data_socket_addr: endpoint.data_endpoint.socket_addr()?,
        publication_nonce: owner.publication_nonce,
        artifact_digest: endpoint.binary_content_digest,
    })
}

fn validate_content_binding(
    owner: &OwnerSpawnReceipt,
    applied: &RuntimeArtifactActivationEvent,
    endpoint: &EndpointReceipt,
) -> Result<(), String> {
    let _previous_owner_epoch = owner.previous_owner_epoch;
    let _monitor_capability = endpoint.monitor_capability;
    if endpoint.schema_id != ENDPOINT_SCHEMA_ID
        || endpoint.schema_version != SCHEMA_VERSION
        || endpoint.owner_process_id != owner.process_id
        || endpoint.runtime_artifact_path != owner.launcher_artifact_path
        || endpoint.binary_content_digest != owner.launcher_artifact_digest
        || endpoint.binary_content_digest != applied.artifact_digest
        || owner.publication_nonce != applied.publication_nonce
        || owner.previous_serving_digest != applied.previous_artifact_digest
        || endpoint.runtime_binary_identity.content_digest() != &endpoint.binary_content_digest
        || endpoint.observed_runtime_binary_identity.content_digest()
            != &endpoint.binary_content_digest
        || endpoint.owner_epoch == 0
        || endpoint.runtime_generation_digest.is_empty()
        || endpoint.schema_digest.is_empty()
        || endpoint.transport_contract_digest.is_empty()
        || endpoint.artifact_catalog_digest.is_empty()
        || endpoint.binding_token.is_empty()
        || !matches!(endpoint.artifact_mode.as_str(), "dev" | "release")
        || endpoint.workspace_store_path.is_empty()
        || endpoint.status_memory_path.is_empty()
    {
        return Err("reasonKind=runtime-serving-identity-invalid Runtime owner, activation, and endpoint do not name one serving artifact".to_owned());
    }
    endpoint.control_endpoint.socket_addr()?;
    endpoint.provider_endpoint.socket_addr()?;
    Ok(())
}
