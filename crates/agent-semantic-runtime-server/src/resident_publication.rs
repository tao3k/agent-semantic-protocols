use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use agent_semantic_client_db::runtime_server_control::RuntimeServerEndpoint;
use agent_semantic_client_protocol::runtime_generation::RuntimeServerGenerationIdentity;
use serde::Deserialize;
use serde::Serialize;
use tokio::sync::Notify;

use crate::readiness::RuntimeServerReadinessReceipt;
use crate::readiness::RuntimeServerReadinessState;

pub const RESIDENT_PUBLICATION_SCHEMA_ID: &str =
    "agent.semantic-protocols.runtime-server-publication";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResidentPublicationReceipt {
    pub schema_id: String,
    pub schema_version: String,
    pub state: String,
    pub identity: RuntimeServerGenerationIdentity,
    pub publication_dir: PathBuf,
    pub endpoint_path: PathBuf,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResidentCandidateReadyReceipt {
    pub readiness: RuntimeServerReadinessReceipt,
    pub identity: RuntimeServerGenerationIdentity,
}

impl ResidentPublicationReceipt {
    fn validate(&self) -> Result<(), String> {
        if self.schema_id != RESIDENT_PUBLICATION_SCHEMA_ID || self.schema_version != "1" {
            return Err("invalid Runtime Server resident publication schema".to_owned());
        }
        if self.state != "candidate" && self.state != "active" && self.state != "healthy" {
            return Err(format!(
                "invalid Runtime Server resident publication state `{}`",
                self.state
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct ResidentPublicationPaths {
    root: PathBuf,
}

impl ResidentPublicationPaths {
    /// Create paths rooted at the Runtime-owned resident publication store.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// Return the immutable publication namespace.
    pub fn publications(&self) -> PathBuf {
        self.root.join("publications")
    }

    /// Return the candidate path for one content-proven Runtime generation.
    pub fn candidate(&self, generation_digest: &str) -> PathBuf {
        self.publications().join(generation_digest)
    }
}

#[derive(Clone, Debug)]
pub struct AtomicResidentPublisher {
    paths: ResidentPublicationPaths,
    slots: agent_semantic_artifacts::runtime_artifact_slots::RuntimeArtifactSlotAuthority,
}

impl AtomicResidentPublisher {
    /// Create the sole publisher for one Runtime-owned resident store.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        let root = root.into();
        Self {
            paths: ResidentPublicationPaths::new(root.clone()),
            slots:
                agent_semantic_artifacts::runtime_artifact_slots::RuntimeArtifactSlotAuthority::new(
                    root,
                ),
        }
    }

    /// Remove candidate publications unreachable from active or healthy slots.
    pub async fn prune_unreachable_publications(&self) -> Result<(), String> {
        self.slots.prune_unreachable_publications().await
    }

    /// Return the immutable path authority used by this publisher.
    pub fn paths(&self) -> &ResidentPublicationPaths {
        &self.paths
    }

    /// Stage a content-proven candidate without switching any serving slot.
    pub async fn stage_candidate(
        &self,
        artifact_path: &Path,
        endpoint: &RuntimeServerEndpoint,
    ) -> Result<ResidentPublicationReceipt, String> {
        let identity = endpoint_generation_identity(endpoint);
        validate_endpoint_identity(endpoint, &identity)?;
        let publication_dir = self.paths.candidate(&identity.runtime_generation_digest);
        tokio::fs::create_dir_all(&publication_dir)
            .await
            .map_err(|error| format!("create candidate publication: {error}"))?;
        self.slots
            .stage_candidate_artifact(&publication_dir, artifact_path)
            .await?;
        let endpoint_path = publication_dir.join("endpoint.json");
        publish_json(&endpoint_path, endpoint).await?;
        let receipt = ResidentPublicationReceipt {
            schema_id: RESIDENT_PUBLICATION_SCHEMA_ID.to_owned(),
            schema_version: "1".to_owned(),
            state: "candidate".to_owned(),
            identity,
            publication_dir,
            endpoint_path,
        };
        receipt.validate()?;
        publish_json(&receipt.publication_dir.join("authority.json"), &receipt).await?;
        Ok(receipt)
    }

    /// Commit a ready candidate atomically to the active resident publication.
    pub async fn publish_ready(
        &self,
        candidate: &ResidentPublicationReceipt,
        readiness: &ResidentCandidateReadyReceipt,
    ) -> Result<ResidentPublicationReceipt, String> {
        candidate.validate()?;
        validate_ready_candidate(candidate, readiness)?;
        self.slots.commit_ready(&candidate.publication_dir).await?;
        let mut active = candidate.clone();
        active.state = "healthy".to_owned();
        publish_json(&active.publication_dir.join("authority.json"), &active).await?;
        Ok(active)
    }

    /// Read the healthy serving publication, if a healthy slot is present.
    pub async fn healthy(&self) -> Result<Option<ResidentPublicationReceipt>, String> {
        let Some(target) = self.slots.healthy_target().await? else {
            return Ok(None);
        };
        let bytes = tokio::fs::read(target.join("authority.json"))
            .await
            .map_err(|error| format!("read healthy Runtime Server authority: {error}"))?;
        let receipt: ResidentPublicationReceipt = serde_json::from_slice(&bytes)
            .map_err(|error| format!("decode healthy Runtime Server authority: {error}"))?;
        receipt.validate()?;
        Ok(Some(receipt))
    }

    /// Read the active client publication, if an active slot is present.
    pub async fn active(&self) -> Result<Option<ResidentPublicationReceipt>, String> {
        let Some(target) = self.slots.active_target().await? else {
            return Ok(None);
        };
        let bytes = tokio::fs::read(target.join("authority.json"))
            .await
            .map_err(|error| format!("read active Runtime Server authority: {error}"))?;
        let receipt: ResidentPublicationReceipt = serde_json::from_slice(&bytes)
            .map_err(|error| format!("decode active Runtime Server authority: {error}"))?;
        receipt.validate()?;
        Ok(Some(receipt))
    }
}

pub fn endpoint_generation_identity(
    endpoint: &RuntimeServerEndpoint,
) -> RuntimeServerGenerationIdentity {
    RuntimeServerGenerationIdentity {
        binary_content_digest: endpoint.binary_content_digest.clone(),
        runtime_generation_digest: endpoint.runtime_generation_digest.clone(),
        schema_digest: endpoint.schema_digest.clone(),
    }
}

fn validate_endpoint_identity(
    endpoint: &RuntimeServerEndpoint,
    observed: &RuntimeServerGenerationIdentity,
) -> Result<(), String> {
    let expected = RuntimeServerGenerationIdentity::derive(
        endpoint.binary_content_digest.clone(),
        &endpoint.schema_id,
        &endpoint.schema_version,
        &endpoint.transport_contract_digest,
        &endpoint.artifact_catalog_digest,
        endpoint.owner_epoch,
    );
    expected
        .validate(observed)
        .map_err(|error| error.to_string())
}

fn validate_ready_candidate(
    candidate: &ResidentPublicationReceipt,
    receipt: &ResidentCandidateReadyReceipt,
) -> Result<(), String> {
    let readiness = &receipt.readiness;
    readiness.validate(&readiness.request_id, &readiness.readiness_token)?;
    if readiness.state != RuntimeServerReadinessState::Ready {
        return Err(
            "Runtime Server candidate is not ready; active authority was not switched".to_owned(),
        );
    }
    candidate
        .identity
        .validate(&receipt.identity)
        .map_err(|error| error.to_string())
}

async fn publish_json(path: &Path, value: &impl Serialize) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("publication path has no parent: {}", path.display()))?;
    tokio::fs::create_dir_all(parent)
        .await
        .map_err(|error| format!("create publication directory: {error}"))?;
    let temporary = parent.join(format!(
        ".{}.{}.tmp",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("publication"),
        std::process::id()
    ));
    let bytes = serde_json::to_vec(value)
        .map_err(|error| format!("encode Runtime Server publication: {error}"))?;
    tokio::fs::write(&temporary, bytes)
        .await
        .map_err(|error| format!("write Runtime Server publication: {error}"))?;
    tokio::fs::rename(&temporary, path)
        .await
        .map_err(|error| format!("publish Runtime Server authority atomically: {error}"))
}

#[derive(Debug)]
pub struct ResidentDrainAuthority {
    accepting: AtomicBool,
    inflight: AtomicUsize,
    drained: Notify,
}

impl ResidentDrainAuthority {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            accepting: AtomicBool::new(true),
            inflight: AtomicUsize::new(0),
            drained: Notify::new(),
        })
    }

    pub fn admit(self: &Arc<Self>) -> Result<ResidentRequestLease, String> {
        if !self.accepting.load(Ordering::Acquire) {
            return Err("Runtime Server generation is draining".to_owned());
        }
        self.inflight.fetch_add(1, Ordering::AcqRel);
        if !self.accepting.load(Ordering::Acquire) {
            self.release();
            return Err("Runtime Server generation is draining".to_owned());
        }
        Ok(ResidentRequestLease {
            authority: self.clone(),
        })
    }

    pub async fn drain(&self) {
        self.accepting.store(false, Ordering::Release);
        while self.inflight.load(Ordering::Acquire) != 0 {
            self.drained.notified().await;
        }
    }

    fn release(&self) {
        if self.inflight.fetch_sub(1, Ordering::AcqRel) == 1 {
            self.drained.notify_waiters();
        }
    }
}

