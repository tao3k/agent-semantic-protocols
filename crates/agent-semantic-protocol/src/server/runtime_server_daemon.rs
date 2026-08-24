//! Owns assembly and coordinated shutdown of the long-lived Runtime Server.

use agent_semantic_client_db::WorkspaceDbRegistry;
use agent_semantic_client_db::runtime_server::RuntimeServer;
use agent_semantic_client_db::runtime_server_runtime::RuntimeServerTaskScope;
use std::path::PathBuf;

use super::{
    daemon_identity, runtime_server_identity_handoff, runtime_server_search_service,
    runtime_server_telemetry_query_socket_path, runtime_server_telemetry_socket_path, state_home,
};
use agent_semantic_client_db::runtime_server_control::remove_stale_socket;
use agent_semantic_runtime::runtime_identity_monitor::spawn_runtime_identity_monitor;
use agent_semantic_runtime_server as runtime_asp_client;
use runtime_server_identity_handoff::RuntimeIdentityHandoffCoordinator;
use runtime_server_search_service::serve_runtime_search_requests;

pub(super) async fn run_daemon() -> Result<(), String> {
    agent_semantic_client_db::AgentSessionRegistry::mark_runtime_server_owner_process();
    let state_home = state_home()?;
    let result = run_daemon_at(&state_home).await;
    if let Err(error) = &result {
        // Endpoint publication happens after singleton election.  Failures before
        // that point used to leave only stderr and forced the supervisor into a
        // client-side timeout loop.  Project the daemon's own terminal state so
        // the lifecycle owner can observe an immediate, typed failure instead.
        let _ = agent_semantic_client_db::runtime_server_lifecycle::publish_with_errors(
            &state_home,
            u64::from(std::process::id()),
            false,
            vec![error.clone()],
        )
        .await;
    }
    result
}

