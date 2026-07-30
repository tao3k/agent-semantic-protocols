use std::path::Path;
use std::time::Instant;

pub(super) fn run_resident_exact_query(
    language_id: &str,
    provider_args: &[String],
    project_root: &Path,
    _started: Instant,
) -> Result<(), String> {
    super::runtime_server::block_on_runtime_server_client(async move {
        let projection = projection_kind(provider_args)?;
        if projection != "source" {
            return Err(
                "callable-skeleton projection is not present in the resident generation".to_owned(),
            );
        }
        let structural_selector = super::provider_selector::provider_owned_structural_selector(
            language_id,
            provider_args,
        )
        .ok_or_else(|| "provider-owned structural query is missing an exact selector".to_owned())?;
        let client =
            super::runtime_server::runtime_server_workspace_generation_client_async(project_root)
                .await?;
        match crate::resident_exact_projection::resolve(&client, structural_selector)? {
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
                let format = if provider_args.iter().any(|argument| argument == "--json") {
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

fn projection_kind(provider_args: &[String]) -> Result<&'static str, String> {
    let mut projection = None;
    for window in provider_args.windows(2) {
        if window[0] == "--projection" {
            projection = Some(window[1].as_str());
        }
    }
    for argument in provider_args {
        if let Some(value) = argument.strip_prefix("--projection=") {
            projection = Some(value);
        }
    }
    match projection {
        Some("source") => Ok("source"),
        Some("callable-skeleton") => Ok("callable-skeleton"),
        Some(value) => Err(format!(
            "unsupported exact query projection `{value}`; expected source or callable-skeleton"
        )),
        None => Ok("source"),
    }
}
