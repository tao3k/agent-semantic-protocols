use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::sync::watch;

pub use candidate::{
    WorkspaceGenerationBuildCompletion, WorkspaceGenerationBuildFailure,
    WorkspaceGenerationBuildFuture, WorkspaceGenerationBuildMode, WorkspaceGenerationBuilder,
    WorkspaceGenerationCandidateBuild, WorkspaceGenerationCandidateBuildFuture,
    WorkspaceGenerationCandidateBuilder, WorkspaceGenerationCandidateIdentity,
    WorkspaceGenerationFailureStage, WorkspaceGenerationProviderTarget,
    discover_workspace_generation_candidate, record_workspace_generation_candidate,
};
pub use mutation::{
    WORKSPACE_GENERATION_MUTATION_ADMISSION_RECEIPT_SCHEMA_ID,
    WORKSPACE_GENERATION_MUTATION_SUBMISSION_RECEIPT_SCHEMA_ID,
    WorkspaceGenerationMutationAdmissionReceipt, WorkspaceGenerationMutationSubmissionReceipt,
    WorkspaceGenerationMutationSubmissionState,
};
pub use query_demand::WorkspaceGenerationReadinessRequestState;

#[path = "runtime_server_admission_artifact_publication.rs"]
mod artifact_publication;
#[path = "runtime_server_admission_candidate.rs"]
mod candidate;
#[path = "runtime_server_admission_contract.rs"]
mod contract;
#[path = "runtime_server_admission_dispatcher.rs"]
pub(crate) mod dispatcher;
#[path = "runtime_server_admission_entry.rs"]
mod entry_authority;
#[path = "runtime_server_admission_lifecycle.rs"]
mod lifecycle;
#[path = "runtime_server_admission_mutation.rs"]
mod mutation;
#[path = "runtime_server_admission_query_coverage.rs"]
mod query_coverage;
#[path = "runtime_server_admission_query_demand.rs"]
mod query_demand;
#[path = "runtime_server_admission_registry.rs"]
mod registry;
#[path = "runtime_server_admission_terminal.rs"]
mod terminal;

use entry_authority::AdmissionEntryAuthority;
use query_coverage::QueryTargetCoverage;
use registry::AdmissionRegistry;

