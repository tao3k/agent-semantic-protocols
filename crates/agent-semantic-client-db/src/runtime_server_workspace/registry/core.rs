// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use crate::runtime_server_workspace::{
    RUNTIME_SERVER_SHUTDOWN_RECEIPT_SCHEMA_ID, ResidentOverlayStore, RuntimeDataPlaneCounters,
    RuntimeServerShutdownReceipt, WorkspaceGenerationDataPlaneClient,
    WorkspaceGenerationDataPlaneOpen, WorkspaceGenerationLease, WorkspaceGenerationPublisher,
    WorkspaceMemoryBackend, WorkspaceMemoryGeneration, WorkspaceOwnerSnapshot,
    WorkspaceRecoveryReceipt, WorkspaceRecoverySource, WorkspaceRuntimeContext,
    WorkspaceRuntimeSelectorOverlay, WorkspaceRuntimeSelectorOverlayReceipt,
    workspace_generation_pointer_path,
};
use parking_lot::RwLock;
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};
use tokio::sync::{mpsc, oneshot, watch};

use super::writer_publication_owner::{
    active_epoch, current_generation, publish_generation, publish_staged_overlay_generation,
    restore_checkpoint,
};
use super::{
    canonical_publication_owner as canonical_publication, owner_identity_owner as owner_identity,
};

#[derive(Debug)]
pub(super) struct WorkspaceEntry {
    project_root: PathBuf,
    pub(super) current: watch::Sender<Option<Arc<WorkspaceMemoryBackend>>>,
    pub(super) durability: watch::Sender<
        Option<crate::runtime_server_workspace::WorkspaceGenerationDurabilityReceipt>,
    >,
    overlays: Arc<ResidentOverlayStore>,
    pub(super) publisher: Arc<WorkspaceGenerationPublisher>,
    pub(super) writer: mpsc::Sender<WorkspaceWriteCommand>,
}

#[derive(Clone, Debug)]
pub(super) struct WorkspaceWriteTarget {
    pub(super) scope_key: String,
    pub(super) current: watch::Sender<Option<Arc<WorkspaceMemoryBackend>>>,
    pub(super) durability: watch::Sender<
        Option<crate::runtime_server_workspace::WorkspaceGenerationDurabilityReceipt>,
    >,
    pub(super) overlays: Arc<ResidentOverlayStore>,
    pub(super) publisher: Arc<WorkspaceGenerationPublisher>,
}

impl WorkspaceEntry {
    pub(super) fn write_target(&self) -> WorkspaceWriteTarget {
        WorkspaceWriteTarget {
            scope_key: self.project_root.to_string_lossy().into_owned(),
            current: self.current.clone(),
            durability: self.durability.clone(),
            overlays: Arc::clone(&self.overlays),
            publisher: Arc::clone(&self.publisher),
        }
    }
}

#[derive(Debug)]
struct WorkspaceResident {
    scopes: RwLock<HashMap<String, Arc<tokio::sync::OnceCell<Arc<WorkspaceEntry>>>>>,
    writer: mpsc::Sender<WorkspaceWriteCommand>,
    task: tokio::sync::Mutex<Option<tokio::task::JoinHandle<()>>>,
    activity: Arc<crate::runtime_server_workspace::lease::WorkspaceResidentActivity>,
    context: Arc<WorkspaceRuntimeContext>,
}

#[cfg(test)]
#[path = "../../../tests/unit/runtime_server_workspace_retirement.rs"]
mod retirement_tests;

impl Drop for WorkspaceResident {
    fn drop(&mut self) {
        if let Some(task) = self.task.get_mut().take() {
            task.abort();
        }
    }
}

