// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Content-bound process-cold exact-owner Query replay.

use std::sync::Arc;

use super::{
    AspClientOperationError, AspClientWorkspaceQueryPlaybookRequest, InitializedWorkspace,
    bind_query_materialization_to_request, materialize_query_playbook_receipt, selector_owner_path,
};

pub(crate) fn durable_projection_is_direct(
    requested_selector: &str,
    read: &agent_semantic_client_db::runtime_server_workspace::WorkspaceRuntimeSelectorRead,
) -> bool {
    matches!(
        read,
        agent_semantic_client_db::runtime_server_workspace::WorkspaceRuntimeSelectorRead::Projection {
            resolved_selector,
            ..
        } if resolved_selector == requested_selector
    )
}

pub(crate) fn durable_exact_execution_matches_current(
    publication: &agent_semantic_content_identity::runtime_workspace_execution_publication::RuntimeWorkspaceExecutionPublication,
    workspace_id: &str,
    current_runtime_bundle_digest: &str,
    exact_generation_digest: &str,
    exact_root_digest: &str,
) -> bool {
    let same_content_digest = |left: &str, right: &str| {
        agent_semantic_search::canonical_blake3_digest(left)
            .ok()
            .zip(agent_semantic_search::canonical_blake3_digest(right).ok())
            .is_some_and(|(left, right)| left == right)
    };
    publication.workspace_identity == workspace_id
        && publication.runtime_bundle_digest.as_str() == current_runtime_bundle_digest
        && publication.generation_digest.as_str() == exact_generation_digest
        && same_content_digest(publication.source_root_digest.as_str(), exact_root_digest)
}

pub(crate) fn process_cold_owner_content_digest(
    owner_path: &std::path::Path,
) -> Result<Option<(String, usize)>, AspClientOperationError> {
    const HASH_BUFFER_BYTES: usize = 64 * 1024;
    use std::io::Read;
    let mut file = match std::fs::File::open(owner_path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(AspClientOperationError::Message(format!(
                "read exact Query owner {}: {error}",
                owner_path.display()
            )));
        }
    };
    let mut hasher = blake3::Hasher::new();
    let mut buffer = [0_u8; HASH_BUFFER_BYTES];
    let mut byte_len = 0usize;
    loop {
        let count = file.read(&mut buffer).map_err(|error| {
            AspClientOperationError::Message(format!(
                "read exact Query owner {}: {error}",
                owner_path.display()
            ))
        })?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
        byte_len = byte_len.saturating_add(count);
    }
    Ok(Some((
        format!("blake3-256:{}", hasher.finalize().to_hex()),
        byte_len,
    )))
}

