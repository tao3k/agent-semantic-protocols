use std::sync::Arc;

use super::generation_builder::{Stage, await_stage};

use tokio::net::UnixListener;
use tokio::sync::watch;
use tokio::task::JoinSet;

use crate::runtime_server_agent_session_status::AgentSessionStatusHandle;
pub use crate::runtime_server_asp_python_graphs_status::AspPythonGraphsStatusHandle;
use crate::runtime_server_control::RuntimeServerEndpoint;
use crate::runtime_server_control::status_memory::RuntimeServerStatusMemoryWriter;

use crate::WorkspaceDbRegistry;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeServerExit {
    ListenerClosed,
    RestartRequested,
    ShutdownRequested,
}

pub use crate::runtime_server_observability::RuntimeServerEvent;
use crate::runtime_server_observability::publish_event;

const CONNECTION_DRAIN_BOUNDARY: std::time::Duration = std::time::Duration::from_millis(100);

#[derive(Clone)]
pub struct RuntimeServerShutdownHandle {
    pub(super) sender: watch::Sender<bool>,
}

impl RuntimeServerShutdownHandle {
    pub fn shutdown(&self) {
        self.sender.send_replace(true);
    }
}

pub struct RuntimeServer {
    pub(super) artifact_catalog:
        Arc<agent_semantic_artifacts::runtime_artifact_catalog::RuntimeArtifactCatalog>,
    pub(super) workspace_registry:
        std::sync::Arc<crate::runtime_server_workspace::RuntimeServerWorkspaceRegistry>,
    pub(super) runtime_search_service:
        Option<crate::runtime_search_service::RuntimeSearchServiceHandle>,
    pub(super) endpoint: RuntimeServerEndpoint,
    pub(super) listener: UnixListener,
    pub(super) provider_register: Arc<crate::runtime_provider_register::RuntimeProviderRegister>,
    pub(super) registry: Arc<WorkspaceDbRegistry>,
    pub(super) workspace_count: watch::Receiver<usize>,
    pub(super) shutdown: watch::Receiver<bool>,
    pub(super) shutdown_handle: RuntimeServerShutdownHandle,
    pub(super) readiness_sender: watch::Sender<crate::runtime_server_control::RuntimeServerState>,
    pub(super) generation_publication:
        crate::runtime_server_publication::WorkspaceGenerationPublication,
    pub(crate) status_memory: RuntimeServerStatusMemoryWriter,
    pub(super) events: Option<crate::runtime_server_observability::RuntimeServerEventPublisher>,
    pub(super) generation_admission:
        Option<Arc<crate::runtime_server_admission::WorkspaceGenerationAdmission>>,
    pub(super) asp_python_graphs_status: Option<AspPythonGraphsStatusHandle>,
    pub(crate) agent_session_registry_owner: Option<Arc<crate::AgentSessionRegistry>>,
    pub(crate) agent_session_status: Option<AgentSessionStatusHandle>,
    pub(super) telemetry_sender: Option<crate::runtime_telemetry_bus::RuntimeTelemetryBusSender>,
}

impl RuntimeServer {
    /// Use one Runtime-owned provider register for provider-plane mutations
    /// and data-plane route resolution.
    #[must_use]
    pub fn with_provider_register(
        mut self,
        provider_register: Arc<crate::runtime_provider_register::RuntimeProviderRegister>,
    ) -> Self {
        self.provider_register = provider_register;
        self
    }

    /// Immutable artifact authority loaded once for this daemon generation.
    pub fn artifact_catalog(
        &self,
    ) -> &Arc<agent_semantic_artifacts::runtime_artifact_catalog::RuntimeArtifactCatalog> {
        &self.artifact_catalog
    }

    pub fn workspace_registry(
        &self,
    ) -> &std::sync::Arc<crate::runtime_server_workspace::RuntimeServerWorkspaceRegistry> {
        &self.workspace_registry
    }

    /// Returns the supervised workspace generation admission plane when configured.
    pub fn workspace_generation_admission(
        &self,
    ) -> Option<Arc<crate::runtime_server_admission::WorkspaceGenerationAdmission>> {
        self.generation_admission.clone()
    }

    pub fn readiness_subscribe(
        &self,
    ) -> watch::Receiver<crate::runtime_server_control::RuntimeServerState> {
        self.readiness_sender.subscribe()
    }