#[derive(Debug)]
pub(super) enum WorkspaceWriteCommand {
    Publish {
        target: WorkspaceWriteTarget,
        request_id: String,
        source: WorkspaceRecoverySource,
        generation: WorkspaceMemoryGeneration,
        reply: oneshot::Sender<Result<WorkspaceRecoveryReceipt, String>>,
    },
    PublishOwnerOverlay {
        target: WorkspaceWriteTarget,
        request_id: String,
        workspace_identity: String,
        owner: WorkspaceOwnerSnapshot,
        reply: oneshot::Sender<Result<WorkspaceRecoveryReceipt, String>>,
    },
    PublishOwnerDelta(super::writer_publication::PublishOwnerDeltaCommand),
    PublishResidentOwnerDelta(super::writer_publication::PublishResidentOwnerDeltaCommand),
    PublishSelectorOverlay {
        target: WorkspaceWriteTarget,
        workspace_identity: String,
        overlay: WorkspaceRuntimeSelectorOverlay,
        reply: oneshot::Sender<Result<WorkspaceRuntimeSelectorOverlayReceipt, String>>,
    },
    RebindSelectorOverlay {
        target: WorkspaceWriteTarget,
        workspace_identity: String,
        rebind: WorkspaceRuntimeSelectorRebind,
        reply: oneshot::Sender<Result<WorkspaceRuntimeSelectorOverlayReceipt, String>>,
    },
    PublishOwnerIdentityDelta {
        target: WorkspaceWriteTarget,
        source_mutation_id: String,
        workspace_identity: String,
        delta: Vec<super::super::owner_identity_journal::RuntimeOwnerIdentityEntry>,
        reply: oneshot::Sender<Result<(), String>>,
    },
    TombstoneOwnerOverlay {
        target: WorkspaceWriteTarget,
        request_id: String,
        workspace_identity: String,
        owner_path: String,
        reply: oneshot::Sender<Result<WorkspaceRecoveryReceipt, String>>,
    },
    RelocateOwnerOverlay {
        target: WorkspaceWriteTarget,
        request_id: String,
        workspace_identity: String,
        previous_owner_path: String,
        owner: WorkspaceOwnerSnapshot,
        reply: oneshot::Sender<Result<WorkspaceRecoveryReceipt, String>>,
    },
    EnsureCanonicalGeneration(canonical_publication::EnsureCanonicalGenerationCommand),
    RestoreCheckpoint {
        target: WorkspaceWriteTarget,
        request_id: String,
        workspace_identity: String,
        path: PathBuf,
        reply: oneshot::Sender<Result<WorkspaceRecoveryReceipt, String>>,
    },
    Shutdown {
        reply: oneshot::Sender<()>,
    },
}

#[derive(Debug)]
pub struct RuntimeServerWorkspaceRegistry {
    pub(crate) root: PathBuf,
    entries: RwLock<HashMap<String, Arc<WorkspaceResident>>>,
    writer_capacity: usize,
    blocking_lane_ready: tokio::sync::OnceCell<()>,
    counters: Arc<RuntimeDataPlaneCounterState>,
    workspace_count: watch::Sender<usize>,
    pub(super) sparse_provider_owners: super::sparse_provider_owner_cache::SparseProviderOwnerCache,
}

#[derive(Debug, Default)]
pub(super) struct RuntimeDataPlaneCounterState {
    pub(super) filesystem_reads: AtomicU64,
    pub(super) filesystem_writes: AtomicU64,
}

#[path = "counter_state.rs"]
mod counter_state;
#[path = "projection_slots.rs"]
mod projection_slots;

impl RuntimeServerWorkspaceRegistry {
    pub fn root(&self) -> &std::path::Path {
        &self.root
    }

    pub fn new(root: PathBuf) -> Result<Self, String> {
        let writer_capacity =
            crate::runtime_concurrency::RuntimeConcurrencyPlan::current().writer_queue_capacity();
        let (workspace_count, _) = watch::channel(0);
        Ok(Self {
            root,
            entries: RwLock::new(HashMap::new()),
            writer_capacity,
            blocking_lane_ready: tokio::sync::OnceCell::new(),
            counters: Arc::new(RuntimeDataPlaneCounterState::default()),
            workspace_count,
            sparse_provider_owners:
                super::sparse_provider_owner_cache::SparseProviderOwnerCache::new(4_096),
        })
    }

