use std::path::Path;
use std::time::Instant;

use agent_semantic_client_db::runtime_server_workspace::{
    WorkspaceDerivedProjectionSnapshot, WorkspaceOwnerSnapshot, WorkspaceRuntimeSelectorRead,
    WorkspaceSelectorSnapshot,
};

pub(super) fn run_resident_exact_query(
    language_id: &str,
    provider_args: &[String],
    project_root: &Path,
    started: Instant,
) -> Result<(), String> {
    let exact = super::provider_exact_args::parse_exact_query_args(provider_args)?;
    super::runtime_server::block_on_runtime_server_client(async move {
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
    })?
}

async fn resident_exact_projection(
    _language_id: &str,
    project_root: &Path,
    exact: &super::provider_exact_args::ExactQueryArgs,
) -> Result<WorkspaceRuntimeSelectorRead, String> {
    let client =
        super::runtime_server::runtime_server_workspace_exact_projection_client_async(project_root)
            .await?;
    client.read_runtime_selector(&exact.projection, &exact.structural_selector)
}

pub(super) async fn build_resident_owner_projection(
    language_id: &str,
    project_root: &Path,
    owner_path: &str,
    owner: WorkspaceOwnerSnapshot,
) -> Result<WorkspaceOwnerSnapshot, String> {
    let activation_path = super::provider_activation::provider_activation_path(project_root);
    let runtime = agent_semantic_hook::registered_language_runtime(
        project_root,
        language_id,
        &activation_path,
    )?;
    let provider = runtime
        .providers
        .iter()
        .find(|provider| provider.language_id == language_id)
        .ok_or_else(|| format!("no activated provider for language {language_id}"))?;
    let profiles = agent_semantic_hook::runtime_profiles_for_runtime(project_root, &runtime);
    let resolved = agent_semantic_client_core::state_core::ResolvedState::resolve(project_root)?;
    let provider_workspace =
        agent_semantic_client::source_index::provider_workspace_identity_v1(project_root)?;
    let scope = agent_semantic_client_db::ProviderIncrementalScoped {
        project_root: resolved.workspace.root.to_string_lossy().into_owned(),
        workspace_identity: resolved.workspace.workspace_id.to_string(),
        provider_workspace_identity_digest: provider_workspace.digest,
        language_id: language_id.to_owned(),
        provider_id: provider.provider_id.as_str().to_owned(),
        provider_workspace_root: provider_workspace.root,
    };
    let content_digest = owner
        .content_digest
        .strip_prefix("blake3-256:")
        .ok_or_else(|| "resident owner content digest is not canonical BLAKE3".to_owned())?
        .to_owned();
    let fingerprint = agent_semantic_client_db::ProviderOwnerFingerprint {
        metadata: agent_semantic_client_db::ProviderOwnerMetadata {
            file_identity: format!("resident-owner:{content_digest}"),
            size_bytes: owner.bytes.len() as u64,
            modified_unix_nanos: 0,
            change_time_unix_nanos: 0,
        },
        content_digest,
    };
    let projections = super::provider_owner_native::run_provider_owner_native_async(
        super::provider_owner_native::ProviderOwnerNativeTransportContext {
            language_id,
            provider,
            profiles: &profiles,
            project_root,
        },
        super::provider_owner_native::ProviderOwnerNativeRequest {
            scope: &scope,
            owner_path,
            fingerprint: &fingerprint,
            source_bytes: owner.bytes.as_slice(),
        },
    )
    .await?;
    Ok(WorkspaceOwnerSnapshot {
        owner_path: owner.owner_path,
        content_digest: owner.content_digest,
        bytes: owner.bytes,
        selectors: projections
            .into_iter()
            .map(|projection| {
                Ok(WorkspaceSelectorSnapshot {
                    selector: projection.structural_selector,
                    byte_start: usize::try_from(projection.source_byte_start).map_err(|_| {
                        "provider owner selector start exceeds platform usize".to_owned()
                    })?,
                    byte_end: usize::try_from(projection.source_byte_end).map_err(|_| {
                        "provider owner selector end exceeds platform usize".to_owned()
                    })?,
                    derived_projections: vec![WorkspaceDerivedProjectionSnapshot {
                        projection_kind: "callable-skeleton".to_owned(),
                        bytes: projection.signature.into_bytes(),
                    }],
                })
            })
            .collect::<Result<Vec<_>, String>>()?,
    })
}

fn registered_provider_id(language_id: &str) -> Result<String, String> {
    agent_semantic_hook::schema_registry_provider_manifests()
        .iter()
        .find(|manifest| manifest.language_id().as_str() == language_id)
        .map(|manifest| manifest.provider_id().as_str().to_owned())
        .ok_or_else(|| format!("no registered provider manifest for language {language_id}"))
}
