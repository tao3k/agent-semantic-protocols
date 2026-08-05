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
    submitted: dashmap::DashMap<String, Vec<String>>,
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

    fn register_submission(
        &self,
        mutation_id: &str,
        changed_paths: &[String],
    ) -> Result<bool, String> {
        match self.submitted.entry(mutation_id.to_owned()) {
            dashmap::mapref::entry::Entry::Occupied(existing) => {
                if existing.get().as_slice() == changed_paths {
                    Ok(false)
                } else {
                    Err(format!(
                        "runtime generation mutation identity was reused with different changed paths: mutationId={mutation_id}"
                    ))
                }
            }
            dashmap::mapref::entry::Entry::Vacant(vacant) => {
                vacant.insert(changed_paths.to_vec());
                Ok(true)
            }
        }
    }

    fn release_submission(&self, mutation_id: &str) {
        self.submitted.remove(mutation_id);
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
    async fn call_runtime_generation_submission(
        &self,
        mutation_id: String,
        project_root: String,
        changed_paths: Vec<String>,
    ) -> Result<crate::runtime_server_admission::WorkspaceGenerationMutationSubmissionReceipt, String>
    {
        let expected_mutation_id = mutation_id.clone();
        match self
            .call_operation(WorkspaceDbIpcOperation::SubmitRuntimeGenerationMutation {
                mutation_id,
                project_root,
                changed_paths,
            })
            .await
        {
            Ok(WorkspaceDbIpcResult::RuntimeGenerationMutationSubmission { receipt }) => {
                receipt.validate()?;
                if receipt.mutation_id != expected_mutation_id {
                    return Err(format!(
                        "Runtime Server returned a mutation submission identity mismatch: expectedMutationId={} actualMutationId={}",
                        expected_mutation_id, receipt.mutation_id
                    ));
                }
                Ok(receipt)
            }
            Ok(_) => {
                Err("Runtime Server returned an unexpected generation submission result".to_owned())
            }
            Err(error) => Err(error),
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
        self.admit_runtime_generation_with_role(mutation_id, changed_paths, false)
            .await?
            .0
            .ok_or_else(|| "runtime generation follower receipt was omitted".to_owned())
    }

    async fn admit_runtime_generation_with_role(
        &self,
        mutation_id: String,
        changed_paths: Vec<String>,
        return_matching_follower_immediately: bool,
    ) -> Result<
        (
            Option<crate::runtime_server_admission::WorkspaceGenerationMutationAdmissionReceipt>,
            bool,
        ),
        String,
    > {
        let lane = self.shared.runtime_generation_mutations.get_or_init(|| {
            super::runtime_generation::runtime_generation_mutation_lane(
                &self.endpoint.socket_path,
                self.endpoint.owner_epoch,
                &self.endpoint.workspace_identity,
            )
        });
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
                    return result.map(|receipt| (Some(receipt), true));
                }
                FlightRole::Follow(flight) => {
                    if return_matching_follower_immediately {
                        return Ok((None, false));
                    }
                    return wait_for_mutation_flight(&flight)
                        .await
                        .map(|receipt| (Some(receipt), false));
                }
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
        let changed_path_count = changed_paths.len();
        let lane = self.shared.runtime_generation_mutations.get_or_init(|| {
            super::runtime_generation::runtime_generation_mutation_lane(
                &self.endpoint.socket_path,
                self.endpoint.owner_epoch,
                &self.endpoint.workspace_identity,
            )
        });
        if !lane.register_submission(&mutation_id, &changed_paths)? {
            let receipt =
                crate::runtime_server_admission::WorkspaceGenerationMutationSubmissionReceipt {
                    schema_id: crate::runtime_server_admission::WORKSPACE_GENERATION_MUTATION_SUBMISSION_RECEIPT_SCHEMA_ID.to_owned(),
                    schema_version: "1".to_owned(),
                    mutation_id,
                    workspace_identity: self.workspace_identity().to_owned(),
                    changed_path_count,
                    state: crate::runtime_server_admission::WorkspaceGenerationMutationSubmissionState::Coalesced,
                };
            receipt.validate()?;
            return Ok(receipt);
        }
        // Candidate discovery and Merkle/provider rebuilding are daemon-owned.
        // The hook process sends only mutation identity and changed paths, then
        // returns after the Runtime Server has taken ownership of the task.
        let submission_id = mutation_id.clone();
        let submitted = self
            .call_runtime_generation_submission(
                mutation_id,
                self.runtime_project_root()?.display().to_string(),
                changed_paths,
            )
            .await;
        if submitted.is_err() {
            lane.release_submission(&submission_id);
        }
        submitted
    }

    /// Route one cache-control request through the Runtime Server data plane.
    pub async fn cache_control(
        &self,
        request: super::RuntimeCacheControlRequest,
    ) -> Result<super::RuntimeCacheControlReceipt, String> {
        let session_root = self.runtime_project_root()?;
        let request_root = std::path::Path::new(request.project_root());
        if request_root != session_root {
            return Err(format!(
                "cache-control project root must match the Runtime Server session: session={} request={}",
                session_root.display(),
                request_root.display()
            ));
        }
        match self
            .call_operation(WorkspaceDbIpcOperation::CacheControl { request })
            .await?
        {
            WorkspaceDbIpcResult::CacheControl { receipt } => Ok(receipt),
            _ => Err("Runtime Server returned an unexpected cache-control result".to_owned()),
        }
    }

    pub async fn ensure_runtime_generation(
        &self,
    ) -> Result<crate::runtime_server_admission::WorkspaceGenerationAdmissionReceipt, String> {
        let project_root = self.runtime_project_root()?.to_path_buf();
        match self
            .call_operation(WorkspaceDbIpcOperation::EnsureRuntimeGeneration {
                project_root: project_root.display().to_string(),
            })
            .await?
        {
            WorkspaceDbIpcResult::RuntimeGenerationAdmission { receipt } => Ok(receipt),
            _ => Err("Runtime Server returned an unexpected ensured generation result".to_owned()),
        }
    }

    /// Hold an explicit search/query lifecycle gate until the resident
    /// generation attempt reaches terminal Ready. The Runtime Server owns the
    /// wait and its notification; clients neither poll nor start a second
    /// generation attempt.
    pub async fn ensure_runtime_generation_ready(
        &self,
    ) -> Result<crate::runtime_server_admission::WorkspaceGenerationReadinessReceipt, String> {
        let project_root = self.runtime_project_root()?.to_path_buf();
        match self
            .call_operation(WorkspaceDbIpcOperation::EnsureRuntimeGenerationReady {
                project_root: project_root.display().to_string(),
            })
            .await?
        {
            WorkspaceDbIpcResult::RuntimeGenerationReadiness { receipt } => {
                receipt.validate()?;
                Ok(receipt)
            }
            _ => Err("Runtime Server returned an unexpected ready generation result".to_owned()),
        }
    }

    /// Ensure the exact owner represented by a resident segment is still
    /// current before the caller reopens that immutable generation.
    pub async fn ensure_runtime_generation_owner_ready(
        &self,
        owner_path: &str,
        admitted_content_digest: &str,
    ) -> Result<crate::runtime_server_admission::WorkspaceOwnerGenerationReadinessReceipt, String>
    {
        let project_root = self.runtime_project_root()?.to_path_buf();
        match self
            .call_operation(WorkspaceDbIpcOperation::EnsureRuntimeGenerationOwnerReady {
                project_root: project_root.display().to_string(),
                owner_path: owner_path.to_owned(),
                admitted_content_digest: admitted_content_digest.to_owned(),
            })
            .await?
        {
            WorkspaceDbIpcResult::RuntimeOwnerGenerationReadiness { receipt } => {
                receipt.validate()?;
                Ok(receipt)
            }
            _ => {
                Err("Runtime Server returned an unexpected exact-owner readiness result".to_owned())
            }
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

    pub async fn runtime_search_generation_authority(
        &self,
    ) -> Result<crate::runtime_server_workspace::WorkspaceSearchGenerationAuthority, String> {
        let expected_workspace_identity = self.workspace_identity().to_owned();
        let expected_project_root = self.runtime_project_root()?.display().to_string();
        if let Some(pointer_path) = self.runtime_generation_pointer_path()
            && let Some(pointer) = crate::runtime_server_workspace::WorkspaceSearchGenerationAuthorityPointerClient::shared_get(
                pointer_path,
                &expected_workspace_identity,
                &expected_project_root,
            )
        {
            return pointer.read();
        }
        let pointer = self
            .shared
            .search_generation_authority
            .get_or_try_init(|| async {
                if let Some(pointer_path) = self.runtime_generation_pointer_path()
                    && let Some(pointer) = crate::runtime_server_workspace::WorkspaceSearchGenerationAuthorityPointerClient::shared_open_path_optional(
                        pointer_path,
                        &expected_workspace_identity,
                        &expected_project_root,
                    )
                    .await?
                {
                    return Ok(pointer);
                }
                let result = self
                    .call_operation(
                        WorkspaceDbIpcOperation::ReadRuntimeSearchGenerationAuthority {
                            project_root: expected_project_root.clone(),
                        },
                    )
                    .await?;
                let receipt = match result {
                    WorkspaceDbIpcResult::RuntimeSearchGenerationAuthority { receipt } => receipt,
                    other => {
                        return Err(format!(
                            "Runtime Server returned unexpected search generation authority result: {other:?}"
                        ));
                    }
                };
                crate::runtime_server_workspace::WorkspaceSearchGenerationAuthorityPointerClient::open(
                    &receipt,
                    &expected_workspace_identity,
                    &expected_project_root,
                )
                .await
                .map(std::sync::Arc::new)
            })
            .await?;
        pointer.read()
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
