// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Owns assembly and coordinated shutdown of the long-lived Runtime Server.

use super::daemon_identity;
use super::runtime_server_generation_builder::{
    build_workspace_generation_candidate_builder, resolve_host_workspace_initialization_binding,
};
use super::runtime_server_identity_handoff;
use super::runtime_server_query_generation_observer::publish_observer_terminal;
use super::runtime_server_search_service;
use super::runtime_server_telemetry_query_socket_path;
use super::runtime_server_telemetry_socket_path;
use super::state_home;
use agent_semantic_client_db::WorkspaceDbRegistry;
use agent_semantic_client_db::runtime_server::RuntimeServer;
use agent_semantic_client_db::runtime_server_control::remove_stale_socket;
use agent_semantic_client_db::runtime_server_runtime::RuntimeServerTaskScope;
use agent_semantic_runtime::runtime_identity_monitor::spawn_runtime_identity_monitor;
use agent_semantic_runtime_server as runtime_asp_client;
use runtime_server_identity_handoff::RuntimeIdentityHandoffCoordinator;
use runtime_server_search_service::serve_runtime_search_requests;

pub(super) async fn run_daemon() -> Result<(), String> {
    agent_semantic_client_db::AgentSessionRegistry::mark_runtime_server_owner_process();
    let state_home = state_home()?;
    // `run_daemon_at` is the sole owner-terminal publisher because only it has
    // the elected owner epoch. Failures before election remain process-launch
    // failures owned by the supervisor; publishing a PID-derived fallback here
    // would create a second terminal authority and overwrite a valid receipt.
    run_daemon_at(&state_home).await
}

