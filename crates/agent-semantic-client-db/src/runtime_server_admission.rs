use std::collections::VecDeque;
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use serde::{Deserialize, Serialize};
use tokio::sync::watch;

pub use mutation::{
    WORKSPACE_GENERATION_MUTATION_ADMISSION_RECEIPT_SCHEMA_ID,
    WORKSPACE_GENERATION_MUTATION_SUBMISSION_RECEIPT_SCHEMA_ID,
    WorkspaceGenerationMutationAdmissionReceipt, WorkspaceGenerationMutationSubmissionReceipt,
    WorkspaceGenerationMutationSubmissionState,
};

#[path = "runtime_server_admission_mutation.rs"]
mod mutation;

pub const WORKSPACE_GENERATION_ADMISSION_RECEIPT_SCHEMA_ID: &str =
    "agent.semantic-protocols.runtime-server-workspace-generation-admission.v1";

pub struct WorkspaceGenerationBuild {
    pub refresh: crate::ClientDbSourceIndexRefreshRequest,
    pub materialization: crate::runtime_server_workspace::WorkspaceCanonicalMaterialization,
}

impl WorkspaceGenerationBuild {
    pub fn new(
        refresh: crate::ClientDbSourceIndexRefreshRequest,
        materialization: crate::runtime_server_workspace::WorkspaceCanonicalMaterialization,
    ) -> Self {
        Self {
            refresh,
            materialization,
        }
    }
}

pub type WorkspaceGenerationBuildFuture = Pin<
    Box<dyn Future<Output = Result<WorkspaceGenerationCommitReceipt, String>> + Send + 'static>,
>;
pub type WorkspaceGenerationBuilder = Arc<
    dyn Fn(String, PathBuf, WorkspaceGenerationBuildMode) -> WorkspaceGenerationBuildFuture
        + Send
        + Sync
        + 'static,
>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkspaceGenerationBuildMode {
    RestoreOrBuild,
    RebuildAfterMutation,
}

pub type WorkspaceGenerationCandidateBuildFuture =
    Pin<Box<dyn Future<Output = Result<WorkspaceGenerationBuild, String>> + Send + 'static>>;
