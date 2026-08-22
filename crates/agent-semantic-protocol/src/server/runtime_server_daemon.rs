//! Owns assembly and coordinated shutdown of the long-lived Runtime Server.

use agent_semantic_client_db::WorkspaceDbRegistry;
use agent_semantic_client_db::runtime_server::RuntimeServer;
use std::path::PathBuf;

use agent_semantic_runtime::runtime_identity_monitor::spawn_runtime_identity_monitor;

use super::{
    cleanup_endpoint, daemon_identity, remove_stale_socket,
    runtime_server_telemetry_query_socket_path, runtime_server_telemetry_socket_path, state_home,
};

async fn serve_runtime_search_requests(
    runtime_provider_catalog: crate::command::global_provider_catalog::RuntimeProviderCatalog,
    mut requests: tokio::sync::mpsc::Receiver<
        agent_semantic_client_db::runtime_search_service::RuntimeSearchServiceRequest,
    >,
) {
    use agent_semantic_client_db::runtime_search_service::RuntimeSearchServiceRequest;

    struct ResidentProviderRuntime {
        authority: agent_semantic_provider_transport::ProviderRuntimeActorAuthority,
        expected_receipt: agent_semantic_provider_transport::ProviderRuntimeContractReceipt,
    }

    fn runtime_state_receipt(
        runtime: &ResidentProviderRuntime,
    ) -> Result<serde_json::Value, String> {
        let receipt = runtime
            .authority
            .client()
            .current_lifecycle(&runtime.expected_receipt)?;
        serde_json::to_value(receipt)
            .map_err(|error| format!("encode ASP Client Server lifecycle receipt: {error}"))
    }

    let mut runtimes = std::collections::BTreeMap::<String, ResidentProviderRuntime>::new();
    let mut provider_backoff = std::collections::BTreeMap::<String, tokio::time::Instant>::new();
    let mut tasks = tokio::task::JoinSet::new();
    while let Some(request) = requests.recv().await {
        match request {
            RuntimeSearchServiceRequest::ProviderRuntime {
                project_root,
                language_id,
                response,
            } => {
                let result = async {
                    let launch =
                        runtime_provider_catalog.runtime_launch(&project_root, &language_id)?;
                    if let Some(next_retry) = provider_backoff.get(&launch.key) {
                        if *next_retry > tokio::time::Instant::now() {
                            return Err(format!("provider-runtime-backoff: key={} nextRetryAt={:?}", launch.key, next_retry));
                        }
                        provider_backoff.remove(&launch.key);
                    }
                    if let Some(runtime) = runtimes.get(&launch.key) {
                        return runtime_state_receipt(runtime);
                    }
                    let launch_key = launch.key.clone();
                    let authority = match launch.spec {
            crate::command::global_provider_catalog::RuntimeProviderLaunchSpec::HttpServer(
                spec,
            ) => {
                let peer = match agent_semantic_provider_transport::AspClientServerPeer::start(spec).await {
                    Ok(peer) => peer,
                    Err(error) => {
                        provider_backoff.insert(launch_key.clone(), tokio::time::Instant::now() + std::time::Duration::from_secs(5));
                        return Err(format!("provider-runtime-unavailable: {error}"));
                    }
                };
                agent_semantic_provider_transport::spawn_provider_runtime_peer_actor(256, peer)
            }
        };
                    let runtime = ResidentProviderRuntime {
                        authority,
                        expected_receipt: launch.expected_receipt,
                    };
                    let receipt = runtime_state_receipt(&runtime)?;
                    runtimes.insert(launch.key, runtime);
                    Ok(receipt)
                }
                .await;
                let _ = response.send(result);
            }
            RuntimeSearchServiceRequest::ProviderRuntimeReady {
                project_root,
                language_id,
                response,
            } => {
                let result = (|| {
                    let launch =
                        runtime_provider_catalog.runtime_launch(&project_root, &language_id)?;
                    let runtime = runtimes.get(&launch.key).ok_or_else(|| {
                        format!(
                            "asp-client-server-not-ready: state=absent languageId={language_id}"
                        )
                    })?;
                    runtime_state_receipt(runtime)
                })();
                let _ = response.send(result);
            }
            RuntimeSearchServiceRequest::ProviderRuntimeAwaitReady {
                project_root,
                language_id,
                response,
            } => {
                let result = (|| {
                    let launch =
                        runtime_provider_catalog.runtime_launch(&project_root, &language_id)?;
                    let runtime = runtimes.get(&launch.key).ok_or_else(|| {
                        format!(
                            "asp-client-server-not-ready: state=absent languageId={language_id}"
                        )
                    })?;
                    Ok((runtime.authority.client(), runtime.expected_receipt.clone()))
                })();
                tasks.spawn(async move {
                    let mut response = response;
                    let result = match result {
                        Ok((mut client, expected_receipt)) => match tokio::select! {
                            ready = client.wait_ready() => Some(ready),
                            _ = response.closed() => None,
                        } {
                            None => return,
                            Some(ready) => match ready {
                            Ok(receipt) if receipt != expected_receipt => Err(format!(
                                "provider-runtime-contract-drift: languageId={language_id}"
                            )),
                Ok(receipt) => {
                    agent_semantic_provider_transport::
    AspClientServerLifecycleReceipt::from_actor_state(
                            &expected_receipt,
                            agent_semantic_provider_transport::ProviderRuntimeActorState::Ready(
                                receipt,
                            ),
                        )
                        .and_then(|receipt| {
                            serde_json::to_value(receipt).map_err(|error| {
                                format!("encode provider runtime authority receipt: {error}")
                            })
                        })
                            }
                            Err(error) => Err(error),
                            },
                        },
                        Err(error) => Err(error),
                    };
                    let _ = response.send(result);
                });
            }
            RuntimeSearchServiceRequest::ProviderRuntimeRelease {
                project_root,
                language_id,
                response,
            } => {
                let result = (|| {
                    let launch =
                        runtime_provider_catalog.runtime_launch(&project_root, &language_id)?;
                    runtimes.remove(&launch.key).ok_or_else(|| {
                        format!(
                            "asp-client-server-not-ready: state=absent languageId={language_id}"
                        )
                    })
                })();
                tasks.spawn(async move {
                    let result = match result {
                        Ok(runtime) => {
                            let expected_receipt = runtime.expected_receipt;
                            runtime
                                .authority
                                .drain()
                                .await
                                .and_then(|()| {
                                    agent_semantic_provider_transport::
                                    AspClientServerLifecycleReceipt::from_actor_state(
                                        &expected_receipt,
                                        agent_semantic_provider_transport::
                                            ProviderRuntimeActorState::Stopped,
                                    )
                                })
                                .and_then(|receipt| {
                                    serde_json::to_value(receipt).map_err(|error| {
                                        format!("encode provider runtime release receipt: {error}")
                                    })
                                })
                        }
                        Err(error) => Err(error),
                    };
                    let _ = response.send(result);
                });
            }
            RuntimeSearchServiceRequest::ProviderOperation {
                project_root,
                language_id,
                operation,
                payload,
                response,
            } => {
                let result = (|| {
                    let launch =
                        runtime_provider_catalog.runtime_launch(&project_root, &language_id)?;
                    let runtime = runtimes.get(&launch.key).ok_or_else(|| {
                        format!(
                            "asp-client-server-not-ready: state=absent languageId={language_id}"
                        )
                    })?;
                    match runtime.authority.client().current() {
                        agent_semantic_provider_transport::ProviderRuntimeActorState::Ready(
                            receipt,
                        ) if receipt == runtime.expected_receipt => Ok(runtime.authority.client()),
                        agent_semantic_provider_transport::ProviderRuntimeActorState::Ready(_) => {
                            Err(format!(
                                "provider-runtime-contract-drift: languageId={language_id}"
                            ))
                        }
                        state => Err(format!(
                            "asp-client-server-not-ready: languageId={language_id} state={state:?}"
                        )),
                    }
                })();
                tasks.spawn(async move {
                    let result = match result {
                        Ok(runtime) => runtime
                            .request(operation, payload)
                            .await
                            .map(|payload| payload.to_vec()),
                        Err(error) => Err(error),
                    };
                    let _ = response.send(result);
                });
            }
            RuntimeSearchServiceRequest::ProviderOwner {
                workspace_identity,
                project_root,
                language_id,
                owner_path,
                response,
            } => {
                let result = (|| {
                    let launch =
                        runtime_provider_catalog.runtime_launch(&project_root, &language_id)?;
                    let runtime = runtimes.get(&launch.key).ok_or_else(|| {
                        format!(
                            "asp-client-server-not-ready: state=absent languageId={language_id}"
                        )
                    })?;
                    match runtime.authority.client().current() {
                        agent_semantic_provider_transport::ProviderRuntimeActorState::Ready(
                            receipt,
                        ) if receipt == runtime.expected_receipt => {}
                        agent_semantic_provider_transport::ProviderRuntimeActorState::Ready(_) => {
                            return Err(format!(
                                "provider-runtime-contract-drift: languageId={language_id}"
                            ));
                        }
                        agent_semantic_provider_transport::ProviderRuntimeActorState::Starting => {
                            return Err(format!(
                                "asp-client-server-not-ready: state=starting languageId={language_id}"
                            ));
                        }
                        agent_semantic_provider_transport::ProviderRuntimeActorState::Warming => {
                            return Err(format!(
                                "asp-client-server-not-ready: state=warming languageId={language_id}"
                            ));
                        }
                        agent_semantic_provider_transport::ProviderRuntimeActorState::Draining => {
                            return Err(format!(
                                "asp-client-server-not-ready: state=draining languageId={language_id}"
                            ));
                        }
                        agent_semantic_provider_transport::ProviderRuntimeActorState::Failed(
                            reason,
                        ) => {
                            return Err(format!(
                                "asp-client-server-not-ready: state=failed languageId={language_id} reason={reason}"
                            ));
                        }
                        agent_semantic_provider_transport::ProviderRuntimeActorState::Stopped => {
                            return Err(format!(
                                "asp-client-server-not-ready: state=stopped languageId={language_id}"
                            ));
                        }
                    }
                    let (registry, _) = crate::command::global_provider_catalog::
                        runtime_provider_registry_snapshot(
                            &project_root,
                            &runtime_provider_catalog,
                        )?;
                    Ok((runtime.authority.client(), registry))
                })();
                tasks.spawn(async move {
                    let result = match result {
                        Ok((runtime, registry)) => {
                            agent_semantic_client::source_index::
                                prepare_runtime_server_owner_projection_with_resident_runtime_async(
                                    runtime,
                                    project_root,
                                    workspace_identity,
                                    owner_path,
                                    registry,
                                )
                                .await
                        }
                        Err(error) => Err(error),
                    };
                    let _ = response.send(result);
                });
            }
            RuntimeSearchServiceRequest::TreeSitterQuery {
                workspace_identity: _,
                project_root,
                language_id,
                args,
                response,
            } => {
                let runtime = (|| {
                    let launch =
                        runtime_provider_catalog.runtime_launch(&project_root, &language_id)?;
                    let runtime = runtimes.get(&launch.key).ok_or_else(|| {
                        format!(
                            "asp-client-server-not-ready: state=absent languageId={language_id}"
                        )
                    })?;
                    match runtime.authority.client().current() {
                        agent_semantic_provider_transport::ProviderRuntimeActorState::Ready(
                            receipt,
                        ) if receipt == runtime.expected_receipt => Ok(runtime.authority.client()),
                        agent_semantic_provider_transport::ProviderRuntimeActorState::Ready(_) => {
                            Err(format!(
                                "provider-runtime-contract-drift: languageId={language_id}"
                            ))
                        }
                        state => Err(format!(
                            "asp-client-server-not-ready: languageId={language_id} state={state:?}"
                        )),
                    }
                })();
                tasks.spawn(async move {
                    let result = match runtime {
                        Ok(runtime) => {
                            crate::command::run_runtime_server_tree_sitter_query(
                                &language_id,
                                &args,
                                &project_root,
                                runtime,
                            )
                            .await
                        }
                        Err(error) => Err(error),
                    };
                    let _ = response.send(result);
                });
            }
        }
        while tasks.try_join_next().is_some() {}
    }
    for (_, runtime) in runtimes {
        let _ = runtime.authority.shutdown().await;
    }
    while tasks.join_next().await.is_some() {}
}

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
            0,
            false,
            vec![error.clone()],
        )
        .await;
    }
    result
}