    pub fn workspace_count(&self) -> usize {
        self.entries.read().len()
    }

    /// Returns the cancellation owner for a resident workspace, if admitted.
    ///
    /// The token is deliberately scoped to one workspace entry so shutdown can
    /// cancel in-flight generation work without introducing a process-global
    /// cancellation domain.
    pub fn workspace_context(
        &self,
        workspace_identity: &str,
    ) -> Option<Arc<WorkspaceRuntimeContext>> {
        self.entries
            .read()
            .get(workspace_identity)
            .map(|resident| Arc::clone(&resident.context))
    }

    /// Resolve the only resident project scope admitted for one workspace and
    /// bind it to the immutable base generation. Semantic owner overlays may
    /// advance their request-serving digest without minting a new content
    /// generation, so they are deliberately excluded from this identity.
    /// Public client sessions do not accept a caller-supplied project root; an
    /// absent or multi-scope workspace fails closed instead of crossing a
    /// RuntimeContext boundary.
    pub fn unique_resident_scope(
        &self,
        workspace_identity: &str,
    ) -> Result<(PathBuf, String), String> {
        let resident = self
            .entries
            .read()
            .get(workspace_identity)
            .cloned()
            .ok_or_else(|| {
                format!(
                    "state=workspace-missing reasonKind=client-workspace-not-resident workspaceIdentity={workspace_identity}"
                )
            })?;
        let project_root = {
            let scopes = resident.scopes.read();
            let mut roots = scopes.keys();
            let root = roots.next().ok_or_else(|| {
                format!(
                    "state=scope-missing reasonKind=client-workspace-scope-not-resident workspaceIdentity={workspace_identity}"
                )
            })?;
            if roots.next().is_some() {
                return Err(format!(
                    "state=scope-ambiguous reasonKind=client-workspace-has-multiple-resident-scopes workspaceIdentity={workspace_identity}"
                ));
            }
            PathBuf::from(root)
        };
        let generation_digest = self
            .lease(workspace_identity, &project_root)?
            .generation()
            .generation_digest
            .clone();
        Ok((project_root, generation_digest))
    }

    pub fn subscribe_workspace_count(&self) -> watch::Receiver<usize> {
        self.workspace_count.subscribe()
    }

    pub fn data_plane_counters(&self) -> RuntimeDataPlaneCounters {
        self.counters.snapshot()
    }

    pub fn lease(
        &self,
        workspace_identity: &str,
        project_root: &std::path::Path,
    ) -> Result<WorkspaceGenerationLease, String> {
        let resident = self
            .entries
            .read()
            .get(workspace_identity)
            .cloned()
            .ok_or_else(|| {
                format!(
                    "runtime workspace is not checked out: workspaceIdentity={workspace_identity}"
                )
            })?;
        let project_root = project_root.to_string_lossy();
        let slot = resident
            .scopes
            .read()
            .get(project_root.as_ref())
            .cloned()
            .ok_or_else(|| {
                format!(
                    "runtime workspace source scope is not checked out: workspaceIdentity={workspace_identity} projectRoot={project_root}"
                )
            })?;
        let entry = slot.get().cloned().ok_or_else(|| {
            format!(
                "runtime workspace entry is still initializing: workspaceIdentity={workspace_identity}"
            )
        })?;
        let backend = entry.current.borrow().clone().ok_or_else(|| {
            format!(
                "runtime workspace generation is not ready: workspaceIdentity={workspace_identity}"
            )
        })?;
        WorkspaceGenerationLease::from_resident(
            Arc::clone(&backend),
            entry.overlays.snapshot(backend.generation()),
            Arc::clone(&resident.activity),
        )
    }

