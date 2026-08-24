//! Tokio-owned mutable lane state for one workspace generation admission entry.

use std::path::PathBuf;
use std::sync::Arc;

use super::{
    PendingWorkspaceMutation, WorkspaceGenerationCandidateIdentity, WorkspaceMutationClaim,
    WorkspaceMutationIdentity,
};

#[derive(Clone, Copy, Debug)]
pub(super) struct AdmissionEntryState {
    pub(super) building: bool,
    pub(super) attempt: u64,
}

pub(super) struct MutationClaimReceipt {
    pub(super) candidate: Arc<WorkspaceGenerationCandidateIdentity>,
    pub(super) attempt: u64,
    pub(super) inserted: bool,
}

enum AdmissionEntryCommand {
    StartBuild(tokio::sync::oneshot::Sender<u64>),
    BeginClaimed {
        attempt: u64,
        completed: tokio::sync::oneshot::Sender<()>,
    },
    ClaimMutation {
        mutation_id: String,
        changed_paths: Arc<std::collections::BTreeSet<PathBuf>>,
        candidate: WorkspaceGenerationCandidateIdentity,
        completed: tokio::sync::oneshot::Sender<Result<MutationClaimReceipt, String>>,
    },
    ClearPending(tokio::sync::oneshot::Sender<()>),
    EnqueueMutation {
        mutation: PendingWorkspaceMutation,
        completed: tokio::sync::oneshot::Sender<()>,
    },
    TakeNextMutation(tokio::sync::oneshot::Sender<Option<PendingWorkspaceMutation>>),
    Complete(tokio::sync::oneshot::Sender<()>),
    Shutdown(tokio::sync::oneshot::Sender<()>),
}

#[derive(Clone)]
pub(super) struct AdmissionEntryAuthority {
    commands: tokio::sync::mpsc::Sender<AdmissionEntryCommand>,
    state: tokio::sync::watch::Receiver<AdmissionEntryState>,
    task: std::sync::Arc<
        tokio::sync::Mutex<Option<crate::runtime_server_runtime::RuntimeServerOwnedTask<()>>>,
    >,
    task_scope: crate::runtime_server_runtime::RuntimeServerTaskScope,
}

impl AdmissionEntryAuthority {
    pub(super) fn new(
        building: bool,
        attempt: u64,
        active_mutation: Option<&WorkspaceMutationIdentity>,
        active_mutation_sender: tokio::sync::watch::Sender<Option<WorkspaceMutationIdentity>>,
    ) -> Self {
        let initial = AdmissionEntryState { building, attempt };
        let mut mutation_claims = std::collections::BTreeMap::new();
        if let Some(active) = active_mutation {
            mutation_claims.insert(
                active.mutation_id.clone(),
                WorkspaceMutationClaim {
                    changed_paths: Arc::clone(&active.changed_paths),
                    candidate: Arc::new(active.candidate.clone()),
                    attempt,
                },
            );
        }
        let (state_sender, state) = tokio::sync::watch::channel(initial);
        let (commands, mut receiver) = tokio::sync::mpsc::channel(64);
        let task_scope = crate::runtime_server_runtime::RuntimeServerTaskScope::new(
            "generation-admission-entry",
        );
        let task = task_scope.spawn("generation-admission-entry-actor", async move {
            let mut current = initial;
            let mut mutation_claims = mutation_claims;
            let mut pending_mutations = std::collections::VecDeque::new();
            while let Some(command) = receiver.recv().await {
                match command {
                    AdmissionEntryCommand::StartBuild(completed) => {
                        if !current.building {
                            current.building = true;
                            current.attempt = current.attempt.saturating_add(1);
                            state_sender.send_replace(current);
                        }
                        let _ = completed.send(current.attempt);
                    }
                    AdmissionEntryCommand::ClaimMutation {
                        mutation_id,
                        changed_paths,
                        candidate,
                        completed,
                    } => {
                        let result = if let Some(claimed) = mutation_claims.get(&mutation_id) {
                            if claimed.changed_paths.as_ref() != changed_paths.as_ref()
                                || claimed.candidate.as_ref() != &candidate
                            {
                                Err(format!(
                                    "workspace mutation identity was reused with different candidate evidence: mutationId={mutation_id}"
                                ))
                            } else {
                                Ok(MutationClaimReceipt {
                                    candidate: Arc::clone(&claimed.candidate),
                                    attempt: claimed.attempt,
                                    inserted: false,
                                })
                            }
                        } else {
                            current.attempt = current.attempt.saturating_add(1);
                            state_sender.send_replace(current);
                            let claim = WorkspaceMutationClaim {
                                changed_paths: Arc::clone(&changed_paths),
                                candidate: Arc::new(candidate),
                                attempt: current.attempt,
                            };
                            let receipt = MutationClaimReceipt {
                                candidate: Arc::clone(&claim.candidate),
                                attempt: claim.attempt,
                                inserted: true,
                            };
                            mutation_claims.insert(mutation_id, claim);
                            Ok(receipt)
                        };
                        let _ = completed.send(result);
                    }
                    AdmissionEntryCommand::ClearPending(completed) => {
                        pending_mutations.clear();
                        active_mutation_sender.send_replace(None);
                        let _ = completed.send(());
                    }
                    AdmissionEntryCommand::EnqueueMutation {
                        mutation,
                        completed,
                    } => {
                        pending_mutations.push_back(mutation);
                        let _ = completed.send(());
                    }
                    AdmissionEntryCommand::TakeNextMutation(completed) => {
                        let mutation = pending_mutations.pop_front();
                        active_mutation_sender.send_replace(mutation.as_ref().map(|mutation| {
                            WorkspaceMutationIdentity {
                                mutation_id: mutation.mutation_id.clone(),
                                changed_paths: Arc::clone(&mutation.changed_paths),
                                candidate: mutation.candidate.clone(),
                            }
                        }));
                        let _ = completed.send(mutation);
                    }
                    AdmissionEntryCommand::BeginClaimed { attempt, completed } => {
                        current.building = true;
                        current.attempt = current.attempt.max(attempt);
                        state_sender.send_replace(current);
                        let _ = completed.send(());
                    }
                    AdmissionEntryCommand::Complete(completed) => {
                        if current.building {
                            current.building = false;
                            state_sender.send_replace(current);
                        }
                        let _ = completed.send(());
                    }
                    AdmissionEntryCommand::Shutdown(completed) => {
                        current.building = false;
                        state_sender.send_replace(current);
                        receiver.close();
                        let _ = completed.send(());
                        break;
                    }
                }
            }
        }).expect("new admission-entry task scope accepts its owner task");
        Self {
            commands,
            state,
            task: std::sync::Arc::new(tokio::sync::Mutex::new(Some(task))),
            task_scope,
        }
    }

