use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use agent_semantic_client_db::runtime_server_control::RuntimeServerEndpoint;
use agent_semantic_client_protocol::runtime_generation::RuntimeServerGenerationIdentity;
use serde::{Deserialize, Serialize};
use tokio::sync::Notify;

use crate::readiness::{RuntimeServerReadinessReceipt, RuntimeServerReadinessState};

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
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn publications(&self) -> PathBuf {
        self.root.join("publications")
    }

    pub fn candidate(&self, generation_digest: &str) -> PathBuf {
        self.publications().join(generation_digest)
    }
}

#[derive(Clone, Debug)]
pub struct AtomicResidentPublisher {
    paths: ResidentPublicationPaths,
    slots: agent_semantic_artifacts::runtime_artifact_catalog::RuntimeArtifactSlotAuthority,
}

impl AtomicResidentPublisher {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        let root = root.into();
        Self {
            paths: ResidentPublicationPaths::new(root.clone()),
            slots:
                agent_semantic_artifacts::runtime_artifact_catalog::RuntimeArtifactSlotAuthority::new(
                    root,
                ),
        }
    }

    pub async fn prune_unreachable_publications(&self) -> Result<(), String> {
        self.slots.prune_unreachable_publications().await
    }

    pub fn paths(&self) -> &ResidentPublicationPaths {
        &self.paths
    }

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
mod tests {
    use super::*;
    use agent_semantic_artifacts::runtime_artifact_catalog::RuntimeBinaryIdentity;

    fn endpoint(binary: &str, owner_epoch: u64) -> RuntimeServerEndpoint {
        let schema_id = "agent.semantic-protocols.runtime-server-endpoint".to_owned();
        let schema_version = "1".to_owned();
        let transport_contract_digest = "transport".to_owned();
        let artifact_catalog_digest = "catalog".to_owned();
        let identity = RuntimeServerGenerationIdentity::derive(
            binary,
            &schema_id,
            &schema_version,
            &transport_contract_digest,
            &artifact_catalog_digest,
            owner_epoch,
        );
        RuntimeServerEndpoint {
            schema_id,
            schema_version,
            binary_content_digest: identity.binary_content_digest,
            runtime_generation_digest: identity.runtime_generation_digest,
            schema_digest: identity.schema_digest,
            transport_contract_digest,
            owner_epoch,
            owner_process_id: 7,
            runtime_artifact_path: format!("/artifacts/{binary}/asp"),
            runtime_binary_identity: RuntimeBinaryIdentity::Content {
                value: binary.to_owned(),
                algorithm: "blake3-256".to_owned(),
            },
            monitor_capability: true,
            observed_runtime_binary_identity: RuntimeBinaryIdentity::Content {
                value: binary.to_owned(),
                algorithm: "blake3-256".to_owned(),
            },
            artifact_mode: "release".to_owned(),
            artifact_catalog_digest,
            binding_token: format!("binding-{owner_epoch}"),
            socket_path: format!("/tmp/{owner_epoch}.sock"),
            data_plane_socket_path: format!("/tmp/{owner_epoch}.data.sock"),
            provider_plane_socket_path: format!("/tmp/{owner_epoch}.provider.sock"),
            client_http_endpoint: "http://127.0.0.1:1".to_owned(),
            workspace_store_path: "/tmp/workspaces".to_owned(),
            status_memory_path: format!("/tmp/{owner_epoch}.status"),
        }
    }

    fn readiness(
        endpoint: &RuntimeServerEndpoint,
        state: RuntimeServerReadinessState,
    ) -> ResidentCandidateReadyReceipt {
        ResidentCandidateReadyReceipt {
            identity: endpoint_generation_identity(endpoint),
            readiness: RuntimeServerReadinessReceipt {
                schema_id: "agent.semantic-protocols.runtime-server-readiness".to_owned(),
                schema_version: "1".to_owned(),
                state,
                request_id: "request".to_owned(),
                readiness_token: "readiness".to_owned(),
                process_id: endpoint.owner_process_id,
                owner_epoch: endpoint.owner_epoch,
                endpoint_binding_token: endpoint.binding_token.clone(),
                runtime_binary_identity: endpoint.runtime_binary_identity.value().to_owned(),
                artifact_catalog_digest: endpoint.artifact_catalog_digest.clone(),
                transport_contract_digest: endpoint.transport_contract_digest.clone(),
                reason_kind: Some("runtime-server-ready".to_owned()),
                error: None,
            },
        }
    }

