//! Resident provider request service owned by the Runtime Server.

pub(super) async fn serve_runtime_search_requests(
    state_home: std::path::PathBuf,
    mut runtime_provider_catalog: crate::command::installed_provider_artifacts::RuntimeProviderArtifacts,
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
                    runtime_provider_catalog =
                        crate::command::installed_provider_artifacts::load_runtime_provider_artifacts(&state_home)
                            .await?;
                    let launch = runtime_provider_catalog.runtime_launch(
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
                    let launch = runtime_provider_catalog.runtime_launch(
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
                    let launch = runtime_provider_catalog.runtime_launch(
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
                    let launch = runtime_provider_catalog.runtime_launch(
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
                    let launch = runtime_provider_catalog.runtime_launch(
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
            RuntimeSearchServiceRequest::GraphsEvaluate {
                project_root: _project_root,
                workspace_identity,
                generation_digest,
                generation_token,
                request_id,
                payload,
                mut response,
            } => {
                // The lifecycle multiplexes one bounded stream. A bounded
                // semaphore keeps the JoinSet from becoming a second mailbox:
                // saturation is a typed immediate terminal, not an unbounded
                // task backlog.
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
                    let cancellation = agent_semantic_client_db::runtime_generation_cancellation::GenerationCancellation::new();
                    let mut operation = Box::pin(async {
                        let identity = agent_semantic_runtime_server::asp_python_graphs_transport::GraphGenerationIdentity::new(
                            workspace_identity,
                            generation_digest,
                        )?;
                        validate_graph_generation(&payload, &identity.generation_digest)?;
                        let graph_payload = payload.get("graph").cloned().ok_or_else(|| {
                            "asp-python-graphs graph evaluation requires payload.graph".to_owned()
                        })?;
                        let open_payload = serde_json::json!({
                            "graph": graph_payload,
                            "sourceSnapshot": payload.get("sourceSnapshot").cloned().unwrap_or(serde_json::Value::Null),
                            "workspaceGeneration": payload.get("workspaceGeneration").cloned().unwrap_or(serde_json::Value::Null),
                        });
                        let lease = graph_server
                            .open_generation_with_token(identity, generation_token, open_payload, cancellation.clone())
                            .await?;
                        let evaluate_payload = adapt_graph_evaluate_payload(&payload)?;
                        let result = lease
                            .evaluate_with_request_id(evaluate_payload, request_id, cancellation.clone())
                            .await;
                        let release = lease.release().await;
                        match (result, release) {
                            (Ok(value), Ok(())) => Ok(value),
                            (Err(primary), Ok(())) => Err(primary),
                            (Ok(_), Err(release_error)) => Err(format!(
                                "asp-python-graphs release failed: {release_error}"
                            )),
                            (Err(primary), Err(release_error)) => Err(format!(
                                "{primary}; asp-python-graphs release failed: {release_error}"
                            )),
                        }
                    });
                    tokio::select! {
                        result = &mut operation => { let _ = response.send(result); }
                        _ = response.closed() => {
                            cancellation.cancel();
                            let _ = operation.await;
                        }
                    }
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
                    let launch = runtime_provider_catalog.runtime_launch(
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
                    let (registry, _) = crate::command::installed_provider_artifacts::
                        runtime_source_index_provider_projection(
                            &runtime_provider_catalog,
                            &provider_register,
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
        }
        while tasks.try_join_next().is_some() {}
    }
    for (_, runtime) in runtimes {
        let _ = runtime.authority.shutdown().await;
    }
    let _ = graph_server.shutdown().await;
    while tasks.join_next().await.is_some() {}
}

fn adapt_graph_evaluate_payload(payload: &serde_json::Value) -> Result<serde_json::Value, String> {
    let object = payload
        .as_object()
        .ok_or_else(|| "asp-python-graphs graph request must be an object".to_owned())?;
    let terms = object
        .get("queryTerms")
        .cloned()
        .ok_or_else(|| "asp-python-graphs graph request requires queryTerms".to_owned())?;
    let mut rank_payload = serde_json::Map::new();
    for (source, target) in [
        ("seedIds", "seedIds"),
        ("kindBudgets", "kindBudgets"),
        ("windowMerge", "windowMerge"),
        ("pathBudget", "pathBudget"),
        ("pathMaxHops", "pathMaxHops"),
        ("cache", "cache"),
        ("queryClauses", "queryClauses"),
    ] {
        if let Some(value) = object.get(source) {
            rank_payload.insert(target.to_owned(), value.clone());
        }
    }
    Ok(serde_json::json!({
        "terms": terms,
        "profile": object.get("profile").cloned().unwrap_or_else(|| serde_json::Value::String("owner-query".to_owned())),
        "budget": object.get("budget").cloned().unwrap_or(serde_json::Value::from(8)),
        "rankPayload": rank_payload,
    }))
}

fn validate_graph_generation(payload: &serde_json::Value, generation_digest: &str) -> Result<(), String> {
    let root = payload
        .get("workspaceGeneration")
        .and_then(|generation| generation.get("rootDigest"))
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "asp-python-graphs graph request requires workspaceGeneration.rootDigest".to_owned())?;
    let matches = root == generation_digest
        || generation_digest
            .strip_prefix("blake3-256:")
            .is_some_and(|digest| digest == root);
    if !matches {
        return Err(format!(
            "state=stale-generation reasonKind=graph-generation-root-mismatch expected={generation_digest} observed={root}"
        ));
    }
    Ok(())
}