    pub fn workspace_generation_publication_subscribe(
        &self,
    ) -> watch::Receiver<Option<crate::runtime_server_publication::WorkspaceGenerationPublished>>
    {
        self.generation_publication.subscribe()
    }

    pub(super) async fn cleanup_bound_artifacts(&self) {
        for path in [
            &self.endpoint.socket_path,
            &self.endpoint.data_plane_socket_path,
            &self.endpoint.provider_plane_socket_path,
            &self.endpoint.status_memory_path,
        ] {
            let _ = tokio::fs::remove_file(path).await;
        }
    }

    pub fn with_event_sender(
        mut self,
        events: crate::runtime_server_observability::RuntimeServerEventPublisher,
    ) -> Self {
        self.events = Some(events);
        self
    }

    pub fn with_runtime_telemetry_sender(
        mut self,
        sender: crate::runtime_telemetry_bus::RuntimeTelemetryBusSender,
    ) -> Self {
        self.telemetry_sender = Some(sender);
        self
    }

    pub fn with_runtime_search_service(
        mut self,
        service: crate::runtime_search_service::RuntimeSearchServiceHandle,
    ) -> Self {
        self.runtime_search_service = Some(service);
        self
    }

    pub fn with_asp_python_graphs_status(mut self, status: AspPythonGraphsStatusHandle) -> Self {
        self.status_memory.set_asp_python_graphs(status.shared());
        self.asp_python_graphs_status = Some(status);
        self
    }

    /// Compose the server with an already-owned generation admission plane.
    pub fn with_workspace_generation_admission(
        mut self,
        admission: Arc<crate::runtime_server_admission::WorkspaceGenerationAdmission>,
    ) -> Self {
        self.generation_admission = Some(admission);
        self
    }

