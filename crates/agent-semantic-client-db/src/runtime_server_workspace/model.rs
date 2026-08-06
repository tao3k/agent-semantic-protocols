use serde::{Deserialize, Serialize};

#[path = "digest.rs"]
mod digest;
#[path = "model_validation.rs"]
mod model_validation;
use digest::typed_digest;
use model_validation::validate_digest;
pub(crate) use model_validation::validate_owners;

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

pub const WORKSPACE_GENERATION_DELTA_SCHEMA_ID: &str =
    "agent.semantic-protocols.workspace-generation-delta.v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceGenerationDelta {
    pub schema_id: String,
    pub schema_version: String,
    pub base_generation_digest: String,
    pub owners: Vec<WorkspaceOwnerSnapshot>,
    pub tombstones: Vec<String>,
}

impl WorkspaceGenerationDelta {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_id != WORKSPACE_GENERATION_DELTA_SCHEMA_ID || self.schema_version != "1" {
            return Err("workspace generation delta schema identity mismatch".to_owned());
        }
        if !self.base_generation_digest.starts_with("blake3-256:") {
            return Err("workspace generation delta base digest is invalid".to_owned());
        }
        if self.owners.is_empty() && self.tombstones.is_empty() {
            return Err("workspace generation delta must contain at least one mutation".to_owned());
        }
        validate_owners(&self.owners)?;
        let mut tombstones = std::collections::HashSet::with_capacity(self.tombstones.len());
        for owner_path in &self.tombstones {
            if owner_path.trim().is_empty() || !tombstones.insert(owner_path.as_str()) {
                return Err("workspace generation delta tombstones must be unique paths".to_owned());
            }
        }
        if self
            .owners
            .iter()
            .any(|owner| tombstones.contains(owner.owner_path.as_str()))
        {
            return Err(
                "workspace generation delta cannot upsert and tombstone the same owner".to_owned(),
            );
        }
        Ok(())
    }
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
        resolved_selector: String,
        bytes: Vec<u8>,
    },
    ProjectionMissing {
        generation_digest: String,
        root_digest: String,
        resolved_selector: String,
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
    RelocationAmbiguous {
        generation_digest: String,
        root_digest: String,
        candidates: Vec<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "state",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase"
)]
pub enum WorkspaceRuntimeOwnerRead {
    GenerationMissing,
    Owner {
        generation_digest: String,
        root_digest: String,
        owner: WorkspaceOwnerSnapshot,
    },
    OwnerMissing {
        generation_digest: String,
        root_digest: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceGenerationBuild {
    pub workspace_identity: String,
    pub project_root: String,
    pub active_epoch: u64,
    pub workspace_snapshot: agent_semantic_content_identity::WorkspaceSnapshot,
    pub source_snapshot: agent_semantic_content_identity::SourceSnapshotEvidence,
    pub module_graph_digest: String,
    pub project_resolutions: Vec<agent_semantic_runtime::AdmittedProjectResolution>,
    pub owners: Vec<WorkspaceOwnerSnapshot>,
    pub relations: Vec<
        agent_semantic_content_identity::provider_projection_relation::ProviderProjectedRelation,
    >,
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
    pub provider_schema_digest: String,
    pub module_graph_digest: String,
    pub selector_set_digest: String,
    pub memory_backend_digest: String,
    pub workspace_source_scope_generation: String,
    pub project_resolutions: Vec<agent_semantic_runtime::AdmittedProjectResolution>,
    pub owners: Vec<WorkspaceOwnerSnapshot>,
    pub relations: Vec<
        agent_semantic_content_identity::provider_projection_relation::ProviderProjectedRelation,
    >,
}

impl WorkspaceMemoryGeneration {
    pub fn try_from_build(input: WorkspaceGenerationBuild) -> Result<Self, String> {
        let workspace_generation =
            agent_semantic_content_identity::workspace_generation_evidence::WorkspaceGenerationEvidenceV1 {
                root_digest: input.source_snapshot.root_digest.clone(),
                root_depth: 1,
                leaf_count: u64::try_from(input.source_snapshot.leaf_count)
                    .map_err(|_| "workspace generation leaf count overflow".to_owned())?,
                owner_count: u64::try_from(input.owners.len())
                    .map_err(|_| "workspace generation owner count overflow".to_owned())?,
            };
        let provider_schema_digest = agent_semantic_runtime::project_resolution_schema_digest();
        let workspace_source_scope_generation =
            agent_semantic_runtime::workspace_source_scope_generation_digest(
                &input.project_resolutions,
            )?;
        let selector_set_digest = typed_digest(
            &input
                .owners
                .iter()
                .map(|owner| (&owner.owner_path, &owner.selectors))
                .collect::<Vec<_>>(),
        )?;
        let memory_backend_digest = typed_digest(&(
            &input.owners,
            &input.relations,
            &workspace_source_scope_generation,
            &input.project_resolutions,
        ))?;
        let generation_digest = typed_digest(&(
            &input.workspace_identity,
            &input.project_root,
            &input.workspace_snapshot,
            &input.source_snapshot,
            &workspace_generation,
            &provider_schema_digest,
            &input.module_graph_digest,
            &selector_set_digest,
            &input.relations,
            &workspace_source_scope_generation,
            &memory_backend_digest,
        ))?;
        let generation = Self {
            workspace_identity: input.workspace_identity,
            project_root: input.project_root,
            state: WorkspaceGenerationState::Ready,
            active_epoch: input.active_epoch,
            generation_digest,
            root_depth: [1, 0],
            workspace_snapshot: input.workspace_snapshot,
            source_snapshot: input.source_snapshot,
            workspace_generation,
            provider_schema_digest,
            module_graph_digest: input.module_graph_digest,
            selector_set_digest,
            memory_backend_digest,
            workspace_source_scope_generation,
            project_resolutions: input.project_resolutions,
            owners: input.owners,
            relations: input.relations,
        };
        // Every digest above was computed from this owned input. Validate the evidence once,
        // without serializing the large owner/projection payloads a second time merely to
        // compare each digest with itself.
        generation.validate_evidence()?;
        Ok(generation)
    }

    pub fn validate(&self) -> Result<(), String> {
        self.validate_evidence()?;
        let selector_set_digest = typed_digest(
            &self
                .owners
                .iter()
                .map(|owner| (&owner.owner_path, &owner.selectors))
                .collect::<Vec<_>>(),
        )?;
        if self.selector_set_digest != selector_set_digest {
            return Err("workspace generation selector-set digest drift".to_owned());
        }
        let memory_backend_digest = typed_digest(&(
            &self.owners,
            &self.relations,
            &self.workspace_source_scope_generation,
            &self.project_resolutions,
        ))?;
        if self.memory_backend_digest != memory_backend_digest {
            return Err("workspace generation MemoryBackend digest drift".to_owned());
        }
        let generation_digest = typed_digest(&(
            &self.workspace_identity,
            &self.project_root,
            &self.workspace_snapshot,
            &self.source_snapshot,
            &self.workspace_generation,
            &self.provider_schema_digest,
            &self.module_graph_digest,
            &self.selector_set_digest,
            &self.relations,
            &self.workspace_source_scope_generation,
            &self.memory_backend_digest,
        ))?;
        if self.generation_digest != generation_digest {
            return Err("workspace generation identity digest drift".to_owned());
        }
        Ok(())
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
    pub source_kind: agent_semantic_content_identity::SourceSnapshotKind,
    pub leaf_count: u64,
    pub owner_count: u64,
    pub provider_schema_digest: String,
    pub source_root_digest: String,
    pub base_root_digest: Option<String>,
    pub source_provider_digest: String,
    pub dirty_paths_digest: Option<String>,
    pub module_graph_digest: String,
    pub selector_set_digest: String,
    pub memory_backend_digest: String,
    pub workspace_source_scope_generation: String,
    pub durable_commit_digest: String,
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
        if self.owner_count > self.leaf_count {
            return Err("workspace generation snapshot owner count exceeds leaf count".to_owned());
        }
        validate_digest("generationDigest", &self.generation_digest)?;
        validate_digest("providerSchemaDigest", &self.provider_schema_digest)?;
        validate_digest("sourceRootDigest", &self.source_root_digest)?;
        if let Some(base_root_digest) = self.base_root_digest.as_deref() {
            validate_digest("baseRootDigest", base_root_digest)?;
        }
        validate_digest("sourceProviderDigest", &self.source_provider_digest)?;
        if let Some(dirty_paths_digest) = self.dirty_paths_digest.as_deref() {
            validate_digest("dirtyPathsDigest", dirty_paths_digest)?;
        }
        validate_digest("moduleGraphDigest", &self.module_graph_digest)?;
        validate_digest("selectorSetDigest", &self.selector_set_digest)?;
        validate_digest("memoryBackendDigest", &self.memory_backend_digest)?;
        validate_digest("durableCommitDigest", &self.durable_commit_digest)?;
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
    pub generation_digest: String,
    pub source_root_digest: String,
    pub old_generation_readable: bool,
    pub resident_publication_elapsed_micros: u64,
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
        if self.generation_digest.trim().is_empty() || self.source_root_digest.trim().is_empty() {
            return Err("workspace recovery receipt generation identity is incomplete".to_owned());
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

#[cfg(test)]
#[path = "../../tests/unit/runtime_server_workspace_projection_validation.rs"]
mod derived_projection_validation_tests;

#[path = "projection_validation.rs"]
mod projection_validation;
