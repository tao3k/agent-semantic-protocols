// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Ordered restoration of admitted workspace Search generations at daemon startup.

async fn await_query_generation(
    authority: &agent_semantic_runtime_server::RuntimeQueryGenerationAuthority,
    entry: &agent_semantic_client_db::runtime_server_admission_catalog::RuntimeWorkspaceAdmissionCatalogEntry,
    expected_digest: &str,
) -> Result<(), String> {
    let key = agent_semantic_runtime_server::query_generation::RuntimeProjectWorkspaceKey::new(
        agent_semantic_client_protocol::ClientProjectId::new(entry.project_id.clone())?,
        agent_semantic_client_protocol::ClientWorkspaceIdentity::new(
            entry.workspace_identity.clone(),
        )?,
    );
    let mut states = authority.subscribe();
    let generation = loop {
        let state = { states.borrow_and_update().get(&key).cloned() };
        match state {
            Some(agent_semantic_runtime_server::RuntimeQueryGenerationState::Ready(generation))
                if generation.generation_digest() == expected_digest =>
            {
                break generation;
            }
            Some(agent_semantic_runtime_server::RuntimeQueryGenerationState::Ready(_)) | None => {
                states.changed().await.map_err(|_| {
                    "Runtime query generation observer closed during startup recovery".to_owned()
                })?;
            }
            Some(agent_semantic_runtime_server::RuntimeQueryGenerationState::Failed {
                expected_generation_digest,
                reason,
            }) if expected_generation_digest.as_ref() == expected_digest => {
                return Err(format!("query generation publication failed: {reason}"));
            }
            Some(agent_semantic_runtime_server::RuntimeQueryGenerationState::Failed { .. }) => {
                states.changed().await.map_err(|_| {
                    "Runtime query generation observer closed during startup recovery".to_owned()
                })?;
            }
        }
    };
    generation.await_lexical_attachment().await
}

fn committed_digest(
    receipt: agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationAdmissionReceipt,
) -> Result<String, String> {
    receipt
        .commit
        .map(|commit| commit.generation_digest)
        .ok_or_else(|| "startup generation Ready receipt has no commit".to_owned())
}

pub(super) async fn recover_startup_workspace_generation(
    admission: &std::sync::Arc<
        agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationAdmission,
    >,
    authority: &agent_semantic_runtime_server::RuntimeQueryGenerationAuthority,
    entry: &agent_semantic_client_db::runtime_server_admission_catalog::RuntimeWorkspaceAdmissionCatalogEntry,
    provider_targets: &[(String, String)],
) -> Result<String, String> {
    let mut last_digest = None;
    for (language_id, provider_id) in provider_targets {
        let digest = committed_digest(
            admission
                .ensure_runtime_generation_ready_for_provider(
                    entry.workspace_identity.clone(),
                    entry.project_root.clone(),
                    Some(
                        agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationProviderTarget {
                            language_id: language_id.clone(),
                            provider_id: Some(provider_id.clone()),
                        },
                    ),
                )
                .await?,
        )?;
        await_query_generation(authority, entry, &digest).await?;
        last_digest = Some(digest);
    }
    if let Some(digest) = last_digest {
        return Ok(digest);
    }
    let digest = committed_digest(
        admission
            .ensure_runtime_generation_ready(
                entry.workspace_identity.clone(),
                entry.project_root.clone(),
            )
            .await?,
    )?;
    await_query_generation(authority, entry, &digest).await?;
    Ok(digest)
}

pub(super) async fn warm_startup_workspace_providers(
    runtime: &agent_semantic_client_db::runtime_search_service::RuntimeSearchServiceHandle,
    entry: &agent_semantic_client_db::runtime_server_admission_catalog::RuntimeWorkspaceAdmissionCatalogEntry,
    provider_targets: &[(String, String)],
) -> Result<(), String> {
    for (language_id, _) in provider_targets {
        runtime
            .provider_runtime(entry.project_root.clone(), language_id.clone())
            .await?;
        runtime
            .provider_runtime_await_ready(
                entry.project_root.clone(),
                language_id.clone(),
                agent_semantic_client_db::runtime_generation_cancellation::GenerationCancellation::new(),
            )
            .await?;
    }
    Ok(())
}
