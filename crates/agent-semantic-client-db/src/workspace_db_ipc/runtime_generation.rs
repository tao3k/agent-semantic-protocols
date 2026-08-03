use super::protocol::{WorkspaceDbIpcOperation, WorkspaceDbIpcResult, WorkspaceDbIpcSession};

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct MutationFlightKey {
    socket_path: String,
    owner_epoch: u64,
    workspace_identity: String,
}

#[derive(Debug)]
struct MutationFlight {
    mutation_id: String,
    previous: parking_lot::Mutex<Option<std::sync::Arc<MutationFlight>>>,
    result: tokio::sync::watch::Sender<
        Option<
            Result<
                crate::runtime_server_admission::WorkspaceGenerationMutationAdmissionReceipt,
                String,
            >,
        >,
    >,
}

#[derive(Debug, Default)]
pub(super) struct MutationWorkspaceLane {
    current: parking_lot::RwLock<Option<std::sync::Arc<MutationFlight>>>,
}

impl MutationWorkspaceLane {
    fn current(&self) -> Option<std::sync::Arc<MutationFlight>> {
        self.current.read().clone()
    }

    fn replace_if(
        &self,
        expected: Option<&std::sync::Arc<MutationFlight>>,
        next: std::sync::Arc<MutationFlight>,
    ) -> bool {
        let mut current = self.current.write();
        let matches = match (current.as_ref(), expected) {
            (None, None) => true,
            (Some(current), Some(expected)) => std::sync::Arc::ptr_eq(current, expected),
            _ => false,
        };
        if matches {
            *current = Some(next);
        }
        matches
    }
}

fn find_mutation_flight(
    current: std::sync::Arc<MutationFlight>,
    mutation_id: &str,
) -> Option<std::sync::Arc<MutationFlight>> {
    let mut candidate = Some(current);
    while let Some(flight) = candidate {
        if flight.mutation_id == mutation_id {
            return Some(flight);
        }
        candidate = flight.previous.lock().clone();
    }
    None
}

static MUTATION_FLIGHTS: std::sync::LazyLock<
    dashmap::DashMap<MutationFlightKey, std::sync::Arc<MutationWorkspaceLane>>,
> = std::sync::LazyLock::new(|| {
    let capacity = std::thread::available_parallelism()
        .map(std::num::NonZeroUsize::get)
        .unwrap_or(1);
    dashmap::DashMap::with_capacity(capacity)
});

fn mutation_flights()
-> &'static dashmap::DashMap<MutationFlightKey, std::sync::Arc<MutationWorkspaceLane>> {
    &MUTATION_FLIGHTS
}

pub(super) fn runtime_generation_mutation_lane(
    socket_path: &str,
    owner_epoch: u64,
    workspace_identity: &str,
) -> std::sync::Arc<MutationWorkspaceLane> {
    let key = MutationFlightKey {
        socket_path: socket_path.to_owned(),
        owner_epoch,
        workspace_identity: workspace_identity.to_owned(),
    };
    mutation_flights()
        .entry(key)
        .or_insert_with(|| std::sync::Arc::new(MutationWorkspaceLane::default()))
        .clone()
}

async fn wait_for_mutation_flight(
    flight: &MutationFlight,
) -> Result<crate::runtime_server_admission::WorkspaceGenerationMutationAdmissionReceipt, String> {
    let mut result = flight.result.subscribe();
    loop {
        if let Some(result) = result.borrow().clone() {
            return result;
        }
        result.changed().await.map_err(|_| {
            "runtime generation mutation flight closed without a receipt".to_owned()
        })?;
    }
}

fn normalize_changed_paths(changed_paths: Vec<String>) -> Result<Vec<String>, String> {
    let changed_paths = changed_paths
        .into_iter()
        .filter(|path| !path.trim().is_empty())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    if changed_paths.is_empty() {
        return Err("runtime generation admission requires changed paths".to_owned());
    }
    Ok(changed_paths)
}

