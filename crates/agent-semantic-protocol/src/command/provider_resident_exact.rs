use std::path::Path;

use agent_semantic_client_db::runtime_server_workspace::WorkspaceRuntimeSelectorRead;

pub(super) async fn run_resident_exact_query(
    language_id: &str,
    provider_args: &[String],
    project_root: &Path,
    started: tokio::time::Instant,
) -> Result<(), String> {
    let exact = super::provider_exact_args::parse_exact_query_args(provider_args)?;
    crate::server::runtime_server::await_agent_facing_runtime_server_client(
        started,
        "query",
        "resident-exact-generation-open",
        project_root,
        async move {
            let read = resident_exact_projection(project_root, language_id, &exact).await?;
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
    .await
}

async fn resident_exact_projection(
    project_root: &Path,
    language_id: &str,
    exact: &super::provider_exact_args::ExactQueryArgs,
) -> Result<WorkspaceRuntimeSelectorRead, String> {
    let language_id = agent_semantic_client_core::LanguageId::try_from(language_id)
        .map_err(|error| format!("decode language id: {error}"))?;
    crate::server::runtime_server::runtime_server_workspace_exact_projection_async(
        project_root,
        language_id,
        &exact.projection,
        &exact.structural_selector,
    )
    .await
}

fn registered_provider_id(language_id: &str) -> Result<String, String> {
    agent_semantic_hook::schema_registry_provider_manifests()
        .iter()
        .find(|manifest| manifest.language_id().as_str() == language_id)
        .map(|manifest| manifest.provider_id().as_str().to_owned())
        .ok_or_else(|| format!("no registered provider manifest for language {language_id}"))
}
