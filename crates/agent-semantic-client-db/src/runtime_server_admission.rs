use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::sync::watch;

pub use candidate::{
    WorkspaceGenerationBuild, WorkspaceGenerationBuildCompletion, WorkspaceGenerationBuildFailure,
    WorkspaceGenerationBuildFuture, WorkspaceGenerationBuildMode, WorkspaceGenerationBuilder,
    WorkspaceGenerationCandidateBuildFuture, WorkspaceGenerationCandidateBuilder,
    WorkspaceGenerationCandidateIdentity, WorkspaceGenerationFailureStage,
    WorkspaceOwnerProjectionBuildFuture, WorkspaceOwnerProjectionBuilder,
    discover_workspace_generation_candidate, record_workspace_generation_candidate,
};
pub use mutation::{
    WORKSPACE_GENERATION_MUTATION_ADMISSION_RECEIPT_SCHEMA_ID,
    WORKSPACE_GENERATION_MUTATION_SUBMISSION_RECEIPT_SCHEMA_ID,
    WorkspaceGenerationMutationAdmissionReceipt, WorkspaceGenerationMutationSubmissionReceipt,
    WorkspaceGenerationMutationSubmissionState,
};

#[path = "runtime_server_admission_candidate.rs"]
mod candidate;
#[path = "runtime_server_admission_dispatcher.rs"]
pub(crate) mod dispatcher;
#[path = "runtime_server_admission_entry.rs"]
mod entry_authority;
#[path = "runtime_server_admission_mutation.rs"]
mod mutation;
#[path = "runtime_server_admission_restore.rs"]
mod restore;

use crate::runtime_server_admission_registry::AdmissionRegistry;
use entry_authority::AdmissionEntryAuthority;