    pub(crate) fn configure_workspace_generation_builder(
        mut self,
        source_builder: impl Into<
            Option<crate::runtime_server_admission::WorkspaceGenerationCandidateBuilder>,
        >,
        owner_projection_builder: Option<
            crate::runtime_server_admission::WorkspaceOwnerProjectionBuilder,
        >,
        catalog: Option<crate::runtime_server_admission_catalog::RuntimeWorkspaceAdmissionCatalog>,
    ) -> Self {
        let source_builder = source_builder.into();
        let durable_registry = Arc::clone(&self.registry);
        let memory_registry = Arc::clone(&self.workspace_registry);
        let generation_publication = self.generation_publication.clone();
        let mutation_owner_projection_builder = owner_projection_builder.clone();
        let events = self.events.clone();
        let builder = Arc::new(
            move |workspace_identity: String,
                  project_root: std::path::PathBuf,
          candidate: crate::runtime_server_admission::WorkspaceGenerationCandidateIdentity,
          build_mode: crate::runtime_server_admission::WorkspaceGenerationBuildMode,
          changed_paths: Arc<std::collections::BTreeSet<std::path::PathBuf>>,
          provider_target: Option<
            crate::runtime_server_admission::WorkspaceGenerationProviderTarget,
          >,
          cancellation: crate::runtime_generation_cancellation::GenerationCancellation| {
                let durable_registry = Arc::clone(&durable_registry);
                let memory_registry = Arc::clone(&memory_registry);
                let generation_publication = generation_publication.clone();
        let source_builder = source_builder.clone();
        let source_builder_cancellation = cancellation.clone();
                let mutation_owner_projection_builder =
                    mutation_owner_projection_builder.clone();
                let events = events.clone();
                let workspace_for_build = workspace_identity.clone();
                Box::pin(async move {
            if cancellation.is_cancelled() {
                        return Err(crate::runtime_server_admission::WorkspaceGenerationBuildFailure::new(
                            crate::runtime_server_admission::WorkspaceGenerationFailureStage::GenerationBuilderSupervision,
                            "generation build cancelled",
                        ));
                    }
                    let diagnostic_workspace_identity = workspace_identity.clone();
                    let diagnostic_events = events.clone();
                    let result = async move {
            if cancellation.is_cancelled() {
                        return Err(crate::runtime_server_admission::WorkspaceGenerationBuildFailure::new(
                            crate::runtime_server_admission::WorkspaceGenerationFailureStage::GenerationBuilderSupervision,
                            "generation build cancelled",
                        ));
                    }
                    let build_started = std::time::Instant::now();
                    let build_mode_label = match build_mode {
                        crate::runtime_server_admission::WorkspaceGenerationBuildMode::RestoreOnly => {
                            "restore-only"
                        }
                        crate::runtime_server_admission::WorkspaceGenerationBuildMode::RestoreOrBuild => {
                            "restore-or-build"
                        }
                        crate::runtime_server_admission::WorkspaceGenerationBuildMode::RebuildAfterMutation => {
                            "rebuild-after-mutation"
                        }
                    };
                    static NEXT_GENERATION_OPERATION_ID: std::sync::atomic::AtomicU64 =
                        std::sync::atomic::AtomicU64::new(1);
                    let operation_id = format!(
                        "workspace-generation-{workspace_identity}-{}",
                        NEXT_GENERATION_OPERATION_ID
                            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
                    );
                    let _memory_operation =
                        crate::runtime_server_opentelemetry::begin_runtime_memory_operation(
                            workspace_identity.clone(),
                            operation_id.clone(),
                        );
let session = await_stage(
                        &workspace_identity,
                        &operation_id,
                        Stage::WorkspaceBootstrap,
                        async {
        async {
            if workspace_identity.trim().is_empty() {
                return Err(
                    "workspace admission requires a non-empty workspace identity".to_owned(),
                );
            }
            if let Some(session) = durable_registry.loaded_session(&workspace_identity) {
                return Ok(session);
            }
            let session = durable_registry.bootstrap_workspace(&project_root).await?;
            if session.workspace_identity() != workspace_identity {
                return Err(format!(
                    "workspace admission identity mismatch: requested={workspace_identity} resolved={}",
                    session.workspace_identity()
                ));
            }
            Ok(session)
        }
        .await
        .map_err(|error| {
                                crate::runtime_server_admission::WorkspaceGenerationBuildFailure::new(
                                    crate::runtime_server_admission::WorkspaceGenerationFailureStage::WorkspaceBootstrap,
                                    error,
                                )
                            })
                        },
                    )
                    .await?;
                    if build_mode.attempts_durable_restore() {
                        let restore_started = std::time::Instant::now();
                        let pointer_restore = await_stage(
                            &workspace_identity,
                            &operation_id,
                            Stage::DurableRestore,
                            async {
                                memory_registry
                                    .restore_published_generation(
                                        format!(
                                            "daemon-admission-restore-{workspace_identity}-{operation_id}"
                                        ),
                                        workspace_identity.clone(),
                                        &project_root,
                                    )
                                    .await
                                    .map_err(|error| {
                                        crate::runtime_server_admission::WorkspaceGenerationBuildFailure::new(
                                            crate::runtime_server_admission::WorkspaceGenerationFailureStage::DurableRestore,
                                            error,
                                        )
                                    })
                            },
                        )
                        .await;
                        let restored_generation_covers_demand = pointer_restore
                            .as_ref()
                            .is_ok_and(|_| {
                                super::generation_builder::restored_generation_covers_demand(
                                    &memory_registry,
                                    &workspace_identity,
                                    &project_root,
                                    changed_paths.as_ref(),
                                    provider_target.as_ref(),
                                )
                            });
                        let restore_elapsed_micros = restore_started
                            .elapsed()
                            .as_micros()
                            .min(u128::from(u64::MAX)) as u64;
                        let restore_budget_micros = 800_000;
                        let mut restore_observation = crate::runtime_server_opentelemetry::RuntimePerformanceObservation::new(
                            "workspace-generation-admission",
                            "durable-restore",
                            restore_elapsed_micros,
                            restore_budget_micros,
                            if restore_elapsed_micros < restore_budget_micros {
                                "within-budget"
                            } else {
                                "budget-exceeded"
                            },
                        )
                        .with_operation_id(operation_id.clone());
                        restore_observation.workspace_identity = Some(workspace_identity.clone());
                        if pointer_restore.is_err() {
                            restore_observation.failure_reason =
                                Some("workspace-generation-pointer-restore-failed".to_owned());
                        } else if !restored_generation_covers_demand {
                            restore_observation.failure_reason = Some(
                                "workspace-generation-restored-provider-coverage-missing".to_owned(),
                            );
                        }
                        let _ = crate::runtime_server_opentelemetry::try_record_to_active_runtime(
                            restore_observation,
                        );
                        match pointer_restore {
                            Ok(published) if restored_generation_covers_demand => {
                            publish_event(
                                events.as_ref(),
                                RuntimeServerEvent::WorkspaceGenerationResidentPublished {
                                    workspace_identity: workspace_identity.clone(),
                                    build_mode: build_mode_label.to_owned(),
                                    generation_digest: published.generation_digest.clone(),
                                    elapsed_micros: u64::try_from(
                                        build_started.elapsed().as_micros(),
                                    )
                                    .unwrap_or(u64::MAX),
                                },
                            );
                            let commit = crate::runtime_server_admission::WorkspaceGenerationCommitReceipt::from_recovery(&published).map_err(|error| crate::runtime_server_admission::WorkspaceGenerationBuildFailure::new(
                                    crate::runtime_server_admission::WorkspaceGenerationFailureStage::CanonicalGenerationPublication,
                                    error,
                                ))?;
                            return crate::runtime_server_admission::WorkspaceGenerationBuildCompletion::new(
                                candidate.clone(),
                                commit,
                            ).map_err(|error| crate::runtime_server_admission::WorkspaceGenerationBuildFailure::new(
                                    crate::runtime_server_admission::WorkspaceGenerationFailureStage::CanonicalGenerationPublication,
                                    error,
                                ));
                            }
                            Ok(_) if build_mode
                                == crate::runtime_server_admission::WorkspaceGenerationBuildMode::RestoreOnly =>
                            {
                                return Err(crate::runtime_server_admission::WorkspaceGenerationBuildFailure::new(
                                    crate::runtime_server_admission::WorkspaceGenerationFailureStage::DurableRestore,
                                    "restored generation does not cover the requested language/provider target",
                                ));
                            }
                            Ok(_) => {}
                            Err(failure)
                                if build_mode
                                    == crate::runtime_server_admission::WorkspaceGenerationBuildMode::RestoreOnly =>
                            {
                                return Err(failure);
                            }
                            Err(_) => {}
                        }
                    }
                    if build_mode
                        == crate::runtime_server_admission::WorkspaceGenerationBuildMode::RebuildAfterMutation
                        && !changed_paths.is_empty()
                    {
                        let owner_projection_builder = mutation_owner_projection_builder
                            .as_ref()
                            .ok_or_else(|| {
                                crate::runtime_server_admission::WorkspaceGenerationBuildFailure::new(
                                    crate::runtime_server_admission::WorkspaceGenerationFailureStage::SourceBuilder,
                                    "runtime mutation owner projection builder is unavailable",
                                )
                            })?;
        return super::generation_builder::publish_mutation_generation(
                            owner_projection_builder,
                            &memory_registry,
                            &workspace_identity,
                            &project_root,
                            changed_paths.as_ref(),
                            format!(
                                "daemon-admission-mutation-{workspace_identity}-{}",
                                candidate.candidate_generation.digest
                            ),
            candidate,
            cancellation.clone(),
        )
                        .await;
                    }
                    let source_builder = source_builder.as_ref().ok_or_else(|| {
                        format!(
                            "canonical workspace generation is unavailable; writer lane publication is required before admission: workspaceIdentity={workspace_identity}"
                        )
                    }).map_err(|error| crate::runtime_server_admission::WorkspaceGenerationBuildFailure::new(
                        crate::runtime_server_admission::WorkspaceGenerationFailureStage::SourceBuilder,
                        error,
                    ))?;
                    let source_build_started = std::time::Instant::now();
                    let source_build = await_stage(
                        &workspace_identity,
                        &operation_id,
                        Stage::SourceBuilder,
                        async {
                            // Cold source construction is owned by the Runtime
                            // generation worker.  It must run to a concrete
                            // publication or builder error; a client-era wall
                            // clock budget both cancels useful I/O and leaves a
                            // facade permanently without a resident generation.
                    source_builder(
                        workspace_for_build,
                        project_root.clone(),
                        changed_paths,
                        provider_target,
                        source_builder_cancellation,
                    )
                            .await
                            .map_err(|error| {
                                crate::runtime_server_admission::WorkspaceGenerationBuildFailure::new(
                                    crate::runtime_server_admission::WorkspaceGenerationFailureStage::SourceBuilder,
                                    error,
                                )
                            })
                        },
                    )
                    .await;
                    let source_build_elapsed_micros = source_build_started
                        .elapsed()
                        .as_micros()
                        .min(u128::from(u64::MAX)) as u64;
                    let mut source_build_observation = crate::runtime_server_opentelemetry::RuntimePerformanceObservation::new(
                        "workspace-generation-admission",
                        "source-builder",
                        source_build_elapsed_micros,
                        0,
                        if source_build.is_ok() { "observed" } else { "failed" },
                    )
                        .with_operation_id(operation_id.clone());
                    source_build_observation.workspace_identity = Some(workspace_identity.clone());
                    if source_build.is_err() {
                        source_build_observation.failure_reason =
                            Some("workspace-generation-source-builder-failed".to_owned());
                    }
                    let _ = crate::runtime_server_opentelemetry::try_record_to_active_runtime(
                        source_build_observation,
                    );
                    let build = source_build?;
                    let captured_candidate = build.candidate.clone();
                    build
                        .materialization
                        .validate_refresh_request(&workspace_identity, &build.refresh)
                        .map_err(|error| {
                            crate::runtime_server_admission::WorkspaceGenerationBuildFailure::new(
                                crate::runtime_server_admission::WorkspaceGenerationFailureStage::SourceBuilder,
                                error,
                            )
                        })?;
                    let (durable, committed_materialization) =
                        await_stage(
                        &workspace_identity,
                        &operation_id,
                            Stage::SourceIndexCommit,
                            async {
                                session
                                    .commit_source_index_generation(
                                        build.refresh,
                                        build.materialization,
                                    )
                                    .await
                                    .map_err(|error| {
                                        crate::runtime_server_admission::WorkspaceGenerationBuildFailure::new(
                                            crate::runtime_server_admission::WorkspaceGenerationFailureStage::SourceIndexCommit,
                                            error,
                                        )
                                    })
                            },
                        )
                        .await?;
                    let committed_materialization =
                        committed_materialization.into_validated(&workspace_identity).map_err(|error| {
                            crate::runtime_server_admission::WorkspaceGenerationBuildFailure::new(
                                crate::runtime_server_admission::WorkspaceGenerationFailureStage::SourceIndexCommit,
                                error,
                            )
                        })?;
                    if !durable
                        .source_snapshot
                        .has_same_content_identity(
                            &committed_materialization.as_materialization().source_snapshot,
                        )
                    {
                        return Err(crate::runtime_server_admission::WorkspaceGenerationBuildFailure::new(
                            crate::runtime_server_admission::WorkspaceGenerationFailureStage::SourceIndexCommit,
                            format!(
                            "Turso generation evidence differs from canonical materialization: durable={:?} materialized={:?}",
                            durable.source_snapshot,
                            committed_materialization.as_materialization().source_snapshot
                            ),
                        ));
                    }
                    let published = await_stage(
                        &workspace_identity,
                        &operation_id,
                        Stage::CanonicalGenerationPublication,
                        async {
                            memory_registry.admit_canonical_generation_resident(
                            format!(
                                "daemon-admission-build-{workspace_identity}-{}-{}",
                                committed_materialization.as_materialization().workspace_generation.root_digest,
                                committed_materialization.as_materialization().selector_set_digest
                            ),
                            &workspace_identity,
                            committed_materialization,
                            )
                            .await
                            .map_err(|error| {
                                crate::runtime_server_admission::WorkspaceGenerationBuildFailure::new(
                                    crate::runtime_server_admission::WorkspaceGenerationFailureStage::CanonicalGenerationPublication,
                                    error,
                                )
                            })
                        },
                    )
                    .await?;
                    let pointer_path = crate::runtime_server_workspace::workspace_generation_pointer_path(
                        memory_registry.root(), &workspace_identity, &project_root,
                    ).map_err(|error| crate::runtime_server_admission::WorkspaceGenerationBuildFailure::new(
                        crate::runtime_server_admission::WorkspaceGenerationFailureStage::CanonicalGenerationPublication,
                        error,
                    ))?;
                    generation_publication.publish(crate::runtime_server_publication::WorkspaceGenerationPublished {
                        workspace_identity: workspace_identity.clone(),
                        project_root: project_root.clone(),
                        resident_pointer_path: pointer_path,
                        generation_digest: published.generation_digest.clone(),
                    });
                    let commit = crate::runtime_server_admission::WorkspaceGenerationCommitReceipt::from_recovery(&published).map_err(|error| {
                        crate::runtime_server_admission::WorkspaceGenerationBuildFailure::new(
                            crate::runtime_server_admission::WorkspaceGenerationFailureStage::CanonicalGenerationPublication,
                            error,
                        )
                    })?;
                    crate::runtime_server_admission::WorkspaceGenerationBuildCompletion::new(
                        captured_candidate,
                        commit,
                    ).map_err(|error| crate::runtime_server_admission::WorkspaceGenerationBuildFailure::new(
                        crate::runtime_server_admission::WorkspaceGenerationFailureStage::CanonicalGenerationPublication,
                        error,
                    ))
                    }
                    .await;
                    if let Err(error) = &result {
                        publish_event(
                            diagnostic_events.as_ref(),
                            RuntimeServerEvent::WorkspaceGenerationAdmissionFailed {
                                workspace_identity: diagnostic_workspace_identity,
                                build_mode: match build_mode {
                                    crate::runtime_server_admission::WorkspaceGenerationBuildMode::RestoreOnly => "restore-only",
                                    crate::runtime_server_admission::WorkspaceGenerationBuildMode::RestoreOrBuild => "restore-or-build",
                                    crate::runtime_server_admission::WorkspaceGenerationBuildMode::RebuildAfterMutation => "rebuild-after-mutation",
                                }
                                .to_owned(),
                                error: error.message.clone(),
                            },
                        );
                    }
                    result
                })
                    as crate::runtime_server_admission::WorkspaceGenerationBuildFuture
            },
        );
        let admission = match self.telemetry_sender.clone() {
            Some(sender) => crate::runtime_server_admission::WorkspaceGenerationAdmission::new_with_telemetry_sender(builder, sender),
            None => crate::runtime_server_admission::WorkspaceGenerationAdmission::new(builder),
        };
        self.generation_admission = Some(Arc::new(match catalog {
            Some(catalog) => admission.with_catalog(catalog),
            None => admission,
        }));
        self
    }

