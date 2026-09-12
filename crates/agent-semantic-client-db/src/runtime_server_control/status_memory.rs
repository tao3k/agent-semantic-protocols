// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::os::unix::fs::MetadataExt;
use std::os::unix::fs::PermissionsExt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};

use memmap2::{Mmap, MmapMut, MmapOptions};

use super::model::{
    RuntimeServerControlReceipt, RuntimeServerEndpoint, RuntimeServerState,
    RuntimeServerStatusSnapshot,
};

const STATUS_MEMORY_BYTES: u64 = 256 * 1024;
const GENERATION_OFFSET: usize = 0;
const PAYLOAD_LENGTH_OFFSET: usize = 8;
const PAYLOAD_OFFSET: usize = 16;

static STATUS_MEMORY_READERS: std::sync::OnceLock<
    dashmap::DashMap<String, Arc<RuntimeServerStatusMemoryReader>>,
> = std::sync::OnceLock::new();

pub(crate) struct RuntimeServerStatusMemoryWriter {
    mapping: MmapMut,
    endpoint: RuntimeServerEndpoint,
    generation: u64,
    asp_python_graphs: Option<std::sync::Arc<std::sync::RwLock<super::AspPythonGraphsStatus>>>,
    agent_sessions:
        Option<std::sync::Arc<std::sync::RwLock<Vec<super::RuntimeServerAgentSessionStatus>>>>,
}

