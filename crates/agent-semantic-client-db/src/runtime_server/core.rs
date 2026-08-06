use std::sync::Arc;

use super::generation_builder::{Stage, await_stage};

use tokio::net::{UnixListener, UnixStream};
use tokio::sync::watch;
use tokio::task::JoinSet;

use crate::runtime_server_agent_session_status::AgentSessionStatusHandle;
use crate::runtime_server_control::status_memory::RuntimeServerStatusMemoryWriter;
use crate::runtime_server_control::{
    RuntimeServerControlReceipt, RuntimeServerEndpoint, RuntimeServerRequestReadError,
    read_runtime_server_requests, write_runtime_server_receipts,
};
pub use crate::runtime_server_graph_turbo_status::GraphTurboResidentStatusHandle;

#[cfg(test)]
#[path = "../../tests/unit/runtime_server_build_stage_deadline.rs"]
mod runtime_server_build_stage_deadline;
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
        Arc<agent_semantic_runtime::runtime_artifact_catalog::RuntimeArtifactCatalog>,
    pub(super) workspace_registry:
        std::sync::Arc<crate::runtime_server_workspace::RuntimeServerWorkspaceRegistry>,
    pub(super) endpoint: RuntimeServerEndpoint,
    pub(super) listener: UnixListener,
    pub(super) data_listener: UnixListener,
    pub(super) registry: Arc<WorkspaceDbRegistry>,
    pub(super) workspace_count: watch::Receiver<usize>,
    pub(super) shutdown: watch::Receiver<bool>,
    pub(super) shutdown_handle: RuntimeServerShutdownHandle,
    pub(crate) status_memory: RuntimeServerStatusMemoryWriter,
    pub(super) events: Option<crate::runtime_server_observability::RuntimeServerEventPublisher>,
    pub(super) generation_admission:
        Option<Arc<crate::runtime_server_admission::WorkspaceGenerationAdmission>>,
    pub(super) graph_turbo_evaluation_builder: Option<GraphTurboEvaluationBuilder>,
    pub(super) graph_turbo_resident_status: Option<GraphTurboResidentStatusHandle>,
    pub(crate) agent_session_registry_owner: Option<Arc<crate::AgentSessionRegistry>>,
    pub(crate) agent_session_status: Option<AgentSessionStatusHandle>,
    pub(super) codex_multi_agent_control_plane_owner:
        Arc<crate::codex_multi_agent_control_plane_owner::CodexMultiAgentControlPlaneOwner>,
    pub(super) telemetry_sender: Option<crate::runtime_telemetry_bus::RuntimeTelemetryBusSender>,
}

pub type GraphTurboEvaluationBuilder = std::sync::Arc<
    dyn Fn(
            String,
            std::path::PathBuf,
            serde_json::Value,
        ) -> std::pin::Pin<
            Box<dyn std::future::Future<Output = Result<serde_json::Value, String>> + Send>,
        > + Send
        + Sync,
>;

