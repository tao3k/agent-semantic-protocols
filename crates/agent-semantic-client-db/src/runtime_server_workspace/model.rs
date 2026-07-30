use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Arc};

pub const WORKSPACE_GENERATION_SCHEMA_ID: &str =
    "agent.semantic-protocols.runtime-server-workspace-generation-snapshot.v1";
pub const WORKSPACE_RECOVERY_RECEIPT_SCHEMA_ID: &str =
    "agent.semantic-protocols.runtime-server-workspace-recovery-receipt.v1";
pub const WORKSPACE_DATA_PLANE_PERFORMANCE_RECEIPT_SCHEMA_ID: &str =
    "agent.semantic-protocols.runtime-server-workspace-data-plane-performance-receipt.v1";
pub const RUNTIME_SERVER_SHUTDOWN_RECEIPT_SCHEMA_ID: &str =
    "agent.semantic-protocols.runtime-server-shutdown-receipt.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WorkspaceGenerationState {
    Absent,
    LoadingPersistent,
    Ready,
    PublishingNext,
    RecoveringOwnerOverlay,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WorkspaceRecoverySource {
    MmapCheckpoint,
    TursoGeneration,
    ProviderOwnerOverlay,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeDataPlaneCounters {
    pub filesystem_reads: u64,
    pub filesystem_writes: u64,
    pub database_opens: u64,
    pub provider_spawns: u64,
    pub schema_bootstraps: u64,
    pub lock_probes: u64,
    pub manifest_reads: u64,
    pub workspace_canonicalizations: u64,
    pub activation_loads: u64,
    pub control_socket_roundtrips: u64,
}

impl RuntimeDataPlaneCounters {
    pub fn validate_zero_io(&self) -> Result<(), String> {
        if self == &Self::default() {
            Ok(())
        } else {
            Err(format!(
                "runtime data-plane I/O counters are non-zero: {self:?}"
            ))
        }
    }

    pub fn delta_since(&self, baseline: &Self) -> Self {
        Self {
            filesystem_writes: self
                .filesystem_writes
                .saturating_sub(baseline.filesystem_writes),
            filesystem_reads: self
                .filesystem_reads
                .saturating_sub(baseline.filesystem_reads),
            database_opens: self.database_opens.saturating_sub(baseline.database_opens),
            provider_spawns: self
                .provider_spawns
                .saturating_sub(baseline.provider_spawns),
            schema_bootstraps: self
                .schema_bootstraps
                .saturating_sub(baseline.schema_bootstraps),
            lock_probes: self.lock_probes.saturating_sub(baseline.lock_probes),
            manifest_reads: self.manifest_reads.saturating_sub(baseline.manifest_reads),
            workspace_canonicalizations: self
                .workspace_canonicalizations
                .saturating_sub(baseline.workspace_canonicalizations),
            activation_loads: self
                .activation_loads
                .saturating_sub(baseline.activation_loads),
            control_socket_roundtrips: self
                .control_socket_roundtrips
                .saturating_sub(baseline.control_socket_roundtrips),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceSelectorSnapshot {
    pub selector: String,
    pub byte_start: usize,
    pub byte_end: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceOwnerSnapshot {
    pub owner_path: String,
    pub content_digest: String,
    pub bytes: Vec<u8>,
    pub selectors: Vec<WorkspaceSelectorSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceMemoryGeneration {
    pub workspace_identity: String,
    pub state: WorkspaceGenerationState,
    pub active_epoch: u64,
    pub generation_digest: String,
    pub root_depth: [u8; 2],
    pub workspace_snapshot: agent_semantic_content_identity::WorkspaceSnapshot,
    pub source_snapshot: agent_semantic_content_identity::SourceSnapshotEvidence,
    pub workspace_generation: agent_semantic_content_identity::workspace_generation_evidence::WorkspaceGenerationEvidenceV1,
    pub memory_backend_digest: String,
    pub owners: Vec<WorkspaceOwnerSnapshot>,
}

impl WorkspaceMemoryGeneration {
    pub fn validate(&self) -> Result<(), String> {
        self.workspace_snapshot.validate()?;
        self.validate_identity()?;
        validate_digest("generationDigest", &self.generation_digest)?;
        validate_digest("memoryBackendDigest", &self.memory_backend_digest)?;
        if self.workspace_snapshot.root_digest() != self.source_snapshot.root_digest
            || self.workspace_generation.root_digest != self.source_snapshot.root_digest
            || self.workspace_generation.root_depth != u32::from(self.root_depth[0])
            || self.workspace_generation.leaf_count
                != u64::try_from(self.source_snapshot.leaf_count)
                    .map_err(|_| "workspace generation leaf count overflow".to_owned())?
            || self.workspace_generation.owner_count
                != u64::try_from(self.owners.len())
                    .map_err(|_| "workspace generation owner count overflow".to_owned())?
        {
            return Err("workspace generation authority evidence drift".to_owned());
        }
        agent_semantic_content_identity::workspace_generation_evidence::ValidatedWorkspaceGenerationV1::new(
            self.workspace_generation.clone(),
        )
        .map_err(|error| format!("workspace generation evidence is incomplete: {error}"))?;
        validate_owners(&self.owners)
    }

    fn validate_identity(&self) -> Result<(), String> {
        if self.workspace_identity.trim().is_empty() {
            return Err("workspace identity must be non-empty text".to_owned());
        }
        if self.state != WorkspaceGenerationState::Ready {
            return Err("only a ready workspace generation may be published".to_owned());
        }
        if self.active_epoch == 0 {
            return Err("workspace generation epoch must be positive".to_owned());
        }
        if self.root_depth != [1, 0] {
            return Err("workspace generation rootDepth must be [1, 0]".to_owned());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceGenerationSnapshot {
    pub schema_id: String,
    pub schema_version: String,
    pub workspace_identity: String,
    pub state: WorkspaceGenerationState,
    pub active_epoch: u64,
    pub generation_digest: String,
    pub root_depth: [u8; 2],
    pub memory_backend_digest: String,
    pub mmap_segment_path: String,
    pub previous_epoch_readable: bool,
}

impl WorkspaceGenerationSnapshot {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_id != WORKSPACE_GENERATION_SCHEMA_ID || self.schema_version != "1" {
            return Err("workspace generation snapshot schema identity mismatch".to_owned());
        }
        if self.workspace_identity.trim().is_empty()
            || self.mmap_segment_path.trim().is_empty()
            || self.active_epoch == 0
        {
            return Err("workspace generation snapshot identity is incomplete".to_owned());
        }
        if self.state != WorkspaceGenerationState::Ready || self.root_depth != [1, 0] {
            return Err("workspace generation snapshot is not publishable".to_owned());
        }
        validate_digest("generationDigest", &self.generation_digest)?;
        validate_digest("memoryBackendDigest", &self.memory_backend_digest)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceRecoveryReceipt {
    pub schema_id: String,
    pub schema_version: String,
    pub request_id: String,
    pub workspace_identity: String,
    pub source: WorkspaceRecoverySource,
    pub state: WorkspaceGenerationState,
    pub active_epoch: u64,
    pub target_epoch: u64,
    pub old_generation_readable: bool,
    pub counters: RuntimeDataPlaneCounters,
}

impl WorkspaceRecoveryReceipt {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_id != WORKSPACE_RECOVERY_RECEIPT_SCHEMA_ID || self.schema_version != "1" {
            return Err("workspace recovery receipt schema identity mismatch".to_owned());
        }
        if self.request_id.trim().is_empty() || self.workspace_identity.trim().is_empty() {
            return Err("workspace recovery receipt identity must be non-empty".to_owned());
        }
        if self.target_epoch == 0 || self.target_epoch <= self.active_epoch {
            return Err("workspace recovery target epoch must advance".to_owned());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceDataPlanePerformanceReceipt {
    pub schema_id: String,
    pub schema_version: String,
    pub workspace_count: usize,
    pub session_count: usize,
    pub sample_count: usize,
    pub p50_micros: u64,
    pub p99_micros: u64,
    pub max_micros: u64,
    pub counters: RuntimeDataPlaneCounters,
}

impl WorkspaceDataPlanePerformanceReceipt {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_id != WORKSPACE_DATA_PLANE_PERFORMANCE_RECEIPT_SCHEMA_ID
            || self.schema_version != "1"
        {
            return Err(format!(
                "workspace data-plane performance schema identity mismatch: expectedSchemaId={} actualSchemaId={} expectedSchemaVersion=1 actualSchemaVersion={}",
                WORKSPACE_DATA_PLANE_PERFORMANCE_RECEIPT_SCHEMA_ID,
                self.schema_id,
                self.schema_version
            ));
        }
        if self.workspace_count < 2 || self.session_count < 2 || self.sample_count == 0 {
            return Err(
                "workspace data-plane performance coverage is not concurrent or multi-workspace"
                    .to_owned(),
            );
        }
        if self.p50_micros > self.p99_micros || self.p99_micros > self.max_micros {
            return Err("workspace data-plane latency percentiles are not monotonic".to_owned());
        }
        if self.p99_micros >= 1_000 {
            return Err(format!(
                "workspace data-plane p99 exceeds the sub-millisecond gate: p99Micros={}",
                self.p99_micros
            ));
        }
        self.counters.validate_zero_io()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeServerShutdownReceipt {
    pub schema_id: String,
    pub schema_version: String,
    pub workspace_count: usize,
    pub writer_lane_count: usize,
    pub queued_publications_drained: bool,
    pub forced_abort_count: usize,
}

impl RuntimeServerShutdownReceipt {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_id != RUNTIME_SERVER_SHUTDOWN_RECEIPT_SCHEMA_ID || self.schema_version != "1"
        {
            return Err("Runtime Server shutdown receipt schema identity mismatch".to_owned());
        }
        if !self.queued_publications_drained || self.forced_abort_count != 0 {
            return Err("Runtime Server shutdown did not drain cleanly".to_owned());
        }
        if self.workspace_count != self.writer_lane_count {
            return Err("Runtime Server shutdown lane count mismatch".to_owned());
        }
        Ok(())
    }
}

#[derive(Debug)]
pub(crate) struct WorkspaceMemoryBackend {
    generation: Arc<WorkspaceMemoryGeneration>,
    owner_index: HashMap<String, usize>,
    selector_index: HashMap<String, (usize, usize)>,
}

impl WorkspaceMemoryBackend {
    pub(crate) fn from_generation(generation: WorkspaceMemoryGeneration) -> Result<Self, String> {
        generation.validate()?;
        let mut owner_index = HashMap::with_capacity(generation.owners.len());
        let mut selector_index = HashMap::new();
        for (owner_position, owner) in generation.owners.iter().enumerate() {
            owner_index.insert(owner.owner_path.clone(), owner_position);
            for (selector_position, selector) in owner.selectors.iter().enumerate() {
                selector_index.insert(
                    selector.selector.clone(),
                    (owner_position, selector_position),
                );
            }
        }
        Ok(Self {
            generation: Arc::new(generation),
            owner_index,
            selector_index,
        })
    }

    pub(crate) fn generation(&self) -> &Arc<WorkspaceMemoryGeneration> {
        &self.generation
    }

    pub(crate) fn projection(self: &Arc<Self>, selector: &str) -> Option<WorkspaceProjectionLease> {
        let (owner_position, selector_position) = *self.selector_index.get(selector)?;
        Some(WorkspaceProjectionLease {
            backend: Arc::clone(self),
            owner_position,
            selector_position,
        })
    }

    pub(crate) fn owner(self: &Arc<Self>, owner_path: &str) -> Option<Arc<[u8]>> {
        let owner_position = *self.owner_index.get(owner_path)?;
        Some(Arc::from(
            self.generation.owners[owner_position].bytes.as_slice(),
        ))
    }
}

#[derive(Debug, Clone)]
pub struct WorkspaceProjectionLease {
    pub(crate) backend: Arc<WorkspaceMemoryBackend>,
    pub(crate) owner_position: usize,
    pub(crate) selector_position: usize,
}

impl WorkspaceProjectionLease {
    pub fn workspace_identity(&self) -> &str {
        &self.backend.generation.workspace_identity
    }

    pub fn epoch(&self) -> u64 {
        self.backend.generation.active_epoch
    }

    pub fn selector(&self) -> &str {
        &self.backend.generation.owners[self.owner_position].selectors[self.selector_position]
            .selector
    }

    pub fn bytes(&self) -> &[u8] {
        let owner = &self.backend.generation.owners[self.owner_position];
        let selector = &owner.selectors[self.selector_position];
        &owner.bytes[selector.byte_start..selector.byte_end]
    }
}

fn validate_digest(field: &str, digest: &str) -> Result<(), String> {
    let Some(value) = digest.strip_prefix("blake3-256:") else {
        return Err(format!("{field} must use blake3-256"));
    };
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(format!("{field} must contain a 64-character hex digest"));
    }
    Ok(())
}

pub(super) fn validate_owners(owners: &[WorkspaceOwnerSnapshot]) -> Result<(), String> {
    let mut owner_paths = HashMap::with_capacity(owners.len());
    let mut selectors = HashMap::new();
    for (owner_index, owner) in owners.iter().enumerate() {
        validate_owner(owner)?;
        if owner_paths
            .insert(owner.owner_path.as_str(), owner_index)
            .is_some()
        {
            return Err(format!(
                "duplicate workspace owner path: {}",
                owner.owner_path
            ));
        }
        for selector in &owner.selectors {
            validate_selector(owner, selector)?;
            if selectors
                .insert(selector.selector.as_str(), owner.owner_path.as_str())
                .is_some()
            {
                return Err(format!(
                    "duplicate workspace selector identity: {}",
                    selector.selector
                ));
            }
        }
    }
    Ok(())
}

fn validate_owner(owner: &WorkspaceOwnerSnapshot) -> Result<(), String> {
    if owner.owner_path.trim().is_empty() {
        return Err("workspace owner path must be non-empty text".to_owned());
    }
    validate_digest("owner contentDigest", &owner.content_digest)?;
    let actual = format!("blake3-256:{}", blake3::hash(&owner.bytes).to_hex());
    if actual != owner.content_digest {
        return Err(format!(
            "workspace owner digest mismatch: ownerPath={}",
            owner.owner_path
        ));
    }
    Ok(())
}

fn validate_selector(
    owner: &WorkspaceOwnerSnapshot,
    selector: &WorkspaceSelectorSnapshot,
) -> Result<(), String> {
    if selector.selector.trim().is_empty() {
        return Err("workspace selector must be non-empty text".to_owned());
    }
    if selector.byte_start > selector.byte_end || selector.byte_end > owner.bytes.len() {
        return Err(format!(
            "workspace selector byte range is invalid: selector={}",
            selector.selector
        ));
    }
    Ok(())
}
