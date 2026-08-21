//! Owns assembly and coordinated shutdown of the long-lived Runtime Server.

use agent_semantic_client_db::runtime_server::RuntimeServer;
use agent_semantic_client_db::{
    WorkspaceDbRegistry, acquire_runtime_server_election,
    runtime_server_endpoint_path,
};
use std::path::PathBuf;

use agent_semantic_runtime::runtime_identity_monitor::{MonitorAction, RuntimeIdentityMonitor};

use super::{
    cleanup_endpoint, daemon_identity, remove_stale_socket,
    runtime_server_telemetry_query_socket_path, runtime_server_telemetry_socket_path,
    state_home,
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
    // Repair receipt publication is the first candidate lifecycle action so
    // bootstrap failures remain diagnosable even when no candidate survives.
    let _candidate_repair = agent_semantic_client_db::runtime_server_candidate_reconciliation::repair_canonical_owner_receipt(state_home).await?;
    let _candidate_outcome = agent_semantic_client_db::runtime_server_candidate_reconciliation::reconcile_candidates(state_home).await?;
    // Provider installation owns the mutable catalog publication path. The
    // daemon only consumes the current immutable v1 snapshot, so startup can
    // publish its endpoint without provider reconciliation I/O.
    let candidate_path = agent_semantic_client_db::runtime_server_candidate_reconciliation::prepare_candidate_path(state_home).await?;
    if let Some(candidate_path) = candidate_path.as_ref() {
        let candidate_bytes = tokio::fs::read(candidate_path).await.map_err(|e| format!("read candidate receipt: {e}"))?;
        let candidate: serde_json::Value = serde_json::from_slice(&candidate_bytes).map_err(|e| format!("decode candidate receipt: {e}"))?;
        let desired = candidate.get("desiredIdentity").and_then(|v| v.as_str()).unwrap_or_default();
        let desired_kind = candidate.get("desiredIdentityKind").and_then(|v| v.as_str()).unwrap_or("content");
        let desired_algorithm = candidate.get("desiredIdentityAlgorithm").and_then(|v| v.as_str()).unwrap_or("blake3-256");
        let desired_identity = match desired_kind {
            "developer-source-generation" => agent_semantic_runtime::runtime_artifact_catalog::RuntimeBinaryIdentity::DeveloperSourceGeneration { value: desired.to_owned(), algorithm: desired_algorithm.to_owned() },
            "content" => agent_semantic_runtime::runtime_artifact_catalog::RuntimeBinaryIdentity::Content { value: desired.to_owned(), algorithm: desired_algorithm.to_owned() },
            _ => return Err(format!("candidate receipt has unsupported identity kind: {desired_kind}")),
        };
        let write_candidate_state = |state: &str| { let state = state.to_owned(); async {
            let mut next = candidate.clone();
            if let Some(object) = next.as_object_mut() {
                object.insert("state".to_owned(), serde_json::Value::String(state));
                object.insert("updatedAtMillis".to_owned(), serde_json::json!(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|duration| duration.as_millis()).unwrap_or_default()));
            }
            tokio::fs::write(candidate_path, serde_json::to_vec(&next).unwrap_or_default()).await
        }};
        let write_candidate_failure = |reason: &str, error: &str| { let reason = reason.to_owned(); let error = error.to_owned(); async {
            let mut next = candidate.clone();
            if let Some(object) = next.as_object_mut() {
                object.insert("state".to_owned(), serde_json::Value::String("failed".to_owned()));
                object.insert("reasonKind".to_owned(), serde_json::Value::String(reason));
                object.insert("error".to_owned(), serde_json::Value::String(error));
                object.insert("updatedAtMillis".to_owned(), serde_json::json!(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|duration| duration.as_millis()).unwrap_or_default()));
            }
            tokio::fs::write(candidate_path, serde_json::to_vec(&next).unwrap_or_default()).await
        }};
        if let Ok(Some(endpoint)) = agent_semantic_client_db::read_runtime_server_endpoint(state_home) {
            if endpoint.monitor_capability && endpoint.runtime_binary_identity == desired_identity {
                let _ = tokio::fs::remove_file(candidate_path).await;
                return Ok(());
            }
            if endpoint.monitor_capability {
                write_candidate_state("drain-requested").await.map_err(|e| format!("candidate receipt heartbeat: {e}"))?;
                let drain = tokio::time::timeout(
                    agent_semantic_client_db::runtime_server_runtime::RUNTIME_SERVER_CONNECTION_IO_BUDGET,
                    crate::server::runtime_server_supervisor::request_runtime_server_drain(state_home),
                ).await;
                if drain.is_err() {
                    let updated = tokio::fs::read(state_home.join("runtime/server/monitor-state.json"))
                        .await.ok()
                        .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
                        .and_then(|value| value.get("updatedAtMillis").and_then(|value| value.as_u64()));
                    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
                        .map(|duration| duration.as_millis() as u64).unwrap_or_default();
                    if updated.is_some_and(|timestamp| now.saturating_sub(timestamp) <= 1_000) {
                        write_candidate_state("failed-prepromotion").await
                            .map_err(|e| format!("candidate receipt heartbeat: {e}"))?;
                        return Err("monitored Runtime Server drain timed out".to_owned());
                    }
                    write_candidate_state("terminating-previous-unresponsive").await
                        .map_err(|e| format!("candidate receipt heartbeat: {e}"))?;
                    if let Err(error) = crate::server::runtime_server_supervisor::escalate_drained_owner(state_home, endpoint.owner_epoch).await {
                        write_candidate_failure("owner-receipt-mismatch", &error).await
                            .map_err(|e| format!("candidate receipt heartbeat: {e}"))?;
                        return Err(error);
                    }
                } else if let Ok(Err(error)) = drain {
                    write_candidate_failure("drain-timeout", &error).await
                        .map_err(|e| format!("candidate receipt heartbeat: {e}"))?;
                    return Err(error);
                }
                write_candidate_state("drain-confirmed").await.map_err(|e| format!("candidate receipt heartbeat: {e}"))?;
                if tokio::time::timeout(std::time::Duration::from_secs(5), agent_semantic_client_db::runtime_server_lifecycle::await_owner_exit(state_home, endpoint.owner_epoch)).await.is_err() {
                    write_candidate_state("terminating").await.map_err(|e| format!("candidate receipt heartbeat: {e}"))?;
                    if let Err(error) = crate::server::runtime_server_supervisor::escalate_drained_owner(state_home, endpoint.owner_epoch).await {
                        write_candidate_failure("owner-receipt-mismatch", &error).await
                            .map_err(|e| format!("candidate receipt heartbeat: {e}"))?;
                        return Err(error);
                    }
                }
                let _ = tokio::fs::remove_file(agent_semantic_client_db::runtime_server_endpoint_path(state_home)?).await;
                write_candidate_state("promoting").await.map_err(|e| format!("candidate receipt heartbeat: {e}"))?;
            }
        } else if let Ok(bytes) = tokio::fs::read(agent_semantic_client_db::runtime_server_endpoint_path(state_home)?).await {
            write_candidate_state("drain-requested").await.map_err(|e| format!("candidate receipt heartbeat: {e}"))?;
            let envelope: serde_json::Value = serde_json::from_slice(&bytes)
                .map_err(|e| format!("previous endpoint envelope mismatch: {e}"))?;
            let socket = envelope.get("socketPath").and_then(|v| v.as_str()).ok_or_else(|| "previous endpoint missing socketPath".to_owned())?;
            let token = envelope.get("bindingToken").and_then(|v| v.as_str()).ok_or_else(|| "previous endpoint missing bindingToken".to_owned())?;
            let epoch = envelope.get("ownerEpoch").and_then(|v| v.as_u64()).ok_or_else(|| "previous endpoint missing ownerEpoch".to_owned())?;
            let transport = envelope.get("transportContractDigest").and_then(|v| v.as_str()).ok_or_else(|| "previous endpoint missing transportContractDigest".to_owned())?;
            let digest = envelope.get("runtimeArtifactDigest").and_then(|v| v.as_str()).ok_or_else(|| "previous endpoint missing runtimeArtifactDigest".to_owned())?;
            let drain_result = tokio::time::timeout(
                agent_semantic_client_db::runtime_server_runtime::RUNTIME_SERVER_CONNECTION_IO_BUDGET,
                agent_semantic_client_db::runtime_server_control::drain_previous_generation(
                    std::path::Path::new(socket), token, epoch, transport, digest,
                    format!("candidate-{}", std::process::id()),
                ),
            )
            .await;
            match drain_result {
                Err(_) => {
                    // This is the isolated previous-generation migration path.
                    // A current monitored endpoint never reaches this branch.
                    // Escalation remains capability-bound by the canonical
                    // owner receipt and executable identity verifier.
                    let old_identity = digest;
                    let state_home_matches = candidate
                        .get("stateHome")
                        .and_then(|value| value.as_str())
                        .is_some_and(|value| value == state_home.to_string_lossy());
                    let endpoint_owned = tokio::fs::metadata(
                        agent_semantic_client_db::runtime_server_endpoint_path(state_home)?,
                    )
                    .await
                    .map(|metadata| metadata.is_file())
                    .unwrap_or(false);
                    if !state_home_matches || !endpoint_owned || old_identity == desired {
                        write_candidate_state("failed-prepromotion").await
                            .map_err(|e| format!("candidate receipt heartbeat: {e}"))?;
                        return Err("previous-generation-drain-timeout".to_owned());
                    }
                    write_candidate_state("terminating-previous-unresponsive").await
                        .map_err(|e| format!("candidate receipt heartbeat: {e}"))?;
                    if let Err(error) = crate::server::runtime_server_supervisor::escalate_drained_owner(state_home, epoch).await {
                        write_candidate_state("failed-prepromotion").await
                            .map_err(|e| format!("candidate receipt heartbeat: {e}"))?;
                        return Err(error);
                    }
                    let termination = tokio::time::timeout(
                        std::time::Duration::from_secs(5),
                        agent_semantic_client_db::runtime_server_lifecycle::await_owner_exit(state_home, epoch),
                    ).await;
                    if termination.is_err() {
                        write_candidate_state("terminating-force").await
                            .map_err(|e| format!("candidate receipt heartbeat: {e}"))?;
                        if let Err(error) = crate::server::runtime_server_supervisor::force_kill_previous_owner(state_home, epoch).await {
                            write_candidate_failure("sigkill-send-failed", &error).await
                                .map_err(|e| format!("candidate receipt heartbeat: {e}"))?;
                            return Err(error);
                        }
                        if tokio::time::timeout(
                            std::time::Duration::from_secs(2),
                            agent_semantic_client_db::runtime_server_lifecycle::await_owner_exit(state_home, epoch),
                        ).await.is_err() {
                            write_candidate_failure("sigkill-timeout", "previous-generation-sigterm-timeout").await
                                .map_err(|e| format!("candidate receipt heartbeat: {e}"))?;
                            return Err("previous-generation-sigterm-timeout".to_owned());
                        }
                    } else if let Ok(Err(error)) = termination {
                        write_candidate_failure("drain-failed", &error).await
                            .map_err(|e| format!("candidate receipt heartbeat: {e}"))?;
                        return Err(error);
                    }
                    let _ = tokio::fs::remove_file(agent_semantic_client_db::runtime_server_endpoint_path(state_home)?).await;
                    write_candidate_state("promoting").await
                        .map_err(|e| format!("candidate receipt heartbeat: {e}"))?;
                }
                Ok(Err(error)) => {
                    write_candidate_state("failed-prepromotion").await
                        .map_err(|e| format!("candidate receipt heartbeat: {e}"))?;
                    return Err(error);
                }
                Ok(Ok(())) => {}
            }
            write_candidate_state("drain-confirmed").await.map_err(|e| format!("candidate receipt heartbeat: {e}"))?;
            agent_semantic_client_db::runtime_server_lifecycle::await_owner_exit(state_home, epoch).await?;
            let _ = tokio::fs::remove_file(agent_semantic_client_db::runtime_server_endpoint_path(state_home)?).await;
            write_candidate_state("promoting").await.map_err(|e| format!("candidate receipt heartbeat: {e}"))?;
        }
    }
    let runtime_provider_catalog =
        crate::command::global_provider_catalog::load_runtime_provider_catalog(&state_home).await?;
    let election = acquire_runtime_server_election(&state_home)
        .await
        .map_err(|error| format!("failed to acquire Runtime Server election: {error}"))?;
    let singleton_socket_guard = match agent_semantic_client_db::runtime_server_singleton::acquire_election(&state_home).await? {
        agent_semantic_client_db::runtime_server_singleton::RuntimeServerSingletonElection::Acquired(guard) => guard,
        agent_semantic_client_db::runtime_server_singleton::RuntimeServerSingletonElection::ResidentExists => return Ok(()),
    };
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
                                provider.language_projection.is_some()
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
    let monitor_owner_epoch = owner_epoch;
    let monitor = tokio::spawn(async move {
        let monitor_shutdown = monitor_shutdown;
        let monitor_receipt = monitor_state_home.join("runtime/server/monitor-state.json");
        let write_monitor = |phase: &str, running: &str, observed: &str| {
            let monitor_receipt = monitor_receipt.clone();
            let phase = phase.to_owned();
            let running = running.to_owned();
            let observed = observed.to_owned();
            async move {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|duration| duration.as_millis())
                    .unwrap_or_default();
                let value = serde_json::json!({
                    "schemaVersion": "1",
                    "phase": phase,
                    "runningIdentity": running,
                    "observedIdentity": observed,
                    "ownerEpoch": monitor_owner_epoch,
                    "processId": std::process::id(),
                    "updatedAtMillis": now,
                    "heartbeat": true,
                });
                let _ = tokio::fs::write(monitor_receipt, value.to_string()).await;
            }
        };
        let mut last = String::new();
        let mut state = RuntimeIdentityMonitor::default();
        let mut tick = tokio::time::interval(std::time::Duration::from_secs(5));
        loop {
            tick.tick().await;
            let Ok(receipt) = agent_semantic_runtime::runtime_artifact_identity::read_runtime_artifact_identity(&monitor_state_home, "asp").await else { continue; };
            let identity = format!("{}:{}:{}", receipt.identity_kind(), receipt.identity_value(), receipt.identity_algorithm());
            write_monitor("watching", &identity, &identity).await;
            if !last.is_empty() && last != identity {
                if state.observe(identity.clone()) == MonitorAction::BeginDrain {
                write_monitor("observed", &last, &identity).await;
                monitor_shutdown.shutdown();
                {
                    write_monitor("draining", &last, &identity).await;
                    let _ = agent_semantic_client_db::runtime_server_lifecycle::await_owner_exit(&monitor_state_home, monitor_owner_epoch).await;
                    let _ = crate::server::runtime_server_supervisor::spawn_detached_runtime_server(&monitor_state_home);
                    let _ = state.drain_completed();
                    write_monitor("successor-started", &identity, &identity).await;
                    break;
                }
                }
            }
            last = identity;
        }
    });
    let server_result = server.serve().await.map(|_| ());
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
    drop(election);
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