pub type WorkspaceGenerationCandidateBuilder =
    Arc<dyn Fn(String, PathBuf) -> WorkspaceGenerationCandidateBuildFuture + Send + Sync + 'static>;

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
struct WorkspaceGenerationAdmissionKey {
    workspace_identity: String,
    project_root: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WorkspaceGenerationAdmissionState {
    Building,
    Ready,
    Failed,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceGenerationAdmissionReceipt {
    pub schema_id: String,
    pub schema_version: String,
    pub workspace_identity: String,
    pub state: WorkspaceGenerationAdmissionState,
    pub accepted: bool,
    pub attempt: u64,
    pub commit: Option<WorkspaceGenerationCommitReceipt>,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceGenerationCommitReceipt {
    pub active_epoch: u64,
    pub generation_digest: String,
    pub source_root_digest: String,
}

impl WorkspaceGenerationCommitReceipt {
    pub fn from_recovery(
        recovery: &crate::runtime_server_workspace::WorkspaceRecoveryReceipt,
    ) -> Result<Self, String> {
        recovery.validate()?;
        let receipt = Self {
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
    pub failed: Vec<WorkspaceGenerationAdmissionReceipt>,
}

impl WorkspaceGenerationAdmissionReceipt {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_id != WORKSPACE_GENERATION_ADMISSION_RECEIPT_SCHEMA_ID
            || self.schema_version != "1"
        {
            return Err("workspace generation admission receipt schema mismatch".to_owned());
        }
        if self.workspace_identity.trim().is_empty() || self.attempt == 0 {
            return Err("workspace generation admission receipt identity is incomplete".to_owned());
        }
        match (&self.state, &self.commit, &self.error) {
            (WorkspaceGenerationAdmissionState::Building, None, None)
            | (WorkspaceGenerationAdmissionState::Failed, None, Some(_)) => Ok(()),
            (WorkspaceGenerationAdmissionState::Ready, Some(commit), None) => commit.validate(),
            _ => Err("workspace generation admission receipt state is inconsistent".to_owned()),
        }
    }
}

#[derive(Clone)]
pub struct WorkspaceGenerationAdmission {
    builder: WorkspaceGenerationBuilder,
    catalog: Option<crate::runtime_server_admission_catalog::RuntimeWorkspaceAdmissionCatalog>,
    entries: Arc<dashmap::DashMap<WorkspaceGenerationAdmissionKey, Arc<AdmissionEntry>>>,
    changes: Arc<tokio::sync::Notify>,
}

struct PendingWorkspaceMutation {
    mutation_id: String,
    changed_paths: Arc<std::collections::BTreeSet<PathBuf>>,
    attempt: u64,
}

#[derive(Clone)]
struct WorkspaceMutationIdentity {
    mutation_id: String,
    changed_paths: Arc<std::collections::BTreeSet<PathBuf>>,
}

#[derive(Default)]
struct WorkspaceMutationLane {
    pending: VecDeque<PendingWorkspaceMutation>,
}

struct AdmissionEntry {
    receipt: watch::Sender<WorkspaceGenerationAdmissionReceipt>,
    active_mutation: watch::Sender<Option<WorkspaceMutationIdentity>>,
    building: AtomicBool,
    attempt: AtomicU64,
    transition: tokio::sync::Mutex<()>,
    mutations: tokio::sync::Mutex<WorkspaceMutationLane>,
    task: parking_lot::Mutex<Option<tokio::task::JoinHandle<()>>>,
}

impl AdmissionEntry {
    fn new(
        receipt: WorkspaceGenerationAdmissionReceipt,
        active_mutation: Option<WorkspaceMutationIdentity>,
    ) -> Self {
        let attempt = receipt.attempt;
        let (sender, _) = watch::channel(receipt);
        let (active_mutation, _) = watch::channel(active_mutation);
        Self {
            receipt: sender,
            active_mutation,
            building: AtomicBool::new(true),
            attempt: AtomicU64::new(attempt),
            transition: tokio::sync::Mutex::new(()),
            mutations: tokio::sync::Mutex::new(WorkspaceMutationLane::default()),
            task: parking_lot::Mutex::new(None),
        }
    }

    fn observed(&self) -> WorkspaceGenerationAdmissionReceipt {
        let mut receipt = self.receipt.borrow().clone();
        receipt.accepted = false;
        receipt
    }
}

impl WorkspaceGenerationAdmission {
    pub fn new(builder: WorkspaceGenerationBuilder) -> Self {
        Self {
            builder,
            catalog: None,
            entries: Arc::new(dashmap::DashMap::new()),
            changes: Arc::new(tokio::sync::Notify::new()),
        }
    }

    pub fn with_catalog(
        mut self,
        catalog: crate::runtime_server_admission_catalog::RuntimeWorkspaceAdmissionCatalog,
    ) -> Self {
        self.catalog = Some(catalog);
        self
    }

    pub async fn admit(
        &self,
        workspace_identity: impl Into<String>,
        project_root: PathBuf,
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
        if let Some(catalog) = &self.catalog {
            catalog
                .record(
                    crate::runtime_server_admission_catalog::RuntimeWorkspaceAdmissionCatalogEntry {
                        workspace_identity: workspace_identity.clone(),
                        project_root: project_root.clone(),
                    },
                )
                .await?;
        }
        if let Some(entry) = self
            .entries
            .get(&key)
            .map(|entry| Arc::clone(entry.value()))
        {
            return self
                .admit_existing(entry, workspace_identity, project_root)
                .await;
        }
        let receipt = WorkspaceGenerationAdmissionReceipt {
            schema_id: WORKSPACE_GENERATION_ADMISSION_RECEIPT_SCHEMA_ID.to_owned(),
            schema_version: "1".to_owned(),
            workspace_identity: workspace_identity.clone(),
            state: WorkspaceGenerationAdmissionState::Building,
            accepted: true,
            attempt: 1,
            commit: None,
            error: None,
        };
        receipt.validate()?;
        let entry = Arc::new(AdmissionEntry::new(receipt.clone(), None));
        let (entry, inserted) = match self.entries.entry(key) {
            dashmap::mapref::entry::Entry::Occupied(existing) => {
                (Arc::clone(existing.get()), false)
            }
            dashmap::mapref::entry::Entry::Vacant(vacant) => {
                vacant.insert(Arc::clone(&entry));
                (entry, true)
            }
        };
        if inserted {
            self.spawn_build(
                Arc::clone(&entry),
                workspace_identity,
                project_root,
                1,
                WorkspaceGenerationBuildMode::RestoreOrBuild,
            );
            return Ok(receipt);
        }
        self.admit_existing(entry, workspace_identity, project_root)
            .await
    }

    async fn admit_existing(
        &self,
        entry: Arc<AdmissionEntry>,
        workspace_identity: String,
        project_root: PathBuf,
    ) -> Result<WorkspaceGenerationAdmissionReceipt, String> {
        let transition = entry.transition.lock().await;
        if entry.building.load(Ordering::Acquire) {
            return Ok(entry.observed());
        }
        entry.building.store(true, Ordering::Release);
        let attempt = entry.attempt.fetch_add(1, Ordering::AcqRel) + 1;
        let receipt = WorkspaceGenerationAdmissionReceipt {
            schema_id: WORKSPACE_GENERATION_ADMISSION_RECEIPT_SCHEMA_ID.to_owned(),
            schema_version: "1".to_owned(),
            workspace_identity: workspace_identity.clone(),
            state: WorkspaceGenerationAdmissionState::Building,
            accepted: true,
            attempt,
            commit: None,
            error: None,
        };
        receipt.validate()?;
        entry.receipt.send_replace(receipt.clone());
        drop(transition);
        self.spawn_build(
            entry,
            workspace_identity,
            project_root,
            attempt,
            WorkspaceGenerationBuildMode::RestoreOrBuild,
        );
        Ok(receipt)
    }

    fn spawn_build(
        &self,
        entry: Arc<AdmissionEntry>,
        workspace_identity: String,
        project_root: PathBuf,
        attempt: u64,
        mut build_mode: WorkspaceGenerationBuildMode,
    ) {
        let builder = Arc::clone(&self.builder);
        let changes = Arc::clone(&self.changes);
        let completed_entry = Arc::clone(&entry);
        let task = tokio::spawn(async move {
            let mut attempt = attempt;
            loop {
                let completed =
                    match builder(workspace_identity.clone(), project_root.clone(), build_mode)
                        .await
                    {
                        Ok(commit) => WorkspaceGenerationAdmissionReceipt {
                            commit: Some(commit),
                            schema_id: WORKSPACE_GENERATION_ADMISSION_RECEIPT_SCHEMA_ID.to_owned(),
                            schema_version: "1".to_owned(),
                            workspace_identity: workspace_identity.clone(),
                            state: WorkspaceGenerationAdmissionState::Ready,
                            accepted: false,
                            attempt,
                            error: None,
                        },
                        Err(error) => WorkspaceGenerationAdmissionReceipt {
                            schema_id: WORKSPACE_GENERATION_ADMISSION_RECEIPT_SCHEMA_ID.to_owned(),
                            schema_version: "1".to_owned(),
                            workspace_identity: workspace_identity.clone(),
                            state: WorkspaceGenerationAdmissionState::Failed,
                            accepted: false,
                            attempt,
                            commit: None,
                            error: Some(error),
                        },
                    };
                let transition = completed_entry.transition.lock().await;
                let mut mutations = completed_entry.mutations.lock().await;
                if let Some(next) = mutations.pending.pop_front() {
                    completed_entry
                        .active_mutation
                        .send_replace(Some(WorkspaceMutationIdentity {
                            mutation_id: next.mutation_id,
                            changed_paths: next.changed_paths,
                        }));
                    attempt = next.attempt;
                    build_mode = WorkspaceGenerationBuildMode::RebuildAfterMutation;
                    completed_entry
                        .receipt
                        .send_replace(WorkspaceGenerationAdmissionReceipt {
                            schema_id: WORKSPACE_GENERATION_ADMISSION_RECEIPT_SCHEMA_ID.to_owned(),
                            schema_version: "1".to_owned(),
                            workspace_identity: workspace_identity.clone(),
                            state: WorkspaceGenerationAdmissionState::Building,
                            accepted: false,
                            attempt,
                            commit: None,
                            error: None,
                        });
                    drop(mutations);
                    drop(transition);
                    changes.notify_waiters();
                    continue;
                }
                completed_entry.receipt.send_replace(completed);
                completed_entry.building.store(false, Ordering::Release);
                drop(mutations);
                drop(transition);
                changes.notify_waiters();
                break;
            }
        });
        *entry.task.lock() = Some(task);
    }

    pub async fn restore_registered(&self) -> Result<WorkspaceGenerationRestoreReport, String> {
        let Some(catalog) = &self.catalog else {
            return Ok(WorkspaceGenerationRestoreReport::default());
        };
        let entries = catalog.snapshot();
        let mut tasks = tokio::task::JoinSet::new();
        for entry in entries.iter().cloned() {
            let admission = self.clone();
            tasks.spawn(async move {
                let receipt = admission
                    .ensure(&entry.workspace_identity, &entry.project_root)
                    .await?;
                if receipt.state == WorkspaceGenerationAdmissionState::Building {
                    admission
                        .wait_terminal(&entry.workspace_identity, &entry.project_root)
                        .await
                } else {
                    Ok(receipt)
                }
            });
        }
        let mut report = WorkspaceGenerationRestoreReport::default();
        while let Some(result) = tasks.join_next().await {
            let receipt = result
                .map_err(|error| format!("workspace admission restore task failed: {error}"))??;
            match receipt.state {
                WorkspaceGenerationAdmissionState::Ready => report.ready.push(receipt),
                WorkspaceGenerationAdmissionState::Failed => report.failed.push(receipt),
                WorkspaceGenerationAdmissionState::Building => {
                    return Err(
                        "workspace admission restore returned a non-terminal receipt".to_owned(),
                    );
                }
            }
        }
        report
            .ready
            .sort_by(|left, right| left.workspace_identity.cmp(&right.workspace_identity));
        report
            .failed
            .sort_by(|left, right| left.workspace_identity.cmp(&right.workspace_identity));
        Ok(report)
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
            .map(|entry| entry.value().receipt.borrow().clone())
    }

    pub async fn ensure(
        &self,
        workspace_identity: &str,
        project_root: &std::path::Path,
    ) -> Result<WorkspaceGenerationAdmissionReceipt, String> {
        if let Some(catalog) = &self.catalog {
            catalog
                .record(
                    crate::runtime_server_admission_catalog::RuntimeWorkspaceAdmissionCatalogEntry {
                        workspace_identity: workspace_identity.to_owned(),
                        project_root: project_root.to_path_buf(),
                    },
                )
                .await?;
        }
        match self.status(workspace_identity, project_root).await {
            Some(receipt) if receipt.state != WorkspaceGenerationAdmissionState::Failed => {
                Ok(receipt)
            }
            Some(_) | None => {
                self.admit(workspace_identity.to_owned(), project_root.to_path_buf())
                    .await
            }
        }
    }

    /// Republish the derived project-root locator for a generation whose
    /// immutable pointer has already been validated by the resident registry.
    /// This path never invokes the generation builder.
    pub async fn publish_resident_generation_locator(
        &self,
        workspace_identity: &str,
        project_root: &std::path::Path,
        commit: WorkspaceGenerationCommitReceipt,
    ) -> Result<WorkspaceGenerationAdmissionReceipt, String> {
        if workspace_identity.trim().is_empty() {
            return Err("workspace generation admission identity must be non-empty".to_owned());
        }
        if !project_root.is_absolute() {
            return Err("workspace generation admission root must be absolute".to_owned());
        }
        let catalog = self
            .catalog
            .as_ref()
            .ok_or_else(|| "workspace generation admission catalog is unavailable".to_owned())?;
        catalog
            .record(
                crate::runtime_server_admission_catalog::RuntimeWorkspaceAdmissionCatalogEntry {
                    workspace_identity: workspace_identity.to_owned(),
                    project_root: project_root.to_path_buf(),
                },
            )
            .await?;
        let attempt = self
            .status(workspace_identity, project_root)
            .await
            .map(|receipt| receipt.attempt)
            .unwrap_or(1);
        let receipt = WorkspaceGenerationAdmissionReceipt {
            schema_id: WORKSPACE_GENERATION_ADMISSION_RECEIPT_SCHEMA_ID.to_owned(),
            schema_version: "1".to_owned(),
            workspace_identity: workspace_identity.to_owned(),
            state: WorkspaceGenerationAdmissionState::Ready,
            accepted: false,
            attempt,
            commit: Some(commit),
            error: None,
        };
        receipt.validate()?;
        Ok(receipt)
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
                return Ok(receipt);
            }
            changed.as_mut().await;
        }
    }

    pub async fn shutdown(&self) -> Result<usize, String> {
        let entries = self
            .entries
            .iter()
            .map(|entry| Arc::clone(entry.value()))
            .collect::<Vec<_>>();
        let mut tasks = Vec::new();
        for entry in entries {
            if let Some(task) = entry.task.lock().take() {
                task.abort();
                tasks.push(task);
            }
        }
        let task_count = tasks.len();
        for task in tasks {
            let result = task.await;
            if let Err(error) = result
                && !error.is_cancelled()
            {
                return Err(format!(
                    "workspace generation admission task failed during shutdown: {error}"
                ));
            }
        }
        Ok(task_count)
    }
}