struct RuntimeServerStatusMemoryReader {
    mapping: Mmap,
    file_identity: StatusMemoryFileIdentity,
    cached_snapshot: RwLock<Option<(u64, Arc<RuntimeServerStatusSnapshot>)>>,
    snapshot_decode_count: AtomicU64,
    snapshot_cache_hit_count: AtomicU64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct StatusMemoryFileIdentity {
    device: u64,
    inode: u64,
}

impl StatusMemoryFileIdentity {
    fn from_metadata(metadata: &std::fs::Metadata) -> Self {
        Self {
            device: metadata.dev(),
            inode: metadata.ino(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeServerStatusMemoryMetrics {
    pub reader_open_count: u64,
    pub snapshot_decode_count: u64,
    pub snapshot_cache_hit_count: u64,
}

impl RuntimeServerStatusMemoryWriter {
    pub(crate) async fn create(endpoint: &RuntimeServerEndpoint) -> Result<Self, String> {
        let file = tokio::fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .read(true)
            .write(true)
            .open(&endpoint.status_memory_path)
            .await
            .map_err(|error| format!("failed to create Runtime Server status memory: {error}"))?;
        file.set_len(STATUS_MEMORY_BYTES)
            .await
            .map_err(|error| format!("failed to size Runtime Server status memory: {error}"))?;
        file.set_permissions(std::fs::Permissions::from_mode(0o600))
            .await
            .map_err(|error| format!("failed to protect Runtime Server status memory: {error}"))?;
        let file = file.into_std().await;
        // SAFETY: the file is exclusively initialized by the elected Runtime Server,
        // has a stable non-zero length, and the mapping is retained for the writer lifetime.
        let mapping = unsafe { MmapOptions::new().map_mut(&file) }
            .map_err(|error| format!("failed to map Runtime Server status memory: {error}"))?;
        let mut writer = Self {
            mapping,
            endpoint: endpoint.clone(),
            generation: 0,
            asp_python_graphs: None,
            agent_sessions: None,
        };
        writer.publish(RuntimeServerState::Starting, 0)?;
        Ok(writer)
    }

    pub(crate) fn publish(
        &mut self,
        state: RuntimeServerState,
        workspace_entry_count: usize,
    ) -> Result<(), String> {
        let committed_generation = self.generation.saturating_add(2);
        let snapshot = RuntimeServerStatusSnapshot::new(
            committed_generation,
            state,
            &self.endpoint,
            workspace_entry_count,
        );
        let snapshot = RuntimeServerStatusSnapshot {
            asp_python_graphs: self
                .asp_python_graphs
                .as_ref()
                .and_then(|status| status.read().ok().map(|status| status.clone())),
            agent_sessions: self
                .agent_sessions
                .as_ref()
                .and_then(|sessions| sessions.read().ok().map(|sessions| sessions.clone()))
                .unwrap_or_default(),
            ..snapshot
        };
        let payload = serde_json::to_vec(&snapshot)
            .map_err(|error| format!("failed to encode Runtime Server status memory: {error}"))?;
        if payload.len() > self.mapping.len().saturating_sub(PAYLOAD_OFFSET) {
            return Err("Runtime Server status memory payload exceeds fixed mapping".to_owned());
        }
        generation(&self.mapping).store(committed_generation - 1, Ordering::Release);
        self.mapping[PAYLOAD_LENGTH_OFFSET..PAYLOAD_OFFSET].fill(0);
        self.mapping[PAYLOAD_LENGTH_OFFSET..PAYLOAD_LENGTH_OFFSET + 4]
            .copy_from_slice(&(payload.len() as u32).to_le_bytes());
        self.mapping[PAYLOAD_OFFSET..PAYLOAD_OFFSET + payload.len()].copy_from_slice(&payload);
        generation(&self.mapping).store(committed_generation, Ordering::Release);
        self.generation = committed_generation;
        Ok(())
    }

    pub(crate) fn set_asp_python_graphs(
        &mut self,
        status: std::sync::Arc<std::sync::RwLock<super::AspPythonGraphsStatus>>,
    ) {
        self.asp_python_graphs = Some(status);
    }

    pub(crate) fn set_agent_sessions(
        &mut self,
        sessions: std::sync::Arc<std::sync::RwLock<Vec<super::RuntimeServerAgentSessionStatus>>>,
    ) {
        self.agent_sessions = Some(sessions);
    }
}

impl RuntimeServerStatusMemoryReader {
    async fn open_path(status_memory_path: &std::path::Path) -> Result<Self, String> {
        let file = tokio::fs::OpenOptions::new()
            .read(true)
            .open(status_memory_path)
            .await
            .map_err(|error| format!("failed to open Runtime Server status memory: {error}"))?;
        let metadata = file
            .metadata()
            .await
            .map_err(|error| format!("failed to inspect Runtime Server status memory: {error}"))?;
        let file_identity = StatusMemoryFileIdentity::from_metadata(&metadata);
        let file = file.into_std().await;
        // SAFETY: the elected Runtime Server owns the fixed-size backing file and
        // publishes payloads under the generation seqlock.
        let mapping = unsafe { MmapOptions::new().map(&file) }
            .map_err(|error| format!("failed to map Runtime Server status memory: {error}"))?;
        if mapping.len() != STATUS_MEMORY_BYTES as usize {
            return Err("Runtime Server status memory has an invalid size".to_owned());
        }
        Ok(Self {
            mapping,
            file_identity,
            cached_snapshot: RwLock::new(None),
            snapshot_decode_count: AtomicU64::new(0),
            snapshot_cache_hit_count: AtomicU64::new(0),
        })
    }

    fn read(&self) -> Result<Arc<RuntimeServerStatusSnapshot>, String> {
        let before = generation(&self.mapping).load(Ordering::Acquire);
        let cached = || {
            self.cached_snapshot
                .read()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .as_ref()
                .map(|(_, snapshot)| Arc::clone(snapshot))
        };
        if before == 0 {
            return Err("Runtime Server status memory is not initialized".to_owned());
        }
        if !before.is_multiple_of(2) {
            return cached()
                .inspect(|_| {
                    self.snapshot_cache_hit_count
                        .fetch_add(1, Ordering::Relaxed);
                })
                .ok_or_else(|| {
                    "Runtime Server status publication is in progress before the first snapshot"
                        .to_owned()
                });
        }
        if let Some((cached_generation, snapshot)) = self
            .cached_snapshot
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .as_ref()
            && *cached_generation == before
            && generation(&self.mapping).load(Ordering::Acquire) == before
        {
            self.snapshot_cache_hit_count
                .fetch_add(1, Ordering::Relaxed);
            return Ok(Arc::clone(snapshot));
        }
        let mut cached_snapshot = self
            .cached_snapshot
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some((cached_generation, snapshot)) = cached_snapshot.as_ref()
            && *cached_generation == before
            && generation(&self.mapping).load(Ordering::Acquire) == before
        {
            self.snapshot_cache_hit_count
                .fetch_add(1, Ordering::Relaxed);
            return Ok(Arc::clone(snapshot));
        }
        let length = u32::from_le_bytes(
            self.mapping[PAYLOAD_LENGTH_OFFSET..PAYLOAD_LENGTH_OFFSET + 4]
                .try_into()
                .expect("fixed status payload length field"),
        ) as usize;
        if length == 0 || PAYLOAD_OFFSET + length > self.mapping.len() {
            return Err("Runtime Server status memory payload length is invalid".to_owned());
        }
        let payload = self.mapping[PAYLOAD_OFFSET..PAYLOAD_OFFSET + length].to_vec();
        let after = generation(&self.mapping).load(Ordering::Acquire);
        if before != after || !after.is_multiple_of(2) {
            return cached().ok_or_else(|| {
                "Runtime Server status publication changed during the first snapshot".to_owned()
            });
        }
        let snapshot: RuntimeServerStatusSnapshot = serde_json::from_slice(&payload)
            .map_err(|error| format!("failed to decode Runtime Server status memory: {error}"))?;
        snapshot.validate(after)?;
        let snapshot = Arc::new(snapshot);
        *cached_snapshot = Some((after, Arc::clone(&snapshot)));
        self.snapshot_decode_count.fetch_add(1, Ordering::Relaxed);
        Ok(snapshot)
    }

    fn metrics(&self) -> RuntimeServerStatusMemoryMetrics {
        RuntimeServerStatusMemoryMetrics {
            reader_open_count: 1,
            snapshot_decode_count: self.snapshot_decode_count.load(Ordering::Relaxed),
            snapshot_cache_hit_count: self.snapshot_cache_hit_count.load(Ordering::Relaxed),
        }
    }
}

pub fn runtime_server_status_memory_metrics(
    endpoint: &RuntimeServerEndpoint,
) -> Option<RuntimeServerStatusMemoryMetrics> {
    let readers = STATUS_MEMORY_READERS.get()?;
    let reader_slot = readers.get(&endpoint.status_memory_path)?;
    Some(reader_slot.metrics())
}

async fn runtime_server_status_memory_reader(
    endpoint: &RuntimeServerEndpoint,
) -> Result<Arc<RuntimeServerStatusMemoryReader>, String> {
    runtime_server_status_memory_reader_at(&endpoint.status_memory_path).await
}

async fn cached_runtime_server_status_memory_reader(
    endpoint: &RuntimeServerEndpoint,
) -> Option<Arc<RuntimeServerStatusMemoryReader>> {
    STATUS_MEMORY_READERS
        .get()?
        .get(&endpoint.status_memory_path)
        .map(|reader| Arc::clone(reader.value()))
}

async fn runtime_server_status_memory_reader_at(
    status_memory_path: &str,
) -> Result<Arc<RuntimeServerStatusMemoryReader>, String> {
    let status_memory_path = std::path::Path::new(status_memory_path);
    let readers = STATUS_MEMORY_READERS.get_or_init(dashmap::DashMap::new);
    let cache_key = status_memory_path.to_string_lossy().into_owned();
    if let Some(reader) = readers.get(&cache_key) {
        return Ok(Arc::clone(reader.value()));
    }
    loop {
        let candidate =
            Arc::new(RuntimeServerStatusMemoryReader::open_path(status_memory_path).await?);
        if candidate.file_identity != status_memory_file_identity(status_memory_path).await? {
            continue;
        }
        if let Some(reader) = readers.get(&cache_key) {
            return Ok(Arc::clone(reader.value()));
        }
        readers.insert(cache_key.clone(), Arc::clone(&candidate));
        return Ok(candidate);
    }
}

async fn status_memory_file_identity(
    status_memory_path: &std::path::Path,
) -> Result<StatusMemoryFileIdentity, String> {
    tokio::fs::metadata(status_memory_path)
        .await
        .map(|metadata| StatusMemoryFileIdentity::from_metadata(&metadata))
        .map_err(|error| format!("failed to inspect Runtime Server status memory: {error}"))
}

pub async fn read_runtime_server_cached_health_status(
    status_memory_path: &std::path::Path,
    request_id: String,
) -> Result<RuntimeServerControlReceipt, String> {
    if let Some(cache_key) = status_memory_path.to_str()
        && let Some(reader) = STATUS_MEMORY_READERS.get().and_then(|readers| {
            readers
                .get(cache_key)
                .map(|reader| Arc::clone(reader.value()))
        })
    {
        let snapshot = reader.read()?;
        let receipt = snapshot.cached_health_receipt(request_id.clone());
        if receipt.state == RuntimeServerState::Healthy {
            return Ok(receipt);
        }
    }
    let status_memory_path = status_memory_path
        .to_str()
        .ok_or_else(|| "Runtime Server status memory path is not UTF-8".to_owned())?;
    if let Some(readers) = STATUS_MEMORY_READERS.get()
        && let Some(reader) = readers.get(status_memory_path)
    {
        let expected_identity =
            status_memory_file_identity(std::path::Path::new(status_memory_path)).await?;
        if expected_identity != reader.file_identity {
            drop(reader);
            readers.remove(status_memory_path);
        }
    }
    let reader = runtime_server_status_memory_reader_at(status_memory_path).await?;
    let snapshot = reader.read()?;
    Ok(snapshot.cached_health_receipt(request_id))
}

pub async fn prewarm_runtime_server_status_memory(
    endpoint: &RuntimeServerEndpoint,
) -> Result<RuntimeServerStatusMemoryMetrics, String> {
    let reader = runtime_server_status_memory_reader(endpoint).await?;
    reader.read()?;
    Ok(reader.metrics())
}

pub async fn read_runtime_server_agent_sessions(
    endpoint: &RuntimeServerEndpoint,
) -> Result<Vec<super::RuntimeServerAgentSessionStatus>, String> {
    endpoint.validate()?;
    let reader = runtime_server_status_memory_reader(endpoint).await?;
    Ok(reader.read()?.agent_sessions.clone())
}

fn validate_observed_session<'a>(
    sessions: &'a [super::RuntimeServerAgentSessionStatus],
    workspace_id: &str,
    observed_session_id: Option<&str>,
    observed_root_session_id: Option<&str>,
) -> Result<Option<&'a super::RuntimeServerAgentSessionStatus>, String> {
    let observed = observed_session_id
        .and_then(|session_id| sessions.iter().find(|entry| entry.session_id == session_id));
    if let Some(record) = observed
        && record.workspace_identity != workspace_id
    {
        return Err(format!(
            "session control-plane workspace mismatch: requestedWorkspace={workspace_id} registeredWorkspace={} sessionId={}",
            record.workspace_identity, record.session_id
        ));
    }
    if let (Some(record), Some(expected_root)) = (observed, observed_root_session_id)
        && record.root_session_id != expected_root
    {
        return Err(format!(
            "session control-plane identity mismatch: observed session `{}` belongs to root `{}`, host reported `{expected_root}`",
            record.session_id, record.root_session_id,
        ));
    }
    Ok(observed)
}

fn missing_root_session_state(name: &str) -> super::AgentSessionControlPlaneState {
    super::AgentSessionControlPlaneState {
        project_id: None,
        root_session_id: None,
        name: name.to_owned(),
        state: "blocked".to_owned(),
        generation: 0,
        reason_kind: Some("root-session-identity-required".to_owned()),
        host_binding: None,
    }
}

fn resolve_session_project_id(
    sessions: &[super::RuntimeServerAgentSessionStatus],
    observed: Option<&super::RuntimeServerAgentSessionStatus>,
    workspace_id: &str,
    root_session_id: &str,
) -> String {
    observed
        .map(|record| record.project_id.clone())
        .or_else(|| {
            sessions
                .iter()
                .find(|entry| {
                    entry.workspace_identity == workspace_id
                        && entry.root_session_id == root_session_id
                })
                .map(|entry| entry.project_id.clone())
        })
        .unwrap_or_else(|| workspace_id.to_owned())
}

fn find_named_session<'a>(
    sessions: &'a [super::RuntimeServerAgentSessionStatus],
    workspace_id: &str,
    project_id: &str,
    root_session_id: &str,
    name: &str,
) -> Option<&'a super::RuntimeServerAgentSessionStatus> {
    sessions.iter().find(|entry| {
        entry.workspace_identity == workspace_id
            && entry.project_id == project_id
            && entry.root_session_id == root_session_id
            && entry.name == name
    })
}

fn has_live_exact_binding(
    record: Option<&super::RuntimeServerAgentSessionStatus>,
    root_session_id: &str,
    name: &str,
) -> bool {
    record.is_some_and(|entry| {
        matches!(
            entry.lifecycle_state,
            super::RuntimeServerAgentSessionLifecycleState::Routable
        ) && entry.host_binding.as_ref().is_some_and(|binding| {
            binding
                .get("schemaVersion")
                .and_then(serde_json::Value::as_str)
                == Some("1")
                && binding
                    .get("rootSessionId")
                    .and_then(serde_json::Value::as_str)
                    == Some(root_session_id)
                && binding
                    .get("hostChildId")
                    .and_then(serde_json::Value::as_str)
                    == Some(entry.session_id.as_str())
                && binding
                    .get("residentId")
                    .and_then(serde_json::Value::as_str)
                    == Some(name)
                && binding
                    .get("generation")
                    .and_then(serde_json::Value::as_u64)
                    == Some(entry.physical_generation)
                && binding
                    .get("lifecycleState")
                    .and_then(serde_json::Value::as_str)
                    == Some("live")
                && binding.get("routable").and_then(serde_json::Value::as_bool) == Some(true)
        })
    })
}

fn resolved_session_state(
    record: Option<&super::RuntimeServerAgentSessionStatus>,
    live_exact_binding: bool,
) -> String {
    match record.map(|entry| &entry.lifecycle_state) {
        Some(super::RuntimeServerAgentSessionLifecycleState::Routable) if live_exact_binding => {
            "registered".to_owned()
        }
        Some(super::RuntimeServerAgentSessionLifecycleState::Routable)
        | Some(super::RuntimeServerAgentSessionLifecycleState::Stopped) => "resumable".to_owned(),
        Some(super::RuntimeServerAgentSessionLifecycleState::Achieved) => "achieved".to_owned(),
        Some(super::RuntimeServerAgentSessionLifecycleState::Invalid) => "blocked".to_owned(),
        None => "registration-required".to_owned(),
    }
}

fn session_resume_reason(
    record: Option<&super::RuntimeServerAgentSessionStatus>,
    live_exact_binding: bool,
) -> Option<String> {
    let is_routable = record.is_some_and(|entry| {
        matches!(
            entry.lifecycle_state,
            super::RuntimeServerAgentSessionLifecycleState::Routable
        )
    });
    (is_routable && !live_exact_binding).then(|| "registered-namespace-needs-resume".to_owned())
}

pub fn resolve_runtime_server_agent_session_status(
    sessions: &[super::RuntimeServerAgentSessionStatus],
    workspace_id: &str,
    observed_session_id: Option<&str>,
    observed_root_session_id: Option<&str>,
    name: &str,
) -> Result<super::AgentSessionControlPlaneState, String> {
    if name.trim().is_empty() {
        return Err("session control-plane route requires a non-empty registered name".to_owned());
    }
    let observed = validate_observed_session(
        sessions,
        workspace_id,
        observed_session_id,
        observed_root_session_id,
    )?;
    let root_session_id = observed
        .map(|record| record.root_session_id.clone())
        .or_else(|| observed_root_session_id.map(str::to_owned));
    let Some(root_session_id) = root_session_id else {
        return Ok(missing_root_session_state(name));
    };
    let project_id = resolve_session_project_id(sessions, observed, workspace_id, &root_session_id);
    let record = find_named_session(sessions, workspace_id, &project_id, &root_session_id, name);
    let host_binding = record.and_then(|entry| entry.host_binding.clone());
    let live_exact_binding = has_live_exact_binding(record, &root_session_id, name);
    Ok(super::AgentSessionControlPlaneState {
        project_id: Some(project_id),
        root_session_id: Some(root_session_id),
        name: name.to_owned(),
        state: resolved_session_state(record, live_exact_binding),
        generation: record.map(|entry| entry.physical_generation).unwrap_or(0),
        reason_kind: session_resume_reason(record, live_exact_binding),
        host_binding,
    })
}

pub(super) fn validate_resident_transaction(
    endpoint: &RuntimeServerEndpoint,
    transaction: crate::runtime_server_owner_receipt::RuntimeServerResidentTransactionReceipt,
) -> Result<crate::runtime_server_owner_receipt::RuntimeServerResidentTransactionReceipt, String> {
    let launcher_path = std::path::Path::new(&transaction.launcher_artifact_path);
    let valid_drain = matches!(
        transaction.previous_drain_state.as_str(),
        "clean" | "not-running" | "not-required"
    );
    if transaction.schema_version != "1"
        || transaction.state != "ready"
        || transaction.publication_nonce.is_empty()
        || transaction.publication_nonce != transaction.applied_publication_nonce
        || transaction.launcher_artifact_digest != transaction.applied_artifact_digest
        || transaction.endpoint_binary_content_digest != transaction.applied_artifact_digest
        || transaction.endpoint_owner_epoch != endpoint.owner_epoch
        || transaction.endpoint_binary_content_digest.to_string() != endpoint.binary_content_digest
        || transaction.endpoint_runtime_generation_digest != endpoint.runtime_generation_digest
        || transaction.control_endpoint != endpoint.control_endpoint
        || transaction.data_endpoint != endpoint.data_endpoint
        || transaction.provider_endpoint != endpoint.provider_endpoint
        || !launcher_path.is_absolute()
        || transaction.launcher_artifact_path != endpoint.runtime_artifact_path
        || transaction.spawn_argv.is_empty()
        || !valid_drain
    {
        return Err(format!(
            "runtime-server-resident-transaction-not-current: publicationNonce={} appliedPublicationNonce={} endpointOwnerEpoch={} observedOwnerEpoch={} launcherPath={} endpointArtifactPath={} drainState={}",
            transaction.publication_nonce,
            transaction.applied_publication_nonce,
            endpoint.owner_epoch,
            transaction.endpoint_owner_epoch,
            transaction.launcher_artifact_path,
            endpoint.runtime_artifact_path,
            transaction.previous_drain_state
        ));
    }
    Ok(transaction)
}

pub(super) async fn attach_resident_transaction(
    state_home: &std::path::Path,
    endpoint: &RuntimeServerEndpoint,
    mut receipt: RuntimeServerControlReceipt,
) -> Result<RuntimeServerControlReceipt, String> {
    if receipt.state != RuntimeServerState::Healthy {
        return Ok(receipt);
    }
    let transaction =
        crate::runtime_server_lifecycle::observe_resident_transaction(state_home).await?;
    let transaction = validate_resident_transaction(endpoint, transaction)?;
    receipt.resident_transaction = Some(transaction);
    Ok(receipt)
}

pub(crate) async fn read_runtime_server_status(
    endpoint: &RuntimeServerEndpoint,
    request_id: String,
) -> Result<RuntimeServerControlReceipt, String> {
    if let Some(reader) = cached_runtime_server_status_memory_reader(endpoint).await {
        match reader
            .read()
            .and_then(|snapshot| snapshot.receipt(request_id.clone(), endpoint))
        {
            Ok(receipt) if receipt.state == RuntimeServerState::Healthy => return Ok(receipt),
            Ok(receipt)
                if status_memory_file_identity(std::path::Path::new(
                    &endpoint.status_memory_path,
                ))
                .await
                .is_ok_and(|identity| identity == reader.file_identity) =>
            {
                return Ok(receipt);
            }
            Ok(_) | Err(_) => {
                if let Some(readers) = STATUS_MEMORY_READERS.get() {
                    readers.remove(&endpoint.status_memory_path);
                }
            }
        }
    }
    let reader = runtime_server_status_memory_reader(endpoint).await?;
    let snapshot = reader.read()?;
    snapshot.receipt(request_id, endpoint)
}

fn generation(mapping: &[u8]) -> &AtomicU64 {
    let pointer = mapping[GENERATION_OFFSET..].as_ptr().cast::<AtomicU64>();
    // SAFETY: mmap bases are page-aligned, GENERATION_OFFSET is aligned for
    // AtomicU64, and the mapping outlives the returned reference.
    unsafe { &*pointer }
}

#[cfg(test)]
#[path = "../../tests/unit/runtime_server_control/status_memory.rs"]
mod tests;
