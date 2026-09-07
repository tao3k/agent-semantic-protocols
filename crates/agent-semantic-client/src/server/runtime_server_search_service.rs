// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Resident provider request service owned by the Runtime Server.

pub(super) async fn serve_runtime_search_requests(
    state_home: std::path::PathBuf,
    mut runtime_active_provider_projection: crate::command::active_provider_projection::RuntimeActiveProviderProjection,
    provider_register: std::sync::Arc<
        agent_semantic_client_db::runtime_provider_register::RuntimeProviderRegister,
    >,
    mut requests: tokio_stream::wrappers::ReceiverStream<
        agent_semantic_client_db::runtime_search_service::RuntimeSearchServiceRequest,
    >,
    graph_server: agent_semantic_runtime_server::asp_python_graphs_transport::AspPythonGraphsServer,
) {
    use agent_semantic_client_db::runtime_search_service::RuntimeSearchServiceRequest;
    use tokio_stream::StreamExt;

    struct ResidentProviderRuntime {
        authority: agent_semantic_provider_transport::ProviderRuntimeActorAuthority,
        expected_receipt: agent_semantic_provider_transport::ProviderRuntimeContractReceipt,
    }

    fn runtime_state_receipt(
        runtime: &ResidentProviderRuntime,
    ) -> Result<agent_semantic_provider_transport::AspClientServerLifecycleReceipt, String> {
        let receipt = runtime
            .authority
            .client()
            .current_lifecycle(&runtime.expected_receipt)?;
        Ok(receipt)
    }

    let mut runtimes = std::collections::BTreeMap::<String, ResidentProviderRuntime>::new();
    let mut tasks = tokio::task::JoinSet::new();
    const GRAPH_REQUEST_CAPACITY: usize = 32;
    let graph_slots = std::sync::Arc::new(tokio::sync::Semaphore::new(GRAPH_REQUEST_CAPACITY));
    while let Some(request) = requests.next().await {
        match request {
            RuntimeSearchServiceRequest::ProviderRuntime {
                project_root,
                language_id,
                response,
            } => {
                let result = async {
                    runtime_active_provider_projection =
                        crate::command::active_provider_projection::load_runtime_active_provider_projection(&state_home)
                            .await?;
                    let launch = runtime_active_provider_projection.runtime_launch(
                        &project_root,
                        &language_id,
                        &provider_register,
                    )?;
                    let stale_keys = runtimes
                        .iter()
                        .filter(|(key, runtime)| {
                            key.as_str() != launch.key
                                && runtime.expected_receipt.language_id == language_id
                                && runtime.expected_receipt.provider_id
                                    == launch.expected_receipt.provider_id
                        })
                        .map(|(key, _)| key.clone())
                        .collect::<Vec<_>>();
                    for stale_key in stale_keys {
                        if let Some(stale_runtime) = runtimes.remove(&stale_key) {
                            stale_runtime.authority.shutdown().await?;
                        }
                    }
                    if let Some(runtime) = runtimes.get(&launch.key) {
                        return runtime_state_receipt(runtime);
                    }
                    let authority = {
                        let peer = match agent_semantic_provider_transport::AspClientServerPeer::start(
                            launch.spec,
                        )
                        .await
                        {
                            Ok(peer) => peer,
                            Err(error) => {
                                return Err(format!(
                                    "state=provider-runtime-terminal reasonKind=asp-client-server-start-failed languageId={language_id} error={error}"
                                ));
                            }
                        };
                        agent_semantic_provider_transport::spawn_provider_runtime_peer_actor(
                            256, peer,
                        )
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
                    let launch = runtime_active_provider_projection.runtime_launch(
                        &project_root,
                        &language_id,
                        &provider_register,
                    )?;
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
                cancellation,
                response,
            } => {
                let result = (|| {
                    let launch = runtime_active_provider_projection.runtime_launch(
                        &project_root,
                        &language_id,
                        &provider_register,
                    )?;
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
                        Ok((mut client, expected_receipt)) => {
                match agent_semantic_runtime_server::provider_readiness::await_provider_runtime_ready_terminal(
                    client.wait_ready(),
                    cancellation,
                    response.closed(),
                ).await {
                                None => return,
                                Some(ready) => match ready {
                                    Ok(receipt) if receipt != expected_receipt => Err(format!(
                                        "provider-runtime-contract-drift: languageId={language_id}"
                                    )),
                                    Ok(receipt) => agent_semantic_provider_transport::
                                        AspClientServerLifecycleReceipt::from_actor_state(
                                            &expected_receipt,
                                            agent_semantic_provider_transport::ProviderRuntimeActorState::Ready(receipt),
                                        ),
                                    Err(error) => Err(error),
                                },
                            }
                        }
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
                    let launch = runtime_active_provider_projection.runtime_launch(
                        &project_root,
                        &language_id,
                        &provider_register,
                    )?;
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
                                            agent_semantic_provider_transport::ProviderRuntimeActorState::Stopped,
                                        )
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
                cancellation,
                response,
            } => {
                let result = (|| {
                    let launch = runtime_active_provider_projection.runtime_launch(
                        &project_root,
                        &language_id,
                        &provider_register,
                    )?;
                    if !launch
                        .expected_receipt
                        .operations
                        .iter()
                        .any(|registered| registered.operation == operation)
                    {
                        return Err(format!(
                            "state=route-missing reasonKind=operation-not-in-runtime-contract languageId={language_id} operation={operation}"
                        ));
                    }
                    let runtime = runtimes.get(&launch.key).ok_or_else(|| {
                        format!(
                            "asp-client-server-not-ready: state=absent languageId={language_id}"
                        )
                    })?;
                    match runtime.authority.client().current() {
                        agent_semantic_provider_transport::ProviderRuntimeActorState::Ready(
                            receipt,
                        ) if receipt == runtime.expected_receipt => {
                            if receipt.provider_id != launch.expected_receipt.provider_id {
                                return Err(format!(
                                    "state=route-target-drift reasonKind=runtime-provider-identity-mismatch operation={operation} expectedProviderId={} actualProviderId={}",
                                    launch.expected_receipt.provider_id, receipt.provider_id
                                ));
                            }
                            Ok(runtime.authority.client())
                        }
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
                    let mut response = response;
                    let result = match result {
                        Ok(runtime) => match runtime.begin_request(operation, payload).await {
                Ok(request) => match tokio::select! {
                    result = request => Some(result.map(|payload| payload.to_vec())),
                    _ = cancellation.cancelled() => Some(Err(
                        "runtime-generation-cancelled: provider operation cancelled".to_owned()
                    )),
                    _ = response.closed() => None,
                            } {
                                Some(result) => result,
                                None => return,
                            },
                            Err(error) => Err(error),
                        },
                        Err(error) => Err(error),
                    };
                    let _ = response.send(result);
                });
            }
            RuntimeSearchServiceRequest::GraphsTimeline {
                project_root: _project_root,
                request_id,
                payload,
                cancellation,
                mut response,
            } => {
                let permit = match graph_slots.clone().try_acquire_owned() {
                    Ok(permit) => permit,
                    Err(_) => {
                        let _ = response.send(Err(
                            "state=busy reasonKind=asp-python-graphs-capacity-exhausted".to_owned(),
                        ));
                        continue;
                    }
                };
                let graph_server = graph_server.clone();
                tasks.spawn(async move {
                    let _permit = permit;
                    let operation =
                        graph_server.timeline(request_id, payload, cancellation.clone());
                    tokio::pin!(operation);
                    tokio::select! {
                        result = &mut operation => { let _ = response.send(result); }
                        _ = response.closed() => { cancellation.cancel(); }
                    }
                });
            }
            RuntimeSearchServiceRequest::GenerationGraph {
                request_id,
                payload,
                cancellation,
                mut response,
            } => {
                let permit = match graph_slots.clone().try_acquire_owned() {
                    Ok(permit) => permit,
                    Err(_) => {
                        let _ = response.send(Err(
                            "state=busy reasonKind=asp-python-graphs-capacity-exhausted".to_owned(),
                        ));
                        continue;
                    }
                };
                let graph_server = graph_server.clone();
                tasks.spawn(async move {
                    let _permit = permit;
                    let operation =
                        graph_server.generation_graph(request_id, payload, cancellation.clone());
                    tokio::pin!(operation);
                    tokio::select! {
                        result = &mut operation => { let _ = response.send(result); }
                        _ = response.closed() => { cancellation.cancel(); }
                    }
                });
            }
            RuntimeSearchServiceRequest::EvaluateResidentGraph {
                workspace_identity,
                generation_digest,
                request_id,
                payload,
                cancellation,
                mut response,
            } => {
                let permit = match graph_slots.clone().try_acquire_owned() {
                    Ok(permit) => permit,
                    Err(_) => {
                        let _ = response.send(Err(
                            "state=busy reasonKind=asp-python-graphs-capacity-exhausted".to_owned(),
                        ));
                        continue;
                    }
                };
                let graph_server = graph_server.clone();
                tasks.spawn(async move {
                    let _permit = permit;
                    let operation = graph_server.evaluate_resident(
                        workspace_identity,
                        generation_digest,
                        request_id,
                        payload,
                        cancellation.clone(),
                    );
                    tokio::pin!(operation);
                    tokio::select! {
                        result = &mut operation => { let _ = response.send(result); }
                        _ = response.closed() => { cancellation.cancel(); }
                    }
                });
            }
            RuntimeSearchServiceRequest::ReleaseGenerationGraph {
                workspace_identity,
                generation_digest,
                request_id,
                response,
            } => {
                let graph_server = graph_server.clone();
                tasks.spawn(async move {
                    let result = graph_server
                        .release_generation(workspace_identity, generation_digest, request_id)
                        .await;
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
                    let launch = runtime_active_provider_projection.runtime_launch(
                        &project_root,
                        &language_id,
                        &provider_register,
                    )?;
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
                    let (registry, _) = crate::command::active_provider_projection::
                        runtime_source_index_provider_projection(
                            &runtime_active_provider_projection,
                            &provider_register,
                            &std::collections::BTreeSet::from([language_id.clone()]),
                        )?;
                    Ok((runtime.authority.client(), registry))
                })();
                tasks.spawn(async move {
                    let result = match result {
                        Ok((runtime, registry)) => {
                            agent_semantic_client_db::server_source_index::
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
        }
        while tasks.try_join_next().is_some() {}
    }
    for (_, runtime) in runtimes {
        let _ = runtime.authority.shutdown().await;
    }
    let _ = graph_server.shutdown().await;
    while tasks.join_next().await.is_some() {}
}
