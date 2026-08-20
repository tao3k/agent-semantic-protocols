use std::collections::{BTreeSet, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

use crate::runtime_server_admission::{
    AdmissionEntry, WorkspaceGenerationAdmissionKey, WorkspaceGenerationAdmissionReceipt,
    WorkspaceMutationIdentity, dispatcher,
};

enum AdmissionRegistryCommand {
    GetOrInsert {
        key: WorkspaceGenerationAdmissionKey,
        receipt: WorkspaceGenerationAdmissionReceipt,
        active_mutation: Option<WorkspaceMutationIdentity>,
        completed: tokio::sync::oneshot::Sender<(Arc<AdmissionEntry>, bool)>,
    },
    ReserveQueryDemand {
        key: WorkspaceGenerationAdmissionKey,
        target_paths: BTreeSet<PathBuf>,
        completed: tokio::sync::oneshot::Sender<bool>,
    },
    ReleaseQueryDemand {
        key: WorkspaceGenerationAdmissionKey,
        target_paths: BTreeSet<PathBuf>,
        completed: tokio::sync::oneshot::Sender<()>,
    },
    Shutdown(tokio::sync::oneshot::Sender<usize>),
}

#[derive(Clone)]
pub(super) struct AdmissionRegistry {
    commands: tokio::sync::mpsc::Sender<AdmissionRegistryCommand>,
    snapshot: tokio::sync::watch::Receiver<
        Arc<std::collections::HashMap<WorkspaceGenerationAdmissionKey, Arc<AdmissionEntry>>>,
    >,
    task: Arc<tokio::sync::Mutex<Option<tokio::task::JoinHandle<()>>>>,
}

impl AdmissionRegistry {
    pub(super) fn new(capacity: usize) -> Self {
        let entries = std::collections::HashMap::with_capacity(capacity);
        let (snapshot_sender, snapshot) = tokio::sync::watch::channel(Arc::new(entries.clone()));
        let (commands, mut receiver) = tokio::sync::mpsc::channel(1);
        let task = dispatcher::spawn_admission_authority(async move {
            let mut entries = entries;
            let mut query_demand_reservations = HashSet::new();
            while let Some(command) = receiver.recv().await {
                let mut batch = vec![command];
                while let Ok(command) = receiver.try_recv() {
                    batch.push(command);
                }
                let mut inserted_replies = Vec::new();
                let mut shutdown = None;
                for command in batch {
                    match command {
                        AdmissionRegistryCommand::GetOrInsert {
                            key,
                            receipt,
                            active_mutation,
                            completed,
                        } => match entries.entry(key) {
                            std::collections::hash_map::Entry::Occupied(existing) => {
                                let _ = completed.send((Arc::clone(existing.get()), false));
                            }
                            std::collections::hash_map::Entry::Vacant(vacant) => {
                                let proposed =
                                    Arc::new(AdmissionEntry::new(receipt, active_mutation));
                                vacant.insert(Arc::clone(&proposed));
                                snapshot_sender.send_replace(Arc::new(entries.clone()));
                                inserted_replies.push((completed, proposed));
                            }
                        },
                        AdmissionRegistryCommand::ReserveQueryDemand {
                            key,
                            target_paths,
                            completed,
                        } => {
                            let _ = completed
                                .send(query_demand_reservations.insert((key, target_paths)));
                        }
                        AdmissionRegistryCommand::ReleaseQueryDemand {
                            key,
                            target_paths,
                            completed,
                        } => {
                            query_demand_reservations.remove(&(key, target_paths));
                            let _ = completed.send(());
                        }
                        AdmissionRegistryCommand::Shutdown(completed) => {
                            receiver.close();
                            shutdown = Some(completed);
                            break;
                        }
                    }
                }
                for (completed, entry) in inserted_replies {
                    let _ = completed.send((entry, true));
                }
                if let Some(completed) = shutdown {
                    let _ = completed.send(entries.len());
                    break;
                }
            }
        });
        Self {
            commands,
            snapshot,
            task: Arc::new(tokio::sync::Mutex::new(Some(task))),
        }
    }

    pub(super) fn get(&self, key: &WorkspaceGenerationAdmissionKey) -> Option<Arc<AdmissionEntry>> {
        self.snapshot.borrow().get(key).cloned()
    }

    pub(super) fn entries(&self) -> Vec<Arc<AdmissionEntry>> {
        self.snapshot.borrow().values().cloned().collect()
    }

    pub(super) async fn get_or_insert(
        &self,
        key: WorkspaceGenerationAdmissionKey,
        receipt: WorkspaceGenerationAdmissionReceipt,
        active_mutation: Option<WorkspaceMutationIdentity>,
    ) -> Result<(Arc<AdmissionEntry>, bool), String> {
        if let Some(entry) = self.get(&key) {
            return Ok((entry, false));
        }
        let (completed, response) = tokio::sync::oneshot::channel();
        self.commands
            .send(AdmissionRegistryCommand::GetOrInsert {
                key,
                receipt,
                active_mutation,
                completed,
            })
            .await
            .map_err(|_| "workspace generation admission authority is unavailable".to_owned())?;
        response.await.map_err(|_| {
            "workspace generation admission authority closed without a receipt".to_owned()
        })
    }

    pub(super) async fn reserve_query_demand(
        &self,
        key: WorkspaceGenerationAdmissionKey,
        target_paths: BTreeSet<PathBuf>,
    ) -> Result<bool, String> {
        let (completed, response) = tokio::sync::oneshot::channel();
        self.commands
            .send(AdmissionRegistryCommand::ReserveQueryDemand {
                key,
                target_paths,
                completed,
            })
            .await
            .map_err(|_| "workspace generation admission authority is unavailable".to_owned())?;
        response.await.map_err(|_| {
            "workspace generation admission authority dropped a query-demand reservation".to_owned()
        })
    }

    pub(super) async fn release_query_demand(
        &self,
        key: WorkspaceGenerationAdmissionKey,
        target_paths: BTreeSet<PathBuf>,
    ) {
        let (completed, response) = tokio::sync::oneshot::channel();
        if self
            .commands
            .send(AdmissionRegistryCommand::ReleaseQueryDemand {
                key,
                target_paths,
                completed,
            })
            .await
            .is_ok()
        {
            let _ = response.await;
        }
    }

    pub(super) async fn shutdown(&self) -> Result<usize, String> {
        let (completed, receipt) = tokio::sync::oneshot::channel();
        if self
            .commands
            .send(AdmissionRegistryCommand::Shutdown(completed))
            .await
            .is_err()
        {
            return Ok(0);
        }
        let entry_count = receipt.await.unwrap_or(0);
        if let Some(task) = self.task.lock().await.take() {
            let _ = task.await;
        }
        Ok(entry_count)
    }
}