async fn run_daemon_at(state_home: &std::path::Path) -> Result<(), String> {
    // Elect before reconciling the immutable provider snapshot. Normal binary,
    // registry, Hook-policy, or receipt drift is refreshed through the shared
    // Artifacts CAS transaction; invalid receipts still fail closed.
    let election = agent_semantic_client_db::wait_for_runtime_server_election(&state_home)
        .await
        .map_err(|error| format!("failed to acquire Runtime Server election: {error}"))?;
    let (owner_epoch, binding_token) = daemon_identity().await?;
    let runtime_serving = agent_semantic_artifacts::StateHomeLayout::new(state_home)
        .runtime_state()
        .serving();
    let workspace_store =
        agent_semantic_client_db::runtime_server_workspace::prepare_runtime_server_workspace_store(
            runtime_serving.root(),
        )
        .await
        .map_err(|error| format!("failed to prepare Runtime Server workspace store: {error}"))?;
    let workspace_store_root = workspace_store.root().to_path_buf();
    let runtime_artifact_path = std::env::current_exe()
        .map_err(|error| format!("failed to resolve running ASP artifact: {error}"))?;
    let runtime_binary_identity = if let Some(expected_digest) =
        std::env::var_os("ASP_RUNTIME_BINARY_CONTENT_DIGEST")
    {
        let expected_digest = expected_digest.to_string_lossy().into_owned();
        let expected_digest =
            agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest::parse(
                &expected_digest,
            )
            .map_err(|error| {
                format!(
                    "owner=runtime_server_daemon field=expectedDigest reasonKind=runtime-binary-identity-invalid {error}"
                )
            })?;
        let canonical_artifact = tokio::fs::canonicalize(&runtime_artifact_path)
            .await
            .map_err(|error| format!("canonicalize candidate Runtime artifact: {error}"))?;
        let observed_digest =
            agent_semantic_content_identity::file_content_digest_v1(&canonical_artifact).map_err(
                |error| {
                    format!(
                        "owner=runtime_server_daemon field=currentExecutable reasonKind=runtime-binary-content-read-failed {error}"
                    )
                },
            )?;
        if observed_digest != expected_digest.content_digest().as_str() {
            return Err(serde_json::json!({
                "schemaId": "agent.semantic-protocols.runtime-server-generation-mismatch",
                "schemaVersion": "1",
                "reasonKind": "runtime-server-generation-mismatch",
                "expectedBinaryContentDigest": expected_digest.to_string(),
                "observedBinaryContentDigest": observed_digest,
            })
            .to_string());
        }
        agent_semantic_artifacts::runtime_artifact_catalog::RuntimeBinaryIdentity::Content {
            digest: expected_digest,
        }
    } else {
        let active_bundle =
            agent_semantic_artifacts::runtime_artifact_slots::verify_runtime_artifact_bound_bundle(
                &agent_semantic_artifacts::RuntimeArtifactStateLayout::new(state_home)
                    .active_slot(),
            )
            .await?;
        let active_asp = active_bundle
            .member_path("asp")
            .ok_or_else(|| "active Runtime bundle omits asp".to_owned())?;
        let active_identity =
            agent_semantic_artifacts::runtime_artifact_catalog::RuntimeBinaryIdentity::from_bytes(
                &tokio::fs::read(&active_asp).await.map_err(|error| {
                    format!("read active Runtime asp {}: {error}", active_asp.display())
                })?,
            );
        let invoker_identity =
            agent_semantic_artifacts::runtime_artifact_catalog::RuntimeBinaryIdentity::from_bytes(
                &tokio::fs::read(&runtime_artifact_path)
                    .await
                    .map_err(|error| {
                        format!(
                            "read Runtime Server executable {}: {error}",
                            runtime_artifact_path.display()
                        )
                    })?,
            );
        if invoker_identity != active_identity {
            return Err(
                "Runtime Server executable differs from the active bound bundle".to_owned(),
            );
        }
        active_identity
    };
    let runtime_active_provider_projection =
        crate::command::active_provider_projection::load_runtime_active_provider_projection(
            state_home,
        )
        .await?;
    runtime_active_provider_projection
        .execution_binding()
        .validate()?;
    let artifact_catalog =
        agent_semantic_artifacts::runtime_artifact_catalog::load_runtime_artifact_catalog(
            &state_home,
        )
        .await?;
    let control_listener =
        agent_semantic_client_db::runtime_server_control::bind_runtime_server_listener().await?;
    let client_protocol_listener = agent_semantic_client_server::bind_asp_client_grpc_tcp().await?;
    let provider_stream_listener = runtime_asp_client::bind_provider_stream_tcp().await?;
    let control_endpoint = agent_semantic_client_db::runtime_server_control::RuntimeServerLoopbackEndpoint::from_socket_addr(
        control_listener.local_addr().map_err(|error| format!("inspect Runtime control listener: {error}"))?,
    )?;
    let data_endpoint = agent_semantic_client_db::runtime_server_control::RuntimeServerLoopbackEndpoint::from_socket_addr(
        client_protocol_listener.local_addr().map_err(|error| format!("inspect Runtime data listener: {error}"))?,
    )?;
    let provider_endpoint = agent_semantic_client_db::runtime_server_control::RuntimeServerLoopbackEndpoint::from_socket_addr(
        provider_stream_listener.local_addr().map_err(|error| format!("inspect Runtime provider listener: {error}"))?,
    )?;
    let endpoint = agent_semantic_client_db::prepare_runtime_server_endpoint_with_workspace_store_and_identity(
        &state_home,
        workspace_store.root(),
        &runtime_artifact_path,
        &runtime_binary_identity,
        artifact_catalog.mode_label(),
        &artifact_catalog.digest(),
        owner_epoch,
        &binding_token,
        control_endpoint,
        data_endpoint,
        provider_endpoint,
    )
    .await?;
    let provider_seed = agent_semantic_provider_protocol::builtin_provider_registrations()?;
    let provider_targets = runtime_active_provider_projection.active_provider_targets();
    let provider_register = agent_semantic_client_db::runtime_provider_register::
        RuntimeProviderRegister::from_bound_seed(provider_seed, &provider_targets)?;
    let provider_register = std::sync::Arc::new(provider_register);
    let transport_layout = agent_semantic_artifacts::StateHomeLayout::new(&state_home)
        .runtime_state()
        .transport();
    agent_semantic_client_db::runtime_server_control::prepare_private_runtime_directory(
        transport_layout.root(),
    )
    .await?;
    let telemetry_socket_path = runtime_server_telemetry_socket_path(&state_home)?;
    let telemetry_query_socket_path = runtime_server_telemetry_query_socket_path(&state_home)?;
    remove_stale_socket(&telemetry_socket_path).await?;
    remove_stale_socket(&telemetry_query_socket_path).await?;

    let admission_catalog =
        agent_semantic_client_db::runtime_server_admission_catalog::RuntimeWorkspaceAdmissionCatalog::load(
            runtime_serving.workspace_admission_catalog(),
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
            runtime_serving.diagnostic_receipt(),
        )
        .await?;
    let lifecycle_bus = agent_semantic_client_db::runtime_telemetry_bus::RuntimeTelemetryBus::new();
    let (runtime_search_service, runtime_search_requests) =
        agent_semantic_client_db::runtime_search_service::runtime_search_service_channel();
    let schema_bundles = runtime_asp_client::RuntimeSchemaBundleCatalog::load_embedded()?;
    // The Runtime Server is the sole owner of the resident Python Graphs
    // lifecycle. The optional executable is admitted only as a content-proven
    // member of the active Runtime bundle. Connection establishment is lazy,
    // and all callers share this one manager and its one long-lived UDS stream.
    let graph_socket = transport_layout.python_graphs_socket();
    let graph_server = match agent_semantic_runtime_server::asp_python_graphs_artifact::VerifiedAspPythonGraphsArtifact::load_from_active_runtime_bundle(state_home).await {
        Ok(artifact) => agent_semantic_runtime_server::asp_python_graphs_transport::AspPythonGraphsServer::from_artifact(artifact, graph_socket.clone(), 32)?,
        Err(error) => agent_semantic_runtime_server::asp_python_graphs_transport::AspPythonGraphsServer::unavailable(graph_socket.clone(), 32, error)?,
    };
    let generation_builder = build_workspace_generation_candidate_builder(
        state_home,
        std::sync::Arc::clone(&provider_register),
        runtime_search_service.clone(),
        schema_bundles.clone(),
        admission_catalog.clone(),
    );
    let mut runtime_search_tasks = tokio::task::JoinSet::new();
    runtime_search_tasks.spawn(serve_runtime_search_requests(
        state_home.to_path_buf(),
        runtime_active_provider_projection.clone(),
        std::sync::Arc::clone(&provider_register),
        runtime_search_requests,
        graph_server.clone(),
    ));
    let endpoint_path =
        agent_semantic_client_db::runtime_server_control::runtime_server_endpoint_path_async(
            &state_home,
        )
        .await?;
    agent_semantic_client_db::AgentSessionRegistry::mark_runtime_server_owner_process();
    let session_registry_root = agent_semantic_artifacts::StateHomeLayout::new(state_home)
        .control()
        .session_registry_root();
    let agent_session_registry = std::sync::Arc::new(
        agent_semantic_client_db::AgentSessionRegistry::open_or_create_state_root_async(
            session_registry_root,
        )
        .await?,
    );
    let server = RuntimeServer::bind_with_artifact_catalog_and_listener(
        endpoint.clone(),
        control_listener,
        std::sync::Arc::new(WorkspaceDbRegistry::default()),
        workspace_store,
        std::sync::Arc::new(artifact_catalog),
    )
    .await
    .map_err(|error| format!("failed to bind Runtime Server control plane: {error}"))?;
    let runtime_bundle_probe_state_home = state_home.to_path_buf();
    let runtime_bundle_digest_probe = std::sync::Arc::new(move || {
        crate::command::active_provider_projection::active_runtime_bundle_digest(
            &runtime_bundle_probe_state_home,
        )
    });
    let server = server
        .with_provider_register(std::sync::Arc::clone(&provider_register))
        .with_event_sender(diagnostic_events)
        .with_runtime_telemetry_sender(lifecycle_bus.sender.clone())
        .with_workspace_generation_builder_catalog_and_runtime_bundle_probe(
            generation_builder,
            admission_catalog,
            runtime_bundle_digest_probe,
        )
        .with_runtime_search_service(runtime_search_service.clone())
        .with_agent_session_registry_owner(std::sync::Arc::clone(&agent_session_registry));
    let task_scope = RuntimeServerTaskScope::new("runtime-server-daemon");
    let resource_supervisor =
        agent_semantic_client_db::runtime_server_runtime::RuntimeServerResourceSupervisor::for_current_daemon();
    let query_generation_authority =
        agent_semantic_runtime_server::RuntimeQueryGenerationAuthority::new_in_task_scope_with_calibration_store(
            task_scope.clone(),
            resource_supervisor,
            Some(workspace_store_root.join("runtime-search-calibration.v1.json")),
        )?;
    let query_generation_workspace_registry = std::sync::Arc::clone(server.workspace_registry());
    let mut generation_publications = server.workspace_generation_publication_subscribe();
    let generation_admission = server.workspace_generation_admission().ok_or_else(|| {
        "Runtime ClientFrame service requires the server-owned workspace generation admission authority"
            .to_owned()
    })?;
    let client_grpc_service = runtime_asp_client::build_frame_service(
        schema_bundles,
        std::sync::Arc::clone(&agent_session_registry),
        runtime_search_service.clone(),
        generation_admission,
        std::sync::Arc::clone(server.workspace_registry()),
        runtime_active_provider_projection.generation().to_owned(),
        std::sync::Arc::from(runtime_active_provider_projection.active_provider_targets()),
        std::sync::Arc::clone(&provider_register),
        workspace_store_root,
        std::sync::Arc::new(resolve_host_workspace_initialization_binding),
        query_generation_authority.clone(),
        lifecycle_bus.sender.clone(),
    )?;
    server
        .publish_endpoint_after_required_planes(&endpoint_path)
        .await
        .map_err(|error| format!("failed to publish ready Runtime Server endpoint: {error}"))?;
    // The monitor owns the in-process cancellation capability. It must never
    // call the server's public IPC endpoint to control the same generation.
    let monitor_shutdown = server.shutdown_handle();
    let service_failure_shutdown = monitor_shutdown.clone();
    let mut server_readiness = server.readiness_subscribe();
    let (server_done_sender, mut server_done_receiver) = tokio::sync::oneshot::channel();
    let server_task = task_scope.spawn("runtime-server", async move {
        let result = server.serve().await;
        let _ = server_done_sender.send(result.clone().map(|_| ()));
        result
    })?;
    let (client_grpc_shutdown, client_grpc_shutdown_receiver) = tokio::sync::watch::channel(false);
    let mut generation_shutdown = client_grpc_shutdown.subscribe();
    let (client_grpc_done_sender, mut client_grpc_done_receiver) = tokio::sync::oneshot::channel();
    let client_grpc_task = task_scope.spawn("asp-client-grpc", async move {
        let result = agent_semantic_client_server::serve_asp_client_grpc_tcp(
            client_protocol_listener,
            client_grpc_service,
            client_grpc_shutdown_receiver,
        )
        .await;
        let _ = client_grpc_done_sender.send(result.clone());
        result
    })?;
    let (provider_stream_shutdown, provider_stream_shutdown_receiver) =
        tokio::sync::watch::channel(false);
    let (provider_stream_done_sender, mut provider_stream_done_receiver) =
        tokio::sync::oneshot::channel();
    let provider_stream_register = std::sync::Arc::clone(&provider_register);
    let provider_stream_task = task_scope.spawn("asp-provider-stream", async move {
        let result = runtime_asp_client::serve_provider_stream_tcp(
            provider_stream_listener,
            provider_stream_register,
            provider_stream_shutdown_receiver,
        )
        .await;
        let _ = provider_stream_done_sender.send(result.clone());
        result
    })?;
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
    let generation_state_home = state_home.to_path_buf();
    let (activation_ready_sender, mut activation_ready) = tokio::sync::watch::channel(false);
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
                changed = activation_ready.changed() => {
                    if changed.is_err() {
                        generation_authority.clear_all();
                        break;
                    }
                }
            }
            if !*activation_ready.borrow() {
                continue;
            }
            let publication = generation_publications.borrow().clone();
            match publication {
                Some(publication) => {
                    let project_workspace_key =
                        agent_semantic_runtime_server::query_generation::RuntimeProjectWorkspaceKey::new(
                            publication.project_id.clone(),
                            publication.workspace_id.clone(),
                        );
                    let resident = query_generation_workspace_registry.resident_read_client(
                        publication.workspace_id.as_str(),
                        &publication.project_root,
                    );
                    match resident {
                        Ok(resident) => {
                            let execution_product = async {
                                let activation = agent_semantic_artifacts::runtime_artifact_activation::
                                    read_applied_runtime_artifact_activation_event(&generation_state_home)
                                    .await?
                                    .ok_or_else(|| {
                                        "Runtime workspace execution publication requires an applied activation receipt"
                                            .to_owned()
                                    })?;
                                let active_projection = crate::command::active_provider_projection::
                                    load_runtime_active_provider_projection(&generation_state_home)
                                    .await?;
                                let active_bundle_digest = agent_semantic_artifacts::
                                    blake3_content_digest::Blake3ContentDigest::parse(
                                        active_projection.generation(),
                                    )?;
                                let host_workspace = resolve_host_workspace_initialization_binding(
                                    &publication.project_root,
                                )?;
                                let execution_root = publication
                                    .resident_pointer_path
                                    .parent()
                                    .ok_or_else(|| {
                                        "Runtime workspace generation pointer has no publication directory"
                                            .to_owned()
                                    })?;
                                let product = agent_semantic_client_db::runtime_server_workspace::
                                    compose_runtime_workspace_execution_product_from_resident(
                                        &resident,
                                        host_workspace,
                                        &activation,
                                        &active_bundle_digest,
                                        active_projection.execution_binding(),
                                    )?;
                                Ok::<_, String>((product, execution_root.to_path_buf()))
                            }
                            .await;
                            let (execution_product, execution_root) = match execution_product {
                                Ok(execution_product) => execution_product,
                                Err(error) => {
                                    generation_authority.publish_failed(
                                        project_workspace_key,
                                        0,
                                        publication.generation_digest,
                                        error,
                                    );
                                    continue;
                                }
                            };
                            let result = generation_authority
                                .ensure_ready_resident_with_execution_publication(
                                    &project_workspace_key,
                                    &publication.project_root,
                                    resident,
                                    &publication.generation_digest,
                                    execution_product.clone(),
                                )
                                .await;
                            if result.is_ok() {
                                let durability = async {
                                    let store = agent_semantic_client_db::runtime_server_workspace::
                                        RuntimeWorkspaceExecutionPublicationStore::open(
                                            &execution_root,
                                        )
                                        .await?;
                                    store.publish(&execution_product).await?;
                                    Ok::<_, String>(())
                                }
                                .await;
                                if let Err(error) = durability {
                                    eprintln!(
                                        "[runtime-workspace-execution-durability] {}",
                                        serde_json::json!({
                                            "schemaId": "agent.semantic-protocols.runtime-workspace-execution-durability-receipt",
                                            "schemaVersion": "1",
                                            "state": "failed",
                                            "workspaceId": publication.workspace_id,
                                            "generationDigest": publication.generation_digest,
                                            "error": error,
                                        })
                                    );
                                }
                            }
                            publish_observer_terminal(
                                &generation_authority,
                                project_workspace_key,
                                &publication,
                                result,
                            );
                        }
                        Err(error) => generation_authority.publish_failed(
                            project_workspace_key,
                            0,
                            publication.generation_digest,
                            error,
                        ),
                    }
                }
                None => generation_authority.clear_all(),
            }
        }
    })?;
    if *server_readiness.borrow()
        != agent_semantic_client_db::runtime_server_control::RuntimeServerState::Healthy
    {
        tokio::select! {
            changed = server_readiness.changed() => {
                changed.map_err(|_| "Runtime Server readiness channel closed before healthy publication".to_owned())?;
            }
            result = &mut server_done_receiver => {
                return Err(format!(
                    "Runtime Server exited before healthy publication: {}",
                    result
                        .map_err(|_| "Runtime Server task dropped its terminal receipt".to_owned())?
                        .err()
                        .unwrap_or_else(|| "unexpected clean exit".to_owned())
                ));
            }
        }
    }
    if *server_readiness.borrow()
        != agent_semantic_client_db::runtime_server_control::RuntimeServerState::Healthy
    {
        return Err(format!(
            "Runtime Server published non-healthy readiness before activation: {:?}",
            *server_readiness.borrow()
        ));
    }
    let running_artifact_digest =
        agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest::parse(
            &std::env::var("ASP_RUNTIME_BINARY_CONTENT_DIGEST").map_err(|_| {
                "Runtime daemon requires ASP_RUNTIME_BINARY_CONTENT_DIGEST activation authority"
                    .to_owned()
            })?,
        )?;
    let pending_startup_activation = agent_semantic_artifacts::runtime_artifact_activation::
        read_runtime_artifact_activation_event(state_home)
        .await?;
    let startup_requires_actor_commit = pending_startup_activation.is_some();
    let startup_activation = match pending_startup_activation {
        Some(event) => event,
        None => agent_semantic_artifacts::runtime_artifact_activation::
            read_applied_runtime_artifact_activation_event(state_home)
            .await?
            .ok_or_else(|| {
                "Runtime daemon requires a pending or applied content-bound publication before owner admission"
                    .to_owned()
            })?,
    };
    if startup_activation.artifact_digest != running_artifact_digest {
        return Err(
            "Runtime daemon pending activation and launcher artifact identities differ".to_owned(),
        );
    }
    let activation_state_home = state_home.to_path_buf();
    let activation_running_artifact_digest = running_artifact_digest.clone();
    let activation_shutdown = monitor_shutdown.clone();
    let (successor_sender, mut successor_receiver) = tokio::sync::mpsc::channel(1);
    let activation_actor =
        runtime_asp_client::artifact_activation::mount_runtime_daemon_artifact_activation(
            activation_state_home.clone(),
            move |event| {
                let state_home = activation_state_home.clone();
                let running_artifact_digest = activation_running_artifact_digest.clone();
                let activation_shutdown = activation_shutdown.clone();
                let successor_sender = successor_sender.clone();
                async move {
                    if running_artifact_digest == event.artifact_digest {
                        agent_semantic_artifacts::runtime_artifact_activation::
                            commit_runtime_artifact_activation(
                                &state_home,
                                &event,
                                event.previous_artifact_digest.as_ref(),
                            )
                            .await?;
                        let transaction = agent_semantic_client_db::runtime_server_lifecycle::
                            observe_resident_transaction(&state_home)
                            .await?;
                        eprintln!(
                            "[runtime-server-resident-transaction] {}",
                            serde_json::to_string(&transaction).map_err(|error| format!(
                                "encode Runtime resident transaction receipt: {error}"
                            ))?
                        );
                        return Ok(runtime_asp_client::artifact_activation::
                            RuntimeArtifactActivationDisposition::Committed);
                    }
                    successor_sender
                        .send(event)
                        .await
                        .map_err(|_| "Runtime successor observation receiver closed".to_owned())?;
                    activation_shutdown.shutdown();
                    Ok(runtime_asp_client::artifact_activation::
                        RuntimeArtifactActivationDisposition::SuccessorRequired)
                }
            },
        )
        .await?;
    if startup_requires_actor_commit {
        let mut activation_receipts = activation_actor.receipts();
        if activation_receipts.borrow().is_none() {
            activation_receipts.changed().await.map_err(|_| {
                "Runtime activation actor closed before startup transaction terminalized".to_owned()
            })?;
        }
        let activation_receipt =
            activation_receipts
                .borrow_and_update()
                .clone()
                .ok_or_else(|| {
                    "Runtime activation actor did not publish startup transaction authority"
                        .to_owned()
                })?;
        if activation_receipt.state != "ready" {
            return Err(format!(
                "Runtime startup activation transaction failed: artifactDigest={} reason={}",
                activation_receipt.artifact_digest,
                activation_receipt
                    .reason
                    .unwrap_or_else(|| "unknown".to_owned())
            ));
        }
    } else {
        let transaction =
            agent_semantic_client_db::runtime_server_lifecycle::observe_resident_transaction(
                state_home,
            )
            .await?;
        eprintln!(
            "[runtime-server-resident-transaction] {}",
            serde_json::to_string(&transaction)
                .map_err(|error| format!("encode Runtime resident transaction receipt: {error}"))?
        );
    }
    activation_ready_sender.send_replace(true);
    // The identity monitor observes the durable applied activation. Starting it
    // before the startup transaction commits lets Tokio's immediate first
    // interval tick see the previous applied generation and incorrectly drain
    // the candidate that is still becoming active.
    let monitor_state_home = state_home.to_path_buf();
    let mut identity_monitor = spawn_runtime_identity_monitor(
        monitor_state_home.clone(),
        owner_epoch,
        startup_activation.activation_generation,
        startup_activation.publication_nonce.clone(),
        running_artifact_digest.clone(),
        endpoint.artifact_mode.as_str(),
    );
    let (identity_change_sender, mut identity_change_receiver) = tokio::sync::mpsc::channel(1);
    let monitor = task_scope.spawn("runtime-identity-monitor", async move {
        if let Some(identity_change) = identity_monitor.next_event().await {
            if identity_change_sender.send(identity_change).await.is_ok() {
                monitor_shutdown.shutdown();
            }
        }
        identity_monitor.shutdown().await;
    })?;
    let winner = tokio::select! {
        result = &mut server_done_receiver => ("server", result),
        result = &mut client_grpc_done_receiver => ("client-grpc", result),
        result = &mut provider_stream_done_receiver => ("provider-stream", result),
    };
    activation_actor.shutdown().await?;
    if winner.0 != "server" {
        service_failure_shutdown.shutdown();
    }
    let _ = client_grpc_shutdown.send(true);
    let _ = provider_stream_shutdown.send(true);
    let grpc_result = client_grpc_task
        .join()
        .await
        .map_err(|error| format!("ASP Client Protocol gRPC task failed: {error}"))?;
    let provider_stream_result = provider_stream_task
        .join()
        .await
        .map_err(|error| format!("ASP ProviderSession stream task failed: {error}"))?;
    let server_result = server_task
        .join()
        .await
        .map_err(|error| format!("Runtime Server task failed: {error}"))?;
    let _ = generation_task.join().await;
    let query_generation_shutdown_error = query_generation_authority.shutdown().await.err();
    let identity_change = identity_change_receiver.try_recv().ok();
    let successor_required = successor_receiver.try_recv().ok();
    let identity_handoff_requested = identity_change.is_some() || successor_required.is_some();
    let server_result = server_result.map(|_| ());
    let service_result = match (winner.0, grpc_result, provider_stream_result, server_result) {
        ("client-grpc", Ok(()), Ok(()), Ok(())) => {
            Err("ASP Client Protocol gRPC service stopped before Runtime Server".to_owned())
        }
        ("client-grpc", Err(error), _, _) => Err(error),
        ("provider-stream", Ok(()), Ok(()), Ok(())) => {
            Err("ASP ProviderSession stream stopped before Runtime Server".to_owned())
        }
        ("provider-stream", _, Err(error), _) => Err(error),
        (_, Err(error), _, _) => Err(error),
        (_, _, Err(error), _) => Err(error),
        (_, _, _, result) => result,
    };
    let server_result =
        normalize_server_shutdown_for_identity_handoff(identity_handoff_requested, service_result);
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
                candidate_digest: None,
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
    if let Some(error) = query_generation_shutdown_error {
        shutdown_errors.push(format!("queryGenerationBuilder={error}"));
    }
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
    for socket in [
        &graph_socket,
        &telemetry_socket_path,
        &telemetry_query_socket_path,
    ] {
        if let Err(error) = remove_stale_socket(socket).await {
            shutdown_errors.push(format!("transportSocketCleanup={error}"));
        }
    }
    match tokio::fs::remove_dir(transport_layout.root()).await {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => shutdown_errors.push(format!(
            "transportDirectoryCleanup path={} error={error}",
            transport_layout.root().display()
        )),
    }
    let mut identity_handoff = identity_handoff_requested
        && !agent_semantic_client_db::runtime_server_lifecycle::operator_stopped(&state_home)
            .await?;
    if identity_handoff {
        if let Err(error) = RuntimeIdentityHandoffCoordinator::new(&state_home, &endpoint)
            .drain()
            .await
        {
            shutdown_errors.push(format!("ownerHandoffDrain={error}"));
            identity_handoff = false;
        }
    }
    if identity_handoff {
        eprintln!(
            "[runtime-server-identity-handoff] {}",
            serde_json::json!({
                "schemaId": "agent.semantic-protocols.runtime-server-identity-handoff",
                "schemaVersion": "1",
                "state": "owner-drained",
                "ownerEpoch": owner_epoch,
                "processId": std::process::id(),
            })
        );
    }
    drop(election);
    // Endpoint cleanup is part of the elected owner's terminal transaction.
    // It must not short-circuit terminal publication: the supervisor needs one
    // typed receipt with the real owner epoch on both success and failure.
    if let Err(error) = RuntimeIdentityHandoffCoordinator::new(&state_home, &endpoint)
        .cleanup()
        .await
    {
        shutdown_errors.push(format!("endpointCleanup={error}"));
    }
    let result = if shutdown_errors.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "Runtime Server shutdown did not close every owned service: {}",
            shutdown_errors.join("; ")
        ))
    };
    agent_semantic_client_db::runtime_server_lifecycle::publish_with_errors(
        &state_home,
        owner_epoch,
        result.is_ok(),
        shutdown_errors.clone(),
    )
    .await?;
    result
}

fn normalize_server_shutdown_for_identity_handoff(
    identity_handoff_requested: bool,
    server_result: Result<(), String>,
) -> Result<(), String> {
    if identity_handoff_requested {
        Ok(())
    } else {
        server_result
    }
}

#[cfg(test)]
#[path = "../../tests/unit/server/runtime_server_identity_monitor_shutdown.rs"]
mod identity_handoff_shutdown_tests;