async fn run_daemon_at(state_home: &std::path::Path) -> Result<(), String> {
    // Elect before touching the persisted provider snapshot.  The snapshot is
    // an untrusted previous publication: current Schema Register reconciliation
    // must prune stale leaves before any materialized path is resolved.
    let election = agent_semantic_client_db::wait_for_runtime_server_election(&state_home)
        .await
        .map_err(|error| format!("failed to acquire Runtime Server election: {error}"))?;
    crate::command::reconcile_global_provider_catalog_for_runtime(state_home).await?;
    let runtime_provider_catalog =
        crate::command::global_provider_catalog::load_runtime_provider_catalog(&state_home).await?;
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
    let runtime_binary_identity = runtime_artifact_identity.identity();
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
    )
    .await?;
    let socket_path = PathBuf::from(&endpoint.socket_path);
    let data_plane_socket_path = PathBuf::from(&endpoint.data_plane_socket_path);
    let telemetry_socket_path = runtime_server_telemetry_socket_path(&state_home)?;
    let telemetry_query_socket_path = runtime_server_telemetry_query_socket_path(&state_home)?;
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
    let provider_catalog_generation =
        crate::command::global_provider_catalog::read_runtime_provider_catalog_readiness(
            &state_home,
        )?
        .catalog_generation;
    let (runtime_search_service, runtime_search_requests) =
        agent_semantic_client_db::runtime_search_service::runtime_search_service_channel();
    let generation_builder_catalog = provider_catalog_generation.clone();
    let generation_builder_runtime_catalog = runtime_provider_catalog.clone();
    let generation_builder_runtime_search = runtime_search_service.clone();
    let generation_builder: agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationCandidateBuilder =
    std::sync::Arc::new(move |_workspace_identity, project_root, changed_paths, provider_target| {
        let provider_catalog_generation = generation_builder_catalog.clone();
        let runtime_provider_catalog = generation_builder_runtime_catalog.clone();
        let runtime_search_service = generation_builder_runtime_search.clone();
        Box::pin(async move {
            let changed_path_count = changed_paths.len();
            let (registry, current_catalog_generation) =
                crate::command::global_provider_catalog::runtime_provider_registry_snapshot(
                    &project_root,
                    &runtime_provider_catalog,
                )?;
            if current_catalog_generation != provider_catalog_generation {
                return Err(format!(
                    "runtime provider catalog advanced after daemon admission: admitted={} current={}",
                    provider_catalog_generation, current_catalog_generation
                ));
            }
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
        });
    let owner_builder_catalog = provider_catalog_generation.clone();
    let owner_builder_runtime_catalog = runtime_provider_catalog.clone();
    let owner_builder_runtime_search = runtime_search_service.clone();
    let owner_projection_builder: agent_semantic_client_db::runtime_server_admission::WorkspaceOwnerProjectionBuilder =
        std::sync::Arc::new(move |workspace_identity, project_root, owner_path| {
        let provider_catalog_generation = owner_builder_catalog.clone();
        let runtime_provider_catalog = owner_builder_runtime_catalog.clone();
        let runtime_search_service = owner_builder_runtime_search.clone();
            Box::pin(async move {
                let (registry, current_catalog_generation) =
        crate::command::global_provider_catalog::runtime_provider_registry_snapshot(
            &project_root,
            &runtime_provider_catalog,
        )?;
                if current_catalog_generation != provider_catalog_generation {
                    return Err(format!(
                        "runtime provider catalog advanced after daemon admission: admitted={} current={}",
                        provider_catalog_generation, current_catalog_generation
                    ));
                }
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
        runtime_provider_catalog.clone(),
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
    .with_event_sender(diagnostic_events)
    .with_runtime_telemetry_sender(lifecycle_bus.sender.clone())
    .with_workspace_generation_and_owner_builders_and_catalog(
        generation_builder,
        owner_projection_builder,
        admission_catalog,
    )
    .with_runtime_search_service(runtime_search_service);
    // The monitor owns the in-process cancellation capability.  It must never
    // call the server's public IPC endpoint to control the same generation.
    let monitor_shutdown = server.shutdown_handle();
    // Telemetry initializes its Turso store in its own Tokio-owned lane.  It
    // cannot delay the already bound control plane from accepting its first
    // status request: endpoint readiness and observability are independent
    // lifecycle concerns.
    let telemetry_task = tokio::spawn(
        agent_semantic_client_db::runtime_server_opentelemetry::RuntimeServerOpenTelemetry::start_with_telemetry_receiver(
            state_home
                .join("runtime")
                .join("server")
                .join("runtime-server-telemetry.turso"),
            telemetry_socket_path.clone(),
            telemetry_query_socket_path.clone(),
            lifecycle_bus.receiver,
        ),
    );
    let monitor_state_home = state_home.to_path_buf();
    let mut identity_monitor =
        spawn_runtime_identity_monitor(monitor_state_home.clone(), "asp".to_owned(), owner_epoch);
    let (identity_change_sender, mut identity_change_receiver) = tokio::sync::mpsc::channel(1);
    let monitor = tokio::spawn(async move {
        if let Some(identity_change) = identity_monitor.next_event().await {
            if identity_change_sender.send(identity_change).await.is_ok() {
                monitor_shutdown.shutdown();
            }
        }
        identity_monitor.shutdown().await;
    });
    let server_result = server.serve().await.map(|_| ());
    let identity_change = identity_change_receiver.try_recv().ok();
    monitor.abort();
    // Once the accept loop has stopped, all independent resident services are
    // drained concurrently. Serial draining made stop latency additive and
    // allowed one stuck read-only lane to postpone every other task owner.
    let drain_started = tokio::time::Instant::now();
    let opentelemetry = telemetry_task
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

struct RuntimeIdentityHandoffCoordinator<'a> {
    state_home: &'a std::path::Path,
    endpoint: &'a agent_semantic_client_db::runtime_server_control::RuntimeServerEndpoint,
}

#[cfg(test)]
trait RuntimeIdentityHandoff {
    fn retire(
        &mut self,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), String>> + '_>>;
    fn cleanup(
        &mut self,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), String>> + '_>>;
    fn admit(
        &mut self,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), String>> + '_>>;
    fn publish(
        &mut self,
        success: bool,
        error: Option<String>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + '_>>;
}

#[cfg(test)]
async fn execute_runtime_identity_handoff<T: RuntimeIdentityHandoff>(
    mut adapter: T,
) -> Result<(), String> {
    adapter.retire().await?;
    adapter.cleanup().await?;
    match adapter.admit().await {
        Ok(()) => {
            adapter.publish(true, None).await;
            Ok(())
        }
        Err(error) => {
            let typed =
                format!("Runtime Server identity successor failed readiness admission: {error}");
            adapter.publish(false, Some(typed.clone())).await;
            Err(typed)
        }
    }
}

#[cfg(test)]
#[path = "../../tests/unit/server/runtime_identity_handoff.rs"]
mod runtime_identity_handoff_tests;

impl<'a> RuntimeIdentityHandoffCoordinator<'a> {
    fn new(
        state_home: &'a std::path::Path,
        endpoint: &'a agent_semantic_client_db::runtime_server_control::RuntimeServerEndpoint,
    ) -> Self {
        Self {
            state_home,
            endpoint,
        }
    }

    async fn retire(&self) -> Result<(), String> {
        agent_semantic_client_db::runtime_server_supervisor::retire_runtime_server_owner_for_handoff(
            self.state_home,
            self.endpoint,
        )
        .await
    }

    async fn cleanup(&self) -> Result<(), String> {
        cleanup_endpoint(self.state_home, self.endpoint).await
    }

    async fn admit_successor(&self) -> Result<(), String> {
        crate::server::runtime_server_wire_adapter::reconcile_healthy_runtime_server(
            self.state_home,
        )
        .await
        .map(|_| ())
    }
}