async fn run_daemon_at(state_home: &std::path::Path) -> Result<(), String> {
    // Elect before admitting the immutable provider snapshot. Provider install
    // publishes it under the shared artifact transaction; Server bootstrap is
    // a read-only consumer and never repairs binaries or receipts.
    let election = agent_semantic_client_db::wait_for_runtime_server_election(&state_home)
        .await
        .map_err(|error| format!("failed to acquire Runtime Server election: {error}"))?;
    let runtime_provider_catalog =
        crate::command::installed_provider_artifacts::load_runtime_provider_artifacts(&state_home)
            .await?;
    let (owner_epoch, binding_token) = daemon_identity().await?;
    let workspace_store =
        agent_semantic_client_db::runtime_server_workspace::prepare_runtime_server_workspace_store(
            &state_home.join("runtime").join("server"),
        )
        .await
        .map_err(|error| format!("failed to prepare Runtime Server workspace store: {error}"))?;
    let runtime_artifact_path = std::env::current_exe()
        .map_err(|error| format!("failed to resolve running ASP artifact: {error}"))?;
    let runtime_artifact_identity =
        agent_semantic_runtime::runtime_artifact_identity::read_runtime_artifact_identity(
            state_home, "asp",
        )
        .await?;
    agent_semantic_runtime::runtime_artifact_identity::admit_runtime_invoker(
        &runtime_artifact_path,
        &runtime_artifact_identity,
        &state_home.join("runtime/bin/asp"),
    )?;
    let runtime_binary_identity = runtime_artifact_identity.identity();
    let (client_http_listener, client_http_endpoint) =
        runtime_asp_client::bind_http_listener().await?;
    let artifact_catalog =
        agent_semantic_runtime::runtime_artifact_catalog::load_runtime_artifact_catalog(
            &state_home,
        )
        .await?;
    let endpoint = agent_semantic_client_db::prepare_runtime_server_endpoint_with_workspace_store_and_identity(
        &state_home,
        workspace_store.root(),
        &runtime_artifact_path,
        &runtime_binary_identity,
        artifact_catalog.mode_label(),
        &artifact_catalog.digest(),
        owner_epoch,
        &binding_token,
        &client_http_endpoint,
    )
    .await?;
    let provider_register_state_path =
        agent_semantic_client_db::runtime_server_control::provider_register_state_path(
            std::path::Path::new(&endpoint.provider_plane_socket_path),
        )?;
    let provider_register = std::sync::Arc::new(
        agent_semantic_client_db::runtime_provider_register::RuntimeProviderRegister::from_seed_with_store(
            agent_semantic_provider_protocol::builtin_provider_registrations()?,
            provider_register_state_path,
        )
        .await?,
    );
    let socket_path = PathBuf::from(&endpoint.socket_path);
    let data_plane_socket_path = PathBuf::from(&endpoint.data_plane_socket_path);
    let provider_plane_socket_path = PathBuf::from(&endpoint.provider_plane_socket_path);
    let telemetry_socket_path = runtime_server_telemetry_socket_path(&state_home)?;
    let telemetry_query_socket_path = runtime_server_telemetry_query_socket_path(&state_home)?;
    remove_stale_socket(&socket_path).await?;
    remove_stale_socket(&data_plane_socket_path).await?;
    remove_stale_socket(&provider_plane_socket_path).await?;
    remove_stale_socket(&telemetry_socket_path).await?;
    remove_stale_socket(&telemetry_query_socket_path).await?;
    let provider_stream_listener =
        runtime_asp_client::bind_provider_stream_listener(&provider_plane_socket_path).await?;

    let admission_catalog =
        agent_semantic_client_db::runtime_server_admission_catalog::RuntimeWorkspaceAdmissionCatalog::load(
            state_home
                .join("runtime")
                .join("server")
                .join("workspace-admissions.v1.json"),
        )
        .await?;
    // Agent-session state is a host/Hook control-plane concern.  It may be
    // contended by a live host process, so acquiring its Turso handle must
    // never be a prerequisite for binding or publishing the global Runtime
    // endpoint.  The Runtime remains the sole owner of workspace generations,
    // incremental writes, and mmap reads; it deliberately does not take over
    // the host's session-registry lock during daemon bootstrap.
    let (diagnostic_events, diagnostics) =
        agent_semantic_client_db::runtime_server_diagnostics::RuntimeServerDiagnostics::start(
            state_home
                .join("runtime")
                .join("server")
                .join("runtime-server-diagnostic.v1.json"),
        )
        .await?;
    let lifecycle_bus = agent_semantic_client_db::runtime_telemetry_bus::RuntimeTelemetryBus::new();
    let (runtime_search_service, runtime_search_requests) =
        agent_semantic_client_db::runtime_search_service::runtime_search_service_channel();
    let generation_builder_state_home = state_home.to_path_buf();
    let generation_builder_provider_register = std::sync::Arc::clone(&provider_register);
    let generation_builder_runtime_search = runtime_search_service.clone();
    let generation_builder: agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationCandidateBuilder =
        std::sync::Arc::new(
            move |_workspace_identity, project_root, changed_paths, provider_target| {
                let state_home = generation_builder_state_home.clone();
                let provider_register =
                    std::sync::Arc::clone(&generation_builder_provider_register);
                let runtime_search_service = generation_builder_runtime_search.clone();
                Box::pin(async move {
                    let changed_path_count = changed_paths.len();
                    let runtime_provider_catalog = crate::command::installed_provider_artifacts::
                        load_runtime_provider_artifacts(&state_home)
                        .await?;
                    let (registry, current_catalog_generation) = crate::command::
                        installed_provider_artifacts::runtime_source_index_provider_projection(
                            &runtime_provider_catalog,
                            &provider_register,
                        )?;
                    let collection_scope = if let Some(provider_target) = provider_target {
                        let provider_id = match provider_target.provider_id {
                            Some(provider_id) => provider_id,
                            None => {
                                let providers = registry
                                    .providers
                                    .iter()
                                    .filter(|provider| {
                                        provider.runtime_operation("projection-batch").is_some()
                                            && provider.language_id.as_str()
                                                == provider_target.language_id
                                    })
                                    .collect::<Vec<_>>();
                                match providers.as_slice() {
                                    [provider] => provider.provider_id.as_str().to_owned(),
                                    [] => return Err(format!(
                                        "query-demand provider target has no registered provider: languageId={}",
                                        provider_target.language_id
                                    )),
                                    _ => return Err(format!(
                                        "query-demand provider target is ambiguous: languageId={}",
                                        provider_target.language_id
                                    )),
                                }
                            }
                        };
                        agent_semantic_client::source_index::SourceIndexCollectionScope::TargetProvider {
                            language_id: agent_semantic_client_core::LanguageId::try_new(
                                provider_target.language_id,
                            )?,
                            provider_id: agent_semantic_client_core::ProviderId::try_new(
                                provider_id,
                            )?,
                        }
                    } else if changed_paths.is_empty() {
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
                    let mut build = agent_semantic_client::source_index::
                        prepare_runtime_server_workspace_generation_with_runtime_service_async(
                            runtime_search_service,
                            project_root,
                            registry,
                            collection_scope,
                        )
                        .await
                        .map_err(|error| {
                            format!(
                                "canonical source generation failed: changedPathCount={changed_path_count} error={error}"
                            )
                        })?;
                    build.materialization.provider_schema_digest = current_catalog_generation;
                    Ok(build)
                })
            },
        );
    let owner_builder_state_home = state_home.to_path_buf();
    let owner_builder_provider_register = std::sync::Arc::clone(&provider_register);
    let owner_builder_runtime_search = runtime_search_service.clone();
    let owner_projection_builder: agent_semantic_client_db::runtime_server_admission::WorkspaceOwnerProjectionBuilder =
        std::sync::Arc::new(move |workspace_identity, project_root, owner_path| {
            let state_home = owner_builder_state_home.clone();
            let provider_register = std::sync::Arc::clone(&owner_builder_provider_register);
            let runtime_search_service = owner_builder_runtime_search.clone();
            Box::pin(async move {
                let runtime_provider_catalog = crate::command::installed_provider_artifacts::
                    load_runtime_provider_artifacts(&state_home).await?;
                let (registry, _) = crate::command::installed_provider_artifacts::
                    runtime_source_index_provider_projection(
                        &runtime_provider_catalog,
                        &provider_register,
                    )?;
                let language_id = registry
                    .providers
                    .iter()
                    .find(|provider| {
                        provider.runtime_operation("projection-batch").is_some()
                            && provider
                                .source_extensions
                                .iter()
                                .any(|extension| owner_path.ends_with(extension.as_str()))
                    })
                    .map(|provider| provider.language_id.as_str().to_owned())
                    .ok_or_else(|| {
                        format!(
                            "runtime owner projection has no registered provider: ownerPath={owner_path}"
                        )
                    })?;
                runtime_search_service
                    .provider_runtime(project_root.clone(), language_id.clone())
                    .await?;
                runtime_search_service
                    .provider_runtime_await_ready(project_root.clone(), language_id.clone())
                    .await?;
                runtime_search_service
                    .provider_owner(
                        workspace_identity,
                        project_root,
                        language_id,
                        owner_path,
                    )
                    .await
            })
        });
    let mut runtime_search_tasks = tokio::task::JoinSet::new();
    runtime_search_tasks.spawn(serve_runtime_search_requests(
        state_home.to_path_buf(),
        runtime_provider_catalog.clone(),
        std::sync::Arc::clone(&provider_register),
        runtime_search_requests,
    ));
    let endpoint_path =
        agent_semantic_client_db::runtime_server_control::runtime_server_endpoint_path_async(
            &state_home,
        )
        .await?;
    let server = RuntimeServer::bind_and_publish_with_artifact_catalog(
        endpoint.clone(),
        std::sync::Arc::new(WorkspaceDbRegistry::default()),
        &endpoint_path,
        workspace_store,
        std::sync::Arc::new(artifact_catalog),
    )
    .await
    .map_err(|error| format!("failed to bind and publish Runtime Server: {error}"))?
    .with_provider_register(std::sync::Arc::clone(&provider_register))
    .with_event_sender(diagnostic_events)
    .with_runtime_telemetry_sender(lifecycle_bus.sender.clone())
    .with_workspace_generation_and_owner_builders_and_catalog(
        generation_builder,
        owner_projection_builder,
        admission_catalog,
    )
    .with_runtime_search_service(runtime_search_service.clone());
    let query_generation_authority =
        agent_semantic_runtime_server::query_generation::RuntimeQueryGenerationAuthority::new();
    let mut generation_publications = server.workspace_generation_publication_subscribe();
    let client_generation_admission = server.workspace_generation_admission().ok_or_else(|| {
        "Runtime Server client activation requires generation admission".to_owned()
    })?;
    let client_http_service = runtime_asp_client::build_http_service(
        runtime_search_service.clone(),
        std::sync::Arc::clone(server.workspace_registry()),
        std::sync::Arc::clone(&provider_register),
        client_generation_admission,
        query_generation_authority.clone(),
        lifecycle_bus.sender.clone(),
    )?;
    let task_scope = RuntimeServerTaskScope::new("runtime-server-daemon");
    let (client_http_shutdown, client_http_shutdown_receiver) = tokio::sync::watch::channel(false);
    let mut generation_shutdown = client_http_shutdown.subscribe();
    let (client_http_done_sender, mut client_http_done_receiver) = tokio::sync::oneshot::channel();
    let client_http_task = task_scope.spawn("asp-client-http", async move {
        let result = agent_semantic_client_server::serve_asp_client_protocol_http(
            client_http_listener,
            client_http_shutdown_receiver,
            client_http_service,
        )
        .await;
        let _ = client_http_done_sender.send(result.clone());
        result
    })?;
    let (provider_stream_shutdown, provider_stream_shutdown_receiver) =
        tokio::sync::watch::channel(false);
    let (provider_stream_done_sender, mut provider_stream_done_receiver) =
        tokio::sync::oneshot::channel();
    let provider_stream_register = std::sync::Arc::clone(&provider_register);
    let provider_stream_task = task_scope.spawn("asp-provider-stream", async move {
        let result = runtime_asp_client::serve_provider_stream(
            provider_stream_listener,
            provider_stream_register,
            provider_stream_shutdown_receiver,
        )
        .await;
        let _ = provider_stream_done_sender.send(result.clone());
        result
    })?;
    // The monitor owns the in-process cancellation capability.  It must never
    // call the server's public IPC endpoint to control the same generation.
    let monitor_shutdown = server.shutdown_handle();
    // Telemetry initializes its Turso store in its own Tokio-owned lane.  It
    // cannot delay the already bound control plane from accepting its first
    // status request: endpoint readiness and observability are independent
    // lifecycle concerns.
    let telemetry_task = task_scope.spawn(
        "runtime-server-telemetry",
        agent_semantic_client_db::runtime_server_opentelemetry::RuntimeServerOpenTelemetry::start_with_telemetry_receiver(
            state_home
                .join("runtime")
                .join("server")
                .join("runtime-server-telemetry.turso"),
            telemetry_socket_path.clone(),
            telemetry_query_socket_path.clone(),
            lifecycle_bus.receiver,
        ),
    )?;
    let generation_authority = query_generation_authority.clone();
    let generation_task = task_scope.spawn("runtime-query-generation", async move {
        loop {
            tokio::select! {
                changed = generation_shutdown.changed() => {
                    if changed.is_err() || *generation_shutdown.borrow() {
                        generation_authority.clear_all();
                        break;
                    }
                    continue;
                }
                changed = generation_publications.changed() => {
                    if changed.is_err() {
                        generation_authority.clear_all();
                        break;
                    }
                }
            }
            let publication = generation_publications.borrow().clone();
            match publication {
                Some(publication) => {
                    let _ = generation_authority
                        .ensure_ready(
                            &publication.workspace_identity,
                            &publication.resident_pointer_path,
                            &publication.project_root,
                            &publication.generation_digest,
                        )
                        .await;
                }
                None => generation_authority.clear_all(),
            }
        }
    })?;
    let monitor_state_home = state_home.to_path_buf();
    let mut identity_monitor =
        spawn_runtime_identity_monitor(monitor_state_home.clone(), "asp".to_owned(), owner_epoch);
    let (identity_change_sender, mut identity_change_receiver) = tokio::sync::mpsc::channel(1);
    let monitor = task_scope.spawn("runtime-identity-monitor", async move {
        if let Some(identity_change) = identity_monitor.next_event().await {
            if identity_change_sender.send(identity_change).await.is_ok() {
                monitor_shutdown.shutdown();
            }
        }
        identity_monitor.shutdown().await;
    })?;
    let service_failure_shutdown = server.shutdown_handle();
    let (server_done_sender, mut server_done_receiver) = tokio::sync::oneshot::channel();
    let server_task = task_scope.spawn("runtime-server", async move {
        let result = server.serve().await;
        let _ = server_done_sender.send(result.clone().map(|_| ()));
        result
    })?;
    let winner = tokio::select! {
        result = &mut server_done_receiver => ("server", result),
        result = &mut client_http_done_receiver => ("client-http", result),
        result = &mut provider_stream_done_receiver => ("provider-stream", result),
    };
    if winner.0 != "server" {
        service_failure_shutdown.shutdown();
    }
    let _ = client_http_shutdown.send(true);
    let _ = provider_stream_shutdown.send(true);
    let http_result = client_http_task
        .join()
        .await
        .map_err(|error| format!("ASP Client Protocol HTTP task failed: {error}"))?;
    let provider_stream_result = provider_stream_task
        .join()
        .await
        .map_err(|error| format!("ASP ProviderSession stream task failed: {error}"))?;
    let server_result = server_task
        .join()
        .await
        .map_err(|error| format!("Runtime Server task failed: {error}"))?;
    let _ = generation_task.join().await;
    query_generation_authority.clear_all();
    let server_result = server_result.map(|_| ());
    let server_result = match (winner.0, http_result, provider_stream_result, server_result) {
        ("client-http", Ok(()), Ok(()), Ok(())) => {
            Err("ASP Client Protocol HTTP service stopped before Runtime Server".to_owned())
        }
        ("client-http", Err(error), _, _) => Err(error),
        ("provider-stream", Ok(()), Ok(()), Ok(())) => {
            Err("ASP ProviderSession stream stopped before Runtime Server".to_owned())
        }
        ("provider-stream", _, Err(error), _) => Err(error),
        (_, Err(error), _, _) => Err(error),
        (_, _, Err(error), _) => Err(error),
        (_, _, _, Err(error)) => Err(error),
        (_, _, _, Ok(())) => Ok(()),
    };
    let identity_change = identity_change_receiver.try_recv().ok();
    monitor.abort();
    // Once the accept loop has stopped, all independent resident services are
    // drained concurrently. Serial draining made stop latency additive and
    // allowed one stuck read-only lane to postpone every other task owner.
    let drain_started = tokio::time::Instant::now();
    let opentelemetry = telemetry_task
        .join()
        .await
        .map_err(|error| format!("Runtime Server OpenTelemetry task failed: {error}"))??;
    let telemetry_handle = opentelemetry.handle();
    let telemetry_drain = async {
        let started = tokio::time::Instant::now();
        (opentelemetry.shutdown().await, started.elapsed())
    };
    let diagnostic_drain = async {
        let started = tokio::time::Instant::now();
        (diagnostics.join().await, started.elapsed())
    };
    let ((telemetry_result, telemetry_elapsed), (diagnostic_result, diagnostic_elapsed)) =
        tokio::join!(telemetry_drain, diagnostic_drain);
    let total_drain_elapsed = drain_started.elapsed();
    for (service, elapsed, state) in [
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
    let telemetry_clean = telemetry_result.is_ok();
    let diagnostic_clean = diagnostic_result.is_ok();
    let telemetry_error = telemetry_result.as_ref().err().map(ToString::to_string);
    let diagnostic_error = diagnostic_result.as_ref().err().map(ToString::to_string);
    let mut shutdown_errors = Vec::new();
    if let Err(error) = task_scope.finish(total_drain_elapsed.as_micros() as u64) {
        shutdown_errors.push(format!("taskScope={error}"));
    }
    if let Err(error) = server_result {
        shutdown_errors.push(format!("server={error}"));
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
            "openTelemetryMicros": telemetry_elapsed.as_micros(),
            "diagnosticsMicros": diagnostic_elapsed.as_micros(),
            "errors": &shutdown_errors,
        })
    );
    let drain_receipt_result = agent_semantic_client_db::runtime_server_lifecycle::publish_drain(
        &state_home,
        agent_semantic_client_db::RuntimeServerDrainReceipt {
            owner_epoch,
            services: serde_json::json!({
                "server": {"state": if shutdown_errors.is_empty() { "drained" } else { "failed" }},
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
    let mut identity_handoff = identity_change.is_some()
        && !agent_semantic_client_db::runtime_server_lifecycle::operator_stopped(&state_home)
            .await?;
    if identity_handoff {
        if let Err(error) = RuntimeIdentityHandoffCoordinator::new(&state_home, &endpoint)
            .retire()
            .await
        {
            shutdown_errors.push(format!("ownerHandoffRetirement={error}"));
            identity_handoff = false;
        }
    }
    if identity_handoff {
        eprintln!(
            "[runtime-server-identity-handoff] {}",
            serde_json::json!({
                "schemaId": "agent.semantic-protocols.runtime-server-identity-handoff",
                "schemaVersion": "1",
                "state": "owner-retired",
                "ownerEpoch": owner_epoch,
                "processId": std::process::id(),
            })
        );
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
    // Publish the owner terminal receipt immediately after every Tokio-owned
    // service has joined. Supervisor stop can now observe the authoritative
    // terminal state without waiting on filesystem/socket cleanup.
    RuntimeIdentityHandoffCoordinator::new(&state_home, &endpoint)
        .cleanup()
        .await?;
    if identity_handoff
        && !agent_semantic_client_db::runtime_server_lifecycle::operator_stopped(&state_home)
            .await?
    {
        if let Err(error) = RuntimeIdentityHandoffCoordinator::new(&state_home, &endpoint)
            .admit_successor()
            .await
        {
            let failure =
                format!("Runtime Server identity successor failed readiness admission: {error}");
            eprintln!(
                "[runtime-server-identity-handoff] {}",
                serde_json::json!({
                    "schemaId": "agent.semantic-protocols.runtime-server-identity-handoff",
                    "schemaVersion": "1",
                    "state": "failed",
                    "ownerEpoch": owner_epoch,
                    "reasonKind": "successor-readiness-failed",
                    "error": failure,
                })
            );
            return Err(failure);
        }
    }
    agent_semantic_client_db::runtime_server_lifecycle::publish_with_errors(
        &state_home,
        owner_epoch,
        result.is_ok()
            && (!identity_handoff
                || !agent_semantic_client_db::runtime_server_lifecycle::operator_stopped(
                    &state_home,
                )
                .await?),
        shutdown_errors.clone(),
    )
    .await?;
    result
}