    pub(super) async fn serve_inner(self) -> Result<RuntimeServerExit, String> {
        let Self {
            artifact_catalog: _artifact_catalog,
            workspace_registry,
            runtime_search_service: _,
            endpoint,
            listener,
            provider_register: _provider_register,
            registry,
            mut workspace_count,
            mut shutdown,
            shutdown_handle: _,
            mut status_memory,
            events,
            generation_admission,
            asp_python_graphs_status,
            agent_session_registry_owner,
            agent_session_status,
            telemetry_sender: _,
            readiness_sender,
            generation_publication,
        } = self;
        let (_lifecycle_state, lifecycle) =
            watch::channel(crate::runtime_server_control::RuntimeServerState::Healthy);
        generation_publication.clear();
        // Socket liveness is Global; generation readiness is workspace-keyed.
        // Durable catalog locators stay lazy. A workspace request restores its
        // published pointer or admits one scope-local rebuild; startup never
        // opens every historical generation segment.
        let entry_counts = registry.workspace_entry_counts();
        let slot_count = entry_counts.slot_count;
        let loaded_entry_count = entry_counts.loaded_entry_count;
        status_memory.publish(
            crate::runtime_server_control::RuntimeServerState::Healthy,
            slot_count
                .max(loaded_entry_count)
                .max(*workspace_count.borrow()),
        )?;
        readiness_sender.send_replace(crate::runtime_server_control::RuntimeServerState::Healthy);
        let mut connections = JoinSet::new();
        let connection_supervisor =
            crate::runtime_server_runtime::RuntimeServerConnectionSupervisor::for_current_runtime(
                "runtime-server-ipc",
            );
        let control_replay_guard = Arc::new(tokio::sync::Mutex::new(
            super::control_connection::RuntimeServerControlReplayGuard::default(),
        ));
        let (drain_sender, drain_receiver) = watch::channel(false);
        let mut retirement_sweep = tokio::time::interval(std::time::Duration::from_secs(60));
        retirement_sweep.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        retirement_sweep.tick().await;
        let mut asp_python_graphs_status_changes = asp_python_graphs_status
            .as_ref()
            .map(AspPythonGraphsStatusHandle::subscribe);
        let mut agent_session_status_changes = agent_session_status
            .as_ref()
            .map(AgentSessionStatusHandle::subscribe);
        if let (Some(status), Some(owner)) = (
            agent_session_status.as_ref(),
            agent_session_registry_owner.as_ref(),
        ) {
            status.refresh(owner).await?;
        }
        let exit = loop {
            tokio::select! {
                connection = listener.accept(), if connection_supervisor.has_capacity() => {
                    let (stream, _) = connection.map_err(|error| {
                        format!("failed to accept runtime server request: {error}")
                    })?;
                    crate::runtime_server_control::validate_runtime_server_peer_fd(
                        std::os::fd::AsRawFd::as_raw_fd(&stream),
                    )?;
                    let lease = connection_supervisor
                        .try_admit()
                        .expect("capacity guard must admit one control connection");
                    let connection_endpoint = endpoint.clone();
                    let connection_registry = Arc::clone(&registry);
                    let connection_generation_admission = generation_admission.clone();
                    let connection_lifecycle = lifecycle.clone();
                    let connection_asp_python_graphs_status = asp_python_graphs_status.clone();
                    let connection_drain = drain_receiver.clone();
                    let connection_replay_guard = Arc::clone(&control_replay_guard);
                    connections.spawn(async move {
                        let result = super::control_connection::serve_connection(
                            stream,
                            connection_endpoint,
                            connection_registry,
                            connection_generation_admission,
                            connection_lifecycle,
                            connection_asp_python_graphs_status,
                            connection_drain,
                            connection_replay_guard,
                        )
                        .await;
                        (lease, result)
                    });
                }
                completed = connections.join_next(), if !connections.is_empty() => {
                    match completed {
                        Some(Ok((_lease, Ok(true)))) => {
                            let entry_counts = registry.workspace_entry_counts();
                            let slot_count = entry_counts.slot_count;
                            let loaded_entry_count = entry_counts.loaded_entry_count;
                            status_memory.publish(
                                crate::runtime_server_control::RuntimeServerState::Draining,
                                slot_count
                                    .max(loaded_entry_count)
                                    .max(*workspace_count.borrow()),
                            )?;
                            let _ = drain_sender.send(true);
                            break RuntimeServerExit::RestartRequested;
                        }
                        Some(Ok((_lease, Ok(false)))) => {}
                        Some(Ok((_lease, Err(error)))) => {
                            publish_event(
                                events.as_ref(),
                                RuntimeServerEvent::ConnectionRejected(error),
                            );
                        }
                        Some(Err(error)) => {
                            publish_event(
                                events.as_ref(),
                                RuntimeServerEvent::ConnectionTaskFailed(error.to_string()),
                            );
                        }
                        None => break RuntimeServerExit::ListenerClosed,
                    }
                }
                changed = workspace_count.changed() => {
                    if changed.is_err() {
                        break RuntimeServerExit::ListenerClosed;
                    }
                    let entry_counts = registry.workspace_entry_counts();
                    let slot_count = entry_counts.slot_count;
                    let loaded_entry_count = entry_counts.loaded_entry_count;
                    status_memory.publish(
                        crate::runtime_server_control::RuntimeServerState::Healthy,
                        slot_count
                            .max(loaded_entry_count)
                            .max(*workspace_count.borrow_and_update()),
                    )?;
                }
                _ = retirement_sweep.tick() => {
                    if let (Some(status), Some(owner)) =
                        (agent_session_status.as_ref(), agent_session_registry_owner.as_ref())
                    {
                        status.refresh(owner).await?;
                    }
                    let retirement_receipts = workspace_registry.retire_inactive().await?;
                    for receipt in retirement_receipts {
                        eprintln!(
                            "[runtime-server-workspace-retirement] schemaId={} schemaVersion={} workspaceIdentity={} reason={:?} checkpointCompleted={} writerLaneDrained={} endpointRetired={}",
                            receipt.schema_id,
                            receipt.schema_version,
                            receipt.workspace_identity,
                            receipt.reason,
                            receipt.checkpoint_completed,
                            receipt.writer_lane_drained,
                            receipt.endpoint_retired,
                        );
                    }
                }
                changed = async {
                    match asp_python_graphs_status_changes.as_mut() {
                        Some(changes) => Some(changes.changed().await),
                        None => std::future::pending().await,
                    }
                }, if asp_python_graphs_status_changes.is_some() => {
                    changed
                        .expect("asp-python-graphs status branch requires a receiver")
                        .map_err(|_| "asp-python-graphs status owner closed".to_owned())?;
                    let entry_counts = registry.workspace_entry_counts();
                    let slot_count = entry_counts.slot_count;
                    let loaded_entry_count = entry_counts.loaded_entry_count;
                    status_memory.publish(
                        crate::runtime_server_control::RuntimeServerState::Healthy,
                        slot_count
                            .max(loaded_entry_count)
                            .max(*workspace_count.borrow()),
                    )?;
                    if let Some(status) = agent_session_status.as_ref() {
                        status.mark_current_published();
                    }
                }
                changed = async {
                    match agent_session_status_changes.as_mut() {
                        Some(changes) => Some(changes.changed().await),
                        None => std::future::pending().await,
                    }
                }, if agent_session_status_changes.is_some() => {
                    changed
                        .expect("Agent session status branch requires a receiver")
                        .map_err(|_| "Agent session status owner closed".to_owned())?;
                    let entry_counts = registry.workspace_entry_counts();
                    let slot_count = entry_counts.slot_count;
                    let loaded_entry_count = entry_counts.loaded_entry_count;
                    status_memory.publish(
                        crate::runtime_server_control::RuntimeServerState::Healthy,
                        slot_count
                            .max(loaded_entry_count)
                            .max(*workspace_count.borrow()),
                    )?;
                    if let Some(status) = agent_session_status.as_ref() {
                        status.mark_current_published();
                    }
                }
                changed = shutdown.changed() => {
                    if changed.is_err() || *shutdown.borrow_and_update() {
                        let entry_counts = registry.workspace_entry_counts();
                        let slot_count = entry_counts.slot_count;
                        let loaded_entry_count = entry_counts.loaded_entry_count;
                        status_memory.publish(
                            crate::runtime_server_control::RuntimeServerState::Draining,
                            slot_count
                                .max(loaded_entry_count)
                                .max(*workspace_count.borrow()),
                        )?;
                        let _ = drain_sender.send(true);
                        break RuntimeServerExit::ShutdownRequested;
                    }
                }
            }
        };
        let _ = drain_sender.send(true);
        drop(listener);
        if tokio::time::timeout(CONNECTION_DRAIN_BOUNDARY, async {
            while let Some(completed) = connections.join_next().await {
                publish_connection_completion(events.as_ref(), completed);
            }
        })
        .await
        .is_err()
        {
            publish_event(
                events.as_ref(),
                RuntimeServerEvent::ConnectionTaskFailed(format!(
                    "Runtime Server connection drain exceeded {}ms; aborting remaining connection tasks",
                    CONNECTION_DRAIN_BOUNDARY.as_millis()
                )),
            );
            connections.abort_all();
            while let Some(completed) = connections.join_next().await {
                if !matches!(&completed, Err(error) if error.is_cancelled()) {
                    publish_connection_completion(events.as_ref(), completed);
                }
            }
        }
        Ok(exit)
    }
}