pub const WORKSPACE_GENERATION_ADMISSION_RECEIPT_SCHEMA_ID: &str =
    "agent.semantic-protocols.runtime-server-workspace-generation-admission.v1";

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct WorkspaceGenerationAdmissionKey {
    workspace_identity: String,
    project_root: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WorkspaceGenerationAdmissionState {
    Building,
    Ready,
    Failed,
    Cancelled,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceGenerationAdmissionReceipt {
    pub schema_id: String,
    pub schema_version: String,
    pub workspace_identity: String,
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

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct WorkspaceGenerationRestoreReport {
    pub ready: Vec<WorkspaceGenerationAdmissionReceipt>,
    pub failed: Vec<WorkspaceGenerationRestoreFailure>,
}

fn registered_workspace_restore_concurrency() -> usize {
    std::thread::available_parallelism()
        .map(std::num::NonZeroUsize::get)
        .unwrap_or(2)
        .max(2)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceGenerationRestoreFailure {
    pub workspace_identity: String,
    pub error: String,
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
        if self.workspace_identity.trim().is_empty() || self.attempt == 0 {
            return Err("workspace generation admission receipt identity is incomplete".to_owned());
        }
        WorkspaceGenerationCandidateIdentity {
            candidate_generation: self.candidate_generation.clone(),
            policy_overlay_digest: self.policy_overlay_digest.clone(),
        }
        .validate()?;
        match (&self.state, &self.commit, &self.error, &self.failure_stage) {
            (WorkspaceGenerationAdmissionState::Building, None, None, None) => Ok(()),
            (WorkspaceGenerationAdmissionState::Failed, None, Some(_), Some(_))
            | (WorkspaceGenerationAdmissionState::Cancelled, None, Some(_), Some(_)) => Ok(()),
            (WorkspaceGenerationAdmissionState::Ready, Some(commit), None, None) => {
                commit.validate()
            }
            _ => Err("workspace generation admission receipt state is inconsistent".to_owned()),
        }
    }
}

fn observed_mutation_build_mode(
    receipt: &WorkspaceGenerationAdmissionReceipt,
) -> WorkspaceGenerationBuildMode {
    if receipt.state == WorkspaceGenerationAdmissionState::Ready && receipt.commit.is_some() {
        WorkspaceGenerationBuildMode::RebuildAfterMutation
    } else {
        WorkspaceGenerationBuildMode::RestoreOrBuild
    }
}

#[derive(Clone)]
pub struct WorkspaceGenerationAdmission {
    builder: WorkspaceGenerationBuilder,
    catalog: Option<crate::runtime_server_admission_catalog::RuntimeWorkspaceAdmissionCatalog>,
    entries: AdmissionRegistry,
    changes: Arc<tokio::sync::Notify>,
    build_dispatcher: dispatcher::RuntimeServerAdmissionDispatcher,
    telemetry_sender: crate::runtime_telemetry_bus::RuntimeTelemetryBusSender,
}

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
    receipt: watch::Sender<WorkspaceGenerationAdmissionReceipt>,
    active_mutation: watch::Sender<Option<WorkspaceMutationIdentity>>,
    mutation_changed: tokio::sync::Notify,
    lane: AdmissionEntryAuthority,
    transition: tokio::sync::Mutex<()>,
    cancellation: crate::runtime_generation_cancellation::GenerationCancellation,
}

impl AdmissionEntry {
    pub(crate) fn new(
        receipt: WorkspaceGenerationAdmissionReceipt,
        active_mutation: Option<WorkspaceMutationIdentity>,
    ) -> Self {
        let attempt = receipt.attempt;
        let (sender, _) = watch::channel(receipt.clone());
        let (active_mutation_sender, _) = watch::channel(active_mutation.clone());
        let lane = AdmissionEntryAuthority::new(
            true,
            attempt,
            active_mutation.as_ref(),
            active_mutation_sender.clone(),
        );
        let active_mutation = active_mutation_sender;
        Self {
            receipt: sender,
            active_mutation,
            mutation_changed: tokio::sync::Notify::new(),
            lane,
            transition: tokio::sync::Mutex::new(()),
            cancellation: crate::runtime_generation_cancellation::GenerationCancellation::new(),
        }
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
        if !catalog.record_resident(entry.clone())? {
            return Ok(());
        }
        let catalog = catalog.clone();
        let telemetry_sender = self.telemetry_sender.clone();
        let workspace_identity = entry.workspace_identity;
        self.track_submission_task(tokio::spawn(async move {
            if catalog.publish_resident_snapshot().await.is_err() {
                let _ = telemetry_sender.try_send_transition(
                    crate::runtime_server_opentelemetry::RuntimeLifecycleEvent {
                        owner_epoch: 0,
                        workspace_identity: Some(workspace_identity),
                        generation_digest: None,
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
        }));
        Ok(())
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
            catalog: None,
            entries: AdmissionRegistry::new(Self::initial_control_plane_capacity()),
            changes: Arc::new(tokio::sync::Notify::new()),
            build_dispatcher: dispatcher::RuntimeServerAdmissionDispatcher::new(),
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

    pub fn current(
        &self,
        workspace_identity: &str,
        project_root: &std::path::Path,
    ) -> Option<WorkspaceGenerationAdmissionReceipt> {
        self.entries
            .get(&WorkspaceGenerationAdmissionKey {
                workspace_identity: workspace_identity.to_owned(),
                project_root: project_root.to_path_buf(),
            })
            .map(|entry| entry.observed())
    }

    pub fn track_submission_task(&self, task: tokio::task::JoinHandle<()>) {
        self.build_dispatcher.track(task);
    }

    pub async fn admit(
        &self,
        workspace_identity: impl Into<String>,
        project_root: PathBuf,
        candidate: WorkspaceGenerationCandidateIdentity,
    ) -> Result<WorkspaceGenerationAdmissionReceipt, String> {
        self.admit_with_mode(
            workspace_identity,
            project_root,
            candidate,
            WorkspaceGenerationBuildMode::RestoreOrBuild,
        )
        .await
    }

    async fn admit_with_mode(
        &self,
        workspace_identity: impl Into<String>,
        project_root: PathBuf,
        candidate: WorkspaceGenerationCandidateIdentity,
        build_mode: WorkspaceGenerationBuildMode,
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
            project_root: project_root.clone(),
        };
        if let Some(entry) = self.entries.get(&key) {
            return self
                .admit_existing(
                    entry,
                    workspace_identity,
                    project_root,
                    candidate,
                    // RestoreOnly must survive coalescing with an existing entry.
                    build_mode,
                )
                .await;
        }
        candidate.validate()?;
        let receipt = WorkspaceGenerationAdmissionReceipt {
            schema_id: WORKSPACE_GENERATION_ADMISSION_RECEIPT_SCHEMA_ID.to_owned(),
            schema_version: "1".to_owned(),
            workspace_identity: workspace_identity.clone(),
            candidate_generation: candidate.candidate_generation.clone(),
            policy_overlay_digest: candidate.policy_overlay_digest.clone(),
            state: WorkspaceGenerationAdmissionState::Building,
            accepted: true,
            attempt: 1,
            failure_stage: None,
            commit: None,
            error: None,
        };
        receipt.validate()?;
        let (entry, inserted) = self
            .entries
            .get_or_insert(key, receipt.clone(), None)
            .await?;
        let accepted_receipt = inserted.then_some(receipt);
        if let Some(receipt) = accepted_receipt {
            self.spawn_build(
                Arc::clone(&entry),
                workspace_identity,
                project_root,
                candidate,
                1,
                build_mode,
            );
            return Ok(receipt);
        }
        self.admit_existing(
            entry,
            workspace_identity,
            project_root,
            candidate,
            build_mode,
        )
        .await
    }

    async fn admit_existing(
        &self,
        entry: Arc<AdmissionEntry>,
        workspace_identity: String,
        project_root: PathBuf,
        candidate: WorkspaceGenerationCandidateIdentity,
        build_mode: WorkspaceGenerationBuildMode,
    ) -> Result<WorkspaceGenerationAdmissionReceipt, String> {
        let same_candidate = |receipt: &WorkspaceGenerationAdmissionReceipt| {
            receipt.candidate_generation == candidate.candidate_generation
                && receipt.policy_overlay_digest == candidate.policy_overlay_digest
        };
        let reusable_ready = |receipt: &WorkspaceGenerationAdmissionReceipt| {
            build_mode != WorkspaceGenerationBuildMode::RebuildAfterMutation
                && receipt.state == WorkspaceGenerationAdmissionState::Ready
                && same_candidate(receipt)
        };
        let observed = entry.observed();
        if reusable_ready(&observed) {
            return Ok(observed);
        }
        if entry.lane.observed().building && same_candidate(&observed) {
            return Ok(observed);
        }
        candidate.validate()?;
        if entry.lane.observed().building {
            return Ok(observed);
        }
        let transition = loop {
            match entry.transition.try_lock() {
                Ok(transition) => break transition,
                Err(_) => {
                    let changed = entry.mutation_changed.notified();
                    tokio::pin!(changed);
                    changed.as_mut().enable();

                    let observed = entry.observed();
                    if reusable_ready(&observed) || entry.lane.observed().building {
                        return Ok(observed);
                    }
                    changed.as_mut().await;
                }
            }
        };
        let observed = entry.observed();
        if reusable_ready(&observed) {
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
        let attempt = entry.lane.start_build().await?;
        let receipt = WorkspaceGenerationAdmissionReceipt {
            schema_id: WORKSPACE_GENERATION_ADMISSION_RECEIPT_SCHEMA_ID.to_owned(),
            schema_version: "1".to_owned(),
            workspace_identity: workspace_identity.clone(),
            candidate_generation: candidate.candidate_generation.clone(),
            policy_overlay_digest: candidate.policy_overlay_digest.clone(),
            state: WorkspaceGenerationAdmissionState::Building,
            accepted: true,
            attempt,
            commit: None,
            error: None,
            failure_stage: None,
        };
        receipt.validate()?;
        entry.receipt.send_replace(receipt.clone());
        self.telemetry_sender.try_send_transition(
            crate::runtime_server_opentelemetry::RuntimeLifecycleEvent {
                owner_epoch: attempt,
                workspace_identity: Some(workspace_identity.clone()),
                generation_digest: None,
                transition: "generation-building".to_owned(),
                state: "building".to_owned(),
                elapsed_micros: 0,
                read_bytes: 0,
                retained_bytes: 0,
                active_task_count: 1,
                active_child_count: 0,
            },
        );
        entry.mutation_changed.notify_waiters();
        drop(transition);
        self.spawn_build(
            entry,
            workspace_identity,
            project_root,
            candidate,
            attempt,
            build_mode,
        );
        Ok(receipt)
    }

    fn spawn_build(
        &self,
        entry: Arc<AdmissionEntry>,
        workspace_identity: String,
        project_root: PathBuf,
        mut candidate: WorkspaceGenerationCandidateIdentity,
        attempt: u64,
        mut build_mode: WorkspaceGenerationBuildMode,
    ) {
        let builder = Arc::clone(&self.builder);
        let changes = Arc::clone(&self.changes);
        let telemetry_sender = self.telemetry_sender.clone();
        let admission_owner = self.clone();
        let cancellation = entry.cancellation.clone();
        let completed_entry = Arc::clone(&entry);
        let task = Box::pin(async move {
            if let Err(error) = admission_owner.record_catalog_resident(
                crate::runtime_server_admission_catalog::RuntimeWorkspaceAdmissionCatalogEntry {
                    workspace_identity: workspace_identity.clone(),
                    project_root: project_root.clone(),
                },
            ) {
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
                let build_started = std::time::Instant::now();
                let changed_paths = completed_entry
                    .active_mutation
                    .borrow()
                    .as_ref()
                    .map(|mutation| Arc::clone(&mutation.changed_paths))
                    .unwrap_or_default();
                let completed =
                    match crate::runtime_server_admission_builder_supervisor::run_with_deadline(
                        Arc::clone(&builder),
                        workspace_identity.clone(),
                        project_root.clone(),
                        candidate.clone(),
                        build_mode,
                        changed_paths,
                        cancellation.clone(),
                    crate::runtime_server_admission_builder_supervisor::DEFAULT_GENERATION_BUILD_LEASE,
                    )
                    .await
                    {
                        Ok(completion) => WorkspaceGenerationAdmissionReceipt {
                            commit: Some(completion.commit),
                            schema_id: WORKSPACE_GENERATION_ADMISSION_RECEIPT_SCHEMA_ID.to_owned(),
                            schema_version: "1".to_owned(),
                            workspace_identity: workspace_identity.clone(),
                            candidate_generation: completion.candidate.candidate_generation,
                            policy_overlay_digest: completion.candidate.policy_overlay_digest,
                            state: WorkspaceGenerationAdmissionState::Ready,
                            accepted: false,
                            attempt,
                            failure_stage: None,
                            error: None,
                        },
                        Err(error) => WorkspaceGenerationAdmissionReceipt {
                            schema_id: WORKSPACE_GENERATION_ADMISSION_RECEIPT_SCHEMA_ID.to_owned(),
                            schema_version: "1".to_owned(),
                            workspace_identity: workspace_identity.clone(),
                            candidate_generation: candidate.candidate_generation.clone(),
                            policy_overlay_digest: candidate.policy_overlay_digest.clone(),
                            state: WorkspaceGenerationAdmissionState::Failed,
                            accepted: false,
                            attempt,
                            commit: None,
                            failure_stage: Some(error.stage.clone()),
                            error: Some(error.message.clone()),
                        },
                    };
                let transition = completed_entry.transition.lock().await;
                let next_mutation = match completed_entry.lane.take_next_mutation().await {
                    Ok(next) => next,
                    Err(_) => {
                        changes.notify_waiters();
                        break;
                    }
                };
                if let Some(next) = next_mutation {
                    build_mode = observed_mutation_build_mode(&completed);
                    candidate = next.candidate;
                    completed_entry.mutation_changed.notify_waiters();
                    attempt = next.attempt;
                    completed_entry
                        .receipt
                        .send_replace(WorkspaceGenerationAdmissionReceipt {
                            schema_id: WORKSPACE_GENERATION_ADMISSION_RECEIPT_SCHEMA_ID.to_owned(),
                            schema_version: "1".to_owned(),
                            workspace_identity: workspace_identity.clone(),
                            candidate_generation: candidate.candidate_generation.clone(),
                            policy_overlay_digest: candidate.policy_overlay_digest.clone(),
                            state: WorkspaceGenerationAdmissionState::Building,
                            accepted: false,
                            failure_stage: None,
                            attempt,
                            commit: None,
                            error: None,
                        });
                    let _ = telemetry_sender.try_send_transition(
                        crate::runtime_server_opentelemetry::RuntimeLifecycleEvent {
                            owner_epoch: attempt,
                            workspace_identity: Some(workspace_identity.clone()),
                            generation_digest: None,
                            transition: "generation-rebuilding".to_owned(),
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
        if self.build_dispatcher.spawn(task).is_err() {
            let mut failed = entry.observed();
            failed.state = WorkspaceGenerationAdmissionState::Failed;
            failed.accepted = false;
            failed.commit = None;
            failed.failure_stage = Some(WorkspaceGenerationFailureStage::GenerationBuilder);
            failed.error =
                Some("workspace generation admission dispatcher is unavailable".to_owned());
            entry.receipt.send_replace(failed);
            entry.lane.complete_now();
            self.changes.notify_waiters();
        }
    }

    pub async fn status(
        &self,
        workspace_identity: &str,
        project_root: &std::path::Path,
    ) -> Option<WorkspaceGenerationAdmissionReceipt> {
        self.entries
            .get(&WorkspaceGenerationAdmissionKey {
                workspace_identity: workspace_identity.to_owned(),
                project_root: project_root.to_path_buf(),
            })
            .map(|entry| entry.receipt.borrow().clone())
    }

    pub async fn ensure(
        &self,
        workspace_identity: &str,
        project_root: &std::path::Path,
        candidate: WorkspaceGenerationCandidateIdentity,
    ) -> Result<WorkspaceGenerationAdmissionReceipt, String> {
        candidate.validate()?;
        self.record_catalog_resident(
            crate::runtime_server_admission_catalog::RuntimeWorkspaceAdmissionCatalogEntry {
                workspace_identity: workspace_identity.to_owned(),
                project_root: project_root.to_path_buf(),
            },
        )?;
        match self.status(workspace_identity, project_root).await {
            Some(receipt)
                if !matches!(
                    receipt.state,
                    WorkspaceGenerationAdmissionState::Failed
                        | WorkspaceGenerationAdmissionState::Cancelled
                ) && receipt.candidate_generation == candidate.candidate_generation
                    && receipt.policy_overlay_digest == candidate.policy_overlay_digest =>
            {
                Ok(receipt)
            }
            Some(receipt)
                if matches!(
                    receipt.state,
                    WorkspaceGenerationAdmissionState::Failed
                        | WorkspaceGenerationAdmissionState::Cancelled
                ) =>
            {
                self.admit(
                    workspace_identity.to_owned(),
                    project_root.to_path_buf(),
                    candidate,
                )
                .await
            }
            Some(_) => {
                let mutation_id = format!(
                    "candidate-generation:{}:{}",
                    candidate.candidate_generation.digest, candidate.policy_overlay_digest
                );
                self.admit_mutation_candidate(
                    mutation_id,
                    workspace_identity.to_owned(),
                    project_root.to_path_buf(),
                    Arc::new(std::collections::BTreeSet::new()),
                    candidate,
                )
                .await
            }
            None => {
                self.admit(
                    workspace_identity.to_owned(),
                    project_root.to_path_buf(),
                    candidate,
                )
                .await
            }
        }
    }

    pub async fn wait_terminal(
        &self,
        workspace_identity: &str,
        project_root: &std::path::Path,
    ) -> Result<WorkspaceGenerationAdmissionReceipt, String> {
        loop {
            let changed = self.changes.notified();
            tokio::pin!(changed);
            changed.as_mut().enable();
            let receipt = self
                .status(workspace_identity, project_root)
                .await
                .ok_or_else(|| {
                    format!(
                        "workspace generation admission is unknown: workspaceIdentity={workspace_identity} projectRoot={}",
                        project_root.display()
                    )
                })?;
            if receipt.state != WorkspaceGenerationAdmissionState::Building {
                if let Some(catalog) = &self.catalog {
                    catalog.wait_durable().await?;
                }
                return Ok(receipt);
            }
            changed.as_mut().await;
        }
    }

    pub async fn wait_terminal_attempt(
        &self,
        workspace_identity: &str,
        project_root: &std::path::Path,
        expected_attempt: u64,
    ) -> Result<WorkspaceGenerationAdmissionReceipt, String> {
        loop {
            let changed = self.changes.notified();
            tokio::pin!(changed);
            changed.as_mut().enable();
            let receipt = self
                .status(workspace_identity, project_root)
                .await
                .ok_or_else(|| {
                    format!(
                        "workspace generation admission is unknown: workspaceIdentity={workspace_identity} projectRoot={}",
                        project_root.display()
                    )
                })?;
            if receipt.attempt > expected_attempt {
                return Err(format!(
                    "workspace generation admission advanced past the requested attempt: workspaceIdentity={workspace_identity} expectedAttempt={expected_attempt} actualAttempt={}",
                    receipt.attempt
                ));
            }
            if receipt.attempt == expected_attempt
                && receipt.state != WorkspaceGenerationAdmissionState::Building
            {
                if let Some(catalog) = &self.catalog {
                    catalog.wait_durable().await?;
                }
                return Ok(receipt);
            }
            changed.as_mut().await;
        }
    }

    pub async fn shutdown(&self) -> Result<usize, String> {
        let entries = self.entries.entries();
        let tracked_generation_lanes = entries.len();
        let _ = self.entries.shutdown().await?;
        let telemetry_sender = self.telemetry_sender.clone();
        for entry in entries {
            let mut cancelled = entry.observed();
            if cancelled.state == WorkspaceGenerationAdmissionState::Building {
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
            entry.cancellation.cancel();
        }
        self.changes.notify_waiters();
        let _ = self.build_dispatcher.shutdown().await?;
        Ok(tracked_generation_lanes)
    }
}
