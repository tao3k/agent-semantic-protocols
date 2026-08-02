use std::path::Path;
use std::time::Instant;

use agent_semantic_client_db::runtime_server_workspace::{
    WorkspaceOwnerSnapshot, WorkspaceRuntimeSelectorOverlay, WorkspaceRuntimeSelectorRead,
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
    language_id: &str,
    project_root: &Path,
    exact: &super::provider_exact_args::ExactQueryArgs,
) -> Result<WorkspaceRuntimeSelectorRead, String> {
    let client =
        super::runtime_server::runtime_server_workspace_exact_projection_client_async(project_root)
            .await?;
    let read = client.read_runtime_selector(&exact.projection, &exact.structural_selector)?;
    if exact.projection != "callable-skeleton"
        || !matches!(&read, WorkspaceRuntimeSelectorRead::OwnerForRepair { .. })
    {
        return Ok(read);
    }
    let owner_path = exact
        .structural_selector
        .split_once("://")
        .and_then(|(_, target)| target.split_once('#'))
        .map(|(owner_path, _)| owner_path)
        .ok_or_else(|| "exact structural selector omitted owner path".to_owned())?;
    let session =
        super::runtime_server::runtime_server_workspace_session_async(project_root).await?;
    session
        .ensure_runtime_owner(language_id, owner_path)
        .await?;
    // The mmap generation is immutable. Once repair is required, read through
    // the resident session so an already-published lazy selector overlay is
    // visible without rebuilding it or mutating the durable generation.
    let refreshed_read = session
        .read_runtime_selector(exact.projection.clone(), exact.structural_selector.clone())
        .await?;
    let WorkspaceRuntimeSelectorRead::OwnerForRepair { owner, .. } = refreshed_read else {
        return Ok(refreshed_read);
    };
    let overlay = build_resident_selector_overlay(
        language_id,
        project_root,
        exact.structural_selector.as_str(),
        owner,
    )
    .await?;
    session.publish_runtime_selector_overlay(overlay).await?;
    session
        .read_runtime_selector(exact.projection.clone(), exact.structural_selector.clone())
        .await
}

async fn build_resident_selector_overlay(
    language_id: &str,
    project_root: &Path,
    structural_selector: &str,
    owner: WorkspaceOwnerSnapshot,
) -> Result<WorkspaceRuntimeSelectorOverlay, String> {
    let selector = owner
        .selectors
        .iter()
        .find(|selector| selector.selector == structural_selector)
        .ok_or_else(|| {
            format!("provider owner projection omitted requested selector: {structural_selector}")
        })?;
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
    let raw_digest = |field: &str, digest: &str| {
        let value = digest.rsplit_once(':').map_or(digest, |(_, value)| value);
        if value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            Ok(value.to_owned())
        } else {
            Err(format!(
                "resident exact {field} is not a canonical digest: {digest}"
            ))
        }
    };
    let generation_identity_digest =
        raw_digest("generation identity", owner.content_digest.as_str())?;
    let parser_identity_digest = raw_digest(
        "parser identity",
        provider.semantic_registry_digest.as_str(),
    )?;
    let query_pack_digest = blake3::hash(
        &serde_json::to_vec(&provider.query_pack_descriptor)
            .map_err(|error| format!("encode provider query-pack identity: {error}"))?,
    )
    .to_hex()
    .to_string();
    let projection_bytes = super::provider_native_exact::run_provider_native_callable_skeleton(
        super::provider_native_exact::ProviderNativeExactContext {
            language_id,
            provider,
            profiles: &profiles,
            project_root,
        },
        super::provider_native_exact::ProviderNativeExactRequest {
            owner_path: owner.owner_path.as_str(),
            structural_selector,
            source_bytes: owner.bytes.as_slice(),
            generation_identity_digest: &generation_identity_digest,
            parser_identity_digest: &parser_identity_digest,
            query_pack_digest: &query_pack_digest,
        },
    )
    .await?;
    Ok(WorkspaceRuntimeSelectorOverlay {
        projection_kind: "callable-skeleton".to_owned(),
        structural_selector: structural_selector.to_owned(),
        owner_path: owner.owner_path,
        owner_content_digest: owner.content_digest,
        byte_start: selector.byte_start,
        byte_end: selector.byte_end,
        projection_bytes,
    })
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
    let selectors = projections
        .into_iter()
        .map(|projection| {
            Ok(WorkspaceSelectorSnapshot {
                selector: projection.structural_selector,
                byte_start: usize::try_from(projection.source_byte_start).map_err(|_| {
                    "provider owner selector start exceeds platform usize".to_owned()
                })?,
                byte_end: usize::try_from(projection.source_byte_end)
                    .map_err(|_| "provider owner selector end exceeds platform usize".to_owned())?,
                derived_projections: Vec::new(),
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