    pub fn resident_read_client(
        &self,
        workspace_identity: &str,
        project_root: &std::path::Path,
    ) -> Result<crate::runtime_resident_read::RuntimeResidentReadClient, String> {
        let lease = self.lease(workspace_identity, project_root)?;
        crate::runtime_resident_read::RuntimeResidentReadClient::from_resident_lease(lease)
    }

    pub async fn prepare_resident_workspace_scope(
        &self,
        workspace_identity: &str,
        project_root: &std::path::Path,
    ) -> Result<std::path::PathBuf, String> {
        self.blocking_lane_ready
            .get_or_try_init(|| async {
                tokio::task::spawn_blocking(|| ())
                    .await
                    .map_err(|error| format!("prepare resident blocking lane failed: {error}"))?;
                Ok::<(), String>(())
            })
            .await?;
        let entry = self.entry(workspace_identity, project_root).await?;
        Ok(entry.project_root.clone())
    }

    pub async fn retire_inactive(
        &self,
    ) -> Result<Vec<crate::runtime_server_workspace::ResidentWorkspaceRetirementReceipt>, String>
    {
        self.retire_inactive_inner().await
    }

    async fn retire_inactive_inner(
        &self,
    ) -> Result<Vec<crate::runtime_server_workspace::ResidentWorkspaceRetirementReceipt>, String>
    {
        use crate::runtime_server_workspace::{
            RESIDENT_WORKSPACE_RETIREMENT_RECEIPT_SCHEMA_ID, ResidentWorkspaceRetirementReason,
            ResidentWorkspaceRetirementReceipt,
        };

        const IDLE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(3_600);

        let candidates = self
            .entries
            .read()
            .iter()
            .map(|(identity, resident)| (identity.clone(), Arc::clone(resident)))
            .collect::<Vec<_>>();
        let mut receipts = Vec::new();
        for (workspace_identity, resident) in candidates {
            let project_roots = resident
                .scopes
                .read()
                .keys()
                .map(std::path::PathBuf::from)
                .collect::<Vec<_>>();
            let mut workspace_path_exists = false;
            for project_root in &project_roots {
                match tokio::fs::try_exists(project_root).await {
                    Ok(true) => {
                        workspace_path_exists = true;
                        break;
                    }
                    Ok(false) => {}
                    Err(_) => {
                        // A transient metadata failure must not retire the workspace or
                        // terminate the resident server. Keep the entry until a later sweep
                        // can establish either a missing workspace or the idle timeout.
                        workspace_path_exists = true;
                        break;
                    }
                }
            }
            if !resident
                .activity
                .try_close_for_retirement(workspace_path_exists, IDLE_TIMEOUT)
            {
                continue;
            }
            let removed = {
                let mut entries = self.entries.write();
                if entries
                    .get(&workspace_identity)
                    .is_some_and(|current| Arc::ptr_eq(current, &resident))
                {
                    entries.remove(&workspace_identity)
                } else {
                    None
                }
            };
            let Some(resident) = removed else {
                resident.activity.reopen();
                continue;
            };
            let (reply, receive) = oneshot::channel();
            resident
                .writer
                .send(WorkspaceWriteCommand::Shutdown { reply })
                .await
                .map_err(|_| "runtime workspace writer lane is unavailable".to_owned())?;
            receive
                .await
                .map_err(|_| "runtime workspace writer lane dropped shutdown receipt".to_owned())?;
            if let Some(task) = resident.task.lock().await.take() {
                task.await
                    .map_err(|error| format!("join runtime workspace writer lane: {error}"))?;
            }
            for project_root in &project_roots {
                let pointer_path = workspace_generation_pointer_path(
                    &self.root,
                    &workspace_identity,
                    project_root,
                )?;
                WorkspaceGenerationDataPlaneClient::invalidate_committed_pointer(&pointer_path);
                crate::runtime_server_workspace::WorkspaceExactProjectionDataPlaneClient::invalidate_committed_pointer(&pointer_path);
            }
            let (live_lease_count, in_flight_request_count) = resident.activity.counts();
            let receipt = ResidentWorkspaceRetirementReceipt {
                schema_id: RESIDENT_WORKSPACE_RETIREMENT_RECEIPT_SCHEMA_ID.to_owned(),
                schema_version: "1".to_owned(),
                workspace_identity,
                reason: if workspace_path_exists {
                    ResidentWorkspaceRetirementReason::IdleTimeout
                } else {
                    ResidentWorkspaceRetirementReason::WorkspaceMissing
                },
                idle_timeout_seconds: IDLE_TIMEOUT.as_secs(),
                live_lease_count,
                in_flight_request_count,
                checkpoint_completed: true,
                writer_lane_drained: true,
                endpoint_retired: true,
            };
            receipt.validate()?;
            receipts.push(receipt);
        }
        self.workspace_count.send_replace(self.entries.read().len());
        Ok(receipts)
    }

