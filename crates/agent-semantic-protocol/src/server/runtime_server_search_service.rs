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
                            match tokio::select! {
                                ready = client.wait_ready() => Some(ready),
                                _ = response.closed() => None,
                            } {
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
    while tasks.join_next().await.is_some() {}
}
