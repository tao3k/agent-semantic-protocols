use std::path::Path;

use agent_semantic_client_db::runtime_server_workspace::{
    WorkspaceDerivedProjectionSnapshot, WorkspaceOwnerSnapshot, WorkspaceRuntimeSelectorRead,
    WorkspaceSelectorSnapshot,
};

pub(super) fn run_resident_exact_query(
    language_id: &str,
    provider_args: &[String],
    project_root: &Path,
    started: tokio::time::Instant,
) -> Result<(), String> {
    let exact = super::provider_exact_args::parse_exact_query_args(provider_args)?;
    super::runtime_server::block_on_agent_facing_runtime_server_client(
        started,
        "query",
        "resident-exact-generation-open",
        project_root,
        async move {
            let read = resident_exact_projection(language_id, project_root, &exact).await?;
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
    language_id: &str,
    project_root: &Path,
    exact: &super::provider_exact_args::ExactQueryArgs,
) -> Result<WorkspaceRuntimeSelectorRead, String> {
    let owner_path = exact
        .structural_selector
        .split_once("://")
        .and_then(|(_, target)| target.split_once('#'))
        .map(|(owner_path, _)| owner_path)
        .ok_or_else(|| "exact structural selector omitted owner path".to_owned())?;
    let session =
        super::runtime_server::runtime_server_workspace_session_for_admission_async(project_root)
            .await?;
    session
        .ensure_runtime_owner(language_id, owner_path)
        .await?
        .validate()?;
    let client =
        super::runtime_server::runtime_server_workspace_exact_projection_client_async(project_root)
            .await?;
    client.read_runtime_selector(&exact.projection, &exact.structural_selector)
}

pub(super) async fn build_resident_owner_projection(
    workspace_identity: &str,
    language_id: &str,
    project_root: &Path,
    owner_path: &str,
    owner: WorkspaceOwnerSnapshot,
) -> Result<WorkspaceOwnerSnapshot, String> {
    let provider = super::global_provider_catalog::runtime_projection_provider(language_id)?;
    let command_binding = agent_semantic_hook::registered_provider_projection_command_binding_v1(
        language_id,
        provider.provider_id.as_str(),
    )?
    .ok_or_else(|| {
        format!(
            "ProviderRegistry is missing language projection transport: languageId={} providerId={}",
            language_id, provider.provider_id
        )
    })?;
    let invocation = provider.argv_prefix.clone();
    let projection_request = agent_semantic_provider_transport::ProviderProjectionBatchRequest {
        language_id: language_id.to_owned(),
        provider_id: provider.provider_id.as_str().to_owned(),
        workspace_identity: workspace_identity.to_owned(),
        generation_root_digest: owner.content_digest.clone(),
        parser_identity_digest: provider.exact_parser_identity_digest.clone(),
        query_pack_digest: provider.exact_query_pack_identity_digest.clone(),
        base_generation_root_digest: None,
        owners: vec![agent_semantic_provider_transport::ProviderProjectionOwner {
            owner_path: owner_path.to_owned(),
            source_leaf_digest: owner.content_digest.clone(),
            source_bytes: owner.bytes.clone(),
        }],
    };
    let projection_response = agent_semantic_provider_transport::run_provider_projection_batch(
        &invocation,
        command_binding,
        project_root,
        &projection_request,
    )
    .await
    .map_err(|error| format!("provider projection batch failed: {error}"))?;
    let mut projected_owners = projection_response.owners.into_iter();
    let projected_owner = projected_owners
        .next()
        .ok_or_else(|| "provider projection batch omitted requested owner".to_owned())?;
    if projected_owners.next().is_some() {
        return Err("provider projection batch returned more than one requested owner".to_owned());
    }
    let selectors = projected_owner
        .items
        .into_iter()
        .map(|projection| {
            let derived_projections = projection
                .projections
                .into_iter()
                .map(|derived| {
                    serde_json::to_vec(&derived.payload)
                        .map(|bytes| WorkspaceDerivedProjectionSnapshot {
                            projection_kind: derived.projection_kind,
                            bytes,
                        })
                        .map_err(|error| {
                            format!("encode provider derived projection payload: {error}")
                        })
                })
                .collect::<Result<Vec<_>, String>>()?;
            Ok(WorkspaceSelectorSnapshot {
                selector: projection.selector,
                byte_start: projection.source_byte_start,
                byte_end: projection.source_byte_end,
                derived_projections,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(WorkspaceOwnerSnapshot {
        owner_path: owner.owner_path,
        content_digest: owner.content_digest,
        bytes: owner.bytes,
        selectors,
    })
}

fn registered_provider_id(language_id: &str) -> Result<String, String> {
    agent_semantic_hook::schema_registry_provider_manifests()
        .iter()
        .find(|manifest| manifest.language_id().as_str() == language_id)
        .map(|manifest| manifest.provider_id().as_str().to_owned())
        .ok_or_else(|| format!("no registered provider manifest for language {language_id}"))
}
