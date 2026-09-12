// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Owns candidate-generation admission and source-index assembly for the daemon.

use agent_semantic_client_db::runtime_provider_register::RuntimeProviderRegister;
use agent_semantic_client_db::runtime_search_service::RuntimeSearchServiceHandle;
use agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationCandidateBuilder;
use agent_semantic_client_db::runtime_server_admission_catalog::RuntimeWorkspaceAdmissionCatalog;
use agent_semantic_runtime_server::RuntimeSchemaBundleCatalog;
use std::path::Path;
use std::sync::Arc;

pub(super) fn resolve_host_workspace_initialization_binding(
    project_root: &Path,
) -> Result<agent_semantic_content_identity::HostWorkspaceInitializationBinding, String> {
    let project_workspace =
        agent_semantic_topology::ProjectTopologyManifest::load_from_project_root(project_root)
            .map(|manifest| manifest.project_workspace().clone())
            .map_err(|error| error.to_string())?;
    let snapshot =
        agent_semantic_runtime::git::discover_repository_candidate_snapshot(project_root)
            .map_err(|error| error.to_string())?
            .ok_or_else(|| {
                "Host workspace initialization requires an admitted Git worktree identity"
                    .to_owned()
            })?;
    agent_semantic_content_identity::HostWorkspaceInitializationBinding::new(
        project_workspace,
        snapshot.worktree_identity.worktree_id,
    )
    .map_err(|error| error.to_string())
}

pub(super) fn build_workspace_generation_candidate_builder(
    state_home: &Path,
    provider_register: Arc<RuntimeProviderRegister>,
    runtime_search_service: RuntimeSearchServiceHandle,
    schema_bundles: RuntimeSchemaBundleCatalog,
    admission_catalog: RuntimeWorkspaceAdmissionCatalog,
) -> WorkspaceGenerationCandidateBuilder {
    let state_home = state_home.to_path_buf();
    Arc::new(
        move |workspace_id,
              project_root,
              candidate,
              changed_paths,
              provider_target,
              cancellation| {
            let state_home = state_home.clone();
            let provider_register = Arc::clone(&provider_register);
            let runtime_search_service = runtime_search_service.clone();
            let schema_bundles = schema_bundles.clone();
            let admission_catalog = admission_catalog.clone();
            Box::pin(async move {
                if cancellation.is_cancelled() {
                    return Err("generation build cancelled before provider admission".to_owned());
                }
                let snapshot = admission_catalog.snapshot();
                let mut admitted = snapshot.iter().filter(|entry| {
                    entry.workspace_identity == workspace_id && entry.project_root == project_root
                });
                let admission = admitted.next().cloned().ok_or_else(|| {
                    format!(
                        "Runtime provider execution binding lacks admitted ProjectId: workspaceId={workspace_id} projectRoot={}",
                        project_root.display()
                    )
                })?;
                if admitted.next().is_some() {
                    return Err(format!(
                        "Runtime provider execution binding has ambiguous ProjectId: workspaceId={workspace_id} projectRoot={}",
                        project_root.display()
                    ));
                }
                admission.validate()?;
                let changed_path_count = changed_paths.len();
                let inventory = agent_semantic_provider_transport::run_fd_inventory(
                    &project_root,
                    agent_semantic_provider_transport::FdInventoryDeadline::CompleteGeneration,
                )
                .await?;
                let active_projection = crate::command::active_provider_projection::
                    load_runtime_active_provider_projection(&state_home)
                    .await?;
                active_projection.execution_binding().validate()?;
                let required_languages = crate::command::active_provider_projection::
                    provider_languages_for_generation_demand(
                        &provider_register,
                        &inventory.owner_paths,
                        provider_target.as_ref(),
                    )?;
                let (registry, current_catalog_generation) = crate::command::
                    active_provider_projection::runtime_source_index_provider_projection(
                        &active_projection,
                        &provider_register,
                        &required_languages,
                    )?;
                // Validate the exact language subset needed by this workspace,
                // but bind the generation to the enclosing active Runtime
                // bundle's schema identity.  The subset digest and the bundle
                // member digest are intentionally different domains; writing
                // the former into RuntimeProviderExecutionBinding makes every
                // fresh generation fail the execution-publication observer.
                let schema_bundle_digest = runtime_schema_bundle_digest(
                    &schema_bundles,
                    required_languages.iter().map(String::as_str),
                    active_projection.execution_binding(),
                )?;
                validate_provider_target(provider_target.as_ref(), &registry)?;
                let mut build = agent_semantic_client_db::server_source_index::
                    prepare_runtime_server_workspace_generation_with_runtime_service_async(
                        runtime_search_service,
                        admission.project_id.clone(),
                        workspace_id.clone(),
                        project_root,
                        registry,
                        agent_semantic_client_db::server_source_index::SourceIndexRecoveryExecution {
                            runtime_bundle_digest: active_projection.generation().to_owned(),
                            schema_bundle_digest: schema_bundle_digest.clone(),
                            workspace_closure_digest: current_catalog_generation.clone(),
                        },
                        agent_semantic_client_db::server_source_index::SourceIndexCollectionScope::CompleteGeneration,
                        candidate,
                        inventory.owner_paths,
                        cancellation,
                    )
                    .await
                    .map_err(|error| {
                        format!(
                            "canonical source generation failed: changedPathCount={changed_path_count} error={error}"
                        )
                    })?;
                let execution_binding = agent_semantic_artifacts::runtime_provider_execution_binding::
                    RuntimeProviderExecutionBinding::build(
                        admission.project_id,
                        workspace_id,
                        active_projection.generation().to_owned(),
                        schema_bundle_digest,
                        current_catalog_generation,
                        build.materialization.source_snapshot.root_integrity_reference()?,
                        build.materialization.import_digest.clone(),
                    )?;
                build
                    .materialization
                    .bind_runtime_provider_execution(execution_binding)?;
                Ok(build)
            })
        },
    )
}

fn runtime_schema_bundle_digest<'a>(
    schema_bundles: &RuntimeSchemaBundleCatalog,
    required_languages: impl IntoIterator<Item = &'a str>,
    execution_binding: &agent_semantic_artifacts::runtime_artifact_slots::RuntimeArtifactBundleBinding,
) -> Result<String, String> {
    let _required_schema_binding_digest = schema_bundles.binding_digest(required_languages)?;
    Ok(execution_binding.schema_bundle_digest().as_str().to_owned())
}

fn validate_provider_target(
    provider_target: Option<
        &agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationProviderTarget,
    >,
    registry: &agent_semantic_client_core::RuntimeProviderProjection,
) -> Result<(), String> {
    let Some(provider_target) = provider_target else {
        return Ok(());
    };
    let provider_id = provider_target.provider_id.as_deref().ok_or_else(|| {
        format!(
            "query-demand provider target requires resolved providerId: languageId={}",
            provider_target.language_id
        )
    })?;
    if registry.providers.iter().any(|provider| {
        provider.language_id.as_str() == provider_target.language_id
            && provider.provider_id.as_str() == provider_id
    }) {
        return Ok(());
    }
    Err(format!(
        "query-demand provider target is absent from the complete Runtime registry: languageId={} providerId={provider_id}",
        provider_target.language_id
    ))
}

#[cfg(test)]
#[path = "../../tests/unit/server/runtime_server_generation_builder.rs"]
mod tests;