pub struct ResidentRequestLease {
    authority: Arc<ResidentDrainAuthority>,
}

impl Drop for ResidentRequestLease {
    fn drop(&mut self) {
        self.authority.release();
    }
}

#[cfg(test)]
#[path = "../tests/unit/resident_publication.rs"]
mod tests;
pub async fn publish_candidate_transaction<Drain, DrainFuture>(
    publisher: &AtomicResidentPublisher,
    binary: &std::path::Path,
    endpoint: &agent_semantic_client_db::runtime_server_control::RuntimeServerEndpoint,
    ready: &ResidentCandidateReadyReceipt,
    drain_previous: Drain,
) -> Result<ResidentPublicationReceipt, String>
where
    Drain: FnOnce() -> DrainFuture,
    DrainFuture: std::future::Future<Output = Result<(), String>>,
{
    let candidate = publisher.stage_candidate(binary, endpoint).await?;
    endpoint.validate_service_reachability().await?;
    let active = match publisher.publish_ready(&candidate, ready).await {
        Ok(active) => active,
        Err(error) => {
            match tokio::fs::remove_dir_all(&candidate.publication_dir).await {
                Ok(()) => {}
                Err(remove_error) if remove_error.kind() == std::io::ErrorKind::NotFound => {}
                Err(remove_error) => {
                    return Err(format!(
                        "{error}; cleanup uncommitted Runtime publication {}: {remove_error}",
                        candidate.publication_dir.display()
                    ));
                }
            }
            return Err(error);
        }
    };
    if let Err(drain_error) = drain_previous().await {
        return Err(format!(
            "resident Runtime handover drain failed after active authority commit; healthy authority remains available: {drain_error}"
        ));
    }
    Ok(active)
}
#[cfg(test)]
#[path = "../tests/unit/resident_publication_transaction.rs"]
mod production_transaction_tests;
pub async fn resident_readiness_root(
    state_home: &std::path::Path,
) -> Result<crate::readiness::RuntimeServerReadinessRoot, String> {
    use std::os::unix::ffi::OsStrExt as _;
    use std::os::unix::fs::MetadataExt as _;
    use std::os::unix::fs::PermissionsExt as _;

    let canonical_state_home = tokio::fs::canonicalize(state_home).await.map_err(|error| {
        format!(
            "canonicalize Runtime State Home {} for readiness identity: {error}",
            state_home.display()
        )
    })?;
    let mut identity = blake3::Hasher::new();
    identity.update(b"agent.semantic-protocols.runtime-server-readiness-socket.v1\0");
    identity.update(canonical_state_home.as_os_str().as_bytes());
    let identity = identity.finalize().to_hex();
    let expected_uid = tokio::fs::symlink_metadata(&canonical_state_home)
        .await
        .map_err(|error| {
            format!(
                "inspect canonical Runtime State Home {}: {error}",
                canonical_state_home.display()
            )
        })?
        .uid();
    let runtime_base =
        agent_semantic_client_db::runtime_server_control::runtime_server_runtime_base(state_home)?;
    let runtime_uid_root = runtime_base
        .parent()
        .ok_or_else(|| "Runtime readiness root has no UID parent".to_owned())?;
    let uid = runtime_uid_root
        .file_name()
        .and_then(|name| name.to_str())
        .and_then(|name| name.strip_prefix("asp-runtime-server-"))
        .filter(|uid| !uid.is_empty() && uid.bytes().all(|byte| byte.is_ascii_digit()))
        .ok_or_else(|| {
            format!(
                "Runtime Server UID root has invalid identity: {}",
                runtime_uid_root.display()
            )
        })?;
    let readiness_uid_root = std::path::PathBuf::from("/tmp").join(format!("asp-r-{uid}"));
    let root = readiness_uid_root.join(&identity[..24]);
    for directory in [readiness_uid_root.as_path(), root.as_path()] {
        match tokio::fs::create_dir(directory).await {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => {
                return Err(format!(
                    "create Runtime readiness directory {}: {error}",
                    directory.display()
                ));
            }
        }
        let metadata = tokio::fs::symlink_metadata(directory)
            .await
            .map_err(|error| {
                format!(
                    "inspect Runtime readiness directory {}: {error}",
                    directory.display()
                )
            })?;
        if !metadata.file_type().is_dir()
            || metadata.file_type().is_symlink()
            || metadata.uid() != expected_uid
        {
            return Err(format!(
                "Runtime readiness directory is not a non-symlink State Home UID directory: {}",
                directory.display()
            ));
        }
        let mut permissions = metadata.permissions();
        permissions.set_mode(0o700);
        tokio::fs::set_permissions(directory, permissions)
            .await
            .map_err(|error| {
                format!(
                    "protect Runtime readiness directory {}: {error}",
                    directory.display()
                )
            })?;
    }
    let socket = root.join("r").join(format!("{}.sock", "0".repeat(32)));
    const MAX_PORTABLE_UNIX_SOCKET_PATH_BYTES: usize = 100;
    if socket.as_os_str().as_bytes().len() > MAX_PORTABLE_UNIX_SOCKET_PATH_BYTES {
        return Err(format!(
            "Runtime readiness socket identity exceeds portable sun_path budget: bytes={} path={}",
            socket.as_os_str().as_bytes().len(),
            socket.display()
        ));
    }
    crate::readiness::RuntimeServerReadinessRoot::new(root)
}