    pub async fn shutdown(&self) -> Result<RuntimeServerShutdownReceipt, String> {
        self.shutdown_inner().await
    }

    async fn shutdown_inner(&self) -> Result<RuntimeServerShutdownReceipt, String> {
        let residents = self
            .entries
            .read()
            .iter()
            .map(|(workspace_identity, resident)| {
                (workspace_identity.clone(), Arc::clone(resident))
            })
            .collect::<Vec<_>>();
        let workspace_count = residents.len();
        // Cancel every workspace before touching writer lanes.  This gives
        // generation builders a deterministic stop signal while preserving
        // independent cancellation domains for concurrently admitted
        // workspaces.
        for (_, resident) in &residents {
            resident.context.cancel();
        }
        let mut acknowledgements = Vec::with_capacity(residents.len());
        for (_, resident) in &residents {
            let (reply, receive) = oneshot::channel();
            resident
                .writer
                .send(WorkspaceWriteCommand::Shutdown { reply })
                .await
                .map_err(|_| "runtime workspace writer lane is unavailable".to_owned())?;
            acknowledgements.push(receive);
        }
        for acknowledgement in acknowledgements {
            acknowledgement
                .await
                .map_err(|_| "runtime workspace writer lane dropped shutdown receipt".to_owned())?;
        }
        for (_, resident) in &residents {
            if let Some(task) = resident.task.lock().await.take() {
                task.await
                    .map_err(|error| format!("join runtime workspace writer lane: {error}"))?;
            }
        }
        let publishers = residents
            .iter()
            .flat_map(|(_, resident)| {
                resident
                    .scopes
                    .read()
                    .values()
                    .filter_map(|slot| slot.get().map(|entry| Arc::clone(&entry.publisher)))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        for publisher in publishers {
            publisher.shutdown().await?;
        }
        for (workspace_identity, resident) in &residents {
            let project_roots = resident.scopes.read().keys().cloned().collect::<Vec<_>>();
            for project_root in project_roots {
                let pointer_path = workspace_generation_pointer_path(
                    &self.root,
                    workspace_identity,
                    std::path::Path::new(&project_root),
                )?;
                WorkspaceGenerationDataPlaneClient::invalidate_committed_pointer(&pointer_path);
                crate::runtime_server_workspace::WorkspaceExactProjectionDataPlaneClient::invalidate_committed_pointer(&pointer_path);
            }
        }
        let receipt = RuntimeServerShutdownReceipt {
            schema_id: RUNTIME_SERVER_SHUTDOWN_RECEIPT_SCHEMA_ID.to_owned(),
            schema_version: "1".to_owned(),
            workspace_count,
            writer_lane_count: residents.len(),
            queued_publications_drained: true,
            forced_abort_count: 0,
        };
        receipt.validate()?;
        Ok(receipt)
    }

    pub(crate) async fn ensure_entry_ready(
        &self,
        workspace_identity: &str,
        project_root: &std::path::Path,
    ) -> Result<(), String> {
        self.entry(workspace_identity, project_root)
            .await
            .map(|_| ())
    }

    pub(super) async fn entry(
        &self,
        workspace_identity: &str,
        project_root: &std::path::Path,
    ) -> Result<Arc<WorkspaceEntry>, String> {
        if workspace_identity.trim().is_empty() {
            return Err("workspace identity must be non-empty text".to_owned());
        }
        if !project_root.is_absolute() {
            return Err(format!(
                "runtime workspace project root must be absolute: {}",
                project_root.display()
            ));
        }
        let project_root_key = project_root.to_string_lossy().into_owned();
        let (resident, inserted, workspace_count, writer_ready) = {
            let mut entries = self.entries.write();
            if let Some(resident) = entries.get(workspace_identity) {
                (Arc::clone(resident), false, entries.len(), None)
            } else {
                let (writer, receiver) = mpsc::channel(self.writer_capacity);
                let (writer_ready, ready) = oneshot::channel();
                let task = tokio::spawn(workspace_writer_lane(
                    Arc::clone(&self.counters),
                    receiver,
                    writer_ready,
                ));
                let resident = Arc::new(WorkspaceResident {
                    scopes: RwLock::new(HashMap::new()),
                    writer,
                    task: tokio::sync::Mutex::new(Some(task)),
                    activity: Arc::new(
                        crate::runtime_server_workspace::lease::WorkspaceResidentActivity::new(),
                    ),
                    context: Arc::new(WorkspaceRuntimeContext::new(workspace_identity)),
                });
                entries.insert(workspace_identity.to_owned(), Arc::clone(&resident));
                (resident, true, entries.len(), Some(ready))
            }
        };
        if let Some(writer_ready) = writer_ready {
            writer_ready
                .await
                .map_err(|_| "runtime workspace writer lane failed to start".to_owned())?;
        }
        if inserted {
            self.workspace_count.send_replace(workspace_count);
        }
        let _request = resident.activity.begin_request()?;
        let slot = {
            let mut scopes = resident.scopes.write();
            scopes
                .entry(project_root_key.clone())
                .or_insert_with(|| Arc::new(tokio::sync::OnceCell::new()))
                .clone()
        };
        let entry = slot
            .get_or_try_init(|| async {
                let directory = crate::runtime_server_workspace::workspace_generation_directory(
                    &self.root,
                    workspace_identity,
                    project_root,
                )?;
                let pointer_path = directory.join("active-generation.pointer");
                let restored =
                    match WorkspaceGenerationDataPlaneClient::open_state(&pointer_path).await? {
                        WorkspaceGenerationDataPlaneOpen::Ready(client) => {
                            Some(Arc::clone(&client.lease().backend))
                        }
                        WorkspaceGenerationDataPlaneOpen::Missing
                        | WorkspaceGenerationDataPlaneOpen::RecoveryRequired { .. } => None,
                    };
                if restored.is_some() {
                    self.counters
                        .filesystem_reads
                        .fetch_add(2, Ordering::Relaxed);
                }
                let publisher = WorkspaceGenerationPublisher::new(directory).await?;
                if let Some(backend) = restored.as_ref() {
                    // Projection segments are reconstructible from the
                    // canonical materialization. Preserve the generic backend
                    // and its epoch when projection authority is incompatible
                    // so the canonical writer can atomically republish the
                    // complete generation instead of failing entry bootstrap.
                    let _projection_restore = publisher
                        .restore_search_generation_authority(
                            workspace_identity,
                            project_root.to_string_lossy().as_ref(),
                            backend.generation().active_epoch,
                        )
                        .await;
                }
                let overlays = Arc::new(ResidentOverlayStore::new(
                    restored
                        .as_ref()
                        .map(|backend| backend.generation().as_ref()),
                ));
                let (current, _) = watch::channel(restored);
                let (durability, _) = watch::channel(None);
                Ok::<_, String>(Arc::new(WorkspaceEntry {
                    project_root: project_root.to_path_buf(),
                    current,
                    durability,
                    overlays,
                    publisher: Arc::new(publisher),
                    writer: resident.writer.clone(),
                }))
            })
            .await?;
        Ok(Arc::clone(entry))
    }

    pub(super) fn ready_entry(
        &self,
        workspace_identity: &str,
        project_root: &std::path::Path,
    ) -> Result<Option<Arc<WorkspaceEntry>>, String> {
        let project_root = project_root.to_string_lossy();
        let Some(resident) = self.entries.read().get(workspace_identity).cloned() else {
            return Ok(None);
        };
        let _request = resident.activity.begin_request()?;
        Ok(resident
            .scopes
            .read()
            .get(project_root.as_ref())
            .cloned()
            .and_then(|slot| slot.get().cloned()))
    }
}

async fn workspace_writer_lane(
    counters: Arc<RuntimeDataPlaneCounterState>,
    mut receiver: mpsc::Receiver<WorkspaceWriteCommand>,
    ready: oneshot::Sender<()>,
) {
    let _ = ready.send(());
    let mut last_receipts = HashMap::<String, WorkspaceRecoveryReceipt>::new();
    let (durability_sender, mut durability_requests) =
        tokio::sync::mpsc::unbounded_channel::<canonical_publication::DurabilityTask>();
    let mut durability_tasks = Some(durability_sender);
    let mut durability_task = Some(tokio::spawn(async move {
        while let Some(task) = durability_requests.recv().await {
            task.await;
        }
    }));
    while let Some(command) = receiver.recv().await {
        match command {
            WorkspaceWriteCommand::Publish {
                target,
                request_id,
                source,
                generation,
                reply,
            } => {
                let WorkspaceWriteTarget {
                    scope_key,
                    current,
                    durability: _,
                    overlays,
                    publisher,
                } = target;
                let active_epoch = active_epoch(&current);
                let result = if generation.active_epoch <= active_epoch {
                    Err(format!(
                        "workspace generation epoch must advance: activeEpoch={active_epoch} targetEpoch={}",
                        generation.active_epoch
                    ))
                } else {
                    publish_generation(
                        &current,
                        publisher.as_ref(),
                        request_id,
                        source,
                        active_epoch,
                        generation,
                        &counters,
                    )
                    .await
                };
                if let Ok(receipt) = &result {
                    if let Some(backend) = current.borrow().clone() {
                        overlays.reset(backend.generation());
                    }
                    last_receipts.insert(scope_key, receipt.clone());
                }
                let _ = reply.send(result);
            }
            WorkspaceWriteCommand::PublishOwnerOverlay {
                target,
                request_id,
                workspace_identity,
                owner,
                reply,
            } => {
                let scope_key = target.scope_key.clone();
                let result = async {
                    let base = current_generation(&target.current, &workspace_identity)?;
                    let staged = target.overlays.publish_owner(base.generation(), owner)?;
                    publish_staged_overlay_generation(
                        &target,
                        request_id,
                        staged,
                        base.generation().active_epoch,
                        &counters,
                    )
                    .await
                }
                .await;
                if let Ok(receipt) = &result {
                    last_receipts.insert(scope_key, receipt.clone());
                }
                let _ = reply.send(result);
            }
            WorkspaceWriteCommand::PublishOwnerDelta(command) => {
                if let Some((scope_key, receipt)) =
                    super::writer_publication::publish_owner_delta_command(command, &counters).await
                {
                    last_receipts.insert(scope_key, receipt);
                }
            }
            WorkspaceWriteCommand::PublishResidentOwnerDelta(command) => {
                super::writer_publication::publish_resident_owner_delta_command(command).await;
            }
            WorkspaceWriteCommand::PublishSelectorOverlay {
                target,
                workspace_identity,
                overlay,
                reply,
            } => {
                let result = async {
                    let base = current_generation(&target.current, &workspace_identity)?;
                    let (receipt, staged) = target.overlays.publish_selector(
                        base.generation(),
                        &workspace_identity,
                        overlay,
                    )?;
                    if let Some(staged) = staged {
                        target.overlays.commit(staged);
                    }
                    Ok(receipt)
                }
                .await;
                let _ = reply.send(result);
            }
            WorkspaceWriteCommand::RebindSelectorOverlay {
                target,
                workspace_identity,
                rebind,
                reply,
            } => {
                let result = async {
                    let base = current_generation(&target.current, &workspace_identity)?;
                    let (receipt, staged) = target.overlays.rebind_selector(
                        base.generation(),
                        &workspace_identity,
                        rebind,
                    )?;
                    if let Some(staged) = staged {
                        target.overlays.commit(staged);
                    }
                    Ok(receipt)
                }
                .await;
                let _ = reply.send(result);
            }
            WorkspaceWriteCommand::PublishOwnerIdentityDelta {
                target,
                source_mutation_id,
                workspace_identity,
                delta,
                reply,
            } => {
                let result = owner_identity::publish_delta(
                    target,
                    source_mutation_id,
                    workspace_identity,
                    delta,
                )
                .await;
                let _ = reply.send(result);
            }
            WorkspaceWriteCommand::TombstoneOwnerOverlay {
                target,
                request_id,
                workspace_identity,
                owner_path,
                reply,
            } => {
                let scope_key = target.scope_key.clone();
                let result = async {
                    let base = current_generation(&target.current, &workspace_identity)?;
                    let staged = target
                        .overlays
                        .tombstone_owner(base.generation(), &owner_path)?;
                    publish_staged_overlay_generation(
                        &target,
                        request_id,
                        staged,
                        base.generation().active_epoch,
                        &counters,
                    )
                    .await
                }
                .await;
                if let Ok(receipt) = &result {
                    last_receipts.insert(scope_key, receipt.clone());
                }
                let _ = reply.send(result);
            }
            WorkspaceWriteCommand::RelocateOwnerOverlay {
                target,
                request_id,
                workspace_identity,
                previous_owner_path,
                owner,
                reply,
            } => {
                let scope_key = target.scope_key.clone();
                let result = async {
                    let base = current_generation(&target.current, &workspace_identity)?;
                    let staged = target.overlays.relocate_owner(
                        base.generation(),
                        &previous_owner_path,
                        owner,
                    )?;
                    publish_staged_overlay_generation(
                        &target,
                        request_id,
                        staged,
                        base.generation().active_epoch,
                        &counters,
                    )
                    .await
                }
                .await;
                if let Ok(receipt) = &result {
                    last_receipts.insert(scope_key, receipt.clone());
                }
                let _ = reply.send(result);
            }
            WorkspaceWriteCommand::EnsureCanonicalGeneration(command) => {
                canonical_publication::publish_canonical_generation(
                    command,
                    &counters,
                    &mut last_receipts,
                    durability_tasks
                        .as_ref()
                        .expect("durability lane exists until workspace shutdown"),
                )
                .await;
            }
            WorkspaceWriteCommand::RestoreCheckpoint {
                target,
                request_id,
                workspace_identity,
                path,
                reply,
            } => {
                let scope_key = target.scope_key;
                let current = target.current;
                let result = restore_checkpoint(
                    &current,
                    request_id,
                    workspace_identity,
                    path,
                    active_epoch(&current),
                    &counters,
                )
                .await;
                if let Ok(receipt) = &result {
                    last_receipts.insert(scope_key, receipt.clone());
                }
                let _ = reply.send(result);
            }
            WorkspaceWriteCommand::Shutdown { reply } => {
                durability_tasks.take();
                if let Some(task) = durability_task.take() {
                    let _ = task.await;
                }
                let _ = reply.send(());
                break;
            }
        }
    }
}
use crate::runtime_server_workspace::WorkspaceRuntimeSelectorRebind;
