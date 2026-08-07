//! Owns assembly and coordinated shutdown of the long-lived Runtime Server.

use agent_semantic_client_db::runtime_server::RuntimeServer;
use agent_semantic_client_db::{
    WorkspaceDbRegistry, acquire_runtime_server_election,
    prepare_runtime_server_endpoint_with_workspace_store, runtime_server_endpoint_path,
};
use std::path::PathBuf;

use super::{
    cleanup_endpoint, daemon_identity, graph_turbo_daemon, remove_stale_socket,
    runtime_server_telemetry_query_socket_path, runtime_server_telemetry_socket_path,
    singleton_socket, state_home,
};

pub(super) async fn run_daemon() -> Result<(), String> {
    agent_semantic_client_db::AgentSessionRegistry::mark_runtime_server_owner_process();
    let state_home = state_home()?;
    let election = acquire_runtime_server_election(&state_home)
        .await
        .map_err(|error| format!("failed to acquire Runtime Server election: {error}"))?;
    let singleton_socket_guard = match singleton_socket::acquire(&state_home).await? {
        singleton_socket::SingletonSocketElection::Acquired(guard) => guard,
        singleton_socket::SingletonSocketElection::ResidentExists => return Ok(()),
    };
    let workspace_store =
        agent_semantic_client_db::runtime_server_workspace::prepare_runtime_server_workspace_store(
            &state_home.join("runtime").join("server"),
        )
        .await
        .map_err(|error| format!("failed to prepare Runtime Server workspace store: {error}"))?;
    let runtime_artifact_path = std::env::current_exe()
        .map_err(|error| format!("failed to resolve running ASP artifact: {error}"))?;
    let runtime_artifact_digest =
        crate::command::protocol_binary::canonical_protocol_binary_artifact_digest(
            &runtime_artifact_path,
        )
        .await?;
    let artifact_catalog =
        agent_semantic_runtime::runtime_artifact_catalog::load_runtime_artifact_catalog(
            &state_home,
        )
        .await?;
    let (owner_epoch, binding_token) = daemon_identity().await?;
    let startup_started = tokio::time::Instant::now();
    crate::server::runtime_server_startup_receipt::publish(
        &state_home,
        owner_epoch,
        "daemon-identity",
        "ready",
        startup_started,
        None,
    )
    .await?;
    crate::server::runtime_server_startup_receipt::publish(
        &state_home,
        owner_epoch,
        "artifact-catalog",
        "ready",
        startup_started,
        None,
    )
    .await?;
    let endpoint = prepare_runtime_server_endpoint_with_workspace_store(
        &state_home,
        workspace_store.root(),
        &runtime_artifact_path,
        &runtime_artifact_digest,
        artifact_catalog.mode_label(),
        &artifact_catalog.digest(),
        owner_epoch,
        &binding_token,
    )
    .await?;
    let socket_path = PathBuf::from(&endpoint.socket_path);
    let data_plane_socket_path = PathBuf::from(&endpoint.data_plane_socket_path);
    let telemetry_socket_path = runtime_server_telemetry_socket_path(&state_home);
    let telemetry_query_socket_path = runtime_server_telemetry_query_socket_path(&state_home);
    remove_stale_socket(&socket_path).await?;
    remove_stale_socket(&data_plane_socket_path).await?;
    remove_stale_socket(&telemetry_socket_path).await?;
    remove_stale_socket(&telemetry_query_socket_path).await?;

    let admission_catalog =
        agent_semantic_client_db::runtime_server_admission_catalog::RuntimeWorkspaceAdmissionCatalog::load(
            state_home
                .join("runtime")
                .join("server")
                .join("workspace-admissions.v1.json"),
        )
        .await?;
    let agent_session_registry_owner = std::sync::Arc::new(
        agent_semantic_client_db::AgentSessionRegistry::open_or_create_state_root_async(
            &state_home,
        )
        .await?,
    );
    let (diagnostic_events, diagnostics) =
        agent_semantic_client_db::runtime_server_diagnostics::RuntimeServerDiagnostics::start(
            state_home
                .join("runtime")
                .join("server")
                .join("runtime-server-diagnostic.v1.json"),
        )
        .await?;
    let lifecycle_bus = agent_semantic_client_db::runtime_telemetry_bus::RuntimeTelemetryBus::new();
    let provider_catalog_generation =
        crate::command::global_provider_catalog::read_runtime_provider_catalog_readiness()?
            .catalog_generation;
    let generation_builder_catalog = provider_catalog_generation.clone();
    let generation_builder: agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationCandidateBuilder =
    std::sync::Arc::new(move |_workspace_identity, project_root, changed_paths| {
            let provider_catalog_generation = generation_builder_catalog.clone();
        Box::pin(async move {
            let collection_scope = if changed_paths.is_empty() {
                agent_semantic_client::source_index::SourceIndexCollectionScope::CompleteGeneration
            } else {
                let owner_paths = changed_paths
                    .iter()
                    .map(|path| {
                        path.strip_prefix(&project_root)
                            .map_err(|_| {
                                format!(
                                    "changed owner is outside Runtime workspace: workspace={} owner={}",
                                    project_root.display(),
                                    path.display()
                                )
                            })
                            .map(|relative| relative.to_string_lossy().into_owned())
                    })
                    .collect::<Result<Vec<_>, String>>()?;
                agent_semantic_client::source_index::SourceIndexCollectionScope::ExplicitOwners {
                    owner_paths,
                }
            };
                let (registry, current_catalog_generation) =
                    crate::command::global_provider_catalog::runtime_provider_registry_snapshot(
                        &project_root,
                    )?;
                if current_catalog_generation != provider_catalog_generation {
                    return Err(format!(
                        "runtime provider catalog advanced after daemon admission: admitted={} current={}",
                        provider_catalog_generation, current_catalog_generation
                    ));
                }
                let mut build = agent_semantic_client::source_index::
                prepare_runtime_server_workspace_generation_with_registry_async(
                    project_root,
                    registry,
                    collection_scope,
                )
                    .await?;
                build.materialization.provider_schema_digest = current_catalog_generation;
                Ok(build)
            })
        });
    let owner_builder_catalog = provider_catalog_generation.clone();
    let owner_projection_builder: agent_semantic_client_db::runtime_server_admission::WorkspaceOwnerProjectionBuilder =
        std::sync::Arc::new(move |workspace_identity, project_root, owner_path, language_id| {
            let provider_catalog_generation = owner_builder_catalog.clone();
            Box::pin(async move {
                let (registry, current_catalog_generation) =
                    crate::command::global_provider_catalog::runtime_provider_registry_snapshot(
                        &project_root,
                    )?;
                if current_catalog_generation != provider_catalog_generation {
                    return Err(format!(
                        "runtime provider catalog advanced after daemon admission: admitted={} current={}",
                        provider_catalog_generation, current_catalog_generation
                    ));
                }
                agent_semantic_client::source_index::
                    prepare_runtime_server_owner_projection_with_registry_async(
                        project_root,
                        workspace_identity,
                        owner_path,
                        language_id,
                        registry,
                    )
                    .await
            })
        });
    let locator_catalog = admission_catalog.clone();
    let server = RuntimeServer::bind_and_publish_with_artifact_catalog(
        endpoint.clone(),
        std::sync::Arc::new(WorkspaceDbRegistry::default()),
        &runtime_server_endpoint_path(&state_home),
        workspace_store,
        std::sync::Arc::new(artifact_catalog),
    )
    .await
    .map_err(|error| format!("failed to bind and publish Runtime Server: {error}"))?
    .with_event_sender(diagnostic_events)
    .with_runtime_telemetry_sender(lifecycle_bus.sender.clone())
    .with_workspace_generation_and_owner_builders_catalog_identity(
        generation_builder,
        owner_projection_builder,
        admission_catalog,
        provider_catalog_generation,
    )
    .with_agent_session_registry_owner(agent_session_registry_owner);
    crate::server::runtime_server_startup_receipt::publish(
        &state_home,
        owner_epoch,
        "control-data-bind",
        "ready",
        startup_started,
        None,
    )
    .await?;
    // The control plane is already bound and published at this point.  Turso
    // telemetry is intentionally initialized afterwards: an optional
    // observability lane must never withhold the Runtime Server endpoint or
    // become a readiness prerequisite.
    let opentelemetry =
        agent_semantic_client_db::runtime_server_opentelemetry::RuntimeServerOpenTelemetry::start_with_telemetry_receiver(
            state_home
                .join("runtime")
                .join("server")
                .join("runtime-server-telemetry.turso"),
            telemetry_socket_path.clone(),
            telemetry_query_socket_path.clone(),
            lifecycle_bus.receiver,
        )
        .await?;
    crate::server::runtime_server_startup_receipt::publish(
        &state_home,
        owner_epoch,
        "opentelemetry",
        "ready",
        startup_started,
        None,
    )
    .await?;
    let hook_admission_locator =
        agent_semantic_client_db::runtime_server_hook_admission_locator::spawn_runtime_hook_admission_locator(
            state_home.clone(),
            endpoint.clone(),
            locator_catalog,
        )
        .await?;
    let mut graph_turbo =
        graph_turbo_daemon::GraphTurboDaemon::start_from_environment(&state_home).await;
    let server = server.with_graph_turbo_resident_status(graph_turbo.status());
    let server = match graph_turbo.evaluation_builder() {
        Some(builder) => server.with_graph_turbo_evaluation_builder(builder),
        None => server,
    };
    let server_result = server.serve().await.map(|_| ());
    // Once the accept loop has stopped, all independent resident services are
    // drained concurrently. Serial draining made stop latency additive and
    // allowed one stuck read-only lane to postpone every other task owner.
    let drain_started = tokio::time::Instant::now();
    let telemetry_handle = opentelemetry.handle();
    let hook_drain = async {
        let started = tokio::time::Instant::now();
        (hook_admission_locator.shutdown().await, started.elapsed())
    };
    let graph_drain = async {
        let started = tokio::time::Instant::now();
        (graph_turbo.shutdown().await, started.elapsed())
    };
    let telemetry_drain = async {
        let started = tokio::time::Instant::now();
        (opentelemetry.shutdown().await, started.elapsed())
    };
    let diagnostic_drain = async {
        let started = tokio::time::Instant::now();
        (diagnostics.join().await, started.elapsed())
    };
    let (
        (hook_result, hook_elapsed),
        (graph_result, graph_elapsed),
        (telemetry_result, telemetry_elapsed),
        (diagnostic_result, diagnostic_elapsed),
    ) = tokio::join!(hook_drain, graph_drain, telemetry_drain, diagnostic_drain);
    let total_drain_elapsed = drain_started.elapsed();
    for (service, elapsed, state) in [
        ("hookAdmissionLocator", hook_elapsed, hook_result.is_ok()),
        ("graphTurbo", graph_elapsed, graph_result.is_ok()),
        ("openTelemetry", telemetry_elapsed, telemetry_result.is_ok()),
        ("diagnostics", diagnostic_elapsed, diagnostic_result.is_ok()),
    ] {
        let _ = telemetry_handle.try_record_lifecycle(
            agent_semantic_client_db::runtime_server_opentelemetry::RuntimeLifecycleEvent {
                owner_epoch,
                workspace_identity: None,
                generation_digest: None,
                transition: format!("service-drain:{service}"),
                state: if state { "drained" } else { "failed" }.to_owned(),
                elapsed_micros: elapsed.as_micros().try_into().unwrap_or(u64::MAX),
                read_bytes: 0,
                retained_bytes: 0,
                active_task_count: 0,
                active_child_count: 0,
            },
        );
    }
    let hook_clean = hook_result.is_ok();
    let graph_clean = graph_result.is_ok();
    let telemetry_clean = telemetry_result.is_ok();
    let diagnostic_clean = diagnostic_result.is_ok();
    let hook_error = hook_result.as_ref().err().map(ToString::to_string);
    let graph_error = graph_result.as_ref().err().map(ToString::to_string);
    let telemetry_error = telemetry_result.as_ref().err().map(ToString::to_string);
    let diagnostic_error = diagnostic_result.as_ref().err().map(ToString::to_string);
    let mut shutdown_errors = Vec::new();
    if let Err(error) = server_result {
        shutdown_errors.push(format!("server={error}"));
    }
    if let Err(error) = hook_result {
        shutdown_errors.push(format!("hookAdmissionLocator={error}"));
    }
    if let Err(error) = graph_result {
        shutdown_errors.push(format!("graphTurbo={error}"));
    }
    match telemetry_result {
        Ok(receipt) => eprintln!(
            "[runtime-server-task-lifecycle] {}",
            serde_json::to_string(&receipt).map_err(|error| format!(
                "failed to encode Runtime Server OpenTelemetry lifecycle receipt: {error}"
            ))?
        ),
        Err(error) => shutdown_errors.push(format!("opentelemetry={error}")),
    }
    if let Err(error) = diagnostic_result {
        shutdown_errors.push(format!("diagnostics={error}"));
    }
    eprintln!(
        "[runtime-server-service-drain] {}",
        serde_json::json!({
            "schemaId": "agent.semantic-protocols.runtime-server-service-drain",
            "schemaVersion": "1",
            "state": if shutdown_errors.is_empty() { "drained" } else { "failed" },
            "totalMicros": total_drain_elapsed.as_micros(),
            "hookAdmissionLocatorMicros": hook_elapsed.as_micros(),
            "graphTurboMicros": graph_elapsed.as_micros(),
            "openTelemetryMicros": telemetry_elapsed.as_micros(),
            "diagnosticsMicros": diagnostic_elapsed.as_micros(),
            "errors": &shutdown_errors,
        })
    );
    let drain_receipt_result = crate::server::runtime_server_exit_receipt::publish_drain(
        &state_home,
        crate::server::runtime_server_exit_receipt::RuntimeServerDrainReceipt {
            owner_epoch,
            services: serde_json::json!({
                "server": {"state": if shutdown_errors.is_empty() { "drained" } else { "failed" }},
                "hookAdmissionLocator": {"drainMicros": hook_elapsed.as_micros(), "state": if hook_clean { "drained" } else { "failed" }, "error": hook_error},
                "graphTurbo": {"drainMicros": graph_elapsed.as_micros(), "state": if graph_clean { "drained" } else { "failed" }, "error": graph_error},
                "openTelemetry": {"drainMicros": telemetry_elapsed.as_micros(), "state": if telemetry_clean { "drained" } else { "failed" }, "error": telemetry_error},
                "diagnostics": {"drainMicros": diagnostic_elapsed.as_micros(), "state": if diagnostic_clean { "drained" } else { "failed" }, "error": diagnostic_error},
            }),
            remaining_task_count: 0,
            remaining_child_count: 0,
            clean_drain: shutdown_errors.is_empty(),
        },
    )
    .await;
    if let Err(error) = drain_receipt_result {
        shutdown_errors.push(format!("drainReceipt={error}"));
    }
    drop(election);
    let result = if shutdown_errors.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "Runtime Server shutdown did not close every owned service: {}",
            shutdown_errors.join("; ")
        ))
    };
    crate::server::runtime_server_exit_receipt::publish_with_errors(
        &state_home,
        owner_epoch,
        result.is_ok(),
        shutdown_errors.clone(),
    )
    .await?;
    // Publish the owner terminal receipt immediately after every Tokio-owned
    // service has joined. Supervisor stop can now observe the authoritative
    // terminal state without waiting on filesystem/socket cleanup.
    cleanup_endpoint(&state_home, &endpoint).await;
    let singleton_result = singleton_socket_guard.release().await;
    if let Err(error) = singleton_result {
        eprintln!(
            "[runtime-server-lifecycle] singleton release failed after terminal receipt: {error}"
        );
    }
    result
}
