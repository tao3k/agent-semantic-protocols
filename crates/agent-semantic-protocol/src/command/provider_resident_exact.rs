use std::path::Path;
use std::time::Instant;

pub(super) fn run_resident_exact_query(
    language_id: &str,
    provider_args: &[String],
    project_root: &Path,
    _started: Instant,
) -> Result<(), String> {
    let exact = super::provider_exact_args::parse_exact_query_args(provider_args)?;
    super::runtime_server::block_on_runtime_server_client(async move {
        if exact.projection != "source" {
            return Err(
                "callable-skeleton projection is not present in the resident generation".to_owned(),
            );
        }
        let client =
            super::runtime_server::runtime_server_workspace_generation_client_async(project_root)
                .await?;
        match crate::resident_exact_projection::resolve(&client, &exact.structural_selector)? {
            crate::resident_exact_projection::ResidentExactProjection::Hit(projection) => {
                crate::exact_projection_diagnostic_io::write_stdout(
                    projection.bytes(),
                    "resident exact projection",
                )
                .await
            }
            crate::resident_exact_projection::ResidentExactProjection::Miss(miss) => {
                let provider =
                    super::global_provider_catalog::global_provider_for_language(language_id)?;
                let format = if exact.json {
                    crate::exact_projection_diagnostic::ProviderExactResolutionFormat::Json
                } else {
                    crate::exact_projection_diagnostic::ProviderExactResolutionFormat::Human
                };
                let resolution = crate::exact_projection_diagnostic::resolution_from_facts(
                    crate::exact_projection_diagnostic::ProviderExactResolutionFacts {
                        language_id: language_id.to_owned(),
                        provider_id: provider.provider_id.clone(),
                        owner_path: miss.owner_path.clone(),
                        structural_selector: miss.structural_selector.clone(),
                        resolution_state: miss.state.to_owned(),
                        reason_kind: miss.reason_kind.to_owned(),
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
                    provider.provider_id.as_str(),
                    format,
                    miss.owner_path.as_str(),
                    miss.structural_selector.as_str(),
                    None,
                )
                .await
            }
        }
    })?
}
