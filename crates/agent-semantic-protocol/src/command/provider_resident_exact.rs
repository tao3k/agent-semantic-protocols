use std::path::Path;

use agent_semantic_client_db::runtime_server_workspace::WorkspaceRuntimeSelectorRead;

pub(super) fn run_resident_exact_query(
    language_id: &str,
    provider_args: &[String],
    project_root: &Path,
    started: tokio::time::Instant,
) -> Result<(), String> {
    let exact = super::provider_exact_args::parse_exact_query_args(provider_args)?;
    crate::server::runtime_server::block_on_agent_facing_runtime_server_client(
        started,
        "query",
        "resident-exact-generation-open",
        project_root,
        async move {
            let read = resident_exact_projection(project_root, &exact).await?;
            crate::exact_projection_trace::stage("mmap-generation-open", started);
            match crate::resident_exact_projection::resolve(read, &exact.structural_selector)? {
                crate::resident_exact_projection::ResidentExactProjection::Hit(projection) => {
                    crate::exact_projection_trace::stage("mmap-resident-hit", started);
                    crate::exact_projection_diagnostic_io::write_stdout(
                        projection.as_slice(),
                        "mmap resident exact projection",
                    )
                    .await
                }
                crate::resident_exact_projection::ResidentExactProjection::Miss(miss) => {
                    crate::exact_projection_trace::stage("mmap-resident-miss", started);
                    let provider_id = registered_provider_id(language_id)?;
                    let format = if exact.json {
                        crate::exact_projection_diagnostic::ProviderExactResolutionFormat::Json
                    } else {
                        crate::exact_projection_diagnostic::ProviderExactResolutionFormat::Human
                    };
                    let resolution = crate::exact_projection_diagnostic::resolution_from_facts(
                        crate::exact_projection_diagnostic::ProviderExactResolutionFacts {
                            language_id: language_id.to_owned(),
                            provider_id: provider_id.clone(),
                            owner_path: miss.owner_path.clone(),
                            structural_selector: miss.structural_selector.clone(),
                            resolution_state: miss.state.to_owned(),
                            reason_kind: miss.reason_kind.to_owned(),
                            active_generation_digest: miss.active_generation_digest,
                            root_digest: miss.root_digest,
                            item_kind: miss.item_kind,
                            item_name: miss.item_name,
                            candidates: miss.candidates,
                            actual_kinds: miss.actual_kinds,
                            workspace: project_root.display().to_string(),
                        },
                    );
                    crate::exact_projection_diagnostic_io::emit_resolution(
                        &resolution,
                        language_id,
                        provider_id.as_str(),
                        format,
                        miss.owner_path.as_str(),
                        miss.structural_selector.as_str(),
                        None,
                    )
                    .await
                }
            }
        },
    )
}

async fn resident_exact_projection(
    project_root: &Path,
    exact: &super::provider_exact_args::ExactQueryArgs,
) -> Result<WorkspaceRuntimeSelectorRead, String> {
    let owner_path = exact_owner_path(&exact.structural_selector)?;
    let mut client =
        crate::server::runtime_server::runtime_server_workspace_exact_projection_client_async(
            project_root,
        )
        .await?;
    crate::exact_projection_trace::generation("owner-freshness-before", client.generation_digest());
    if let Some(admitted_content_digest) = client.owner_content_digest(owner_path)? {
        let readiness = crate::server::runtime_server_generation::ensure_runtime_generation_owner_ready_for_projection_async(
            project_root,
            owner_path,
            &admitted_content_digest,
        )
        .await?;
        crate::exact_projection_trace::generation(
            "owner-freshness-ready",
            &readiness.commit.generation_digest,
        );
        client =
            crate::server::runtime_server::runtime_server_workspace_exact_projection_client_async(
                project_root,
            )
            .await?;
        crate::exact_projection_trace::generation(
            "owner-freshness-after",
            client.generation_digest(),
        );
        let mapped_owner_content_digest = client.owner_content_digest(owner_path)?;
        if !reopened_generation_covers_ready_owner(
            readiness.owner_content_digest.as_deref(),
            mapped_owner_content_digest.as_deref(),
        ) {
            return Err(format!(
                "resident exact pointer does not cover the daemon-ready owner: ownerPath={} readyOwnerDigest={:?} mappedOwnerDigest={:?} readyGeneration={} mappedGeneration={}",
                owner_path,
                readiness.owner_content_digest,
                mapped_owner_content_digest,
                readiness.commit.generation_digest,
                client.generation_digest(),
            ));
        }
    }
    let mut read = client.read_runtime_selector(&exact.projection, &exact.structural_selector)?;
    if matches!(
        read,
        WorkspaceRuntimeSelectorRead::GenerationMissing
            | WorkspaceRuntimeSelectorRead::OwnerMissing { .. }
    ) {
        crate::server::runtime_server_generation::ensure_runtime_generation_ready_for_projection_async(
            project_root,
        )
        .await?;
        client =
            crate::server::runtime_server::runtime_server_workspace_exact_projection_client_async(
                project_root,
            )
            .await?;
        read = client.read_runtime_selector(&exact.projection, &exact.structural_selector)?;
    }
    Ok(read)
}

fn reopened_generation_covers_ready_owner(
    ready_owner_content_digest: Option<&str>,
    mapped_owner_content_digest: Option<&str>,
) -> bool {
    mapped_owner_content_digest == ready_owner_content_digest
}

fn exact_owner_path(structural_selector: &str) -> Result<&str, String> {
    structural_selector
        .split_once("://")
        .and_then(|(_, selector)| selector.split_once('#'))
        .map(|(owner_path, _)| owner_path)
        .filter(|owner_path| !owner_path.is_empty())
        .ok_or_else(|| "exact structural selector is missing its owner path".to_owned())
}

fn registered_provider_id(language_id: &str) -> Result<String, String> {
    agent_semantic_hook::schema_registry_provider_manifests()
        .iter()
        .find(|manifest| manifest.language_id().as_str() == language_id)
        .map(|manifest| manifest.provider_id().as_str().to_owned())
        .ok_or_else(|| format!("no registered provider manifest for language {language_id}"))
}

#[cfg(test)]
#[path = "../../tests/unit/provider_resident_exact.rs"]
mod tests;