#[expect(
    clippy::too_many_arguments,
    reason = "the replay gate binds current Runtime, workspace, durable source, and request identities"
)]
pub(super) async fn try_process_cold_exact_owner_replay(
    request_id: &str,
    workspace_id: &str,
    params: &AspClientWorkspaceQueryPlaybookRequest,
    initialized: &InitializedWorkspace,
    workspace_store_root: &std::path::Path,
    current_runtime_bundle_digest: &str,
    resource_supervisor: &agent_semantic_workspace_scheduler::RuntimeServerResourceSupervisor,
    task_scope: &agent_semantic_workspace_scheduler::RuntimeServerTaskScope,
    active_provider_targets: &[(String, String)],
    started: tokio::time::Instant,
) -> Result<Option<agent_semantic_client_protocol::ClientResponsePayload>, AspClientOperationError>
{
    use agent_semantic_client_db::runtime_server_workspace::{
        WorkspaceExactProjectionDataPlaneClient, WorkspaceExactProjectionDataPlaneOpen,
    };

    let pointer_path =
        agent_semantic_client_db::runtime_server_workspace::workspace_generation_pointer_path(
            workspace_store_root,
            workspace_id,
            &initialized.project_root,
        )?;
    let exact = match WorkspaceExactProjectionDataPlaneClient::open_state(&pointer_path).await? {
        WorkspaceExactProjectionDataPlaneOpen::Ready(exact) => exact,
        WorkspaceExactProjectionDataPlaneOpen::Missing
        | WorkspaceExactProjectionDataPlaneOpen::RecoveryRequired { .. } => return Ok(None),
    };
    let Some(execution_root) = pointer_path.parent() else {
        return Ok(None);
    };
    let Some(execution_publication) = agent_semantic_client_db::runtime_server_workspace::
        RuntimeWorkspaceExecutionPublicationStore::read_active_optional(execution_root)
        .await?
    else {
        return Ok(None);
    };
    execution_publication.validate().map_err(|error| {
        AspClientOperationError::Message(format!(
            "validate durable exact-owner execution publication: {error:?}"
        ))
    })?;
    if !durable_exact_execution_matches_current(
        &execution_publication,
        workspace_id,
        current_runtime_bundle_digest,
        &exact.generation_digest(),
        &exact.root_digest(),
    ) {
        return Ok(None);
    }

    let projection =
        agent_semantic_client_db::runtime_server_workspace::ExactProjectionKind::try_from(
            params.projection.as_str(),
        )?;
    let mut owner_metadata = std::collections::BTreeMap::new();
    let mut projection_bytes_upper_bound = 0usize;
    for selector in &params.selectors {
        let Some(owner_path) = selector_owner_path(selector) else {
            return Ok(None);
        };
        if !owner_metadata.contains_key(owner_path) {
            let Some(metadata) = exact.owner_content_metadata(owner_path)? else {
                return Ok(None);
            };
            owner_metadata.insert(owner_path.to_owned(), metadata);
        }
        let Some(projection_byte_len) = exact.direct_projection_byte_len(projection, selector)?
        else {
            return Ok(None);
        };
        projection_bytes_upper_bound =
            projection_bytes_upper_bound.saturating_add(projection_byte_len);
    }

    let template_request_id = request_id.to_owned();
    let params = params.clone();
    let project_root = initialized.project_root.clone();
    let runtime_binding = execution_publication.runtime_execution_binding.clone();
    let execution_publication_digest = execution_publication.publication_digest.to_string();
    let runtime_bundle_digest = execution_publication.runtime_bundle_digest.to_string();
    let source_generation_digest = execution_publication.generation_digest.to_string();
    let source_root_digest = exact.root_digest();
    let project_workspace = initialized.host_workspace.project_workspace().clone();
    let active_provider_targets = active_provider_targets.to_vec();
    let receipt_memory_bytes = projection_bytes_upper_bound
        .saturating_mul(std::mem::size_of::<serde_json::Value>().saturating_add(2))
        .saturating_add(320 * 1024);
    let pipeline_permit = resource_supervisor
        .acquire(
            agent_semantic_workspace_scheduler::RuntimeServerResourceRequest {
                cpu: 1,
                work_bytes: projection_bytes_upper_bound.max(1),
                memory_bytes: receipt_memory_bytes,
            },
        )
        .await
        .map_err(AspClientOperationError::Message)?;
    let pipeline_task = task_scope
        .spawn_blocking("process-cold-query-pipeline", move || {
            let _permit = pipeline_permit;
            let mut current_owner_digests = std::collections::BTreeMap::new();
            let mut projections = std::collections::VecDeque::with_capacity(params.selectors.len());
            for selector in &params.selectors {
                let Some(owner_path) = selector_owner_path(selector) else {
                    return Ok(None);
                };
                if !current_owner_digests.contains_key(owner_path) {
                    let Some((current_digest, current_byte_len)) =
                        process_cold_owner_content_digest(&project_root.join(owner_path))?
                    else {
                        return Ok(None);
                    };
                    let Some(expected) = owner_metadata.get(owner_path) else {
                        return Ok(None);
                    };
                    if current_digest != expected.content_digest
                        || current_byte_len != expected.byte_len
                    {
                        return Ok(None);
                    }
                    current_owner_digests.insert(owner_path.to_owned(), current_digest);
                }
                let read = exact.read_runtime_selector(projection, selector)?;
                if !durable_projection_is_direct(selector, &read) {
                    return Ok(None);
                }
                projections.push_back(read);
            }
            materialize_query_playbook_receipt(
                &template_request_id,
                &params,
                &runtime_binding,
                &execution_publication_digest,
                &runtime_bundle_digest,
                &source_generation_digest,
                &source_root_digest,
                &project_workspace,
                &active_provider_targets,
                None,
                |_projection, _selector| {
                    projections.pop_front().ok_or_else(|| {
                        "durable exact-owner replay ended before the requested selector".to_owned()
                    })
                },
            )
            .map(Some)
        })
        .map_err(AspClientOperationError::Message)?;
    let Some(receipt) = pipeline_task
        .join()
        .await
        .map_err(AspClientOperationError::Message)??
    else {
        return Ok(None);
    };
    bind_query_materialization_to_request(Arc::new(receipt), request_id, "materialized", started)
        .map(Some)
}
