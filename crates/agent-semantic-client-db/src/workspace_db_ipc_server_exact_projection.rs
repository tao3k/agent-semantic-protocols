use std::path::Path;

use agent_semantic_client_core::LanguageId;

use crate::runtime_search_service::RuntimeSearchServiceHandle;
use crate::workspace_db_ipc::WorkspaceDbIpcResult;

pub(super) async fn tree_sitter_query(
    runtime_search_service: Option<&RuntimeSearchServiceHandle>,
    workspace_identity: &str,
    project_root: String,
    language_id: LanguageId,
    args: Vec<String>,
) -> WorkspaceDbIpcResult {
    let Some(service) = runtime_search_service else {
        return WorkspaceDbIpcResult::Failed {
            code: "runtime-server-search-service-unavailable".to_owned(),
            message: "Runtime search service is not configured".to_owned(),
        };
    };
    match service
        .tree_sitter_query(
            workspace_identity.to_owned(),
            Path::new(&project_root).to_path_buf(),
            language_id.to_string(),
            args,
        )
        .await
    {
        Ok(rendered) => WorkspaceDbIpcResult::TreeSitterQuery { rendered },
        Err(message) => WorkspaceDbIpcResult::Failed {
            code: "runtime-server-tree-sitter-query-failed".to_owned(),
            message,
        },
    }
}

pub(super) async fn provider_owner(
    _runtime_search_service: Option<&RuntimeSearchServiceHandle>,
    memory_registry: &crate::runtime_server_workspace::RuntimeServerWorkspaceRegistry,
    generation_admission: Option<
        &std::sync::Arc<crate::runtime_server_admission::WorkspaceGenerationAdmission>,
    >,
    request_id: String,
    workspace_identity: &str,
    project_root: String,
    language_id: LanguageId,
    owner_path: String,
) -> WorkspaceDbIpcResult {
    let project_root = Path::new(&project_root).to_path_buf();
    if let Err(message) = crate::workspace_db_ipc_server::generation::require_or_submit_terminal_generation_for_read_with_provider(
        memory_registry,
        generation_admission,
        workspace_identity,
        &project_root,
        vec![Path::new(&owner_path).to_path_buf()],
        Some(crate::runtime_server_admission::WorkspaceGenerationProviderTarget {
            language_id: language_id.as_str().to_owned(),
            provider_id: None,
        }),
    )
    .await
    {
        return WorkspaceDbIpcResult::Failed {
            code: "active-workspace-generation-required".to_owned(),
            message,
        };
    }
    match memory_registry
        .read_projection_owner(workspace_identity, &project_root, &owner_path)
        .await
    {
        Ok(
            crate::runtime_server_workspace::WorkspaceRuntimeOwnerRead::Owner { owner, .. }
            | crate::runtime_server_workspace::WorkspaceRuntimeOwnerRead::SparseProviderOwner {
                owner,
                ..
            },
        ) => WorkspaceDbIpcResult::ProviderOwnerProjection { owner },
        Ok(crate::runtime_server_workspace::WorkspaceRuntimeOwnerRead::GenerationMissing) => {
            WorkspaceDbIpcResult::Failed {
                code: "active-workspace-generation-required".to_owned(),
                message: format!(
                    "active workspace generation is required before resident owner read: workspaceIdentity={workspace_identity} languageId={language_id} requestId={request_id}"
                ),
            }
        }
        Ok(crate::runtime_server_workspace::WorkspaceRuntimeOwnerRead::OwnerMissing {
            generation_digest,
            root_digest,
        }) => WorkspaceDbIpcResult::Failed {
            code: "runtime-server-resident-owner-missing".to_owned(),
            message: format!(
                "resident owner is absent from ready generation: ownerPath={owner_path} generationDigest={generation_digest} rootDigest={root_digest}"
            ),
        },
        Err(message) => WorkspaceDbIpcResult::Failed {
            code: "runtime-server-resident-owner-read-failed".to_owned(),
            message,
        },
    }
}

#[cfg(test)]
#[path = "../tests/unit/workspace_db_ipc_server_exact_projection.rs"]
mod tests;