    async fn stage(
        publisher: &AtomicResidentPublisher,
        root: &Path,
        binary: &str,
        owner_epoch: u64,
    ) -> (RuntimeServerEndpoint, ResidentPublicationReceipt) {
        let artifact = root.join(format!("artifact-{binary}"));
        tokio::fs::write(&artifact, binary).await.unwrap();
        let endpoint = endpoint(binary, owner_epoch);
        let candidate = publisher
            .stage_candidate(&artifact, &endpoint)
            .await
            .unwrap();
        (endpoint, candidate)
    }

    #[tokio::test]
    async fn candidate_not_ready_does_not_switch_active_authority() {
        let root = tempfile::tempdir().unwrap();
        let publisher = AtomicResidentPublisher::new(root.path().join("resident"));
        let (endpoint, candidate) = stage(&publisher, root.path(), "new", 2).await;
        let error = publisher
            .publish_ready(
                &candidate,
                &readiness(&endpoint, RuntimeServerReadinessState::Starting),
            )
            .await
            .unwrap_err();
        assert!(error.contains("not ready"));
        assert!(publisher.active().await.unwrap().is_none());
    }

    #[tokio::test]
    async fn ready_switch_is_atomic_and_healthy_retains_previous() {
        let root = tempfile::tempdir().unwrap();
        let publisher = AtomicResidentPublisher::new(root.path().join("resident"));
        let (old_endpoint, old_candidate) = stage(&publisher, root.path(), "old", 1).await;
        publisher
            .publish_ready(
                &old_candidate,
                &readiness(&old_endpoint, RuntimeServerReadinessState::Ready),
            )
            .await
            .unwrap();
        let (new_endpoint, new_candidate) = stage(&publisher, root.path(), "new", 2).await;
        publisher
            .publish_ready(
                &new_candidate,
                &readiness(&new_endpoint, RuntimeServerReadinessState::Ready),
            )
            .await
            .unwrap();
        assert_eq!(
            publisher
                .active()
                .await
                .unwrap()
                .unwrap()
                .identity
                .binary_content_digest,
            "new"
        );
        assert_eq!(
            publisher
                .healthy()
                .await
                .unwrap()
                .unwrap()
                .identity
                .binary_content_digest,
            "old"
        );
    }

