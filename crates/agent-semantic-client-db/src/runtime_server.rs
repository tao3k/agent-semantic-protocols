use std::path::Path;
use std::sync::Arc;

use tokio::net::{UnixListener, UnixStream};
use tokio::sync::{mpsc, watch};
use tokio::task::JoinSet;

use crate::WorkspaceDbRegistry;
use crate::runtime_server_control::status_memory::RuntimeServerStatusMemoryWriter;
use crate::runtime_server_control::{
    RuntimeServerControlReceipt, RuntimeServerEndpoint, RuntimeServerRequestReadError,
    bind_runtime_server_listener, read_runtime_server_requests, write_runtime_server_receipts,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeServerExit {
    ListenerClosed,
    RestartRequested,
    ShutdownRequested,
}

pub use crate::runtime_server_observability::RuntimeServerEvent;
use crate::runtime_server_observability::publish_event;

#[derive(Clone)]
pub struct RuntimeServerShutdownHandle {
    sender: watch::Sender<bool>,
}

impl RuntimeServerShutdownHandle {
    pub fn shutdown(&self) {
        self.sender.send_replace(true);
    }
}

pub struct RuntimeServer {
    artifact_catalog: Arc<agent_semantic_runtime::runtime_artifact_catalog::RuntimeArtifactCatalog>,
    workspace_registry:
        std::sync::Arc<crate::runtime_server_workspace::RuntimeServerWorkspaceRegistry>,
    endpoint: RuntimeServerEndpoint,
    listener: UnixListener,
    data_listener: UnixListener,
    registry: Arc<WorkspaceDbRegistry>,
    workspace_count: watch::Receiver<usize>,
    shutdown: watch::Receiver<bool>,
    shutdown_handle: RuntimeServerShutdownHandle,
    status_memory: RuntimeServerStatusMemoryWriter,
    events: Option<mpsc::UnboundedSender<RuntimeServerEvent>>,
    generation_admission:
        Option<Arc<crate::runtime_server_admission::WorkspaceGenerationAdmission>>,
    owner_projection_builder:
        Option<crate::runtime_server_workspace::WorkspaceOwnerProjectionBuilder>,
    hook_evaluation_builder: Option<HookEvaluationBuilder>,
    graph_turbo_evaluation_builder: Option<GraphTurboEvaluationBuilder>,
    graph_turbo_resident_status: Option<GraphTurboResidentStatusHandle>,
    pub(crate) agent_session_registry_owner: Option<Arc<crate::AgentSessionRegistry>>,
    codex_multi_agent_control_plane_owner:
        Arc<crate::codex_multi_agent_control_plane_owner::CodexMultiAgentControlPlaneOwner>,
}

pub type HookEvaluationBuilder = std::sync::Arc<
    dyn Fn(
            String,
            std::path::PathBuf,
            Vec<String>,
            String,
        )
            -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String, String>> + Send>>
        + Send
        + Sync,
>;

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

#[derive(Clone)]
pub struct GraphTurboResidentStatusHandle {
    value: Arc<std::sync::RwLock<crate::runtime_server_control::GraphTurboResidentStatus>>,
    generation: watch::Sender<u64>,
}

impl GraphTurboResidentStatusHandle {
    pub fn new(status: crate::runtime_server_control::GraphTurboResidentStatus) -> Self {
        let (generation, _) = watch::channel(0);
        Self {
            value: Arc::new(std::sync::RwLock::new(status)),
            generation,
        }
    }

    pub fn snapshot(&self) -> crate::runtime_server_control::GraphTurboResidentStatus {
        self.value
            .read()
            .map(|status| status.clone())
            .unwrap_or_else(
                |_| crate::runtime_server_control::GraphTurboResidentStatus {
                    state: crate::runtime_server_control::GraphTurboResidentState::Failed,
                    process_id: None,
                    runtime_artifact: None,
                    execution_command_digest: None,
                    reason: Some("Graph Turbo resident status lock poisoned".to_owned()),
                },
            )
    }

    pub fn update(&self, status: crate::runtime_server_control::GraphTurboResidentStatus) {
        if let Ok(mut current) = self.value.write() {
            *current = status;
            self.generation.send_modify(|generation| {
                *generation = generation.saturating_add(1);
            });
        }
    }

    pub fn mutate(
        &self,
        update: impl FnOnce(&mut crate::runtime_server_control::GraphTurboResidentStatus),
    ) {
        if let Ok(mut current) = self.value.write() {
            update(&mut current);
            self.generation.send_modify(|generation| {
                *generation = generation.saturating_add(1);
            });
        }
    }

    fn shared(
        &self,
    ) -> Arc<std::sync::RwLock<crate::runtime_server_control::GraphTurboResidentStatus>> {
        Arc::clone(&self.value)
    }

    fn subscribe(&self) -> watch::Receiver<u64> {
        self.generation.subscribe()
    }
}

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

    pub async fn serve(self) -> Result<RuntimeServerExit, String> {
        let workspace_registry = std::sync::Arc::clone(&self.workspace_registry);
        let generation_admission = self.generation_admission.clone();
        let shutdown_handle = self.shutdown_handle();
        let serve = self.serve_inner();
        tokio::pin!(serve);
        let serve_result = tokio::select! {
            result = &mut serve => result,
            signal = runtime_server_shutdown_signal() => {
                match signal {
                    Ok(()) => {
                        shutdown_handle.shutdown();
                        serve.await
                    }
                    Err(error) => Err(error),
                }
            }
        };
        let shutdown_result = workspace_registry.shutdown().await;
        let admission_shutdown_result = match generation_admission {
            Some(admission) => admission.shutdown().await.map(|_| ()),
            None => Ok(()),
        };
        let shutdown_result = match (shutdown_result, admission_shutdown_result) {
            (Ok(_), Ok(())) => Ok(()),
            (Err(error), Ok(())) | (Ok(_), Err(error)) => Err(error),
            (Err(workspace_error), Err(admission_error)) => Err(format!(
                "workspace lane shutdown failed: {workspace_error}; admission lane shutdown failed: {admission_error}"
            )),
        };
        match (serve_result, shutdown_result) {
            (Ok(exit), Ok(_)) => Ok(exit),
            (Err(error), Ok(_)) => Err(error),
            (Ok(_), Err(error)) => Err(error),
            (Err(serve_error), Err(shutdown_error)) => Err(format!(
                "Runtime Server failed and workspace lanes did not drain: serve={serve_error}; shutdown={shutdown_error}"
            )),
        }
    }

    pub async fn bind(
        endpoint: RuntimeServerEndpoint,
        registry: Arc<WorkspaceDbRegistry>,
    ) -> Result<Self, String> {
        let state_home = agent_semantic_runtime::resolve_state_home()?;
        let workspace_store =
            crate::runtime_server_workspace::prepare_runtime_server_workspace_store(
                &state_home.join("runtime").join("server"),
            )
            .await?;
        let artifact_catalog = Arc::new(
            agent_semantic_runtime::runtime_artifact_catalog::load_runtime_artifact_catalog(
                &state_home,
            )
            .await?,
        );
        Self::bind_with_artifact_catalog(endpoint, registry, workspace_store, artifact_catalog)
            .await
    }

    pub async fn bind_with_artifact_catalog(
        endpoint: RuntimeServerEndpoint,
        registry: Arc<WorkspaceDbRegistry>,
        workspace_store: crate::runtime_server_workspace::RuntimeServerWorkspaceStore,
        artifact_catalog: Arc<
            agent_semantic_runtime::runtime_artifact_catalog::RuntimeArtifactCatalog,
        >,
    ) -> Result<Self, String> {
        endpoint.validate()?;
        if endpoint.artifact_mode != artifact_catalog.mode_label()
            || endpoint.artifact_catalog_digest != artifact_catalog.digest()
        {
            return Err(format!(
                "Runtime Server endpoint artifact catalog mismatch: endpointMode={} runtimeMode={} endpointDigest={} runtimeDigest={}",
                endpoint.artifact_mode,
                artifact_catalog.mode_label(),
                endpoint.artifact_catalog_digest,
                artifact_catalog.digest()
            ));
        }
        let listener = bind_runtime_server_listener(Path::new(&endpoint.socket_path))?;
        let data_listener =
            bind_runtime_server_listener(Path::new(&endpoint.data_plane_socket_path))?;
        let mut status_memory = RuntimeServerStatusMemoryWriter::create(&endpoint).await?;
        let (slot_count, loaded_entry_count) = registry.workspace_entry_counts();
        status_memory.publish(
            crate::runtime_server_control::RuntimeServerState::Starting,
            slot_count.max(loaded_entry_count),
        )?;
        let workspace_registry = std::sync::Arc::new(
            crate::runtime_server_workspace::RuntimeServerWorkspaceRegistry::new(
                workspace_store.root().to_path_buf(),
            )?,
        );
        let workspace_count = workspace_registry.subscribe_workspace_count();
        let (shutdown_sender, shutdown) = watch::channel(false);
        Ok(Self {
            artifact_catalog,
            workspace_registry,
            endpoint,
            listener,
            data_listener,
            registry,
            workspace_count,
            shutdown,
            shutdown_handle: RuntimeServerShutdownHandle {
                sender: shutdown_sender,
            },
            status_memory,
            events: None,
            generation_admission: None,
            owner_projection_builder: None,
            hook_evaluation_builder: None,
            graph_turbo_evaluation_builder: None,
            graph_turbo_resident_status: None,
            agent_session_registry_owner: None,
            codex_multi_agent_control_plane_owner: Arc::new(
                crate::codex_multi_agent_control_plane_owner::CodexMultiAgentControlPlaneOwner::new(
                ),
            ),
        })
    }

    pub fn with_workspace_owner_projection_builder(
        mut self,
        builder: crate::runtime_server_workspace::WorkspaceOwnerProjectionBuilder,
    ) -> Self {
        self.owner_projection_builder = Some(builder);
        self
    }

    pub fn with_hook_evaluation_builder(mut self, builder: HookEvaluationBuilder) -> Self {
        self.hook_evaluation_builder = Some(builder);
        self
    }

    pub fn shutdown_handle(&self) -> RuntimeServerShutdownHandle {
        self.shutdown_handle.clone()
    }

    pub async fn bind_and_publish(
        endpoint: RuntimeServerEndpoint,
        registry: Arc<WorkspaceDbRegistry>,
        endpoint_path: &std::path::Path,
    ) -> Result<Self, String> {
        let server = Self::bind(endpoint, registry).await?;
        if let Err(error) = crate::runtime_server_control::publish_runtime_server_endpoint(
            endpoint_path,
            &server.endpoint,
        )
        .await
        {
            server.cleanup_bound_artifacts().await;
            return Err(error);
        }
        Ok(server)
    }

    pub async fn bind_and_publish_with_artifact_catalog(
        endpoint: RuntimeServerEndpoint,
        registry: Arc<WorkspaceDbRegistry>,
        endpoint_path: &std::path::Path,
        workspace_store: crate::runtime_server_workspace::RuntimeServerWorkspaceStore,
        artifact_catalog: Arc<
            agent_semantic_runtime::runtime_artifact_catalog::RuntimeArtifactCatalog,
        >,
    ) -> Result<Self, String> {
        let server =
            Self::bind_with_artifact_catalog(endpoint, registry, workspace_store, artifact_catalog)
                .await?;
        if let Err(error) = crate::runtime_server_control::publish_runtime_server_endpoint(
            endpoint_path,
            &server.endpoint,
        )
        .await
        {
            server.cleanup_bound_artifacts().await;
            return Err(error);
        }
        Ok(server)
    }

    async fn cleanup_bound_artifacts(&self) {
        for path in [
            &self.endpoint.socket_path,
            &self.endpoint.data_plane_socket_path,
            &self.endpoint.status_memory_path,
        ] {
            let _ = tokio::fs::remove_file(path).await;
        }
    }

    pub fn with_event_sender(mut self, events: mpsc::UnboundedSender<RuntimeServerEvent>) -> Self {
        self.events = Some(events);
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

    pub fn with_workspace_generation_builder(
        self,
        source_builder: crate::runtime_server_admission::WorkspaceGenerationCandidateBuilder,
    ) -> Self {
        self.configure_workspace_generation_builder(source_builder, None, None)
    }

    pub fn with_workspace_generation_builder_and_catalog(
        self,
        source_builder: crate::runtime_server_admission::WorkspaceGenerationCandidateBuilder,
        catalog: crate::runtime_server_admission_catalog::RuntimeWorkspaceAdmissionCatalog,
    ) -> Self {
        self.configure_workspace_generation_builder(source_builder, Some(catalog), None)
    }

    pub fn with_workspace_generation_builder_catalog_identity(
        self,
        source_builder: crate::runtime_server_admission::WorkspaceGenerationCandidateBuilder,
        catalog: crate::runtime_server_admission_catalog::RuntimeWorkspaceAdmissionCatalog,
        provider_catalog_generation: String,
    ) -> Self {
        self.configure_workspace_generation_builder(
            source_builder,
            Some(catalog),
            Some(provider_catalog_generation),
        )
    }

    /// Compose the server with an already-owned generation admission plane.
    ///
    /// This keeps lifecycle supervision independent from the concrete writer
    /// lane used to build a canonical generation.
    pub fn with_workspace_generation_admission(
        mut self,
        admission: Arc<crate::runtime_server_admission::WorkspaceGenerationAdmission>,
    ) -> Self {
        self.generation_admission = Some(admission);
        self
    }

    /// Restores only canonical generations already committed to the workspace Turso authority.
    /// Missing generations remain scope-local failures and must be published by the writer lane.
    pub fn with_workspace_generation_catalog(
        self,
        catalog: crate::runtime_server_admission_catalog::RuntimeWorkspaceAdmissionCatalog,
    ) -> Self {
        self.configure_workspace_generation_builder(None, Some(catalog), None)
    }

    fn configure_workspace_generation_builder(
        mut self,
        source_builder: impl Into<
            Option<crate::runtime_server_admission::WorkspaceGenerationCandidateBuilder>,
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
                  build_mode: crate::runtime_server_admission::WorkspaceGenerationBuildMode| {
                let durable_registry = Arc::clone(&durable_registry);
                let memory_registry = Arc::clone(&memory_registry);
                let source_builder = source_builder.clone();
                let provider_catalog_generation = provider_catalog_generation.clone();
                let events = events.clone();
                let workspace_for_build = workspace_identity.clone();
                Box::pin(async move {
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
                    let current_candidate =
                        crate::runtime_server_admission::discover_workspace_generation_candidate(
                            &project_root,
                        )
                        .await?;
                    if current_candidate != candidate {
                        return Err(format!(
                            "workspace candidate generation advanced before admission: requested={} current={}",
                            candidate.candidate_generation.digest,
                            current_candidate.candidate_generation.digest
                        ));
                    }
                    let session = crate::workspace_db_ipc_server::admitted_or_bootstrap_workspace(
                        &durable_registry,
                        &workspace_identity,
                        &project_root,
                    )
                    .await?;
                    match session
                        .load_active_workspace_generation_materialization_state(&project_root)
                        .await?
                    {
                        crate::runtime_server_workspace::WorkspaceCanonicalMaterializationLoad::Ready(materialization)
                            if build_mode
                                != crate::runtime_server_admission::WorkspaceGenerationBuildMode::RebuildAfterMutation => {
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
                                let published = memory_registry
                                    .ensure_canonical_generation(
                                        format!(
                                            "daemon-admission-restore-{workspace_identity}-{}-{}",
                                            materialization.as_materialization().workspace_generation.root_digest,
                                            materialization.as_materialization().selector_set_digest
                                        ),
                                        &workspace_identity,
                                        materialization,
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
                                return crate::runtime_server_admission::WorkspaceGenerationCommitReceipt::from_recovery(&published);
                            }
                        }
                        crate::runtime_server_workspace::WorkspaceCanonicalMaterializationLoad::Ready(_)
                        | crate::runtime_server_workspace::WorkspaceCanonicalMaterializationLoad::Missing
                        | crate::runtime_server_workspace::WorkspaceCanonicalMaterializationLoad::Incompatible { .. } => {}
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
                    let build = source_builder(workspace_for_build, project_root.clone()).await?;
                    if build.candidate != candidate {
                        return Err(format!(
                            "workspace generation builder returned a different candidate identity: requested={} built={}",
                            candidate.candidate_generation.digest,
                            build.candidate.candidate_generation.digest
                        ));
                    }
                    let commit_candidate =
                        crate::runtime_server_admission::discover_workspace_generation_candidate(
                            &project_root,
                        )
                        .await?;
                    if commit_candidate != candidate {
                        return Err(format!(
                            "workspace candidate generation advanced before commit: requested={} current={}",
                            candidate.candidate_generation.digest,
                            commit_candidate.candidate_generation.digest
                        ));
                    }
                    build
                        .materialization
                        .validate_refresh_request(&workspace_identity, &build.refresh)?;
                    let committed_materialization = build.materialization.clone();
                    let durable = session
                        .commit_source_index_generation(build.refresh, build.materialization)
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
                    let published = memory_registry
                        .ensure_canonical_generation(
                            format!(
                                "daemon-admission-build-{workspace_identity}-{}-{}",
                                committed_materialization.as_materialization().workspace_generation.root_digest,
                                committed_materialization.as_materialization().selector_set_digest
                            ),
                            &workspace_identity,
                            committed_materialization,
                        )
                        .await?;
                    crate::runtime_server_admission::WorkspaceGenerationCommitReceipt::from_recovery(&published)
                })
                    as crate::runtime_server_admission::WorkspaceGenerationBuildFuture
            },
        );
        let admission = crate::runtime_server_admission::WorkspaceGenerationAdmission::new(builder);
        self.generation_admission = Some(Arc::new(match catalog {
            Some(catalog) => admission.with_catalog(catalog),
            None => admission,
        }));
        self
    }

    async fn serve_inner(self) -> Result<RuntimeServerExit, String> {
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
            owner_projection_builder,
            hook_evaluation_builder,
            graph_turbo_evaluation_builder,
            graph_turbo_resident_status,
            agent_session_registry_owner,
            codex_multi_agent_control_plane_owner,
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
        let (drain_sender, drain_receiver) = watch::channel(false);
        let mut retirement_sweep = tokio::time::interval(std::time::Duration::from_secs(60));
        retirement_sweep.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        retirement_sweep.tick().await;
        let mut graph_turbo_status_changes = graph_turbo_resident_status
            .as_ref()
            .map(GraphTurboResidentStatusHandle::subscribe);
        let exit = loop {
            tokio::select! {
                connection = listener.accept() => {
                    let (stream, _) = connection.map_err(|error| {
                        format!("failed to accept runtime server request: {error}")
                    })?;
                    connections.spawn(serve_connection(
                        stream,
                        endpoint.clone(),
                        Arc::clone(&registry),
                        lifecycle.clone(),
                        graph_turbo_resident_status.clone(),
                        drain_receiver.clone(),
                    ));
                }
                connection = data_listener.accept() => {
                    let (stream, _) = connection.map_err(|error| {
                        format!("failed to accept Runtime Server data-plane request: {error}")
                    })?;
                    let endpoint = endpoint.clone();
                    let registry = Arc::clone(&registry);
                    let memory_registry = Arc::clone(&workspace_registry);
                    let generation_admission = generation_admission.clone();
                    let owner_projection_builder = owner_projection_builder.clone();
                    let hook_evaluation_builder = hook_evaluation_builder.clone();
                    let graph_turbo_evaluation_builder = graph_turbo_evaluation_builder.clone();
                    let agent_session_registry_owner = agent_session_registry_owner.clone();
                    let codex_multi_agent_control_plane_owner =
                        Arc::clone(&codex_multi_agent_control_plane_owner);
                    let connection_drain = drain_receiver.clone();
                    connections.spawn(async move {
                        crate::workspace_db_ipc_server::serve_runtime_server_workspace_stream(
                            stream,
                            &endpoint,
                            &registry,
                            &memory_registry,
                            generation_admission.as_ref(),
                            owner_projection_builder.as_ref(),
                            hook_evaluation_builder.as_ref(),
                            graph_turbo_evaluation_builder.as_ref(),
                            agent_session_registry_owner.as_ref(),
                            &codex_multi_agent_control_plane_owner,
                            connection_drain,
                        )
                        .await?;
                        Ok(false)
                    });
                }
                completed = connections.join_next(), if !connections.is_empty() => {
                    match completed {
                        Some(Ok(Ok(true))) => {
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
                        Some(Ok(Ok(false))) => {}
                        Some(Ok(Err(error))) => {
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
        while let Some(completed) = connections.join_next().await {
            match completed {
                Ok(Ok(_)) => {}
                Ok(Err(error)) => publish_event(
                    events.as_ref(),
                    RuntimeServerEvent::ConnectionRejected(error),
                ),
                Err(error) => publish_event(
                    events.as_ref(),
                    RuntimeServerEvent::ConnectionTaskFailed(error.to_string()),
                ),
            }
        }
        Ok(exit)
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
async fn runtime_server_shutdown_signal() -> Result<(), String> {
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
async fn runtime_server_shutdown_signal() -> Result<(), String> {
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
