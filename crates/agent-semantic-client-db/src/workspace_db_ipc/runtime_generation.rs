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
        // Duplicate submissions are the steady-state hot path.  Resolve them
        // under the shard's shared read lock so a burst of identical Runtime
        // mutation notifications does not serialize every caller through the
        // DashMap entry writer.  The entry path below remains the atomic
        // first-publisher authority and closes the read-to-insert race.
        if let Some(existing) = self.submitted.get(mutation_id) {
            return if existing.as_slice() == changed_paths {
                Ok(false)
            } else {
                Err(format!(
                    "runtime generation mutation identity was reused with different changed paths: mutationId={mutation_id}"
                ))
            };
        }
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

fn normalize_changed_paths(mut changed_paths: Vec<String>) -> Result<Vec<String>, String> {
    changed_paths.retain(|path| !path.trim().is_empty());
    // Normalize in place.  Mutation notifications overwhelmingly contain one
    // path, so constructing a BTreeSet here made every coalesced notification
    // pay an avoidable tree allocation on the Runtime IPC hot path.
    if changed_paths.len() > 1 {
        changed_paths.sort_unstable();
        changed_paths.dedup();
    }
    Ok(changed_paths)
}

impl WorkspaceDbIpcSession {
    pub async fn resolve_provider_runtime(
        &self,
        language_id: agent_semantic_client_core::LanguageId,
    ) -> Result<serde_json::Value, String> {
        match self
            .call_operation(WorkspaceDbIpcOperation::ResolveProviderRuntime {
                project_root: self.runtime_project_root()?.display().to_string(),
                language_id,
            })
            .await?
        {
            WorkspaceDbIpcResult::ProviderRuntime { runtime } => Ok(runtime),
            _ => Err("Runtime Server returned an unexpected provider runtime result".to_owned()),
        }
    }
    pub async fn runtime_graph_facts(
        &self,
        sources: Vec<super::RuntimeGraphFactSource>,
    ) -> Result<super::RuntimeGraphFactsRead, String> {
        match self
            .call_operation(WorkspaceDbIpcOperation::ReadRuntimeGraphFacts {
                project_root: self.runtime_project_root()?.display().to_string(),
                sources,
            })
            .await?
        {
            WorkspaceDbIpcResult::RuntimeGraphFacts { read } => {
                read.validate()?;
                Ok(read)
            }
            other => Err(format!(
                "Runtime Server returned unexpected graph facts result: {other:?}"
            )),
        }
    }

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
        language_id: agent_semantic_client_core::LanguageId,
        projection_kind: crate::runtime_server_workspace::ExactProjectionKind,
        structural_selector: impl Into<String>,
    ) -> Result<crate::runtime_server_workspace::WorkspaceRuntimeSelectorRead, String> {
        match self
            .call_operation(WorkspaceDbIpcOperation::ReadRuntimeSelector {
                project_root: self.runtime_project_root()?.display().to_string(),
                language_id,
                projection_kind,
                structural_selector: structural_selector.into(),
            })
            .await?
        {
            WorkspaceDbIpcResult::RuntimeSelector { read, .. } => Ok(read),
            _ => Err("Runtime Server returned an unexpected runtime selector result".to_owned()),
        }
    }

    async fn read_runtime_owner_via_server(
        &self,
        project_root: &std::path::Path,
        owner_path: String,
    ) -> Result<crate::runtime_server_workspace::WorkspaceRuntimeOwnerRead, String> {
        match self
            .call_operation(WorkspaceDbIpcOperation::ReadRuntimeOwner {
                project_root: project_root.display().to_string(),
                owner_path,
            })
            .await?
        {
            WorkspaceDbIpcResult::RuntimeOwner { read, .. } => Ok(read),
            _ => Err("Runtime Server returned an unexpected owner read result".to_owned()),
        }
    }

    pub async fn read_runtime_owner(
        &self,
        owner_path: impl Into<String>,
    ) -> Result<crate::runtime_server_workspace::WorkspaceRuntimeOwnerRead, String> {
        let owner_path = owner_path.into();
        let project_root = self.runtime_project_root()?.to_path_buf();
        let Some(generation_pointer) = self.runtime_generation_pointer_path() else {
            return self
                .read_runtime_owner_via_server(&project_root, owner_path)
                .await;
        };
        if !generation_pointer.is_file() {
            return self
                .read_runtime_owner_via_server(&project_root, owner_path)
                .await;
        }
        let resident_read = crate::runtime_resident_read::RuntimeResidentReadClient::open(
            &generation_pointer,
            &project_root,
        )
        .await?;
        let started = std::time::Instant::now();
        let read = resident_read.read_runtime_owner(&owner_path)?;
        let elapsed_micros = u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX);
        let work = resident_read.work_counters();
        if work.database_read_count != 0
            || work.filesystem_read_count != 0
            || work.provider_process_count != 0
            || work.scheduler_task_count != 0
            || work.socket_operation_count != 0
        {
            return Err(format!(
                "Runtime owner read violated synchronous mmap authority: databaseReads={} filesystemReads={} providerProcesses={} schedulerTasks={} socketOperations={}",
                work.database_read_count,
                work.filesystem_read_count,
                work.provider_process_count,
                work.scheduler_task_count,
                work.socket_operation_count,
            ));
        }
        resident_read.try_record_read_observation(
            "runtime-owner-read",
            "qualified",
            &owner_path,
            None,
            "owner-snapshot",
            elapsed_micros,
            1_000,
            "within-budget",
        );
        Ok(read)
    }

    pub async fn project_provider_owner(
        &self,
        language_id: agent_semantic_client_core::LanguageId,
        owner_path: impl Into<String>,
    ) -> Result<crate::runtime_server_workspace::WorkspaceOwnerSnapshot, String> {
        match self
            .call_operation(WorkspaceDbIpcOperation::ProjectProviderOwner {
                project_root: self.runtime_project_root()?.display().to_string(),
                language_id,
                owner_path: owner_path.into(),
            })
            .await?
        {
            WorkspaceDbIpcResult::ProviderOwnerProjection { owner } => Ok(owner),
            _ => Err(
                "Runtime Server returned an unexpected provider owner projection result".to_owned(),
            ),
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

    pub async fn rebind_runtime_selector_overlay(
        &self,
        rebind: crate::runtime_server_workspace::WorkspaceRuntimeSelectorRebind,
    ) -> Result<crate::runtime_server_workspace::WorkspaceRuntimeSelectorOverlayReceipt, String>
    {
        match self
            .call_operation(WorkspaceDbIpcOperation::RebindRuntimeSelectorOverlay {
                project_root: self.runtime_project_root()?.display().to_string(),
                rebind,
            })
            .await?
        {
            WorkspaceDbIpcResult::RuntimeSelectorOverlay { receipt } => Ok(receipt),
            _ => Err(
                "Runtime Server returned an unexpected runtime selector rebind result".to_owned(),
            ),
        }
    }

    /// Requires a prepublished canonical source-index generation to be resident and durable.
    pub async fn require_runtime_generation(
        &self,
    ) -> Result<crate::runtime_server_workspace::WorkspaceRecoveryReceipt, String> {
        let project_root = self.runtime_project_root()?.display().to_string();
        let result = self
            .call_operation(WorkspaceDbIpcOperation::RequireRuntimeGeneration { project_root })
            .await?;
        match result {
            WorkspaceDbIpcResult::RuntimeGenerationResidentReady { receipt } => {
                receipt.validate()?;
                Ok(receipt)
            }
            _ => {
                Err("Runtime Server returned an unexpected canonical generation result".to_owned())
            }
        }
    }

    pub async fn admit_runtime_generation_for_read(
        &self,
        language_id: impl Into<String>,
        provider_id: impl Into<String>,
    ) -> Result<crate::runtime_server_workspace::WorkspaceRecoveryReceipt, String> {
        let project_root = self.runtime_project_root()?.display().to_string();
        let result = self
            .call_operation(WorkspaceDbIpcOperation::AdmitRuntimeGenerationForRead {
                project_root,
                language_id: language_id.into(),
                provider_id: provider_id.into(),
            })
            .await?;
        match result {
            WorkspaceDbIpcResult::RuntimeGenerationResidentReady { receipt } => {
                receipt.validate()?;
                Ok(receipt)
            }
            _ => Err("Runtime Server returned an unexpected read admission result".to_owned()),
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
        let changed_paths = if changed_paths.is_empty() {
            Vec::new()
        } else {
            normalize_changed_paths(changed_paths)?
        };
        self.admit_runtime_generation_with_role(mutation_id, changed_paths, false)
            .await?
            .0
            .ok_or_else(|| "runtime generation follower receipt was omitted".to_owned())
    }

    /// Discovers and commits the initial canonical source-index generation.
    ///
    /// Mutation admission is intentionally incremental and requires this
    /// lifecycle generation to exist first.
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
        if changed_paths.is_empty() {
            return Err("runtime generation submission requires changed paths".to_owned());
        }
        let changed_paths = if changed_paths.is_empty() {
            Vec::new()
        } else {
            normalize_changed_paths(changed_paths)?
        };
        self.submit_runtime_generation_request(mutation_id, changed_paths)
            .await
    }

    async fn submit_runtime_generation_request(
        &self,
        mutation_id: String,
        changed_paths: Vec<String>,
    ) -> Result<crate::runtime_server_admission::WorkspaceGenerationMutationSubmissionReceipt, String>
    {
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
        match self
            .call_operation(
                WorkspaceDbIpcOperation::ReadRuntimeSearchGenerationAuthority {
                    project_root: self.runtime_project_root()?.display().to_string(),
                },
            )
            .await?
        {
            WorkspaceDbIpcResult::RuntimeSearchGenerationAuthority {
                authority: Some(authority),
            } => Ok(authority),
            WorkspaceDbIpcResult::RuntimeSearchGenerationAuthority { authority: None } => Err(
                crate::runtime_server_workspace::ACTIVE_WORKSPACE_GENERATION_REQUIRED.to_owned(),
            ),
            _ => Err(
                "Runtime Server returned an unexpected search generation authority result"
                    .to_owned(),
            ),
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
