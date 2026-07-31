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
pub const WORKSPACE_RUNTIME_SELECTOR_OVERLAY_RECEIPT_SCHEMA_ID: &str =
    "agent.semantic-protocols.runtime-server-selector-overlay-receipt.v1";

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
    pub derived_projections: Vec<WorkspaceDerivedProjectionSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceDerivedProjectionSnapshot {
    pub projection_kind: String,
    pub bytes: Vec<u8>,
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
pub struct WorkspaceRuntimeSelectorOverlay {
    pub projection_kind: String,
    pub structural_selector: String,
    pub owner_path: String,
    pub owner_content_digest: String,
    pub byte_start: usize,
    pub byte_end: usize,
    pub projection_bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceRuntimeSelectorOverlayReceipt {
    pub schema_id: String,
    pub schema_version: String,
    pub workspace_identity: String,
    pub generation_digest: String,
    pub projection_kind: String,
    pub structural_selector: String,
    pub inserted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Proof-carrying result of reconciling one resident owner with its workspace file.
pub struct WorkspaceRuntimeOwnerFreshnessReceipt {
    pub schema_id: String,
    pub schema_version: String,
    pub workspace_identity: String,
    pub generation_digest: String,
    pub owner_path: String,
    pub owner_content_digest: Option<String>,
    pub changed: bool,
    pub removed: bool,
}

impl WorkspaceRuntimeOwnerFreshnessReceipt {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_id != "asp.runtime-owner-freshness-receipt.v1" || self.schema_version != "1"
        {
            return Err("runtime owner freshness receipt schema identity mismatch".to_owned());
        }
        if self.workspace_identity.trim().is_empty() || self.owner_path.trim().is_empty() {
            return Err("runtime owner freshness receipt identity is incomplete".to_owned());
        }
        if !valid_blake3_wire_digest(&self.generation_digest) {
            return Err("runtime owner freshness generation digest is invalid".to_owned());
        }
        match (&self.owner_content_digest, self.removed) {
            (None, true) => Ok(()),
            (Some(digest), false) if valid_blake3_wire_digest(digest) => Ok(()),
            _ => Err("runtime owner freshness content identity is inconsistent".to_owned()),
        }
    }
}

fn valid_blake3_wire_digest(value: &str) -> bool {
    value.strip_prefix("blake3-256:").is_some_and(|digest| {
        digest.len() == 64
            && digest
                .bytes()
                .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "state",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase"
)]
pub enum WorkspaceRuntimeSelectorRead {
    GenerationMissing,
    Projection {
        generation_digest: String,
        root_digest: String,
        bytes: Vec<u8>,
    },
    OwnerForRepair {
        generation_digest: String,
        root_digest: String,
        owner: WorkspaceOwnerSnapshot,
    },
    OwnerMissing {
        generation_digest: String,
        root_digest: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceMemoryGeneration {
    pub workspace_identity: String,
    pub project_root: String,
    pub state: WorkspaceGenerationState,
    pub active_epoch: u64,
    pub generation_digest: String,
    pub root_depth: [u8; 2],
    pub workspace_snapshot: agent_semantic_content_identity::WorkspaceSnapshot,
    pub source_snapshot: agent_semantic_content_identity::SourceSnapshotEvidence,
    pub workspace_generation: agent_semantic_content_identity::workspace_generation_evidence::WorkspaceGenerationEvidenceV1,
    pub memory_backend_digest: String,
    pub workspace_source_scope_generation: String,
    pub project_resolutions: Vec<agent_semantic_runtime::AdmittedProjectResolution>,
    pub owners: Vec<WorkspaceOwnerSnapshot>,
}

impl WorkspaceMemoryGeneration {
    pub fn validate(&self) -> Result<(), String> {
        self.workspace_snapshot.validate()?;
        self.validate_identity()?;
        validate_digest("generationDigest", &self.generation_digest)?;
        validate_digest("memoryBackendDigest", &self.memory_backend_digest)?;
        validate_digest(
            "workspaceSourceScopeGeneration",
            &self.workspace_source_scope_generation,
        )?;
        if self.workspace_source_scope_generation
            != agent_semantic_runtime::workspace_source_scope_generation_digest(
                &self.project_resolutions,
            )?
        {
            return Err("workspace generation ProjectResolution evidence drift".to_owned());
        }
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
        if self.project_root.trim().is_empty() {
            return Err("workspace project root must be non-empty text".to_owned());
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
    pub workspace_source_scope_generation: String,
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
        validate_digest("memoryBackendDigest", &self.memory_backend_digest)?;
        validate_digest(
            "workspaceSourceScopeGeneration",
            &self.workspace_source_scope_generation,
        )
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
    selector_index: HashMap<String, (usize, usize)>,
    term_index: HashMap<String, Vec<usize>>,
}

impl WorkspaceMemoryBackend {
    pub(crate) fn from_generation(generation: WorkspaceMemoryGeneration) -> Result<Self, String> {
        generation.validate()?;
        let mut selector_index = HashMap::new();
        let mut term_index = HashMap::<String, Vec<usize>>::new();
        for (owner_position, owner) in generation.owners.iter().enumerate() {
            let text = std::str::from_utf8(&owner.bytes).unwrap_or_default();
            for term in crate::source_index::source_query_keys(&owner.owner_path, text) {
                term_index.entry(term).or_default().push(owner_position);
            }
            for (selector_position, selector) in owner.selectors.iter().enumerate() {
                selector_index.insert(
                    selector.selector.clone(),
                    (owner_position, selector_position),
                );
            }
        }
        Ok(Self {
            generation: Arc::new(generation),
            selector_index,
            term_index,
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

    pub(crate) fn source_index_owner_positions(&self, query: &str, limit: usize) -> Vec<usize> {
        let query_terms = crate::source_index::source_query_keys("", query);
        let query = query.trim();
        if !query.is_empty()
            && query.chars().all(|character| {
                character.is_alphanumeric() || character == '-' || character == '_'
            })
            && query.contains(['-', '_'])
        {
            return self
                .term_index
                .get(&query.to_ascii_lowercase())
                .into_iter()
                .flatten()
                .copied()
                .take(limit)
                .collect();
        }
        let mut scores = HashMap::<usize, usize>::new();
        for term in query_terms {
            if let Some(positions) = self.term_index.get(&term) {
                for &position in positions {
                    *scores.entry(position).or_default() += 1;
                }
            }
        }
        let mut ranked = scores.into_iter().collect::<Vec<_>>();
        ranked.sort_unstable_by(
            |(left_position, left_score), (right_position, right_score)| {
                right_score
                    .cmp(left_score)
                    .then_with(|| left_position.cmp(right_position))
            },
        );
        ranked.truncate(limit);
        ranked
            .into_iter()
            .map(|(position, _score)| position)
            .collect()
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

pub(crate) fn validate_owners(owners: &[WorkspaceOwnerSnapshot]) -> Result<(), String> {
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
    let (_, selector_target) = selector
        .selector
        .split_once("://")
        .ok_or_else(|| "workspace selector must include a language scheme".to_owned())?;
    let (selector_owner, _) = selector_target
        .split_once('#')
        .ok_or_else(|| "workspace selector must include an owner fragment".to_owned())?;
    if selector_owner != owner.owner_path {
        return Err(format!(
            "workspace selector owner drift: selector={} ownerPath={}",
            selector.selector, owner.owner_path
        ));
    }
    if selector.byte_start > selector.byte_end || selector.byte_end > owner.bytes.len() {
        return Err(format!(
            "workspace selector byte range is invalid: selector={}",
            selector.selector
        ));
    }
    let mut projection_kinds = std::collections::HashSet::new();
    for projection in &selector.derived_projections {
        if projection.projection_kind != "callable-skeleton" {
            return Err(format!(
                "workspace derived selector projection kind is unsupported: projectionKind={}",
                projection.projection_kind
            ));
        }
        if projection.bytes.is_empty() {
            return Err("workspace derived selector projection bytes are empty".to_owned());
        }
        if !projection_kinds.insert(projection.projection_kind.as_str()) {
            return Err(format!(
                "duplicate workspace derived selector projection: selector={} projectionKind={}",
                selector.selector, projection.projection_kind
            ));
        }
    }
    Ok(())
}
