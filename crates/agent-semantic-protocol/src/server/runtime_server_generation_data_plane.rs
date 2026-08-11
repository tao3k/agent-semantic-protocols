//! Opens immutable Runtime Server generation data planes for resident queries.

use std::path::Path;




/// Reads one exact projection through the resident Runtime Server authority.
///
/// The CLI process must never open the generation pointer or mmap segment. The
/// daemon owns the load-once data plane, epoch replacement, telemetry, and
/// generation reconciliation for every workspace.
pub(crate) async fn runtime_server_workspace_exact_projection_async(
    project_root: &Path,
    language_id: agent_semantic_client_core::LanguageId,
    projection_kind: agent_semantic_client_db::runtime_server_workspace::ExactProjectionKind,
    structural_selector: &str,
) -> Result<agent_semantic_client_db::runtime_server_workspace::WorkspaceRuntimeSelectorRead, String>
{
    let session =
        crate::server::runtime_server::runtime_server_workspace_session_async(project_root).await?;
    let read_result = session
        .read_runtime_selector(language_id.clone(), projection_kind, structural_selector)
        .await;
    let read_error = read_result.as_ref().err().cloned();
    let read = read_result.ok();
    if read.as_ref().is_some_and(|read| {
        matches!(
            read,
            agent_semantic_client_db::runtime_server_workspace::WorkspaceRuntimeSelectorRead::ProviderProjection { .. }
                | agent_semantic_client_db::runtime_server_workspace::WorkspaceRuntimeSelectorRead::Projection { .. }
                | agent_semantic_client_db::runtime_server_workspace::WorkspaceRuntimeSelectorRead::RelocationAmbiguous { .. }
        )
    }) {
        return Ok(read.expect("terminal selector read checked above"));
    }

    let owner_path = structural_selector
        .split_once("://")
        .and_then(|(_, selector)| selector.split_once('#'))
        .map(|(owner_path, _)| owner_path)
        .ok_or_else(|| "exact structural selector is missing its owner path".to_owned())?;
    let live_owner = match session
        .project_provider_owner(language_id.clone(), owner_path)
        .await
    {
        Ok(owner) => owner,
        Err(error) => {
            return match read {
                Some(read) => Ok(read),
                None => Err(read_error.unwrap_or(error)),
            };
        }
    };
    let requested_selector = match read.as_ref() {
        Some(agent_semantic_client_db::runtime_server_workspace::WorkspaceRuntimeSelectorRead::ProjectionMissing {
            resolved_selector,
            ..
        }) => resolved_selector.clone(),
        _ => structural_selector.to_owned(),
    };
    let Some(selector) = live_owner
        .selectors
        .iter()
        .find(|selector| selector.selector == requested_selector)
    else {
        return read.ok_or_else(|| {
            format!(
                "provider live owner omitted exact selector: languageId={} ownerPath={} selector={}",
                language_id, owner_path, requested_selector
            )
        });
    };
    let owner_content_digest = live_owner.content_digest.clone();
    let projection_bytes = match projection_kind {
        agent_semantic_client_db::runtime_server_workspace::ExactProjectionKind::Source =>
            live_owner
                .bytes
                .get(selector.byte_start..selector.byte_end)
                .ok_or_else(|| {
                    format!(
                        "provider live owner selector range is invalid: languageId={} ownerPath={} selector={} byteStart={} byteEnd={} ownerBytes={}",
                        language_id,
                        owner_path,
                        requested_selector,
                        selector.byte_start,
                        selector.byte_end,
                        live_owner.bytes.len()
                    )
                })?
                .to_vec(),
        agent_semantic_client_db::runtime_server_workspace::ExactProjectionKind::CallableSkeleton =>
            selector
                .derived_projections
                .iter()
                .find(|projection| projection.projection_kind == projection_kind)
                .map(|projection| projection.bytes.clone())
                .ok_or_else(|| {
                    format!(
                        "provider live owner omitted projection mode: languageId={} ownerPath={} selector={} projection={:?}",
                        language_id, owner_path, requested_selector, projection_kind
                    )
                })?,
    };
    session
        .publish_runtime_selector_overlay(
            agent_semantic_client_db::runtime_server_workspace::WorkspaceRuntimeSelectorOverlay {
                projection_kind,
                structural_selector: requested_selector.clone(),
                owner_path: live_owner.owner_path.clone(),
                owner_content_digest: live_owner.content_digest.clone(),
                byte_start: selector.byte_start,
                byte_end: selector.byte_end,
                projection_bytes: projection_bytes.clone(),
            },
        )
        .await?;
    Ok(
        agent_semantic_client_db::runtime_server_workspace::WorkspaceRuntimeSelectorRead::ProviderProjection {
            owner_content_digest,
            resolved_selector: requested_selector,
            bytes: projection_bytes,
        },
    )
}

#[cfg(test)]
#[path = "../../tests/unit/runtime_server_generation_data_plane.rs"]
mod tests;
