// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::sync::Arc;

use super::generation_builder::{Stage, await_stage};

use tokio::net::TcpListener;
use tokio::sync::watch;
use tokio::task::JoinSet;

use crate::runtime_server_agent_session_status::AgentSessionStatusHandle;
pub use crate::runtime_server_asp_python_graphs_status::AspPythonGraphsStatusHandle;
use crate::runtime_server_control::RuntimeServerEndpoint;
use crate::runtime_server_control::status_memory::RuntimeServerStatusMemoryWriter;
use crate::runtime_server_observability::publish_event;

use crate::WorkspaceDbRegistry;

type RuntimeBundleDigestProbe = Arc<dyn Fn() -> Result<Option<String>, String> + Send + Sync>;

fn workspace_generation_publication(
    registry_root: &std::path::Path,
    workspace_identity: &str,
    project_root: &std::path::Path,
    generation_digest: String,
) -> Result<crate::runtime_server_publication::WorkspaceGenerationPublished, String> {
    let pointer_path = crate::runtime_server_workspace::workspace_generation_pointer_path(
        registry_root,
        workspace_identity,
        project_root,
    )?;
    let resolved = agent_semantic_client_core::state_core::ResolvedState::resolve(project_root)?;
    let project_id =
        agent_semantic_client_protocol::ClientProjectId::new(resolved.repo.repo_id.to_string())?;
    let workspace_id = agent_semantic_client_protocol::ClientWorkspaceIdentity::new(
        workspace_identity.to_owned(),
    )?;
    Ok(
        crate::runtime_server_publication::WorkspaceGenerationPublished {
            project_id,
            workspace_id,
            project_root: project_root.to_path_buf(),
            resident_pointer_path: pointer_path,
            generation_digest,
        },
    )
}

#[path = "durability.rs"]
mod durability;
use durability::{
    durable_runtime_bundle_matches_current, emit_source_index_durability_attachment,
    spawn_runtime_owned_durability_task,
};