fn publish_connection_completion(
    events: Option<&crate::runtime_server_observability::RuntimeServerEventPublisher>,
    completed: Result<
        (
            crate::runtime_server_runtime::RuntimeServerConnectionLease,
            Result<bool, String>,
        ),
        tokio::task::JoinError,
    >,
) {
    match completed {
        Ok((_lease, Ok(_))) => {}
        Ok((_lease, Err(error))) => {
            publish_event(events, RuntimeServerEvent::ConnectionRejected(error))
        }
        Err(error) => publish_event(
            events,
            RuntimeServerEvent::ConnectionTaskFailed(error.to_string()),
        ),
    }
}

#[cfg(test)]
#[path = "../../tests/unit/runtime_server_connection_completion.rs"]
mod connection_completion_tests;

#[cfg(unix)]
pub(super) async fn runtime_server_shutdown_signal() -> Result<(), String> {
    let mut terminate = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        .map_err(|error| {
        format!("failed to install Runtime Server SIGTERM handler: {error}")
    })?;
    tokio::select! {
        result = tokio::signal::ctrl_c() => {
            result.map_err(|error| format!("failed to await Runtime Server Ctrl-C signal: {error}"))
        }
        _ = terminate.recv() => Ok(()),
    }
}

#[cfg(not(unix))]
pub(super) async fn runtime_server_shutdown_signal() -> Result<(), String> {
    tokio::signal::ctrl_c()
        .await
        .map_err(|error| format!("failed to await Runtime Server Ctrl-C signal: {error}"))
}
