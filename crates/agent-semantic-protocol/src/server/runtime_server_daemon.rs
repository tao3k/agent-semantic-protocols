//! Owns assembly and coordinated shutdown of the long-lived Runtime Server.

use agent_semantic_client_db::runtime_server::RuntimeServer;
use agent_semantic_client_db::{
    WorkspaceDbRegistry, acquire_runtime_server_election,
    prepare_runtime_server_endpoint_with_workspace_store, runtime_server_endpoint_path,
};
use std::path::PathBuf;

use super::{
    cleanup_endpoint, daemon_identity, remove_stale_socket,
    runtime_server_telemetry_query_socket_path, runtime_server_telemetry_socket_path,
    singleton_socket, state_home,
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
        let receipt =
            agent_semantic_provider_transport::ProviderRuntimeAuthorityReceipt::from_actor_state(
                &runtime.expected_receipt,
                runtime.authority.client().current(),
            )?;
        serde_json::to_value(receipt)
            .map_err(|error| format!("encode provider runtime authority receipt: {error}"))
    }

    let mut runtimes = std::collections::BTreeMap::<String, ResidentProviderRuntime>::new();
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
                    if let Some(runtime) = runtimes.get(&launch.key) {
                        return runtime_state_receipt(runtime);
                    }
                    let authority = match launch.spec {
            crate::command::global_provider_catalog::RuntimeProviderLaunchSpec::Process(spec) => {
                let peer = agent_semantic_provider_transport::ProviderRuntimeProcessPeer::start(
                    spec,
                )
                .await?;
                agent_semantic_provider_transport::spawn_provider_runtime_peer_actor(256, peer)
            }
            crate::command::global_provider_catalog::RuntimeProviderLaunchSpec::HttpServer(
                spec,
            ) => {
                let peer = agent_semantic_provider_transport::ProviderHttpServerPeer::start(spec)
                    .await?;
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
                        format!("provider-runtime-not-ready: state=absent languageId={language_id}")
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
                        format!("provider-runtime-not-ready: state=absent languageId={language_id}")
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
                        ProviderRuntimeAuthorityReceipt::from_actor_state(
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
                        format!("provider-runtime-not-ready: state=absent languageId={language_id}")
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
                            "provider-runtime-not-ready: languageId={language_id} state={state:?}"
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
                        format!("provider-runtime-not-ready: state=absent languageId={language_id}")
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
                                "provider-runtime-not-ready: state=starting languageId={language_id}"
                            ));
                        }
                        agent_semantic_provider_transport::ProviderRuntimeActorState::Warming => {
                            return Err(format!(
                                "provider-runtime-not-ready: state=warming languageId={language_id}"
                            ));
                        }
                        agent_semantic_provider_transport::ProviderRuntimeActorState::Draining => {
                            return Err(format!(
                                "provider-runtime-not-ready: state=draining languageId={language_id}"
                            ));
                        }
                        agent_semantic_provider_transport::ProviderRuntimeActorState::Failed(
                            reason,
                        ) => {
                            return Err(format!(
                                "provider-runtime-not-ready: state=failed languageId={language_id} reason={reason}"
                            ));
                        }
                        agent_semantic_provider_transport::ProviderRuntimeActorState::Stopped => {
                            return Err(format!(
                                "provider-runtime-not-ready: state=stopped languageId={language_id}"
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
                tasks.spawn(async move {
                    let result = crate::command::run_runtime_server_tree_sitter_query(
                        &language_id,
                        &args,
                        &project_root,
                    )
                    .await;
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
    let runtime_provider_catalog =
        crate::command::global_provider_catalog::load_runtime_provider_catalog(&state_home).await?;
    let election = acquire_runtime_server_election(&state_home)
        .await
        .map_err(|error| format!("failed to acquire Runtime Server election: {error}"))?;
    let singleton_socket_guard = match singleton_socket::acquire(&state_home).await? {
        singleton_socket::SingletonSocketElection::Acquired(guard) => guard,
        singleton_socket::SingletonSocketElection::ResidentExists => return Ok(()),
    };
    let (owner_epoch, binding_token) = daemon_identity().await?;
    let startup_started = tokio::time::Instant::now();
    crate::server::runtime_server_startup_receipt::publish(
        &state_home,
        owner_epoch,
        "owner-election",
        "ready",
        startup_started,
        None,
    )
    .await?;
    let workspace_store =
        agent_semantic_client_db::runtime_server_workspace::prepare_runtime_server_workspace_store(
            &state_home.join("runtime").join("server"),
        )
        .await
        .map_err(|error| format!("failed to prepare Runtime Server workspace store: {error}"))?;
    crate::server::runtime_server_startup_receipt::publish(
        &state_home,
        owner_epoch,
        "workspace-store",
        "ready",
        startup_started,
        None,
    )
    .await?;
    let runtime_artifact_path = std::env::current_exe()
        .map_err(|error| format!("failed to resolve running ASP artifact: {error}"))?;
    let runtime_artifact_digest =
        crate::command::protocol_binary::protocol_binary_artifact_path_digest(
            &runtime_artifact_path,
        )
        .ok_or_else(|| {
            format!(
                "runtime-server-artifact-identity-missing: canonical Runtime artifact has no published digest: {}",
                runtime_artifact_path.display()
            )
        })?;
    crate::server::runtime_server_startup_receipt::publish(
        &state_home,
        owner_epoch,
        "runtime-artifact-digest",
        "ready",
        startup_started,
        None,
    )
    .await?;
    let artifact_catalog =
        agent_semantic_runtime::runtime_artifact_catalog::load_runtime_artifact_catalog(
            &state_home,
        )
        .await?;
    super::runtime_server_agent_config::synchronize_for_reconcile(&artifact_catalog, &state_home)
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
    crate::command::reconcile_global_provider_catalog_for_runtime(&state_home)?;
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
    std::sync::Arc::new(move |_workspace_identity, project_root, changed_paths| {
        let provider_catalog_generation = generation_builder_catalog.clone();
        let runtime_provider_catalog = generation_builder_runtime_catalog.clone();
        let runtime_search_service = generation_builder_runtime_search.clone();
        Box::pin(async move {
            let changed_path_count = changed_paths.len();
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
            &runtime_provider_catalog,
        )?;
                if current_catalog_generation != provider_catalog_generation {
                    return Err(format!(
                        "runtime provider catalog advanced after daemon admission: admitted={} current={}",
                        provider_catalog_generation, current_catalog_generation
                    ));
                }
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
                    provider.language_projection.is_some()
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
                .provider_runtime_ready(project_root.clone(), language_id.clone())
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
    let server = RuntimeServer::bind_and_publish_with_artifact_catalog(
        endpoint.clone(),
        std::sync::Arc::new(WorkspaceDbRegistry::default()),
        &runtime_server_endpoint_path(&state_home)?,
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
    .with_runtime_search_service(runtime_search_service)
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
    let server_result = server.serve().await.map(|_| ());
    // Once the accept loop has stopped, all independent resident services are
    // drained concurrently. Serial draining made stop latency additive and
    // allowed one stuck read-only lane to postpone every other task owner.
    let drain_started = tokio::time::Instant::now();
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
    let drain_receipt_result = crate::server::runtime_server_exit_receipt::publish_drain(
        &state_home,
        crate::server::runtime_server_exit_receipt::RuntimeServerDrainReceipt {
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
    cleanup_endpoint(&state_home, &endpoint).await?;
    let singleton_result = singleton_socket_guard.release().await;
    if let Err(error) = singleton_result {
        eprintln!(
            "[runtime-server-lifecycle] singleton release failed after terminal receipt: {error}"
        );
    }
    result
}