pub const WORKSPACE_GENERATION_ADMISSION_RECEIPT_SCHEMA_ID: &str =
    "agent.semantic-protocols.runtime-server-workspace-generation-admission";

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct WorkspaceGenerationAdmissionKey {
    workspace_identity: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WorkspaceGenerationAdmissionState {
    Queued,
    Building,
    Ready,
    Failed,
    Cancelled,
}

pub use contract::{WorkspaceGenerationAdmissionMode, WorkspaceGenerationAdmissionTrigger};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceGenerationAdmissionReceipt {
    pub schema_id: String,
    pub schema_version: String,
    pub workspace_identity: String,
    pub trigger: WorkspaceGenerationAdmissionTrigger,
    pub admission_mode: WorkspaceGenerationAdmissionMode,
    pub build_owner: String,
    pub cancellation_authority: String,
    pub request_lifetime_independent: bool,
    pub candidate_generation: agent_semantic_runtime::git::RepositoryCandidateGeneration,
    pub policy_overlay_digest: String,
    pub state: WorkspaceGenerationAdmissionState,
    pub accepted: bool,
    pub attempt: u64,
    pub commit: Option<WorkspaceGenerationCommitReceipt>,
    pub error: Option<String>,
    pub failure_stage: Option<WorkspaceGenerationFailureStage>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceGenerationCommitReceipt {
    pub active_epoch: u64,
    pub generation_digest: String,
    pub source_root_digest: String,
    pub projection_capability:
        crate::active_generation_projection_capability::ActiveGenerationProjectionCapabilityReceipt,
}

impl WorkspaceGenerationCommitReceipt {
    /// Validates that the committed generation and projection authority are one epoch.
    pub fn validate_projection_capability(&self) -> Result<(), String> {
        self.projection_capability.validate()?;
        if self.projection_capability.state
            != crate::active_generation_projection_capability::ActiveGenerationCapabilityState::Ready
        {
            return Err("committed generation projection capability is not ready".to_owned());
        }
        if self.projection_capability.publication_epoch != self.active_epoch {
            return Err("committed generation projection capability epoch drift".to_owned());
        }
        if self.projection_capability.generation_digest != self.generation_digest {
            return Err(
                "committed generation projection capability generation digest drift".to_owned(),
            );
        }
        if self.projection_capability.root_digest != self.source_root_digest {
            return Err("committed generation projection capability root digest drift".to_owned());
        }
        Ok(())
    }
}

impl WorkspaceGenerationCommitReceipt {
    pub fn from_recovery(
        recovery: &crate::runtime_server_workspace::WorkspaceRecoveryReceipt,
    ) -> Result<Self, String> {
        recovery.validate()?;
        let receipt = Self {
            projection_capability: recovery.projection_capability.clone(),
            active_epoch: recovery.target_epoch,
            generation_digest: recovery.generation_digest.clone(),
            source_root_digest: recovery.source_root_digest.clone(),
        };
        receipt.validate()?;
        Ok(receipt)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.active_epoch == 0
            || self.generation_digest.trim().is_empty()
            || self.source_root_digest.trim().is_empty()
        {
            return Err("workspace generation commit receipt is incomplete".to_owned());
        }
        Ok(())
    }
}

impl WorkspaceGenerationAdmissionReceipt {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_id != WORKSPACE_GENERATION_ADMISSION_RECEIPT_SCHEMA_ID
            || self.schema_version != "1"
        {
            return Err(format!(
                "workspace generation admission receipt schema mismatch: observed schemaId={} schemaVersion={} expected schemaId={} schemaVersion=1",
                self.schema_id,
                self.schema_version,
                WORKSPACE_GENERATION_ADMISSION_RECEIPT_SCHEMA_ID,
            ));
        }
        if !matches!(
            self.trigger.as_str(),
            "query-demand"
                | "workspace-change"
                | "operator-mutation"
                | "artifact-publication"
                | "runtime-recovery"
        ) {
            return Err(format!(
                "workspace generation admission receipt trigger is unsupported: {}",
                self.trigger
            ));
        }
        if !matches!(
            self.admission_mode.as_str(),
            "complete-generation" | "full-recovery"
        ) {
            return Err(format!(
                "workspace generation admission receipt mode is unsupported: {}",
                self.admission_mode
            ));
        }
        if self.build_owner != "runtime-server"
            || self.cancellation_authority != "runtime-server"
            || !self.request_lifetime_independent
        {
            return Err(
                "workspace generation admission receipt is not Runtime-owned and request-independent"
                    .to_owned(),
            );
        }
        if self.workspace_identity.trim().is_empty() || self.attempt == 0 {
            return Err("workspace generation admission receipt identity is incomplete".to_owned());
        }
        WorkspaceGenerationCandidateIdentity {
            candidate_generation: self.candidate_generation.clone(),
            policy_overlay_digest: self.policy_overlay_digest.clone(),
        }
        .validate()?;
        match (&self.state, &self.commit, &self.error, &self.failure_stage) {
            (
                WorkspaceGenerationAdmissionState::Queued
                | WorkspaceGenerationAdmissionState::Building,
                None,
                None,
                None,
            ) => Ok(()),
            (WorkspaceGenerationAdmissionState::Failed, None, Some(_), Some(_))
            | (WorkspaceGenerationAdmissionState::Cancelled, None, Some(_), Some(_)) => Ok(()),
            (WorkspaceGenerationAdmissionState::Ready, Some(commit), None, None) => {
                commit.validate()
            }
            _ => Err("workspace generation admission receipt state is inconsistent".to_owned()),
        }
    }
}

#[derive(Clone)]
pub struct WorkspaceGenerationAdmission {
    builder: WorkspaceGenerationBuilder,
    ready_validator: Option<WorkspaceGenerationReadyValidator>,
    catalog: Option<crate::runtime_server_admission_catalog::RuntimeWorkspaceAdmissionCatalog>,
    entries: AdmissionRegistry,
    changes: Arc<tokio::sync::Notify>,
    build_dispatcher: dispatcher::RuntimeServerAdmissionDispatcher,
    pending_readiness_requests:
        Arc<std::sync::Mutex<std::collections::BTreeMap<WorkspaceGenerationAdmissionKey, PathBuf>>>,
    telemetry_sender: crate::runtime_telemetry_bus::RuntimeTelemetryBusSender,
}

pub type WorkspaceGenerationReadyValidator =
    Arc<dyn Fn(&str, &std::path::Path) -> Result<(), String> + Send + Sync + 'static>;

struct PendingWorkspaceMutation {
    mutation_id: String,
    changed_paths: Arc<std::collections::BTreeSet<PathBuf>>,
    attempt: u64,
    candidate: WorkspaceGenerationCandidateIdentity,
}

#[derive(Clone)]
pub(crate) struct WorkspaceMutationIdentity {
    mutation_id: String,
    changed_paths: Arc<std::collections::BTreeSet<PathBuf>>,
    candidate: WorkspaceGenerationCandidateIdentity,
}

#[derive(Clone)]
struct WorkspaceMutationClaim {
    changed_paths: Arc<std::collections::BTreeSet<PathBuf>>,
    candidate: Arc<WorkspaceGenerationCandidateIdentity>,
    attempt: u64,
}

pub(crate) struct AdmissionEntry {
    project_root: PathBuf,
    receipt: watch::Sender<WorkspaceGenerationAdmissionReceipt>,
    active_mutation: watch::Sender<Option<WorkspaceMutationIdentity>>,
    mutation_changed: tokio::sync::Notify,
    lane: AdmissionEntryAuthority,
    transition: tokio::sync::Mutex<()>,
    cancellation: std::sync::Mutex<crate::runtime_generation_cancellation::GenerationCancellation>,
    query_target_coverage: std::sync::Mutex<QueryTargetCoverage>,
}

impl AdmissionEntry {
    pub(crate) fn new(
        project_root: PathBuf,
        receipt: WorkspaceGenerationAdmissionReceipt,
        active_mutation: Option<WorkspaceMutationIdentity>,
    ) -> Self {
        let attempt = receipt.attempt;
        let (sender, _) = watch::channel(receipt.clone());
        let (active_mutation_sender, _) = watch::channel(active_mutation.clone());
        let lane = AdmissionEntryAuthority::new(
            receipt.state == WorkspaceGenerationAdmissionState::Building,
            attempt,
            active_mutation.as_ref(),
            active_mutation_sender.clone(),
        );
        let active_mutation = active_mutation_sender;
        Self {
            project_root,
            receipt: sender,
            active_mutation,
            mutation_changed: tokio::sync::Notify::new(),
            lane,
            transition: tokio::sync::Mutex::new(()),
            cancellation: std::sync::Mutex::new(
                crate::runtime_generation_cancellation::GenerationCancellation::new(),
            ),
            query_target_coverage: std::sync::Mutex::new(QueryTargetCoverage::default()),
        }
    }

    pub(crate) fn current_cancellation(
        &self,
    ) -> crate::runtime_generation_cancellation::GenerationCancellation {
        self.cancellation
            .lock()
            .expect("workspace generation cancellation authority poisoned")
            .clone()
    }

    pub(crate) fn cancel_and_renew_generation(&self) {
        let mut cancellation = self
            .cancellation
            .lock()
            .expect("workspace generation cancellation authority poisoned");
        cancellation.cancel();
        *cancellation = crate::runtime_generation_cancellation::GenerationCancellation::new();
    }

    pub(crate) fn cancel_generation(&self) {
        self.cancellation
            .lock()
            .expect("workspace generation cancellation authority poisoned")
            .cancel();
    }

    pub(crate) fn matches_project_root(&self, project_root: &std::path::Path) -> bool {
        self.project_root == project_root
    }

    fn observed(&self) -> WorkspaceGenerationAdmissionReceipt {
        let mut receipt = self.receipt.borrow().clone();
        receipt.accepted = false;
        receipt
    }
}

impl WorkspaceGenerationAdmission {
    fn initial_control_plane_capacity() -> usize {
        std::thread::available_parallelism()
            .map(std::num::NonZeroUsize::get)
            .unwrap_or(1)
            .saturating_mul(64)
    }

    fn record_catalog_resident(
        &self,
        entry: crate::runtime_server_admission_catalog::RuntimeWorkspaceAdmissionCatalogEntry,
    ) -> Result<(), String> {
        let Some(catalog) = &self.catalog else {
            return Ok(());
        };
        if !catalog.record_resident(entry.clone())?.changed() {
            return Ok(());
        }
        let catalog = catalog.clone();
        let telemetry_sender = self.telemetry_sender.clone();
        let workspace_identity = entry.workspace_identity;
        self.submit_background_mutation(async move {
            if catalog.publish_resident_snapshot().await.is_err() {
                let _ = telemetry_sender.try_send_transition(
                    crate::runtime_server_opentelemetry::RuntimeLifecycleEvent {
                        owner_epoch: 0,
                        workspace_identity: Some(workspace_identity),
                        generation_digest: None,
                        candidate_digest: None,
                        transition: "workspace-admission-catalog-durability-failed".to_owned(),
                        state: "failed".to_owned(),
                        elapsed_micros: 0,
                        read_bytes: 0,
                        retained_bytes: 0,
                        active_task_count: 0,
                        active_child_count: 0,
                    },
                );
            }
        })?;
        Ok(())
    }

    /// Admit only the canonical ProjectId/workspaceId/root identity.
    ///
    /// This is the cold control-plane predecessor of ClientFrame Initialize.
    /// It deliberately performs no generation discovery, build, publication,
    /// workspace database open, or provider work. Generation readiness has its
    /// own single authority in `ensure_runtime_generation_ready_for_provider`.
    pub async fn admit_project_workspace_identity(
        &self,
        project_root: std::path::PathBuf,
    ) -> Result<
        crate::runtime_server_admission_catalog::RuntimeWorkspaceAdmissionCatalogEntry,
        String,
    > {
        let workspace_identity = crate::AgentSessionRegistry::workspace_id(&project_root)?;
        let entry =
            crate::runtime_server_admission_catalog::RuntimeWorkspaceAdmissionCatalogEntry::resolve(
                workspace_identity,
                project_root,
            )?;
        self.catalog
            .as_ref()
            .ok_or_else(|| "Runtime Server workspace admission catalog is unavailable".to_owned())?
            .record(entry.clone())
            .await?;
        Ok(entry)
    }

    #[must_use]
    pub fn admitted_project_workspace_count(&self) -> usize {
        self.catalog
            .as_ref()
            .map_or(0, |catalog| catalog.snapshot().len())
    }

    pub fn new(builder: WorkspaceGenerationBuilder) -> Self {
        let bus = crate::runtime_telemetry_bus::RuntimeTelemetryBus::new();
        Self::new_with_telemetry_sender(builder, bus.sender)
    }

    pub fn new_with_telemetry_sender(
        builder: WorkspaceGenerationBuilder,
        telemetry_sender: crate::runtime_telemetry_bus::RuntimeTelemetryBusSender,
    ) -> Self {
        Self {
            builder,
            ready_validator: None,
            catalog: None,
            entries: AdmissionRegistry::new(Self::initial_control_plane_capacity()),
            changes: Arc::new(tokio::sync::Notify::new()),
            build_dispatcher: dispatcher::RuntimeServerAdmissionDispatcher::new(),
            pending_readiness_requests: Arc::new(std::sync::Mutex::new(
                std::collections::BTreeMap::new(),
            )),
            telemetry_sender,
        }
    }

    pub fn with_catalog(
        mut self,
        catalog: crate::runtime_server_admission_catalog::RuntimeWorkspaceAdmissionCatalog,
    ) -> Self {
        self.catalog = Some(catalog);
        self
    }

    pub fn resolve_project_workspace_root(
        &self,
        project_id: &str,
        workspace_id: &str,
    ) -> Result<std::path::PathBuf, String> {
        self.catalog
            .as_ref()
            .ok_or_else(|| "Runtime Server workspace admission catalog is unavailable".to_owned())?
            .resolve_project_workspace(project_id, workspace_id)
            .map(|entry| entry.project_root)
    }

    pub fn with_ready_validator(
        mut self,
        ready_validator: WorkspaceGenerationReadyValidator,
    ) -> Self {
        self.ready_validator = Some(ready_validator);
        self
    }

    /// Admit mutation work into the generation authority's owned dispatcher.
    pub fn submit_background_mutation(
        &self,
        task: impl std::future::Future<Output = ()> + Send + 'static,
    ) -> Result<(), String> {
        self.build_dispatcher
            .spawn(dispatcher::AdmissionBuildEnvelope::detached(Box::pin(task)))
            .map_err(|_| {
                "Runtime generation admission dispatcher is not accepting mutations".to_owned()
            })
    }

    pub async fn admit(
        &self,
        workspace_identity: impl Into<String>,
        project_root: PathBuf,
        candidate: WorkspaceGenerationCandidateIdentity,
    ) -> Result<WorkspaceGenerationAdmissionReceipt, String> {
        let expected_candidate = candidate.clone();
        let receipt = self
            .admit_with_mode(
                workspace_identity,
                project_root,
                candidate,
                WorkspaceGenerationBuildMode::RestoreOrBuild,
                WorkspaceGenerationAdmissionTrigger::QueryDemand,
                WorkspaceGenerationAdmissionMode::CompleteGeneration,
                None,
                Arc::default(),
            )
            .await?;
        if receipt.candidate_generation != expected_candidate.candidate_generation
            || receipt.policy_overlay_digest != expected_candidate.policy_overlay_digest
        {
            return Err(serde_json::json!({
                "schemaId": "agent.semantic-protocols.workspace-generation-admission-binding-mismatch",
                "schemaVersion": "1",
                "reasonKind": "workspace-generation-admission-binding-mismatch",
                "workspaceIdentity": receipt.workspace_identity,
                "expectedCandidateGeneration": expected_candidate.candidate_generation,
                "observedCandidateGeneration": receipt.candidate_generation,
                "expectedPolicyOverlayDigest": expected_candidate.policy_overlay_digest,
                "observedPolicyOverlayDigest": receipt.policy_overlay_digest,
            })
            .to_string());
        }
        Ok(receipt)
    }

    async fn admit_with_mode(
        &self,
        workspace_identity: impl Into<String>,
        project_root: PathBuf,
        candidate: WorkspaceGenerationCandidateIdentity,
        build_mode: WorkspaceGenerationBuildMode,
        trigger: WorkspaceGenerationAdmissionTrigger,
        admission_mode: WorkspaceGenerationAdmissionMode,
        provider_target: Option<WorkspaceGenerationProviderTarget>,
        cold_target_paths: Arc<std::collections::BTreeSet<PathBuf>>,
    ) -> Result<WorkspaceGenerationAdmissionReceipt, String> {
        let workspace_identity = workspace_identity.into();
        if workspace_identity.trim().is_empty() {
            return Err("workspace generation admission identity must be non-empty".to_owned());
        }
        if !project_root.is_absolute() {
            return Err("workspace generation admission root must be absolute".to_owned());
        }
        let key = WorkspaceGenerationAdmissionKey {
            workspace_identity: workspace_identity.clone(),
        };
        if let Some(entry) = self.entries.get(&key) {
            if !entry.matches_project_root(&project_root) {
                return Err(format!(
                    "workspace generation admission root drift: workspaceIdentity={workspace_identity} requestedRoot={} admittedRoot={}",
                    project_root.display(),
                    entry.project_root.display(),
                ));
            }
            return self
                .admit_existing(
                    entry,
                    workspace_identity,
                    project_root,
                    candidate,
                    // RestoreOnly must survive coalescing with an existing entry.
                    build_mode,
                    trigger,
                    admission_mode,
                    provider_target,
                    cold_target_paths,
                )
                .await;
        }
        candidate.validate()?;
        let receipt = WorkspaceGenerationAdmissionReceipt {
            schema_id: WORKSPACE_GENERATION_ADMISSION_RECEIPT_SCHEMA_ID.to_owned(),
            schema_version: "1".to_owned(),
            workspace_identity: workspace_identity.clone(),
            trigger,
            admission_mode,
            build_owner: "runtime-server".to_owned(),
            cancellation_authority: "runtime-server".to_owned(),
            request_lifetime_independent: true,
            candidate_generation: candidate.candidate_generation.clone(),
            policy_overlay_digest: candidate.policy_overlay_digest.clone(),
            state: WorkspaceGenerationAdmissionState::Queued,
            accepted: true,
            attempt: 1,
            failure_stage: None,
            commit: None,
            error: None,
        };
        receipt.validate()?;
        let (entry, inserted) = self
            .entries
            .get_or_insert(key, project_root.clone(), receipt.clone(), None)
            .await?;
        let accepted_receipt = inserted.then_some(receipt);
        if let Some(receipt) = accepted_receipt {
            entry.begin_query_targets(&cold_target_paths)?;
            self.spawn_build(
                Arc::clone(&entry),
                workspace_identity,
                project_root,
                candidate,
                1,
                build_mode,
                trigger,
                admission_mode,
                provider_target.clone(),
                cold_target_paths,
            )
            .await;
            return Ok(receipt);
        }
        self.admit_existing(
            entry,
            workspace_identity,
            project_root,
            candidate,
            build_mode,
            trigger,
            admission_mode,
            provider_target,
            cold_target_paths,
        )
        .await
    }

    async fn admit_existing(
        &self,
        entry: Arc<AdmissionEntry>,
        workspace_identity: String,
        project_root: PathBuf,
        candidate: WorkspaceGenerationCandidateIdentity,
        mut build_mode: WorkspaceGenerationBuildMode,
        trigger: WorkspaceGenerationAdmissionTrigger,
        admission_mode: WorkspaceGenerationAdmissionMode,
        provider_target: Option<WorkspaceGenerationProviderTarget>,
        cold_target_paths: Arc<std::collections::BTreeSet<PathBuf>>,
    ) -> Result<WorkspaceGenerationAdmissionReceipt, String> {
        let same_candidate = |receipt: &WorkspaceGenerationAdmissionReceipt| {
            receipt.candidate_generation == candidate.candidate_generation
                && receipt.policy_overlay_digest == candidate.policy_overlay_digest
        };
        let reusable_ready =
            |receipt: &WorkspaceGenerationAdmissionReceipt,
             observed_build_mode: WorkspaceGenerationBuildMode| {
                observed_build_mode != WorkspaceGenerationBuildMode::RebuildAfterMutation
                    && receipt.state == WorkspaceGenerationAdmissionState::Ready
                    && same_candidate(receipt)
                    && entry.ready_covers(&cold_target_paths)
            };
        let observed = loop {
            let observed = entry.observed();
            if reusable_ready(&observed, build_mode) {
                return Ok(observed);
            }
            if observed.state == WorkspaceGenerationAdmissionState::Queued {
                return Ok(observed);
            }
            if entry.lane.observed().building
                && same_candidate(&observed)
                && (cold_target_paths.is_empty() || entry.building_covers(&cold_target_paths))
            {
                return Ok(observed);
            }
            if !entry.lane.observed().building {
                break observed;
            }
            let changed = entry.mutation_changed.notified();
            tokio::pin!(changed);
            changed.as_mut().enable();
            if !entry.lane.observed().building {
                continue;
            }
            changed.as_mut().await;
        };
        candidate.validate()?;
        if observed.state == WorkspaceGenerationAdmissionState::Ready
            && !entry.ready_covers(&cold_target_paths)
        {
            build_mode = WorkspaceGenerationBuildMode::RebuildAfterMutation;
        }
        let transition = loop {
            match entry.transition.try_lock() {
                Ok(transition) => break transition,
                Err(_) => {
                    let changed = entry.mutation_changed.notified();
                    tokio::pin!(changed);
                    changed.as_mut().enable();

                    let observed = entry.observed();
                    if reusable_ready(&observed, build_mode)
                        || observed.state == WorkspaceGenerationAdmissionState::Queued
                        || (entry.lane.observed().building
                            && (cold_target_paths.is_empty()
                                || entry.building_covers(&cold_target_paths)))
                    {
                        return Ok(observed);
                    }
                    changed.as_mut().await;
                }
            }
        };
        let observed = entry.observed();
        if reusable_ready(&observed, build_mode) {
            return Ok(observed);
        }
        if observed.state == WorkspaceGenerationAdmissionState::Queued {
            return Ok(observed);
        }
        if entry.lane.observed().building {
            return Ok(observed);
        }
        if build_mode == WorkspaceGenerationBuildMode::RestoreOrBuild
            && matches!(
                observed.state,
                WorkspaceGenerationAdmissionState::Failed
                    | WorkspaceGenerationAdmissionState::Cancelled
            )
        {
            entry.lane.clear_pending().await?;
        }
        let attempt = entry.lane.queue_build().await?;
        entry.begin_query_targets(&cold_target_paths)?;
        let receipt = WorkspaceGenerationAdmissionReceipt {
            schema_id: WORKSPACE_GENERATION_ADMISSION_RECEIPT_SCHEMA_ID.to_owned(),
            schema_version: "1".to_owned(),
            workspace_identity: workspace_identity.clone(),
            trigger,
            admission_mode,
            build_owner: "runtime-server".to_owned(),
            cancellation_authority: "runtime-server".to_owned(),
            request_lifetime_independent: true,
            candidate_generation: candidate.candidate_generation.clone(),
            policy_overlay_digest: candidate.policy_overlay_digest.clone(),
            state: WorkspaceGenerationAdmissionState::Queued,
            accepted: true,
            attempt,
            commit: None,
            error: None,
            failure_stage: None,
        };
        receipt.validate()?;
        entry.receipt.send_replace(receipt.clone());
        entry.mutation_changed.notify_waiters();
        drop(transition);
        self.spawn_build(
            entry,
            workspace_identity,
            project_root,
            candidate,
            attempt,
            build_mode,
            trigger,
            admission_mode,
            provider_target,
            cold_target_paths,
        )
        .await;
        Ok(receipt)
    }

    async fn spawn_build(
        &self,
        entry: Arc<AdmissionEntry>,
        workspace_identity: String,
        project_root: PathBuf,
        mut candidate: WorkspaceGenerationCandidateIdentity,
        attempt: u64,
        mut build_mode: WorkspaceGenerationBuildMode,
        trigger: WorkspaceGenerationAdmissionTrigger,
        admission_mode: WorkspaceGenerationAdmissionMode,
        provider_target: Option<WorkspaceGenerationProviderTarget>,
        cold_target_paths: Arc<std::collections::BTreeSet<PathBuf>>,
    ) {
        let builder = Arc::clone(&self.builder);
        let changes = Arc::clone(&self.changes);
        let telemetry_sender = self.telemetry_sender.clone();
        let admission_owner = self.clone();
        let completed_entry = Arc::clone(&entry);
        let dispatcher_start_entry = Arc::clone(&entry);
        let dispatcher_start_changes = Arc::clone(&self.changes);
        let dispatcher_start_telemetry = telemetry_sender.clone();
        let dispatcher_start_workspace_identity = workspace_identity.clone();
        let dispatcher_start_candidate_digest = candidate.candidate_generation.digest.clone();
        let dispatcher_terminal_entry = Arc::clone(&entry);
        let dispatcher_terminal_changes = Arc::clone(&self.changes);
        let dispatcher_terminal_telemetry = telemetry_sender.clone();
        let dispatcher_terminal_workspace_identity = workspace_identity.clone();
        let dispatcher_terminal_candidate_digest = candidate.candidate_generation.digest.clone();
        let dispatcher_terminal_started = std::time::Instant::now();
        let task = Box::pin(async move {
            if let Err(error) =
                crate::runtime_server_admission_catalog::RuntimeWorkspaceAdmissionCatalogEntry::resolve(
                    workspace_identity.clone(),
                    project_root.clone(),
                )
                .and_then(|entry| admission_owner.record_catalog_resident(entry))
            {
                let mut failed = completed_entry.observed();
                failed.state = WorkspaceGenerationAdmissionState::Failed;
                failed.accepted = false;
                failed.commit = None;
                failed.failure_stage = Some(WorkspaceGenerationFailureStage::GenerationBuilder);
                failed.error = Some(error);
                completed_entry.lane.complete().await;
                completed_entry.receipt.send_replace(failed);
                admission_owner.changes.notify_waiters();
                return;
            }
            let mut attempt = attempt;
            loop {
                let cancellation = completed_entry.current_cancellation();
                let build_started = std::time::Instant::now();
                let changed_paths = completed_entry
                    .active_mutation
                    .borrow()
                    .as_ref()
                    .map(|mutation| Arc::clone(&mutation.changed_paths))
                    .unwrap_or_else(|| Arc::clone(&cold_target_paths));
                let build_result = crate::runtime_server_admission_builder_supervisor::run(
                    Arc::clone(&builder),
                    workspace_identity.clone(),
                    project_root.clone(),
                    candidate.clone(),
                    build_mode,
                    changed_paths,
                    provider_target.clone(),
                    cancellation.clone(),
                )
                .await;
                let completed = terminal::from_build_result(
                    build_result,
                    &workspace_identity,
                    &candidate,
                    trigger,
                    admission_mode,
                    attempt,
                );
                let _ = completed_entry.complete_query_targets(
                    completed.state == WorkspaceGenerationAdmissionState::Ready,
                );
                let transition = completed_entry.transition.lock().await;
                let next_mutation = match completed_entry.lane.take_next_mutation().await {
                    Ok(next) => next,
                    Err(_) => {
                        changes.notify_waiters();
                        break;
                    }
                };
                if let Some(next) = next_mutation {
                    build_mode = mutation::observed_mutation_build_mode(&completed);
                    candidate = next.candidate;
                    completed_entry.mutation_changed.notify_waiters();
                    attempt = next.attempt;
                    completed_entry
                        .receipt
                        .send_replace(WorkspaceGenerationAdmissionReceipt {
                            schema_id: WORKSPACE_GENERATION_ADMISSION_RECEIPT_SCHEMA_ID.to_owned(),
                            schema_version: "1".to_owned(),
                            workspace_identity: workspace_identity.clone(),
                            trigger: WorkspaceGenerationAdmissionTrigger::WorkspaceChange,
                            admission_mode: WorkspaceGenerationAdmissionMode::CompleteGeneration,
                            build_owner: "runtime-server".to_owned(),
                            cancellation_authority: "runtime-server".to_owned(),
                            request_lifetime_independent: true,
                            candidate_generation: candidate.candidate_generation.clone(),
                            policy_overlay_digest: candidate.policy_overlay_digest.clone(),
                            state: WorkspaceGenerationAdmissionState::Queued,
                            accepted: false,
                            failure_stage: None,
                            attempt,
                            commit: None,
                            error: None,
                        });
                    completed_entry.mutation_changed.notify_waiters();
                    changes.notify_waiters();
                    let mut building = completed_entry.observed();
                    building.state = WorkspaceGenerationAdmissionState::Building;
                    completed_entry.receipt.send_replace(building);
                    let _ = telemetry_sender.try_send_transition(
                        crate::runtime_server_opentelemetry::RuntimeLifecycleEvent {
                            owner_epoch: attempt,
                            workspace_identity: Some(workspace_identity.clone()),
                            generation_digest: None,
                            candidate_digest: Some(candidate.candidate_generation.digest.clone()),
                            transition: "generation-build-started".to_owned(),
                            state: "building".to_owned(),
                            elapsed_micros: 0,
                            read_bytes: 0,
                            retained_bytes: 0,
                            active_task_count: 1,
                            active_child_count: 0,
                        },
                    );
                    drop(transition);
                    changes.notify_waiters();
                    continue;
                }
                telemetry_sender.try_send_transition(
                    crate::runtime_server_opentelemetry::RuntimeLifecycleEvent {
                        owner_epoch: attempt,
                        workspace_identity: Some(workspace_identity.clone()),
                        generation_digest: completed
                            .commit
                            .as_ref()
                            .map(|commit| commit.source_root_digest.clone()),
                        candidate_digest: Some(candidate.candidate_generation.digest.clone()),
                        transition: "generation-terminal".to_owned(),
                        state: format!("{:?}", completed.state).to_lowercase(),
                        elapsed_micros: build_started.elapsed().as_micros() as u64,
                        read_bytes: 0,
                        retained_bytes: 0,
                        active_task_count: 1,
                        active_child_count: 0,
                    },
                );
                completed_entry.lane.complete().await;
                completed_entry.receipt.send_replace(completed);
                completed_entry.mutation_changed.notify_waiters();
                drop(transition);
                changes.notify_waiters();
                break;
            }
        });
        let start_build = Box::pin(async move {
            let transition = dispatcher_start_entry.transition.lock().await;
            let mut building = dispatcher_start_entry.observed();
            if building.state != WorkspaceGenerationAdmissionState::Queued {
                return Err(format!(
                    "workspace generation admission dispatcher cannot acquire non-queued attempt: state={:?} attempt={attempt}",
                    building.state
                ));
            }
            dispatcher_start_entry.lane.start_queued(attempt).await?;
            building.state = WorkspaceGenerationAdmissionState::Building;
            building.accepted = false;
            dispatcher_start_entry.receipt.send_replace(building);
            dispatcher_start_telemetry.try_send_transition(
                crate::runtime_server_opentelemetry::RuntimeLifecycleEvent {
                    owner_epoch: attempt,
                    workspace_identity: Some(dispatcher_start_workspace_identity),
                    generation_digest: None,
                    candidate_digest: Some(dispatcher_start_candidate_digest),
                    transition: "generation-build-started".to_owned(),
                    state: "building".to_owned(),
                    elapsed_micros: 0,
                    read_bytes: 0,
                    retained_bytes: 0,
                    active_task_count: 1,
                    active_child_count: 0,
                },
            );
            dispatcher_start_entry.mutation_changed.notify_waiters();
            dispatcher_start_changes.notify_waiters();
            drop(transition);
            Ok(())
        });
        let terminalize_dispatcher_exit = Box::pin(async move {
            let observed_state = dispatcher_terminal_entry.observed().state;
            if !matches!(
                observed_state,
                WorkspaceGenerationAdmissionState::Queued
                    | WorkspaceGenerationAdmissionState::Building
            ) {
                return;
            }
            dispatcher_terminal_entry.cancel_generation();
            let mut failed = dispatcher_terminal_entry.observed();
            if !matches!(
                failed.state,
                WorkspaceGenerationAdmissionState::Queued
                    | WorkspaceGenerationAdmissionState::Building
            ) {
                return;
            }
            let terminal_state = if failed.state == WorkspaceGenerationAdmissionState::Queued {
                WorkspaceGenerationAdmissionState::Cancelled
            } else {
                WorkspaceGenerationAdmissionState::Failed
            };
            failed.state = terminal_state.clone();
            failed.accepted = false;
            failed.commit = None;
            failed.failure_stage =
                Some(WorkspaceGenerationFailureStage::GenerationBuilderSupervision);
            failed.error = Some(
                if terminal_state == WorkspaceGenerationAdmissionState::Cancelled {
                    "workspace generation admission dispatcher closed before build start".to_owned()
                } else {
                    "workspace generation admission dispatcher exited before terminal publication"
                        .to_owned()
                },
            );
            dispatcher_terminal_telemetry.try_send_transition(
                crate::runtime_server_opentelemetry::RuntimeLifecycleEvent {
                    owner_epoch: attempt,
                    workspace_identity: Some(dispatcher_terminal_workspace_identity),
                    generation_digest: None,
                    candidate_digest: Some(dispatcher_terminal_candidate_digest),
                    transition: "generation-terminal".to_owned(),
                    state: format!("{terminal_state:?}").to_lowercase(),
                    elapsed_micros: dispatcher_terminal_started.elapsed().as_micros() as u64,
                    read_bytes: 0,
                    retained_bytes: 0,
                    active_task_count: 0,
                    active_child_count: 0,
                },
            );
            dispatcher_terminal_entry.receipt.send_replace(failed);
            dispatcher_terminal_entry.mutation_changed.notify_waiters();
            dispatcher_terminal_changes.notify_waiters();
            dispatcher_terminal_entry.lane.complete().await;
        });
        let task =
            dispatcher::AdmissionBuildEnvelope::new(start_build, task, terminalize_dispatcher_exit);
        if self.build_dispatcher.spawn(task).is_err() {
            let mut failed = entry.observed();
            failed.state = WorkspaceGenerationAdmissionState::Failed;
            failed.accepted = false;
            failed.commit = None;
            failed.failure_stage = Some(WorkspaceGenerationFailureStage::GenerationBuilder);
            failed.error =
                Some("workspace generation admission dispatcher is unavailable".to_owned());
            entry.receipt.send_replace(failed);
            entry.lane.complete().await;
            self.changes.notify_waiters();
        }
    }

    pub async fn shutdown(&self) -> Result<usize, String> {
        let entries = self.entries.entries();
        let tracked_generation_lanes = entries.len();
        let _ = self.entries.shutdown().await?;
        let telemetry_sender = self.telemetry_sender.clone();
        for entry in entries {
            let mut cancelled = entry.observed();
            if matches!(
                cancelled.state,
                WorkspaceGenerationAdmissionState::Queued
                    | WorkspaceGenerationAdmissionState::Building
            ) {
                cancelled.state = WorkspaceGenerationAdmissionState::Cancelled;
                cancelled.error = Some(
                    "workspace generation admission cancelled during Runtime Server shutdown"
                        .to_owned(),
                );
                cancelled.commit = None;
                entry.receipt.send_replace(cancelled);
                let _ = telemetry_sender.try_send_transition(
                    crate::runtime_server_opentelemetry::RuntimeLifecycleEvent {
                        owner_epoch: entry.lane.observed().attempt,
                        workspace_identity: Some(entry.observed().workspace_identity),
                        generation_digest: None,
                        candidate_digest: Some(entry.observed().candidate_generation.digest),
                        transition: "generation-cancelled".to_owned(),
                        state: "cancelled".to_owned(),
                        elapsed_micros: 0,
                        read_bytes: 0,
                        retained_bytes: 0,
                        active_task_count: 0,
                        active_child_count: 0,
                    },
                );
            }
            entry.lane.shutdown().await;
            entry.cancel_generation();
        }
        self.changes.notify_waiters();
        let _ = self.build_dispatcher.shutdown().await?;
        Ok(tracked_generation_lanes)
    }
}