impl WorkspaceDbIpcSession {
    pub async fn ensure_runtime_owner(
        &self,
        language_id: impl Into<String>,
        owner_path: impl Into<String>,
    ) -> Result<crate::runtime_server_workspace::WorkspaceRuntimeOwnerFreshnessReceipt, String>
    {
        match self
            .call_operation(WorkspaceDbIpcOperation::EnsureRuntimeOwner {
                project_root: self.runtime_project_root()?.display().to_string(),
                language_id: language_id.into(),
                owner_path: owner_path.into(),
            })
            .await?
        {
            WorkspaceDbIpcResult::RuntimeOwnerFreshness { receipt } => Ok(receipt),
            _ => Err(
                "Runtime Server returned an unexpected runtime owner freshness result".to_owned(),
            ),
        }
    }

    pub async fn read_runtime_selector(
        &self,
        projection_kind: impl Into<String>,
        structural_selector: impl Into<String>,
    ) -> Result<crate::runtime_server_workspace::WorkspaceRuntimeSelectorRead, String> {
        match self
            .call_operation(WorkspaceDbIpcOperation::ReadRuntimeSelector {
                project_root: self.runtime_project_root()?.display().to_string(),
                projection_kind: projection_kind.into(),
                structural_selector: structural_selector.into(),
            })
            .await?
        {
            WorkspaceDbIpcResult::RuntimeSelector { read } => Ok(read),
            _ => Err("Runtime Server returned an unexpected runtime selector result".to_owned()),
        }
    }

    pub async fn publish_runtime_selector_overlay(
        &self,
        overlay: crate::runtime_server_workspace::WorkspaceRuntimeSelectorOverlay,
    ) -> Result<crate::runtime_server_workspace::WorkspaceRuntimeSelectorOverlayReceipt, String>
    {
        match self
            .call_operation(WorkspaceDbIpcOperation::PublishRuntimeSelectorOverlay {
                project_root: self.runtime_project_root()?.display().to_string(),
                overlay,
            })
            .await?
        {
            WorkspaceDbIpcResult::RuntimeSelectorOverlay { receipt } => Ok(receipt),
            _ => Err(
                "Runtime Server returned an unexpected runtime selector overlay result".to_owned(),
            ),
        }
    }

    pub async fn admit_runtime_generation(
        &self,
        mutation_id: impl Into<String>,
        changed_paths: Vec<String>,
    ) -> Result<crate::runtime_server_admission::WorkspaceGenerationMutationAdmissionReceipt, String>
    {
        let mutation_id = mutation_id.into();
        if mutation_id.trim().is_empty() {
            return Err("runtime generation admission requires a mutation id".to_owned());
        }
        let changed_paths = normalize_changed_paths(changed_paths)?;
        let lane = &self.shared.runtime_generation_mutations;
        loop {
            enum FlightRole {
                Lead(std::sync::Arc<MutationFlight>),
                Follow(std::sync::Arc<MutationFlight>),
                WaitThenRetry(std::sync::Arc<MutationFlight>),
            }

            let current = lane.current();
            let role = if let Some(current) = current {
                if let Some(matching) = find_mutation_flight(current.clone(), &mutation_id) {
                    FlightRole::Follow(matching)
                } else if current.result.borrow().is_some() {
                    let (result, _) = tokio::sync::watch::channel(None);
                    let flight = std::sync::Arc::new(MutationFlight {
                        mutation_id: mutation_id.clone(),
                        previous: parking_lot::Mutex::new(None),
                        result,
                    });
                    if lane.replace_if(Some(&current), std::sync::Arc::clone(&flight)) {
                        FlightRole::Lead(flight)
                    } else {
                        continue;
                    }
                } else {
                    FlightRole::WaitThenRetry(current)
                }
            } else {
                let (result, _) = tokio::sync::watch::channel(None);
                let flight = std::sync::Arc::new(MutationFlight {
                    mutation_id: mutation_id.clone(),
                    previous: parking_lot::Mutex::new(None),
                    result,
                });
                if lane.replace_if(None, std::sync::Arc::clone(&flight)) {
                    FlightRole::Lead(flight)
                } else {
                    continue;
                }
            };

            match role {
                FlightRole::Lead(flight) => {
                    let result = self
                        .call_runtime_generation_admission(
                            mutation_id.clone(),
                            self.runtime_project_root()?.display().to_string(),
                            changed_paths.clone(),
                        )
                        .await;
                    flight.result.send_replace(Some(result.clone()));
                    return result;
                }
                FlightRole::Follow(flight) => return wait_for_mutation_flight(&flight).await,
                FlightRole::WaitThenRetry(flight) => {
                    let _ = wait_for_mutation_flight(&flight).await;
                }
            }
        }
    }