    pub(super) fn observed(&self) -> AdmissionEntryState {
        *self.state.borrow()
    }

    pub(super) async fn start_build(&self) -> Result<u64, String> {
        let (completed, receipt) = tokio::sync::oneshot::channel();
        self.commands
            .send(AdmissionEntryCommand::StartBuild(completed))
            .await
            .map_err(|_| "workspace generation admission entry authority is closed".to_owned())?;
        receipt.await.map_err(|_| {
            "workspace generation admission entry closed without a start receipt".to_owned()
        })
    }

    pub(super) async fn claim_mutation(
        &self,
        mutation_id: String,
        changed_paths: Arc<std::collections::BTreeSet<PathBuf>>,
        candidate: WorkspaceGenerationCandidateIdentity,
    ) -> Result<MutationClaimReceipt, String> {
        let (completed, receipt) = tokio::sync::oneshot::channel();
        self.commands
            .send(AdmissionEntryCommand::ClaimMutation {
                mutation_id,
                changed_paths,
                candidate,
                completed,
            })
            .await
            .map_err(|_| "workspace generation admission entry authority is closed".to_owned())?;
        receipt.await.map_err(|_| {
            "workspace generation admission entry closed without a mutation claim receipt"
                .to_owned()
        })?
    }

    pub(super) async fn clear_pending(&self) -> Result<(), String> {
        let (completed, receipt) = tokio::sync::oneshot::channel();
        self.commands
            .send(AdmissionEntryCommand::ClearPending(completed))
            .await
            .map_err(|_| "workspace generation admission entry authority is closed".to_owned())?;
        receipt.await.map_err(|_| {
            "workspace generation admission entry closed without a reset receipt".to_owned()
        })
    }

    pub(super) async fn enqueue_mutation(
        &self,
        mutation: PendingWorkspaceMutation,
    ) -> Result<(), String> {
        let (completed, receipt) = tokio::sync::oneshot::channel();
        self.commands
            .send(AdmissionEntryCommand::EnqueueMutation {
                mutation,
                completed,
            })
            .await
            .map_err(|_| "workspace generation admission entry authority is closed".to_owned())?;
        receipt.await.map_err(|_| {
            "workspace generation admission entry closed without an enqueue receipt".to_owned()
        })
    }

    pub(super) async fn take_next_mutation(
        &self,
    ) -> Result<Option<PendingWorkspaceMutation>, String> {
        let (completed, receipt) = tokio::sync::oneshot::channel();
        self.commands
            .send(AdmissionEntryCommand::TakeNextMutation(completed))
            .await
            .map_err(|_| "workspace generation admission entry authority is closed".to_owned())?;
        receipt.await.map_err(|_| {
            "workspace generation admission entry closed without a dequeue receipt".to_owned()
        })
    }

    pub(super) async fn begin_claimed(&self, attempt: u64) -> Result<(), String> {
        let (completed, receipt) = tokio::sync::oneshot::channel();
        self.commands
            .send(AdmissionEntryCommand::BeginClaimed { attempt, completed })
            .await
            .map_err(|_| "workspace generation admission entry authority is closed".to_owned())?;
        receipt.await.map_err(|_| {
            "workspace generation admission entry closed without a claimed start receipt".to_owned()
        })
    }

    pub(super) async fn complete(&self) {
        let (completed, receipt) = tokio::sync::oneshot::channel();
        if self
            .commands
            .send(AdmissionEntryCommand::Complete(completed))
            .await
            .is_ok()
        {
            let _ = receipt.await;
        }
    }

    pub(super) async fn shutdown(&self) {
        let (completed, receipt) = tokio::sync::oneshot::channel();
        if self
            .commands
            .send(AdmissionEntryCommand::Shutdown(completed))
            .await
            .is_ok()
        {
            let _ = receipt.await;
        }
        if let Some(task) = self.task.lock().await.take() {
            self.task_scope.begin_drain();
            let _ = task.join().await;
            let _ = self.task_scope.finish(0);
        }
    }
}
