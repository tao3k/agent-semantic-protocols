use std::sync::Arc;

use crate::runtime_server_admission::{
    AdmissionEntry, WorkspaceGenerationAdmissionKey, WorkspaceGenerationAdmissionReceipt,
    WorkspaceMutationIdentity,
};

enum AdmissionRegistryCommand {
    Shutdown(tokio::sync::oneshot::Sender<usize>),
}

#[derive(Clone)]
pub(super) struct AdmissionRegistry {
    // Server-local generation leases are claimed synchronously in this map.
    // The mailbox below is retained only for shutdown/reservation lifecycle;
    // no client demand is allowed to queue behind it.
    entries: Arc<dashmap::DashMap<WorkspaceGenerationAdmissionKey, Arc<AdmissionEntry>>>,
    snapshot_sender: tokio::sync::watch::Sender<
        Arc<std::collections::HashMap<WorkspaceGenerationAdmissionKey, Arc<AdmissionEntry>>>,
    >,
    commands: tokio::sync::mpsc::Sender<AdmissionRegistryCommand>,
    snapshot: tokio::sync::watch::Receiver<
        Arc<std::collections::HashMap<WorkspaceGenerationAdmissionKey, Arc<AdmissionEntry>>>,
    >,
    task:
        Arc<tokio::sync::Mutex<Option<crate::runtime_server_runtime::RuntimeServerOwnedTask<()>>>>,
    task_scope: crate::runtime_server_runtime::RuntimeServerTaskScope,
}

impl AdmissionRegistry {
    pub(super) fn new(capacity: usize) -> Self {
        let entries = std::collections::HashMap::with_capacity(capacity);
        let (snapshot_sender, snapshot) = tokio::sync::watch::channel(Arc::new(entries));
        let server_entries = Arc::new(dashmap::DashMap::with_capacity(capacity));
        // Admission is single-owner, but it is not single-request.  A one-slot
        // mailbox serialized a concurrent cold burst *before* the owner could
        // coalesce it, inflating the request-path p99 even though only one
        // generation was ever built.  Size the mailbox from the Runtime
        // Server's host-adaptive control-plane capacity and publish one
        // immutable snapshot per drained batch.
        let (commands, mut receiver) = tokio::sync::mpsc::channel(capacity.max(1));
        let server_entries_for_actor = Arc::clone(&server_entries);
        let task_scope =
            crate::runtime_server_runtime::RuntimeServerTaskScope::new("generation-registry");
        let task = task_scope
            .spawn("generation-registry-actor", async move {
                while let Some(command) = receiver.recv().await {
                    let mut batch = vec![command];
                    while let Ok(command) = receiver.try_recv() {
                        batch.push(command);
                    }
                    let mut shutdown = None;
                    for command in batch {
                        match command {
                            AdmissionRegistryCommand::Shutdown(completed) => {
                                receiver.close();
                                shutdown = Some(completed);
                                break;
                            }
                        }
                    }
                    if let Some(completed) = shutdown {
                        let entry_count = server_entries_for_actor.len();
                        let _ = completed.send(entry_count);
                        break;
                    }
                }
            })
            .expect("new generation-registry task scope accepts its owner task");
        Self {
            entries: server_entries,
            snapshot_sender,
            commands,
            snapshot,
            task: Arc::new(tokio::sync::Mutex::new(Some(task))),
            task_scope,
        }
    }

    pub(super) fn get(&self, key: &WorkspaceGenerationAdmissionKey) -> Option<Arc<AdmissionEntry>> {
        self.entries.get(key).map(|entry| Arc::clone(entry.value()))
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
        match self.entries.entry(key) {
            dashmap::mapref::entry::Entry::Occupied(existing) => {
                Ok((Arc::clone(existing.get()), false))
            }
            dashmap::mapref::entry::Entry::Vacant(vacant) => {
                let entry = Arc::new(AdmissionEntry::new(receipt, active_mutation));
                vacant.insert(Arc::clone(&entry));
                self.snapshot_sender.send_replace(Arc::new(
                    self.entries
                        .iter()
                        .map(|entry| (entry.key().clone(), Arc::clone(entry.value())))
                        .collect(),
                ));
                Ok((entry, true))
            }
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
        let _actor_entry_count = receipt.await.unwrap_or(0);
        self.task_scope.begin_drain();
        if let Some(task) = self.task.lock().await.take() {
            task.join().await?;
        }
        self.task_scope.finish(0)?;
        Ok(self.entries.len())
    }
}