    pub async fn submit_runtime_generation_mutation(
        &self,
        mutation_id: impl Into<String>,
        changed_paths: Vec<String>,
    ) -> Result<crate::runtime_server_admission::WorkspaceGenerationMutationSubmissionReceipt, String>
    {
        let mutation_id = mutation_id.into();
        if mutation_id.trim().is_empty() {
            return Err("runtime generation submission requires a mutation id".to_owned());
        }
        let changed_paths = normalize_changed_paths(changed_paths)?;
        // Submission is acknowledged only after the resident Runtime Server has
        // accepted the mutation.  Merkle/provider rebuilding remains
        // server-owned background work; a short-lived CLI process must never
        // own the task whose exit could silently discard changed paths.
        let admission = self
            .call_runtime_generation_admission(
                mutation_id.clone(),
                self.runtime_project_root()?.display().to_string(),
                changed_paths.clone(),
            )
            .await?;
        let state = if admission.receipts.iter().all(|receipt| !receipt.accepted) {
            crate::runtime_server_admission::WorkspaceGenerationMutationSubmissionState::Coalesced
        } else {
            crate::runtime_server_admission::WorkspaceGenerationMutationSubmissionState::Queued
        };

        let receipt =
            crate::runtime_server_admission::WorkspaceGenerationMutationSubmissionReceipt {
                schema_id: crate::runtime_server_admission::WORKSPACE_GENERATION_MUTATION_SUBMISSION_RECEIPT_SCHEMA_ID.to_owned(),
                schema_version: "1".to_owned(),
                mutation_id,
                workspace_identity: self.workspace_identity().to_owned(),
                changed_path_count: changed_paths.len(),
                state,
            };
        receipt.validate()?;
        Ok(receipt)
    }

    pub async fn ensure_runtime_generation(
        &self,
    ) -> Result<crate::runtime_server_admission::WorkspaceGenerationAdmissionReceipt, String> {
        let pending_mutation = self.shared.runtime_generation_mutations.current();
        if let Some(pending_mutation) = pending_mutation {
            wait_for_mutation_flight(&pending_mutation).await?;
        }
        let project_root = self.runtime_project_root()?.to_path_buf();
        let candidate =
            crate::runtime_server_admission::discover_workspace_generation_candidate(&project_root)
                .await?;
        match self
            .call_operation(WorkspaceDbIpcOperation::EnsureRuntimeGeneration {
                project_root: project_root.display().to_string(),
                candidate,
            })
            .await?
        {
            WorkspaceDbIpcResult::RuntimeGenerationAdmission { receipt } => Ok(receipt),
            _ => Err("Runtime Server returned an unexpected ensured generation result".to_owned()),
        }
    }

    pub async fn repair_runtime_generation_locator(
        &self,
    ) -> Result<crate::runtime_server_admission::WorkspaceGenerationReadinessReceipt, String> {
        match self
            .call_operation(WorkspaceDbIpcOperation::RepairRuntimeGenerationLocator {
                project_root: self.runtime_project_root()?.display().to_string(),
            })
            .await?
        {
            WorkspaceDbIpcResult::RuntimeGenerationReadiness { receipt } => Ok(receipt),
            _ => Err(
                "Runtime Server returned an unexpected generation locator repair result".to_owned(),
            ),
        }
    }

    pub async fn evaluate_hook(
        &self,
        arguments: Vec<String>,
        input: String,
    ) -> Result<String, String> {
        let expected_workspace_identity = self.workspace_identity().to_owned();
        let expected_project_root = self.runtime_project_root()?.display().to_string();
        let result = self
            .call_operation(WorkspaceDbIpcOperation::EvaluateHook {
                project_root: expected_project_root.clone(),
                arguments,
                input,
            })
            .await?;
        admit_hook_evaluation_result(&expected_workspace_identity, &expected_project_root, result)
    }