impl RuntimeServer {
    /// Immutable artifact authority loaded once for this daemon generation.
    pub fn artifact_catalog(
        &self,
    ) -> &Arc<agent_semantic_runtime::runtime_artifact_catalog::RuntimeArtifactCatalog> {
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

    pub(super) async fn cleanup_bound_artifacts(&self) {
        for path in [
            &self.endpoint.socket_path,
            &self.endpoint.data_plane_socket_path,
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

    pub fn with_graph_turbo_evaluation_builder(
        mut self,
        builder: GraphTurboEvaluationBuilder,
    ) -> Self {
        self.graph_turbo_evaluation_builder = Some(builder);
        self
    }

    pub fn with_graph_turbo_resident_status(
        mut self,
        status: GraphTurboResidentStatusHandle,
    ) -> Self {
        self.status_memory.set_graph_turbo_resident(status.shared());
        self.graph_turbo_resident_status = Some(status);
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
        provider_catalog_generation: Option<String>,
    ) -> Self {
        let source_builder = source_builder.into();
        let durable_registry = Arc::clone(&self.registry);
        let memory_registry = Arc::clone(&self.workspace_registry);
        let events = self.events.clone();
        let builder = Arc::new(
            move |workspace_identity: String,
                  project_root: std::path::PathBuf,
                  candidate: crate::runtime_server_admission::WorkspaceGenerationCandidateIdentity,
                  build_mode: crate::runtime_server_admission::WorkspaceGenerationBuildMode,
                  _cancellation: crate::runtime_generation_cancellation::GenerationCancellation,
                  absolute_deadline: tokio::time::Instant| {
                let durable_registry = Arc::clone(&durable_registry);
                let memory_registry = Arc::clone(&memory_registry);
                let source_builder = source_builder.clone();
                let provider_catalog_generation = provider_catalog_generation.clone();
                let events = events.clone();
                let workspace_for_build = workspace_identity.clone();
                Box::pin(async move {
                    if _cancellation.is_cancelled() {
                        return Err("generation build cancelled".to_owned());
                    }
                    let diagnostic_workspace_identity = workspace_identity.clone();
                    let diagnostic_events = events.clone();
                    let result = async move {
                    if _cancellation.is_cancelled() {
                        return Err("generation build cancelled".to_owned());
                    }
                    let build_started = std::time::Instant::now();
                    let build_deadline = absolute_deadline
                        .checked_sub(std::time::Duration::from_millis(20))
                        .unwrap_or(absolute_deadline);
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
                        build_started,
                        build_deadline,
                        Stage::WorkspaceBootstrap,
                        crate::workspace_db_ipc_server::admitted_or_bootstrap_workspace(
                            &durable_registry,
                            &workspace_identity,
                            &project_root,
                        ),
                    )
                    .await?;
                    if build_mode.attempts_durable_restore() {
                        let restore_started = std::time::Instant::now();
                        let materialization_load = await_stage(
                            build_started,
                            build_deadline,
                            Stage::DurableRestore,
                            session.load_active_workspace_generation_materialization_state(
                                &project_root,
                            ),
                        )
                        .await;
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
                        if materialization_load.is_err() {
                            restore_observation.failure_reason =
                                Some("workspace-generation-durable-restore-failed".to_owned());
                        }
                        let _ = crate::runtime_server_opentelemetry::try_record_to_active_runtime(
                            restore_observation,
                        );
                        match materialization_load? {
                            crate::runtime_server_workspace::WorkspaceCanonicalMaterializationLoad::Ready(materialization) => {
                            publish_event(
                                events.as_ref(),
                                RuntimeServerEvent::WorkspaceGenerationMaterializationObserved {
                                    workspace_identity: workspace_identity.clone(),
                                    build_mode: build_mode_label.to_owned(),
                                    owner_count: materialization.owners.len(),
                                    owner_source_bytes: materialization
                                        .owners
                                        .iter()
                                        .fold(0_u64, |total, owner| {
                                            total.saturating_add(owner.bytes.len() as u64)
                                        }),
                                    selector_count: materialization
                                        .owners
                                        .iter()
                                        .map(|owner| owner.selectors.len())
                                        .sum(),
                                    relation_count: materialization.relations.len(),
                                },
                            );
                            let materialization = materialization.into_validated(&workspace_identity)?;
                            if canonical_materialization_matches_admitted_generation(
                                materialization.as_materialization(),
                                provider_catalog_generation.as_deref(),
                                &candidate,
                            ) {
                                let published = await_stage(
                                    build_started,
                                    build_deadline,
                                    Stage::CanonicalGenerationPublication,
                                    memory_registry.ensure_canonical_generation(
                                        format!(
                                            "daemon-admission-restore-{workspace_identity}-{}-{}",
                                            materialization.as_materialization().workspace_generation.root_digest,
                                            materialization.as_materialization().selector_set_digest
                                        ),
                                        &workspace_identity,
                                        materialization,
                                    ),
                                )
                                .await?;
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
                                let commit = crate::runtime_server_admission::WorkspaceGenerationCommitReceipt::from_recovery(&published)?;
                                return crate::runtime_server_admission::WorkspaceGenerationBuildCompletion::new(
                                    candidate.clone(),
                                    commit,
                                );
                            }
                            }
                            crate::runtime_server_workspace::WorkspaceCanonicalMaterializationLoad::Missing
                            | crate::runtime_server_workspace::WorkspaceCanonicalMaterializationLoad::Incompatible { .. } => {}
                        }
                    }
                    if build_mode
                        == crate::runtime_server_admission::WorkspaceGenerationBuildMode::RestoreOnly
                    {
                        return Err(format!(
                            "registered workspace restore requires a current canonical materialization; an explicit admission is required before rebuild: workspaceIdentity={workspace_identity}"
                        ));
                    }
                    let source_builder = source_builder.as_ref().ok_or_else(|| {
                        format!(
                            "canonical workspace generation is unavailable; writer lane publication is required before admission: workspaceIdentity={workspace_identity}"
                        )
                    })?;
                    let source_build_started = std::time::Instant::now();
                    let source_build = await_stage(
                        build_started,
                        build_deadline,
                        Stage::SourceBuilder,
                        source_builder(workspace_for_build, project_root.clone()),
                    )
                    .await;
                    let source_build_elapsed_micros = source_build_started
                        .elapsed()
                        .as_micros()
                        .min(u128::from(u64::MAX)) as u64;
                    let source_build_budget_micros = 800_000;
                    let mut source_build_observation = crate::runtime_server_opentelemetry::RuntimePerformanceObservation::new(
                        "workspace-generation-admission",
                        "source-builder",
                        source_build_elapsed_micros,
                        source_build_budget_micros,
                        if source_build_elapsed_micros < source_build_budget_micros {
                            "within-budget"
                        } else {
                            "budget-exceeded"
                        },
                    )
                    .with_operation_id(operation_id);
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
                        .validate_refresh_request(&workspace_identity, &build.refresh)?;
                    let (durable, committed_materialization) =
                        await_stage(
                            build_started,
                            build_deadline,
                            Stage::SourceIndexCommit,
                            session
                                .commit_source_index_generation(
                                    build.refresh,
                                    build.materialization,
                                ),
                        )
                        .await?;
                    let committed_materialization =
                        committed_materialization.into_validated(&workspace_identity)?;
                    if !durable
                        .source_snapshot
                        .has_same_content_identity(
                            &committed_materialization.as_materialization().source_snapshot,
                        )
                    {
                        return Err(format!(
                            "Turso generation evidence differs from canonical materialization: durable={:?} materialized={:?}",
                            durable.source_snapshot,
                            committed_materialization.as_materialization().source_snapshot
                        ));
                    }
                    let published = await_stage(
                        build_started,
                        build_deadline,
                        Stage::CanonicalGenerationPublication,
                        memory_registry.ensure_canonical_generation(
                            format!(
                                "daemon-admission-build-{workspace_identity}-{}-{}",
                                committed_materialization.as_materialization().workspace_generation.root_digest,
                                committed_materialization.as_materialization().selector_set_digest
                            ),
                            &workspace_identity,
                            committed_materialization,
                        ),
                    )
                    .await?;
                    let commit = crate::runtime_server_admission::WorkspaceGenerationCommitReceipt::from_recovery(&published)?;
                    crate::runtime_server_admission::WorkspaceGenerationBuildCompletion::new(
                        captured_candidate,
                        commit,
                    )
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
                                error: error.clone(),
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
        let admission = match owner_projection_builder {
            Some(builder) => admission.with_owner_projection_builder(builder),
            None => admission,
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
            endpoint,
            listener,
            data_listener,
            registry,
            mut workspace_count,
            mut shutdown,
            shutdown_handle: _,
            mut status_memory,
            events,
            generation_admission,
            graph_turbo_evaluation_builder,
            graph_turbo_resident_status,
            agent_session_registry_owner,
            agent_session_status,
            codex_multi_agent_control_plane_owner,
            telemetry_sender: _,
        } = self;
        let (_lifecycle_state, lifecycle) =
            watch::channel(crate::runtime_server_control::RuntimeServerState::Healthy);
        // Socket liveness is Global; generation readiness is workspace-keyed.
        // Registered durable generations are restored on demand by the typed
        // workspace admission path. Daemon startup must not materialize every
        // catalog entry into resident memory.
        let (slot_count, loaded_entry_count) = registry.workspace_entry_counts();
        status_memory.publish(
            crate::runtime_server_control::RuntimeServerState::Healthy,
            slot_count
                .max(loaded_entry_count)
                .max(*workspace_count.borrow()),
        )?;
        let mut connections = JoinSet::new();
        let connection_supervisor =
            crate::runtime_server_runtime::RuntimeServerConnectionSupervisor::for_current_runtime(
                "runtime-server-ipc",
            );
        let (drain_sender, drain_receiver) = watch::channel(false);
        let mut retirement_sweep = tokio::time::interval(std::time::Duration::from_secs(60));
        retirement_sweep.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        retirement_sweep.tick().await;
        let mut graph_turbo_status_changes = graph_turbo_resident_status
            .as_ref()
            .map(GraphTurboResidentStatusHandle::subscribe);
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
                    let lease = connection_supervisor
                        .try_admit()
                        .expect("capacity guard must admit one control connection");
                    let connection_endpoint = endpoint.clone();
                    let connection_registry = Arc::clone(&registry);
                    let connection_lifecycle = lifecycle.clone();
                    let connection_graph_turbo_status = graph_turbo_resident_status.clone();
                    let connection_drain = drain_receiver.clone();
                    connections.spawn(async move {
                        let result = serve_connection(
                            stream,
                            connection_endpoint,
                            connection_registry,
                            connection_lifecycle,
                            connection_graph_turbo_status,
                            connection_drain,
                        )
                        .await;
                        (lease, result)
                    });
                }
                connection = data_listener.accept(), if connection_supervisor.has_capacity() => {
                    let (stream, _) = connection.map_err(|error| {
                        format!("failed to accept Runtime Server data-plane request: {error}")
                    })?;
                    let endpoint = endpoint.clone();
                    let registry = Arc::clone(&registry);
                    let memory_registry = Arc::clone(&workspace_registry);
                    let generation_admission = generation_admission.clone();
                    let graph_turbo_evaluation_builder = graph_turbo_evaluation_builder.clone();
                    let agent_session_registry_owner = agent_session_registry_owner.clone();
                    let agent_session_status = agent_session_status.clone();
                    let codex_multi_agent_control_plane_owner =
                        Arc::clone(&codex_multi_agent_control_plane_owner);
                    let connection_drain = drain_receiver.clone();
                    let lease = connection_supervisor
                        .try_admit()
                        .expect("capacity guard must admit one data-plane connection");
                    connections.spawn(async move {
                        let result = crate::workspace_db_ipc_server::serve_runtime_server_workspace_stream(
                            stream,
                            &endpoint,
                            &registry,
                            &memory_registry,
                            generation_admission.as_ref(),
                            graph_turbo_evaluation_builder.as_ref(),
                            agent_session_registry_owner.as_ref(),
                            agent_session_status.as_ref(),
                            &codex_multi_agent_control_plane_owner,
                            connection_drain,
                        )
                        .await
                        .map(|()| false);
                        (lease, result)
                    });
                }
                completed = connections.join_next(), if !connections.is_empty() => {
                    match completed {
                        Some(Ok((_lease, Ok(true)))) => {
                            let (slot_count, loaded_entry_count) =
                                registry.workspace_entry_counts();
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
                    let (slot_count, loaded_entry_count) = registry.workspace_entry_counts();
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
                    for receipt in workspace_registry.retire_inactive().await? {
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
                    match graph_turbo_status_changes.as_mut() {
                        Some(changes) => Some(changes.changed().await),
                        None => std::future::pending().await,
                    }
                }, if graph_turbo_status_changes.is_some() => {
                    changed
                        .expect("Graph Turbo status branch requires a receiver")
                        .map_err(|_| "Graph Turbo resident status owner closed".to_owned())?;
                    let (slot_count, loaded_entry_count) = registry.workspace_entry_counts();
                    status_memory.publish(
                        crate::runtime_server_control::RuntimeServerState::Healthy,
                        slot_count
                            .max(loaded_entry_count)
                            .max(*workspace_count.borrow()),
                    )?;
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
                    let (slot_count, loaded_entry_count) = registry.workspace_entry_counts();
                    status_memory.publish(
                        crate::runtime_server_control::RuntimeServerState::Healthy,
                        slot_count
                            .max(loaded_entry_count)
                            .max(*workspace_count.borrow()),
                    )?;
                }
                changed = shutdown.changed() => {
                    if changed.is_err() || *shutdown.borrow_and_update() {
                        let (slot_count, loaded_entry_count) =
                            registry.workspace_entry_counts();
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
        drop(data_listener);
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

fn canonical_materialization_matches_admitted_generation(
    materialization: &crate::runtime_server_workspace::WorkspaceCanonicalMaterialization,
    provider_catalog_generation: Option<&str>,
    candidate: &crate::runtime_server_admission::WorkspaceGenerationCandidateIdentity,
) -> bool {
    provider_catalog_generation
        .is_none_or(|generation| materialization.provider_schema_digest == generation)
        && !materialization.project_resolutions.is_empty()
        && materialization
            .project_resolutions
            .iter()
            .all(|resolution| {
                resolution.resolution.candidate_generation_digest
                    == candidate.candidate_generation.digest
            })
}

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

async fn serve_connection(
    mut stream: UnixStream,
    endpoint: RuntimeServerEndpoint,
    registry: Arc<WorkspaceDbRegistry>,
    lifecycle: watch::Receiver<crate::runtime_server_control::RuntimeServerState>,
    graph_turbo_resident_status: Option<GraphTurboResidentStatusHandle>,
    mut drain: watch::Receiver<bool>,
) -> Result<bool, String> {
    loop {
        let requests = tokio::select! {
            requests = read_runtime_server_requests(&mut stream) => match requests {
                Ok(requests) => requests,
                Err(RuntimeServerRequestReadError::Closed) => return Ok(false),
                Err(RuntimeServerRequestReadError::Invalid(error)) => return Err(error),
            },
            changed = drain.changed() => {
                let _ = changed;
                return Ok(false);
            }
        };
        let (slot_count, loaded_entry_count) = registry.workspace_entry_counts();
        let workspace_entry_count = slot_count.max(loaded_entry_count);
        let mut restart = false;
        let mut receipts = Vec::with_capacity(requests.len());
        for request in requests {
            let request_restart = request.requires_restart(&endpoint)?;
            restart |= request_restart;
            let mut receipt = if request_restart {
                RuntimeServerControlReceipt::draining(
                    request.request_id,
                    &endpoint,
                    workspace_entry_count,
                )
            } else if *lifecycle.borrow()
                == crate::runtime_server_control::RuntimeServerState::Starting
            {
                let mut receipt = RuntimeServerControlReceipt::starting(
                    request.request_id,
                    endpoint.runtime_artifact_digest.clone(),
                    endpoint.artifact_mode.clone(),
                    endpoint.artifact_catalog_digest.clone(),
                    "workspace-generation-restore".to_owned(),
                );
                receipt.workspace_entry_count = workspace_entry_count;
                receipt
            } else {
                RuntimeServerControlReceipt::healthy(
                    request.request_id,
                    &endpoint,
                    workspace_entry_count,
                )
            };
            receipt.graph_turbo_resident = graph_turbo_resident_status
                .as_ref()
                .map(|status| status.snapshot());
            receipts.push(receipt);
        }
        write_runtime_server_receipts(&mut stream, &receipts).await?;
        if restart {
            return Ok(true);
        }
    }
}