    #[tokio::test]
    async fn old_generation_drains_only_after_inflight_request_releases() {
        let authority = ResidentDrainAuthority::new();
        let lease = authority.admit().unwrap();
        let draining = authority.clone();
        let task = tokio::spawn(async move {
            draining.drain().await;
        });
        tokio::task::yield_now().await;
        assert!(!task.is_finished());
        drop(lease);
        task.await.unwrap();
        assert!(authority.admit().is_err());
    }
}
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
mod production_transaction_tests {
    use super::{
        AtomicResidentPublisher, ResidentCandidateReadyReceipt, publish_candidate_transaction,
    };
    use crate::readiness::{RuntimeServerReadinessReceipt, RuntimeServerReadinessState};
    use agent_semantic_artifacts::runtime_artifact_catalog::RuntimeBinaryIdentity;
    use agent_semantic_client_db::runtime_server_control::{
        RuntimeServerEndpoint, runtime_server_transport_contract_digest,
    };
    use agent_semantic_client_protocol::runtime_generation::RuntimeServerGenerationIdentity;
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };

    #[tokio::test]
    async fn production_transaction_switches_only_after_ready_and_invokes_previous_drain() {
        let temporary = tempfile::tempdir().expect("temporary resident publication");
        let binary = temporary.path().join("candidate-asp");
        tokio::fs::write(&binary, b"resident-candidate")
            .await
            .expect("write candidate binary");
        let binary_content_digest =
            "blake3-256:0000000000000000000000000000000000000000000000000000000000000000"
                .to_owned();
        let artifact_catalog_digest =
            "blake3-256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                .to_owned();
        let transport_contract_digest = runtime_server_transport_contract_digest();
        let derived_identity = RuntimeServerGenerationIdentity::derive(
            binary_content_digest.clone(),
            "agent.semantic-protocols.runtime-server-endpoint",
            "1",
            &transport_contract_digest,
            &artifact_catalog_digest,
            7,
        );
        let runtime_generation_digest = derived_identity.runtime_generation_digest.clone();
        let schema_digest = derived_identity.schema_digest.clone();
        let endpoint = RuntimeServerEndpoint {
            schema_id: "agent.semantic-protocols.runtime-server-endpoint".to_owned(),
            schema_version: "1".to_owned(),
            binary_content_digest: binary_content_digest.clone(),
            runtime_generation_digest: runtime_generation_digest.clone(),
            schema_digest: schema_digest.clone(),
            transport_contract_digest: transport_contract_digest.clone(),
            owner_epoch: 7,
            owner_process_id: 77,
            runtime_artifact_path: binary.to_string_lossy().into_owned(),
            runtime_binary_identity: RuntimeBinaryIdentity::Content {
                value: binary_content_digest.clone(),
                algorithm: "blake3-256".to_owned(),
            },
            monitor_capability: true,
            observed_runtime_binary_identity: RuntimeBinaryIdentity::Content {
                value: binary_content_digest.clone(),
                algorithm: "blake3-256".to_owned(),
            },
            artifact_mode: "release".to_owned(),
            artifact_catalog_digest: artifact_catalog_digest.clone(),
            binding_token: "candidate-binding".to_owned(),
            socket_path: "/runtime/candidate-control.sock".to_owned(),
            data_plane_socket_path: "/runtime/candidate-data.sock".to_owned(),
            provider_plane_socket_path: "/runtime/candidate-provider.sock".to_owned(),
            client_http_endpoint: "http://127.0.0.1:1".to_owned(),
            workspace_store_path: "/runtime/workspaces".to_owned(),
            status_memory_path: "/runtime/status.memory".to_owned(),
        };
        let ready = ResidentCandidateReadyReceipt {
            identity: derived_identity,
            readiness: RuntimeServerReadinessReceipt {
                schema_id: "agent.semantic-protocols.runtime-server-readiness".to_owned(),
                schema_version: "1".to_owned(),
                state: RuntimeServerReadinessState::Ready,
                request_id: "resident-transaction-test".to_owned(),
                readiness_token: "resident-transaction-token".to_owned(),
                process_id: endpoint.owner_process_id,
                owner_epoch: endpoint.owner_epoch,
                endpoint_binding_token: endpoint.binding_token.clone(),
                runtime_binary_identity: endpoint.binary_content_digest.clone(),
                artifact_catalog_digest,
                transport_contract_digest,
                reason_kind: Some("runtime-server-ready".to_owned()),
                error: None,
            },
        };
        let drain_called = Arc::new(AtomicBool::new(false));
        let drain_observer = Arc::clone(&drain_called);
        let publisher = AtomicResidentPublisher::new(temporary.path().join("resident"));
        let receipt = publish_candidate_transaction(
            &publisher,
            &binary,
            &endpoint,
            &ready,
            move || async move {
                drain_observer.store(true, Ordering::Release);
                Ok(())
            },
        )
        .await
        .expect("publish ready resident candidate");
        assert!(drain_called.load(Ordering::Acquire));
        assert!(receipt.publication_dir.join("asp").is_file());
        assert_eq!(receipt.identity, ready.identity);

        let drain_error =
            publish_candidate_transaction(&publisher, &binary, &endpoint, &ready, || async {
                Err("old Runtime drain failed".to_owned())
            })
            .await
            .expect_err("drain failure is a typed terminal after authority commit");
        assert!(drain_error.contains("after active authority commit"));
        assert_eq!(
            publisher.active().await.unwrap().unwrap().identity,
            ready.identity
        );
        assert_eq!(
            publisher.healthy().await.unwrap().unwrap().identity,
            ready.identity
        );
    }
}
pub async fn resident_readiness_root(
    state_home: &std::path::Path,
) -> Result<crate::readiness::RuntimeServerReadinessRoot, String> {
    use std::os::unix::ffi::OsStrExt as _;
    use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};

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
    let root = std::path::PathBuf::from("/tmp")
        .join(format!("asp-r-{uid}"))
        .join(&identity[..24]);
    for directory in [root.as_path()] {
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
