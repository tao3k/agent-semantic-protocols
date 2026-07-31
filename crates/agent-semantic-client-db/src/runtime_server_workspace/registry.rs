use super::{
    model::{
        RuntimeDataPlaneCounters, WORKSPACE_RECOVERY_RECEIPT_SCHEMA_ID, WorkspaceGenerationState,
        WorkspaceMemoryBackend, WorkspaceMemoryGeneration, WorkspaceOwnerSnapshot,
        WorkspaceProjectionLease, WorkspaceRecoveryReceipt, WorkspaceRecoverySource,
    },
    segment::{MappedWorkspaceGeneration, WorkspaceGenerationPublisher},
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
struct WorkspaceEntry {
    current: watch::Sender<Option<Arc<WorkspaceMemoryBackend>>>,
    writer: mpsc::Sender<WorkspaceWriteCommand>,
    task: tokio::sync::Mutex<Option<tokio::task::JoinHandle<()>>>,
}

impl Drop for WorkspaceEntry {
    fn drop(&mut self) {
        if let Some(task) = self.task.get_mut().take() {
            task.abort();
        }
    }
}

#[derive(Debug)]
enum WorkspaceWriteCommand {
    Publish {
        request_id: String,
        source: WorkspaceRecoverySource,
        generation: WorkspaceMemoryGeneration,
        reply: oneshot::Sender<Result<WorkspaceRecoveryReceipt, String>>,
    },
    PublishOwnerOverlay {
        request_id: String,
        workspace_identity: String,
        owner: WorkspaceOwnerSnapshot,
        reply: oneshot::Sender<Result<WorkspaceRecoveryReceipt, String>>,
    },
    TombstoneOwnerOverlay {
        request_id: String,
        workspace_identity: String,
        owner_path: String,
        reply: oneshot::Sender<Result<WorkspaceRecoveryReceipt, String>>,
    },
    RelocateOwnerOverlay {
        request_id: String,
        workspace_identity: String,
        previous_owner_path: String,
        owner: WorkspaceOwnerSnapshot,
        reply: oneshot::Sender<Result<WorkspaceRecoveryReceipt, String>>,
    },
    EnsureCanonicalGeneration {
        request_id: String,
        workspace_identity: String,
        materialization: super::canonical_materialization::WorkspaceCanonicalMaterialization,
        reply: oneshot::Sender<Result<WorkspaceRecoveryReceipt, String>>,
    },
    RestoreCheckpoint {
        request_id: String,
        workspace_identity: String,
        path: PathBuf,
        reply: oneshot::Sender<Result<WorkspaceRecoveryReceipt, String>>,
    },
    Shutdown {
        reply: oneshot::Sender<()>,
    },
}

#[derive(Debug, Clone)]
pub struct WorkspaceGenerationLease {
    backend: Arc<WorkspaceMemoryBackend>,
}

impl WorkspaceGenerationLease {
    pub(crate) fn from_backend(backend: Arc<WorkspaceMemoryBackend>) -> Self {
        Self { backend }
    }

    pub fn workspace_identity(&self) -> &str {
        &self.backend.generation().workspace_identity
    }

    pub fn epoch(&self) -> u64 {
        self.backend.generation().active_epoch
    }

    pub fn generation(&self) -> &WorkspaceMemoryGeneration {
        self.backend.generation()
    }

    pub fn project(&self, selector: &str) -> Option<WorkspaceProjectionLease> {
        self.backend.projection(selector)
    }

    pub fn owner(&self, owner_path: &str) -> Option<Arc<[u8]>> {
        self.backend.owner(owner_path)
    }
}

#[derive(Debug)]
pub struct RuntimeServerWorkspaceRegistry {
    root: PathBuf,
    entries: RwLock<HashMap<String, Arc<tokio::sync::OnceCell<Arc<WorkspaceEntry>>>>>,
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

    pub fn lease(&self, workspace_identity: &str) -> Result<WorkspaceGenerationLease, String> {
        let slot = self
            .entries
            .read()
            .get(workspace_identity)
            .cloned()
            .ok_or_else(|| {
                format!(
                    "runtime workspace is not checked out: workspaceIdentity={workspace_identity}"
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
        Ok(WorkspaceGenerationLease { backend })
    }

    pub async fn publish(
        &self,
        request_id: impl Into<String>,
        source: WorkspaceRecoverySource,
        generation: WorkspaceMemoryGeneration,
    ) -> Result<WorkspaceRecoveryReceipt, String> {
        generation.validate()?;
        let entry = self.entry(&generation.workspace_identity).await?;
        let (reply, receive) = oneshot::channel();
        entry
            .writer
            .send(WorkspaceWriteCommand::Publish {
                request_id: request_id.into(),
                source,
                generation,
                reply,
            })
            .await
            .map_err(|_| "runtime workspace writer lane is unavailable".to_owned())?;
        receive
            .await
            .map_err(|_| "runtime workspace writer lane dropped its receipt".to_owned())?
    }

    pub async fn publish_owner_overlay(
        &self,
        request_id: impl Into<String>,
        workspace_identity: impl Into<String>,
        owner: WorkspaceOwnerSnapshot,
    ) -> Result<WorkspaceRecoveryReceipt, String> {
        let workspace_identity = workspace_identity.into();
        let entry = self.entry(&workspace_identity).await?;
        let (reply, receive) = oneshot::channel();
        entry
            .writer
            .send(WorkspaceWriteCommand::PublishOwnerOverlay {
                request_id: request_id.into(),
                workspace_identity,
                owner,
                reply,
            })
            .await
            .map_err(|_| "runtime workspace writer lane is unavailable".to_owned())?;
        receive
            .await
            .map_err(|_| "runtime workspace writer lane dropped its receipt".to_owned())?
    }

    pub async fn tombstone_owner_overlay(
        &self,
        request_id: impl Into<String>,
        workspace_identity: impl Into<String>,
        owner_path: impl Into<String>,
    ) -> Result<WorkspaceRecoveryReceipt, String> {
        let workspace_identity = workspace_identity.into();
        let entry = self.entry(&workspace_identity).await?;
        let (reply, receive) = oneshot::channel();
        entry
            .writer
            .send(WorkspaceWriteCommand::TombstoneOwnerOverlay {
                request_id: request_id.into(),
                workspace_identity,
                owner_path: owner_path.into(),
                reply,
            })
            .await
            .map_err(|_| "runtime workspace writer lane is unavailable".to_owned())?;
        receive
            .await
            .map_err(|_| "runtime workspace writer lane dropped its receipt".to_owned())?
    }

    pub async fn relocate_owner_overlay(
        &self,
        request_id: impl Into<String>,
        workspace_identity: impl Into<String>,
        previous_owner_path: impl Into<String>,
        owner: WorkspaceOwnerSnapshot,
    ) -> Result<WorkspaceRecoveryReceipt, String> {
        let workspace_identity = workspace_identity.into();
        let entry = self.entry(&workspace_identity).await?;
        let (reply, receive) = oneshot::channel();
        entry
            .writer
            .send(WorkspaceWriteCommand::RelocateOwnerOverlay {
                request_id: request_id.into(),
                workspace_identity,
                previous_owner_path: previous_owner_path.into(),
                owner,
                reply,
            })
            .await
            .map_err(|_| "runtime workspace writer lane is unavailable".to_owned())?;
        receive
            .await
            .map_err(|_| "runtime workspace writer lane dropped its receipt".to_owned())?
    }

    pub async fn ensure_canonical_generation(
        &self,
        request_id: impl Into<String>,
        workspace_identity: impl Into<String>,
        materialization: super::canonical_materialization::WorkspaceCanonicalMaterialization,
    ) -> Result<WorkspaceRecoveryReceipt, String> {
        let workspace_identity = workspace_identity.into();
        materialization.validate_persisted(&workspace_identity)?;
        let entry = self.entry(&workspace_identity).await?;
        let (reply, receive) = oneshot::channel();
        entry
            .writer
            .send(WorkspaceWriteCommand::EnsureCanonicalGeneration {
                request_id: request_id.into(),
                workspace_identity,
                materialization,
                reply,
            })
            .await
            .map_err(|_| "runtime workspace writer lane is unavailable".to_owned())?;
        receive
            .await
            .map_err(|_| "runtime workspace writer lane dropped its receipt".to_owned())?
    }

    pub async fn restore_checkpoint(
        &self,
        request_id: impl Into<String>,
        workspace_identity: impl Into<String>,
        path: PathBuf,
    ) -> Result<WorkspaceRecoveryReceipt, String> {
        let workspace_identity = workspace_identity.into();
        let entry = self.entry(&workspace_identity).await?;
        let (reply, receive) = oneshot::channel();
        entry
            .writer
            .send(WorkspaceWriteCommand::RestoreCheckpoint {
                request_id: request_id.into(),
                workspace_identity,
                path,
                reply,
            })
            .await
            .map_err(|_| "runtime workspace writer lane is unavailable".to_owned())?;
        receive
            .await
            .map_err(|_| "runtime workspace writer lane dropped its receipt".to_owned())?
    }

    pub async fn shutdown(&self) -> Result<super::model::RuntimeServerShutdownReceipt, String> {
        let entries = self
            .entries
            .read()
            .values()
            .filter_map(|slot| slot.get().cloned())
            .collect::<Vec<_>>();
        let mut acknowledgements = Vec::with_capacity(entries.len());
        for entry in &entries {
            let (reply, receive) = oneshot::channel();
            entry
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
        for entry in &entries {
            if let Some(task) = entry.task.lock().await.take() {
                task.await
                    .map_err(|error| format!("join runtime workspace writer lane: {error}"))?;
            }
        }
        let receipt = super::model::RuntimeServerShutdownReceipt {
            schema_id: super::model::RUNTIME_SERVER_SHUTDOWN_RECEIPT_SCHEMA_ID.to_owned(),
            schema_version: "1".to_owned(),
            workspace_count: entries.len(),
            writer_lane_count: entries.len(),
            queued_publications_drained: true,
            forced_abort_count: 0,
        };
        receipt.validate()?;
        Ok(receipt)
    }

    async fn entry(&self, workspace_identity: &str) -> Result<Arc<WorkspaceEntry>, String> {
        if workspace_identity.trim().is_empty() {
            return Err("workspace identity must be non-empty text".to_owned());
        }
        let (slot, inserted, workspace_count) = {
            let mut entries = self.entries.write();
            if let Some(slot) = entries.get(workspace_identity) {
                (Arc::clone(slot), false, entries.len())
            } else {
                let slot = Arc::new(tokio::sync::OnceCell::new());
                entries.insert(workspace_identity.to_owned(), Arc::clone(&slot));
                (slot, true, entries.len())
            }
        };
        if inserted {
            self.workspace_count.send_replace(workspace_count);
        }
        let entry = slot
            .get_or_try_init(|| async {
                let directory = self.root.join(workspace_identity).join("generations");
                let pointer_path = directory.join("active-generation.pointer");
                let restored = match super::client::WorkspaceGenerationDataPlaneClient::open_state(
                    &pointer_path,
                )
                .await?
                {
                    super::client::WorkspaceGenerationDataPlaneOpen::Ready(client) => {
                        Some(client.lease().backend)
                    }
                    super::client::WorkspaceGenerationDataPlaneOpen::Missing
                    | super::client::WorkspaceGenerationDataPlaneOpen::RecoveryRequired {
                        ..
                    } => None,
                };
                if restored.is_some() {
                    self.counters
                        .filesystem_reads
                        .fetch_add(2, Ordering::Relaxed);
                }
                let publisher = WorkspaceGenerationPublisher::new(directory).await?;
                let (current, _) = watch::channel(restored);
                let (writer, receiver) = mpsc::channel(self.writer_capacity);
                let task = tokio::spawn(workspace_writer_lane(
                    current.clone(),
                    publisher,
                    Arc::clone(&self.counters),
                    receiver,
                ));
                Ok::<_, String>(Arc::new(WorkspaceEntry {
                    current,
                    writer,
                    task: tokio::sync::Mutex::new(Some(task)),
                }))
            })
            .await?;
        Ok(Arc::clone(entry))
    }
}

async fn workspace_writer_lane(
    current: watch::Sender<Option<Arc<WorkspaceMemoryBackend>>>,
    publisher: WorkspaceGenerationPublisher,
    counters: Arc<RuntimeDataPlaneCounterState>,
    mut receiver: mpsc::Receiver<WorkspaceWriteCommand>,
) {
    let mut last_receipt: Option<WorkspaceRecoveryReceipt> = None;
    while let Some(command) = receiver.recv().await {
        match command {
            WorkspaceWriteCommand::Publish {
                request_id,
                source,
                generation,
                reply,
            } => {
                let active_epoch = active_epoch(&current);
                let result = if generation.active_epoch <= active_epoch {
                    Err(format!(
                        "workspace generation epoch must advance: activeEpoch={active_epoch} targetEpoch={}",
                        generation.active_epoch
                    ))
                } else {
                    publish_generation(
                        &current,
                        &publisher,
                        request_id,
                        source,
                        active_epoch,
                        generation,
                        &counters,
                    )
                    .await
                };
                if let Ok(receipt) = &result {
                    last_receipt = Some(receipt.clone());
                }
                let _ = reply.send(result);
            }
            WorkspaceWriteCommand::PublishOwnerOverlay {
                request_id,
                workspace_identity,
                owner,
                reply,
            } => {
                let result =
                    super::overlay::prepare_owner_overlay(&current, workspace_identity, owner)
                        .and_then(|(active_epoch, generation)| {
                            generation.validate().map(|()| (active_epoch, generation))
                        });
                let result = match result {
                    Ok((active_epoch, generation)) => {
                        publish_generation(
                            &current,
                            &publisher,
                            request_id,
                            WorkspaceRecoverySource::ProviderOwnerOverlay,
                            active_epoch,
                            generation,
                            &counters,
                        )
                        .await
                    }
                    Err(error) => Err(error),
                };
                if let Ok(receipt) = &result {
                    last_receipt = Some(receipt.clone());
                }
                let _ = reply.send(result);
            }
            WorkspaceWriteCommand::TombstoneOwnerOverlay {
                request_id,
                workspace_identity,
                owner_path,
                reply,
            } => {
                let result = super::overlay::prepare_owner_tombstone(
                    &current,
                    workspace_identity,
                    owner_path,
                )
                .and_then(|(active_epoch, generation)| {
                    generation.validate().map(|()| (active_epoch, generation))
                });
                let result = match result {
                    Ok((active_epoch, generation)) => {
                        publish_generation(
                            &current,
                            &publisher,
                            request_id,
                            WorkspaceRecoverySource::ProviderOwnerOverlay,
                            active_epoch,
                            generation,
                            &counters,
                        )
                        .await
                    }
                    Err(error) => Err(error),
                };
                if let Ok(receipt) = &result {
                    last_receipt = Some(receipt.clone());
                }
                let _ = reply.send(result);
            }
            WorkspaceWriteCommand::RelocateOwnerOverlay {
                request_id,
                workspace_identity,
                previous_owner_path,
                owner,
                reply,
            } => {
                let result = super::overlay::prepare_owner_relocation(
                    &current,
                    workspace_identity,
                    previous_owner_path,
                    owner,
                )
                .and_then(|(active_epoch, generation)| {
                    generation.validate().map(|()| (active_epoch, generation))
                });
                let result = match result {
                    Ok((active_epoch, generation)) => {
                        publish_generation(
                            &current,
                            &publisher,
                            request_id,
                            WorkspaceRecoverySource::ProviderOwnerOverlay,
                            active_epoch,
                            generation,
                            &counters,
                        )
                        .await
                    }
                    Err(error) => Err(error),
                };
                if let Ok(receipt) = &result {
                    last_receipt = Some(receipt.clone());
                }
                let _ = reply.send(result);
            }
            WorkspaceWriteCommand::EnsureCanonicalGeneration {
                request_id,
                workspace_identity,
                materialization,
                reply,
            } => {
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
                        last_receipt.clone().or_else(|| {
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
                        publish_generation(
                            &current,
                            &publisher,
                            request_id,
                            WorkspaceRecoverySource::TursoGeneration,
                            active_epoch,
                            generation,
                            &counters,
                        )
                        .await
                    }
                    Err(error) => Err(error),
                }
                .and_then(|receipt| {
                    receipt.validate()?;
                    Ok(receipt)
                });
                if let Ok(receipt) = &result {
                    last_receipt = Some(receipt.clone());
                }
                let _ = reply.send(result);
            }
            WorkspaceWriteCommand::RestoreCheckpoint {
                request_id,
                workspace_identity,
                path,
                reply,
            } => {
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
                    last_receipt = Some(receipt.clone());
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

pub(super) fn generation_digest(
    workspace_identity: &str,
    source_snapshot: &agent_semantic_content_identity::SourceSnapshotEvidence,
    workspace_generation: &agent_semantic_content_identity::workspace_generation_evidence::WorkspaceGenerationEvidenceV1,
    owners: &[WorkspaceOwnerSnapshot],
) -> Result<String, String> {
    let bytes = serde_json::to_vec(&(
        workspace_identity,
        source_snapshot,
        workspace_generation,
        owners,
    ))
    .map_err(|error| format!("encode workspace generation digest input: {error}"))?;
    Ok(format!("blake3-256:{}", blake3::hash(&bytes).to_hex()))
}
