use crate::runtime_server_workspace::{
    MappedWorkspaceGeneration, RUNTIME_SERVER_SHUTDOWN_RECEIPT_SCHEMA_ID, ResidentOverlaySnapshot,
    ResidentOverlayStore, RuntimeDataPlaneCounters, RuntimeServerShutdownReceipt,
    WORKSPACE_RECOVERY_RECEIPT_SCHEMA_ID, WorkspaceCanonicalMaterialization,
    WorkspaceGenerationDataPlaneClient, WorkspaceGenerationDataPlaneOpen, WorkspaceGenerationLease,
    WorkspaceGenerationPublisher, WorkspaceGenerationState, WorkspaceMemoryBackend,
    WorkspaceMemoryGeneration, WorkspaceOwnerSnapshot, WorkspaceRecoveryReceipt,
    WorkspaceRecoverySource, WorkspaceRuntimeSelectorOverlay,
    WorkspaceRuntimeSelectorOverlayReceipt, WorkspaceRuntimeSelectorRead, validate_projection_kind,
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

#[derive(Debug)]
pub(super) struct WorkspaceEntry {
    project_root: PathBuf,
    current: watch::Sender<Option<Arc<WorkspaceMemoryBackend>>>,
    overlays: Arc<ResidentOverlayStore>,
    publisher: Arc<WorkspaceGenerationPublisher>,
    pub(super) writer: mpsc::Sender<WorkspaceWriteCommand>,
}

#[derive(Clone, Debug)]
pub(super) struct WorkspaceWriteTarget {
    scope_key: String,
    current: watch::Sender<Option<Arc<WorkspaceMemoryBackend>>>,
    overlays: Arc<ResidentOverlayStore>,
    publisher: Arc<WorkspaceGenerationPublisher>,
}

impl WorkspaceEntry {
    pub(super) fn write_target(&self) -> WorkspaceWriteTarget {
        WorkspaceWriteTarget {
            scope_key: self.project_root.to_string_lossy().into_owned(),
            current: self.current.clone(),
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
}

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
    PublishSelectorOverlay {
        target: WorkspaceWriteTarget,
        workspace_identity: String,
        overlay: WorkspaceRuntimeSelectorOverlay,
        reply: oneshot::Sender<Result<WorkspaceRuntimeSelectorOverlayReceipt, String>>,
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
    EnsureCanonicalGeneration {
        target: WorkspaceWriteTarget,
        request_id: String,
        workspace_identity: String,
        materialization: WorkspaceCanonicalMaterialization,
        reply: oneshot::Sender<Result<WorkspaceRecoveryReceipt, String>>,
    },
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
    root: PathBuf,
    entries: RwLock<HashMap<String, Arc<WorkspaceResident>>>,
    pub(crate) owner_admissions:
        tokio::sync::Mutex<HashMap<(String, String), Arc<tokio::sync::Mutex<()>>>>,
    writer_capacity: usize,
    counters: Arc<RuntimeDataPlaneCounterState>,
    workspace_count: watch::Sender<usize>,
}

#[derive(Debug, Default)]
struct RuntimeDataPlaneCounterState {
    filesystem_reads: AtomicU64,
    filesystem_writes: AtomicU64,
}

impl RuntimeDataPlaneCounterState {
    fn snapshot(&self) -> RuntimeDataPlaneCounters {
        RuntimeDataPlaneCounters {
            filesystem_reads: self.filesystem_reads.load(Ordering::Relaxed),
            filesystem_writes: self.filesystem_writes.load(Ordering::Relaxed),
            ..RuntimeDataPlaneCounters::default()
        }
    }
}

impl RuntimeServerWorkspaceRegistry {
    pub fn new(root: PathBuf) -> Result<Self, String> {
        let writer_capacity =
            crate::runtime_concurrency::RuntimeConcurrencyPlan::current().writer_queue_capacity();
        let (workspace_count, _) = watch::channel(0);
        Ok(Self {
            root,
            entries: RwLock::new(HashMap::new()),
            owner_admissions: tokio::sync::Mutex::new(HashMap::new()),
            writer_capacity,
            counters: Arc::new(RuntimeDataPlaneCounterState::default()),
            workspace_count,
        })
    }

    pub fn workspace_count(&self) -> usize {
        self.entries.read().len()
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
        Ok(WorkspaceGenerationLease {
            overlay: entry.overlays.snapshot(backend.generation()),
            backend,
        })
    }

    pub(crate) async fn admit_runtime_workspace_root(
        &self,
        project_root: &std::path::Path,
        workspace_identity: &str,
    ) -> Result<PathBuf, String> {
        if let Ok(entry) = self.ready_entry(workspace_identity, project_root) {
            return Ok(entry.project_root.clone());
        }
        let canonical_root = tokio::fs::canonicalize(project_root)
            .await
            .map_err(|error| {
                format!(
                    "failed to resolve runtime project root {}: {error}",
                    project_root.display()
                )
            })?;
        let resolved =
            agent_semantic_client_core::state_core::ResolvedState::resolve(&canonical_root)?;
        if resolved.workspace.workspace_id.to_string() != workspace_identity {
            return Err("runtime owner freshness workspace identity drift".to_owned());
        }
        if !canonical_root.starts_with(&resolved.workspace.root) {
            return Err(format!(
                "runtime project root escapes its workspace: projectRoot={} workspaceRoot={}",
                canonical_root.display(),
                resolved.workspace.root.display()
            ));
        }
        let entry = self.entry(workspace_identity, &canonical_root).await?;
        Ok(entry.project_root.clone())
    }

    pub fn read_runtime_selector(
        &self,
        workspace_identity: &str,
        project_root: &std::path::Path,
        projection_kind: &str,
        structural_selector: &str,
    ) -> Result<WorkspaceRuntimeSelectorRead, String> {
        validate_projection_kind(projection_kind)?;
        let entry = match self.ready_entry(workspace_identity, project_root) {
            Ok(entry) => entry,
            Err(_) => return Ok(WorkspaceRuntimeSelectorRead::GenerationMissing),
        };
        let backend = match entry.current.borrow().clone() {
            Some(backend) => backend,
            None => return Ok(WorkspaceRuntimeSelectorRead::GenerationMissing),
        };
        let lease = WorkspaceGenerationLease {
            overlay: entry.overlays.snapshot(backend.generation()),
            backend,
        };
        lease.read_runtime_selector(projection_kind, structural_selector)
    }

    pub async fn shutdown(&self) -> Result<RuntimeServerShutdownReceipt, String> {
        let residents = self.entries.read().values().cloned().collect::<Vec<_>>();
        let workspace_count = residents.len();
        let mut acknowledgements = Vec::with_capacity(residents.len());
        for resident in &residents {
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
        for resident in &residents {
            if let Some(task) = resident.task.lock().await.take() {
                task.await
                    .map_err(|error| format!("join runtime workspace writer lane: {error}"))?;
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
        let (resident, inserted, workspace_count) = {
            let mut entries = self.entries.write();
            if let Some(resident) = entries.get(workspace_identity) {
                (Arc::clone(resident), false, entries.len())
            } else {
                let (writer, receiver) = mpsc::channel(self.writer_capacity);
                let task =
                    tokio::spawn(workspace_writer_lane(Arc::clone(&self.counters), receiver));
                let resident = Arc::new(WorkspaceResident {
                    scopes: RwLock::new(HashMap::new()),
                    writer,
                    task: tokio::sync::Mutex::new(Some(task)),
                });
                entries.insert(workspace_identity.to_owned(), Arc::clone(&resident));
                (resident, true, entries.len())
            }
        };
        if inserted {
            self.workspace_count.send_replace(workspace_count);
        }
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
                let restored = match WorkspaceGenerationDataPlaneClient::open_state(&pointer_path)
                    .await?
                {
                    WorkspaceGenerationDataPlaneOpen::Ready(client) => Some(client.lease().backend),
                    WorkspaceGenerationDataPlaneOpen::Missing
                    | WorkspaceGenerationDataPlaneOpen::RecoveryRequired { .. } => None,
                };
                if restored.is_some() {
                    self.counters
                        .filesystem_reads
                        .fetch_add(2, Ordering::Relaxed);
                }
                let publisher = WorkspaceGenerationPublisher::new(directory).await?;
                let overlays = Arc::new(ResidentOverlayStore::new(
                    restored
                        .as_ref()
                        .map(|backend| backend.generation().as_ref()),
                ));
                let (current, _) = watch::channel(restored);
                Ok::<_, String>(Arc::new(WorkspaceEntry {
                    project_root: project_root.to_path_buf(),
                    current,
                    overlays,
                    publisher: Arc::new(publisher),
                    writer: resident.writer.clone(),
                }))
            })
            .await?;
        Ok(Arc::clone(entry))
    }

    fn ready_entry(
        &self,
        workspace_identity: &str,
        project_root: &std::path::Path,
    ) -> Result<Arc<WorkspaceEntry>, String> {
        let project_root = project_root.to_string_lossy();
        self.entries
            .read()
            .get(workspace_identity)
            .and_then(|resident| resident.scopes.read().get(project_root.as_ref()).cloned())
            .and_then(|slot| slot.get().cloned())
            .ok_or_else(|| {
                format!(
                    "runtime workspace source scope is not checked out: workspaceIdentity={workspace_identity} projectRoot={project_root}"
                )
            })
    }
}

async fn workspace_writer_lane(
    counters: Arc<RuntimeDataPlaneCounterState>,
    mut receiver: mpsc::Receiver<WorkspaceWriteCommand>,
) {
    let mut last_receipts = HashMap::<String, WorkspaceRecoveryReceipt>::new();
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
            WorkspaceWriteCommand::EnsureCanonicalGeneration {
                target,
                request_id,
                workspace_identity,
                materialization,
                reply,
            } => {
                let WorkspaceWriteTarget {
                    scope_key,
                    current,
                    overlays,
                    publisher,
                } = target;
                let active = current.borrow().clone();
                let active_epoch = active
                    .as_ref()
                    .map_or(0, |backend| backend.generation().active_epoch);
                let generation = materialization.into_generation(active_epoch);
                let result = match generation {
                    Ok(generation)
                        if active.as_ref().is_some_and(|backend| {
                            backend.generation().generation_digest == generation.generation_digest
                        })
                            && tokio::fs::try_exists(publisher.pointer_path())
                                .await
                                .unwrap_or(false) =>
                    {
                        last_receipts.get(&scope_key).cloned().or_else(|| {
                            let target_epoch = active_epoch;
                            target_epoch.checked_sub(1).map(|previous_epoch| {
                                WorkspaceRecoveryReceipt {
                                    schema_id: WORKSPACE_RECOVERY_RECEIPT_SCHEMA_ID.to_owned(),
                                    schema_version: "1".to_owned(),
                                    request_id,
                                    workspace_identity,
                                    source: WorkspaceRecoverySource::MmapCheckpoint,
                                    state: WorkspaceGenerationState::Ready,
                                    active_epoch: previous_epoch,
                                    target_epoch,
                                    old_generation_readable: previous_epoch != 0,
                                    counters: RuntimeDataPlaneCounters::default(),
                                }
                            })
                        })
                        .ok_or_else(|| {
                            "runtime workspace canonical generation has no reusable recovery receipt"
                                .to_owned()
                        })
                    }
                    Ok(generation) => {
                        let result = publish_generation(
                            &current,
                            publisher.as_ref(),
                            request_id,
                            WorkspaceRecoverySource::TursoGeneration,
                            active_epoch,
                            generation,
                            &counters,
                        )
                        .await;
                        if result.is_ok()
                            && let Some(backend) = current.borrow().clone()
                        {
                            overlays.reset(backend.generation());
                        }
                        result
                    }
                    Err(error) => Err(error),
                }
                .and_then(|receipt| {
                    receipt.validate()?;
                    Ok(receipt)
                });
                if let Ok(receipt) = &result {
                    if let Some(backend) = current.borrow().clone() {
                        overlays.reset(backend.generation());
                    }
                    last_receipts.insert(scope_key, receipt.clone());
                }
                let _ = reply.send(result);
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
                let _ = reply.send(());
                break;
            }
        }
    }
}

fn current_generation(
    current: &watch::Sender<Option<Arc<WorkspaceMemoryBackend>>>,
    workspace_identity: &str,
) -> Result<Arc<WorkspaceMemoryBackend>, String> {
    current.borrow().clone().ok_or_else(|| {
        format!(
            "runtime owner overlay requires an admitted canonical generation: workspaceIdentity={workspace_identity}"
        )
    })
}

async fn publish_staged_overlay_generation(
    target: &WorkspaceWriteTarget,
    request_id: String,
    staged: ResidentOverlaySnapshot,
    active_epoch: u64,
    counters: &RuntimeDataPlaneCounterState,
) -> Result<WorkspaceRecoveryReceipt, String> {
    let base = current_generation(&target.current, "staged-overlay")?;
    if base.generation().active_epoch != active_epoch {
        return Err(format!(
            "runtime overlay base epoch drift: expected={active_epoch} actual={}",
            base.generation().active_epoch
        ));
    }
    let generation = staged.materialize_generation(base.generation())?;
    let receipt = publish_generation(
        &target.current,
        target.publisher.as_ref(),
        request_id,
        WorkspaceRecoverySource::ProviderOwnerOverlay,
        active_epoch,
        generation,
        counters,
    )
    .await?;
    if let Some(backend) = target.current.borrow().clone() {
        target.overlays.reset(backend.generation());
    }
    Ok(receipt)
}

fn active_epoch(current: &watch::Sender<Option<Arc<WorkspaceMemoryBackend>>>) -> u64 {
    current
        .borrow()
        .as_ref()
        .map_or(0, |backend| backend.generation().active_epoch)
}

async fn publish_generation(
    current: &watch::Sender<Option<Arc<WorkspaceMemoryBackend>>>,
    publisher: &WorkspaceGenerationPublisher,
    request_id: String,
    source: WorkspaceRecoverySource,
    active_epoch: u64,
    generation: WorkspaceMemoryGeneration,
    counters: &RuntimeDataPlaneCounterState,
) -> Result<WorkspaceRecoveryReceipt, String> {
    let target_epoch = generation.active_epoch;
    let workspace_identity = generation.workspace_identity.clone();
    let (_, mapped) = publisher.publish(&generation, active_epoch != 0).await?;
    counters.filesystem_reads.fetch_add(1, Ordering::Relaxed);
    counters.filesystem_writes.fetch_add(1, Ordering::Relaxed);
    current.send_replace(Some(mapped.backend()));
    let receipt = WorkspaceRecoveryReceipt {
        schema_id: WORKSPACE_RECOVERY_RECEIPT_SCHEMA_ID.to_owned(),
        schema_version: "1".to_owned(),
        request_id,
        workspace_identity,
        source,
        state: WorkspaceGenerationState::Ready,
        active_epoch,
        target_epoch,
        old_generation_readable: active_epoch != 0,
        counters: RuntimeDataPlaneCounters {
            filesystem_reads: 1,
            filesystem_writes: 1,
            ..RuntimeDataPlaneCounters::default()
        },
    };
    receipt.validate()?;
    Ok(receipt)
}

async fn restore_checkpoint(
    current: &watch::Sender<Option<Arc<WorkspaceMemoryBackend>>>,
    request_id: String,
    workspace_identity: String,
    path: PathBuf,
    active_epoch: u64,
    counters: &RuntimeDataPlaneCounterState,
) -> Result<WorkspaceRecoveryReceipt, String> {
    let mapped = MappedWorkspaceGeneration::open(&path).await?;
    counters.filesystem_reads.fetch_add(1, Ordering::Relaxed);
    let backend = mapped.backend();
    if backend.generation().workspace_identity != workspace_identity {
        return Err(format!(
            "workspace checkpoint identity mismatch: expected={workspace_identity} actual={}",
            backend.generation().workspace_identity
        ));
    }
    let target_epoch = backend.generation().active_epoch;
    if target_epoch <= active_epoch {
        return Err(format!(
            "workspace checkpoint epoch must advance: activeEpoch={active_epoch} targetEpoch={target_epoch}"
        ));
    }
    current.send_replace(Some(backend));
    let receipt = WorkspaceRecoveryReceipt {
        schema_id: WORKSPACE_RECOVERY_RECEIPT_SCHEMA_ID.to_owned(),
        schema_version: "1".to_owned(),
        request_id,
        workspace_identity,
        source: WorkspaceRecoverySource::MmapCheckpoint,
        state: WorkspaceGenerationState::Ready,
        active_epoch,
        target_epoch,
        old_generation_readable: active_epoch != 0,
        counters: RuntimeDataPlaneCounters {
            filesystem_reads: 1,
            ..RuntimeDataPlaneCounters::default()
        },
    };
    receipt.validate()?;
    Ok(receipt)
}