#[path = "lifecycle_support.rs"]
mod lifecycle_support;
pub(super) use lifecycle_support::publish_connection_completion;
pub(crate) async fn runtime_server_shutdown_signal() -> Result<(), String> {
    lifecycle_support::runtime_server_shutdown_signal().await
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeServerExit {
    ListenerClosed,
    RestartRequested,
    ShutdownRequested,
}

pub use crate::runtime_server_observability::RuntimeServerEvent;

pub(super) const CONNECTION_DRAIN_BOUNDARY: std::time::Duration =
    std::time::Duration::from_millis(100);
pub(super) const DURABILITY_DRAIN_BOUNDARY: std::time::Duration =
    std::time::Duration::from_millis(100);

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
    pub(super) listener: TcpListener,
    pub(super) provider_register: Arc<crate::runtime_provider_register::RuntimeProviderRegister>,
    pub(super) registry: Arc<WorkspaceDbRegistry>,
    pub(super) workspace_count: watch::Receiver<usize>,
    pub(super) shutdown: watch::Receiver<bool>,
    pub(super) shutdown_handle: RuntimeServerShutdownHandle,
    pub(super) readiness_sender: watch::Sender<crate::runtime_server_control::RuntimeServerState>,
    pub(super) startup_readiness: Option<watch::Receiver<bool>>,
    pub(super) generation_publication:
        crate::runtime_server_publication::WorkspaceGenerationPublication,
    pub(super) durability_tasks: Arc<tokio::sync::Mutex<JoinSet<()>>>,
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

    /// Keep public health in `Starting` until the daemon closes its resident
    /// recovery barrier. The listener remains available to the activation
    /// owner while the barrier is open.
    #[must_use]
    pub fn with_startup_readiness_barrier(mut self, readiness: watch::Receiver<bool>) -> Self {
        self.startup_readiness = Some(readiness);
        self
    }

    pub fn workspace_generation_publication_subscribe(
        &self,
    ) -> watch::Receiver<Option<crate::runtime_server_publication::WorkspaceGenerationPublished>>
    {
        self.generation_publication.subscribe()
    }

    pub(super) async fn cleanup_bound_artifacts(&self) {
        let _ = tokio::fs::remove_file(&self.endpoint.status_memory_path).await;
    }

    pub(crate) fn configure_workspace_generation_builder(
        mut self,
        source_builder: impl Into<
            Option<crate::runtime_server_admission::WorkspaceGenerationCandidateBuilder>,
        >,
        catalog: Option<crate::runtime_server_admission_catalog::RuntimeWorkspaceAdmissionCatalog>,
        runtime_bundle_digest_probe: Option<RuntimeBundleDigestProbe>,
    ) -> Self {
        let source_builder = source_builder.into();
        let durable_registry = Arc::clone(&self.registry);
        let memory_registry = Arc::clone(&self.workspace_registry);
        let generation_publication = self.generation_publication.clone();
        let durability_tasks = Arc::clone(&self.durability_tasks);
        let durability_lanes =
            Arc::new(dashmap::DashMap::<String, Arc<tokio::sync::Mutex<()>>>::new());
        let events = self.events.clone();
        let durable_restore_runtime_bundle_probe = runtime_bundle_digest_probe.clone();
        let ready_validator = runtime_bundle_digest_probe.map(|probe| {
            let memory_registry = Arc::clone(&memory_registry);
            std::sync::Arc::new(
                move |
                    workspace_identity: &str,
                    project_root: &std::path::Path,
                    receipt: &crate::runtime_server_admission::WorkspaceGenerationAdmissionReceipt,
                | {
                    let expected = probe()?;
                    let lease = memory_registry.lease(workspace_identity, project_root)?;
                    let generation = lease.generation();
                    let committed_generation_digest = receipt
                        .commit
                        .as_ref()
                        .ok_or_else(|| {
                            "ready Runtime generation admission lacks its commit".to_owned()
                        })?
                        .generation_digest
                        .as_str();
                    if generation.generation_digest != committed_generation_digest {
                        return Err(format!(
                            "Runtime resident generation drift: expected={} observed={}",
                            committed_generation_digest, generation.generation_digest,
                        ));
                    }
                    let observed = match generation.runtime_provider_execution_binding.as_ref() {
                        Some(binding) => {
                            binding.validate()?;
                            Some(binding.runtime_bundle_digest.clone())
                        }
                        None => None,
                    };
                    if observed != expected {
                        return Err(format!(
                            "Runtime bundle identity drift: expected={} observed={}",
                            expected.as_deref().unwrap_or("absent"),
                            observed.as_deref().unwrap_or("absent")
                        ));
                    }
                    Ok(())
                },
            ) as crate::runtime_server_admission::WorkspaceGenerationReadyValidator
        });
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
                let durability_tasks = Arc::clone(&durability_tasks);
                let durability_lane = Arc::clone(
                    durability_lanes
                        .entry(workspace_identity.clone())
                        .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
                        .value(),
                );
                let durable_restore_runtime_bundle_probe =
                    durable_restore_runtime_bundle_probe.clone();
                let source_builder = source_builder.clone();
        let source_builder_cancellation = cancellation.clone();
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
                    // Resident Ready intentionally precedes durable Source
                    // Index attachment. Keep the same workspace lane through
                    // that attachment so a targeted successor cannot reopen
                    // Turso while the previous generation still commits.
                    let generation_durability_guard = durability_lane.lock_owned().await;
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
                        agent_semantic_runtime_observability::begin_runtime_memory_operation(
                            workspace_identity.clone(),
                            operation_id.clone(),
                        );
                    if workspace_identity.trim().is_empty() {
                        return Err(crate::runtime_server_admission::WorkspaceGenerationBuildFailure::new(
                            crate::runtime_server_admission::WorkspaceGenerationFailureStage::WorkspaceBootstrap,
                            "workspace admission requires a non-empty workspace identity",
                        ));
                    }
                    // Process-cold reuse requires the separate V1 admission
                    // binding to match the freshly discovered Git candidate.
                    // The pointer alone proves durable bytes, not that those
                    // bytes still describe the current worktree and policy.
                    let durable_restore_admitted = if !build_mode.attempts_durable_restore() {
                        false
                    } else {
                        let generation_directory =
                            crate::runtime_server_workspace::workspace_generation_directory(
                                memory_registry.root(),
                                &workspace_identity,
                                &project_root,
                            )
                            .ok();
                        let read_snapshot = async {
                            let pointer_path = crate::runtime_server_workspace::workspace_generation_pointer_path(
                                memory_registry.root(),
                                &workspace_identity,
                                &project_root,
                            )
                            .ok()?;
                            let reader = crate::runtime_server_workspace::WorkspaceGenerationPointerReader::open_optional(
                                &pointer_path,
                            )
                            .await
                            .ok()??;
                            let snapshot = reader.read().ok()?;
                            snapshot.validate().ok()?;
                            Some(snapshot)
                        };
                        let read_admission_binding = async {
                            match generation_directory.as_deref() {
                                Some(directory) => {
                                    crate::runtime_server_admission_binding::read(directory)
                                        .await
                                        .ok()
                                }
                                None => None,
                            }
                        };
                        // The immutable pointer and its candidate proof are
                        // independent reads. Join them under the Runtime Tokio
                        // scheduler, then admit only their exact conjunction.
                        let (snapshot, admission_binding) =
                            tokio::join!(read_snapshot, read_admission_binding);
                        let identity_admitted = snapshot.as_ref().is_some_and(|snapshot| {
                            admission_binding.as_ref().is_some_and(|binding| {
                                binding.admits(
                                    &workspace_identity,
                                    &project_root,
                                    &candidate,
                                    snapshot,
                                )
                            })
                        });
                        let current_generation = durable_restore_runtime_bundle_probe
                            .as_ref()
                            .and_then(|probe| probe().ok().flatten());
                        let observed_generation = snapshot
                            .and_then(|snapshot| snapshot
                                .runtime_provider_execution_binding
                                .map(|binding| binding.runtime_bundle_digest));
                        let provider_admitted = match observed_generation.as_deref() {
                            Some(observed) => durable_runtime_bundle_matches_current(
                                Some(observed),
                                current_generation.as_deref(),
                            ),
                            None => provider_target.is_none(),
                        };
                        identity_admitted && provider_admitted
                    };
                    if durable_restore_admitted {
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
                        let mut restore_observation = agent_semantic_runtime_observability::RuntimePerformanceObservation::new(
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
                        restore_observation.freshness_authority = Some(
                            agent_semantic_runtime_observability::RuntimeFreshnessAuthority::CurrentSnapshot,
                        );
                        if pointer_restore.is_err() {
                            restore_observation.failure_reason =
                                Some("workspace-generation-pointer-restore-failed".to_owned());
                        } else if !restored_generation_covers_demand {
                            restore_observation.failure_reason = Some(
                                "workspace-generation-restored-provider-coverage-missing".to_owned(),
                            );
                        }
                        let _ = agent_semantic_runtime_observability::try_record_to_active_runtime(
                            restore_observation,
                        );
                        match pointer_restore {
                            Ok(published) if restored_generation_covers_demand => {
                            generation_publication.publish(
                                workspace_generation_publication(
                                    memory_registry.root(),
                                    &workspace_identity,
                                    &project_root,
                                    published.generation_digest.clone(),
                                )
                                .map_err(|error| {
                                    crate::runtime_server_admission::WorkspaceGenerationBuildFailure::new(
                                        crate::runtime_server_admission::WorkspaceGenerationFailureStage::CanonicalGenerationPublication,
                                        error,
                                    )
                                })?,
                            );
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
                        candidate.clone(),
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
                    let mut source_build_observation = agent_semantic_runtime_observability::RuntimePerformanceObservation::new(
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
                    let _ = agent_semantic_runtime_observability::try_record_to_active_runtime(
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
                    build
                        .materialization
                        .require_content_search_generation()
                        .map_err(|error| {
                            crate::runtime_server_admission::WorkspaceGenerationBuildFailure::new(
                                crate::runtime_server_admission::WorkspaceGenerationFailureStage::AdmissionValidation,
                                error,
                            )
                        })?;
                    // The immutable byte generation is the Search authority.
                    // Turso is a recoverability attachment and must not sit in
                    // front of the cold-query linearization point.
                    let source_index_refresh = build.refresh;
                    let source_index_materialization = build.materialization.clone();
                    let committed_materialization = build
                        .materialization
                        .into_validated(&workspace_identity)
                        .map_err(|error| {
                            crate::runtime_server_admission::WorkspaceGenerationBuildFailure::new(
                                crate::runtime_server_admission::WorkspaceGenerationFailureStage::AdmissionValidation,
                                error,
                            )
                        })?;
                    let published_materialization_identity = committed_materialization
                        .as_materialization()
                        .clone();
                    if cancellation.is_cancelled() {
                        return Err(crate::runtime_server_admission::WorkspaceGenerationBuildFailure::new(
                            crate::runtime_server_admission::WorkspaceGenerationFailureStage::GenerationBuilderSupervision,
                            "generation build was superseded before canonical publication",
                        ));
                    }
                    let resident_publication_started = std::time::Instant::now();
                    let published = await_stage(
                        &workspace_identity,
                        &operation_id,
                        Stage::CanonicalGenerationPublication,
                        async {
                            memory_registry.admit_canonical_generation_resident_with_candidate(
                            format!(
                                "daemon-admission-build-{workspace_identity}-{}-{}",
                                committed_materialization.as_materialization().workspace_generation.root_digest,
                                committed_materialization.as_materialization().selector_set_digest
                            ),
                            &workspace_identity,
                            committed_materialization,
                            captured_candidate.clone(),
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
                    eprintln!(
                        "[resident-generation-publication-timing] {}",
                        serde_json::json!({
                            "schemaId": "agent.semantic-protocols.resident-generation-publication-timing-receipt",
                            "schemaVersion": "1",
                            "workspaceId": &workspace_identity,
                            "generationDigest": &published.generation_digest,
                            "elapsedMicros": u64::try_from(resident_publication_started.elapsed().as_micros()).unwrap_or(u64::MAX),
                        })
                    );
                    let query_publication = workspace_generation_publication(
                        memory_registry.root(),
                        &workspace_identity,
                        &project_root,
                        published.generation_digest.clone(),
                    )
                    .map_err(|error| {
                        crate::runtime_server_admission::WorkspaceGenerationBuildFailure::new(
                            crate::runtime_server_admission::WorkspaceGenerationFailureStage::CanonicalGenerationPublication,
                            error,
                        )
                    })?;
                    let project_id = query_publication.project_id.clone();
                    let workspace_id = query_publication.workspace_id.clone();
                    generation_publication.publish(query_publication);
                    let commit = crate::runtime_server_admission::WorkspaceGenerationCommitReceipt::from_recovery(&published).map_err(|error| {
                        crate::runtime_server_admission::WorkspaceGenerationBuildFailure::new(
                            crate::runtime_server_admission::WorkspaceGenerationFailureStage::CanonicalGenerationPublication,
                            error,
                        )
                    })?;
                    let completion = crate::runtime_server_admission::WorkspaceGenerationBuildCompletion::new(
                        captured_candidate,
                        commit,
                    ).map_err(|error| crate::runtime_server_admission::WorkspaceGenerationBuildFailure::new(
                        crate::runtime_server_admission::WorkspaceGenerationFailureStage::CanonicalGenerationPublication,
                        error,
                    ))?;
                    let durability_generation_digest = published.generation_digest.clone();
                    let durability_source_root_digest = published.source_root_digest.clone();
                    spawn_runtime_owned_durability_task(&durability_tasks, async move {
                        let _generation_durability_guard = generation_durability_guard;
                        let durability_started = std::time::Instant::now();
                        let durability = async {
                            let session = await_stage(
                                &workspace_identity,
                                &operation_id,
                                Stage::WorkspaceBootstrap,
                                async {
                                    let session = match durable_registry
                                        .loaded_session(&workspace_identity)
                                    {
                                        Some(session) => session,
                                        None => durable_registry
                                            .bootstrap_workspace(&project_root)
                                            .await
                                            .map_err(|error| {
                                                crate::runtime_server_admission::WorkspaceGenerationBuildFailure::new(
                                                    crate::runtime_server_admission::WorkspaceGenerationFailureStage::WorkspaceBootstrap,
                                                    error,
                                                )
                                            })?,
                                    };
                                    if session.workspace_identity() != workspace_identity {
                                        return Err(crate::runtime_server_admission::WorkspaceGenerationBuildFailure::new(
                                            crate::runtime_server_admission::WorkspaceGenerationFailureStage::WorkspaceBootstrap,
                                            format!(
                                                "workspace admission identity mismatch: requested={workspace_identity} resolved={}",
                                                session.workspace_identity()
                                            ),
                                        ));
                                    }
                                    Ok(session)
                                },
                            )
                            .await?;
                            await_stage(
                                &workspace_identity,
                                &operation_id,
                                Stage::SourceIndexCommit,
                                async {
                                    session
                                        .commit_source_index_generation(
                                            source_index_refresh,
                                            source_index_materialization,
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
                            .await
                        }
                        .await;
                        let elapsed_micros = u64::try_from(
                            durability_started.elapsed().as_micros(),
                        )
                        .unwrap_or(u64::MAX);
                        let error = match durability {
                            Ok((durable, durable_materialization))
                                if durable.source_snapshot.has_same_content_identity(
                                    &durable_materialization.source_snapshot,
                                ) && durable_materialization.has_same_generation_identity(
                                    &published_materialization_identity,
                                ) => None,
                            Ok(_) => Some(
                                "durable Source Index identity differs from the published resident generation"
                                    .to_owned(),
                            ),
                            Err(error) => Some(error.message),
                        };
                        emit_source_index_durability_attachment(
                            if error.is_none() { "ready" } else { "failed" },
                            project_id.as_str(),
                            workspace_id.as_str(),
                            &durability_generation_digest,
                            &durability_source_root_digest,
                            elapsed_micros,
                            error.as_deref(),
                        );
                    })
                    .await;
                    Ok(completion)
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
        let admission = match ready_validator {
            Some(ready_validator) => admission.with_ready_validator(ready_validator),
            None => admission,
        };
        self.generation_admission = Some(Arc::new(match catalog {
            Some(catalog) => admission.with_catalog(catalog),
            None => admission,
        }));
        self
    }
}

#[cfg(test)]
#[path = "../../tests/unit/runtime_server_durability_attachment.rs"]
mod durability_attachment_tests;