    pub async fn evaluate_graph_turbo(
        &self,
        message: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        let expected_workspace_identity = self.workspace_identity().to_owned();
        let expected_project_root = self.runtime_project_root()?.display().to_string();
        let result = self
            .call_operation(WorkspaceDbIpcOperation::EvaluateGraphTurbo {
                project_root: expected_project_root.clone(),
                message,
            })
            .await?;
        match result {
            WorkspaceDbIpcResult::GraphTurboEvaluation {
                workspace_identity,
                project_root,
                receipt,
            } if workspace_identity == expected_workspace_identity
                && project_root == expected_project_root =>
            {
                Ok(receipt)
            }
            WorkspaceDbIpcResult::GraphTurboEvaluation {
                workspace_identity,
                project_root,
                ..
            } => Err(format!(
                "Runtime Server Graph Turbo response binding mismatch: expectedWorkspace={} actualWorkspace={} expectedProjectRoot={} actualProjectRoot={}",
                expected_workspace_identity,
                workspace_identity,
                expected_project_root,
                project_root,
            )),
            other => Err(format!(
                "Runtime Server returned unexpected Graph Turbo result: {other:?}"
            )),
        }
    }

    pub async fn publish_runtime_owner(
        &self,
        owner: crate::runtime_server_workspace::WorkspaceOwnerSnapshot,
    ) -> Result<crate::runtime_server_workspace::WorkspaceRecoveryReceipt, String> {
        match self
            .call_operation(WorkspaceDbIpcOperation::PublishRuntimeOwner {
                project_root: self.runtime_project_root()?.display().to_string(),
                owner,
            })
            .await?
        {
            WorkspaceDbIpcResult::RuntimeGeneration { receipt } => Ok(receipt),
            _ => {
                Err("Runtime Server returned an unexpected workspace generation result".to_owned())
            }
        }
    }

    pub async fn tombstone_runtime_owner(
        &self,
        owner_path: impl Into<String>,
    ) -> Result<crate::runtime_server_workspace::WorkspaceRecoveryReceipt, String> {
        match self
            .call_operation(WorkspaceDbIpcOperation::TombstoneRuntimeOwner {
                project_root: self.runtime_project_root()?.display().to_string(),
                owner_path: owner_path.into(),
            })
            .await?
        {
            WorkspaceDbIpcResult::RuntimeGeneration { receipt } => Ok(receipt),
            _ => {
                Err("Runtime Server returned an unexpected workspace generation result".to_owned())
            }
        }
    }

    pub async fn relocate_runtime_owner(
        &self,
        previous_owner_path: impl Into<String>,
        owner: crate::runtime_server_workspace::WorkspaceOwnerSnapshot,
    ) -> Result<crate::runtime_server_workspace::WorkspaceRecoveryReceipt, String> {
        match self
            .call_operation(WorkspaceDbIpcOperation::RelocateRuntimeOwner {
                project_root: self.runtime_project_root()?.display().to_string(),
                previous_owner_path: previous_owner_path.into(),
                owner,
            })
            .await?
        {
            WorkspaceDbIpcResult::RuntimeGeneration { receipt } => Ok(receipt),
            _ => {
                Err("Runtime Server returned an unexpected workspace generation result".to_owned())
            }
        }
    }
}

fn admit_hook_evaluation_result(
    expected_workspace_identity: &str,
    expected_project_root: &str,
    result: WorkspaceDbIpcResult,
) -> Result<String, String> {
    match result {
        WorkspaceDbIpcResult::HookEvaluation {
            workspace_identity,
            project_root,
            output,
        } if workspace_identity == expected_workspace_identity
            && project_root == expected_project_root =>
        {
            Ok(output)
        }
        WorkspaceDbIpcResult::HookEvaluation {
            workspace_identity,
            project_root,
            ..
        } => Err(format!(
            "Runtime Server hook evaluation identity mismatch: expectedWorkspaceIdentity={expected_workspace_identity} actualWorkspaceIdentity={workspace_identity} expectedProjectRoot={expected_project_root} actualProjectRoot={project_root}"
        )),
        _ => Err("Runtime Server returned an unexpected hook evaluation result".to_owned()),
    }
}

#[cfg(test)]
#[path = "../../tests/unit/workspace_db_ipc_hook_evaluation.rs"]
mod hook_evaluation_tests;
