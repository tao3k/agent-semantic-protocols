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
    runtime_search_service: Option<&RuntimeSearchServiceHandle>,
    memory_registry: &crate::runtime_server_workspace::RuntimeServerWorkspaceRegistry,
    request_id: String,
    workspace_identity: &str,
    project_root: String,
    language_id: LanguageId,
    owner_path: String,
) -> WorkspaceDbIpcResult {
    let Some(service) = runtime_search_service else {
        return WorkspaceDbIpcResult::Failed {
            code: "runtime-server-provider-owner-query-unavailable".to_owned(),
            message: "Runtime Server provider owner query service is unavailable".to_owned(),
        };
    };
    let project_root = Path::new(&project_root).to_path_buf();
    let publication_project_root = project_root.clone();
    let language_id = language_id.to_string();
    let result = async {
        service
            .provider_runtime(project_root.clone(), language_id.clone())
            .await?;
        service
            .provider_runtime_await_ready(project_root.clone(), language_id.clone())
            .await?;
        service
            .provider_owner(
                workspace_identity.to_owned(),
                project_root,
                language_id,
                owner_path,
            )
            .await
    }
    .await;
    match result {
        Ok(owner) => match memory_registry
            .publish_provider_owner(
                request_id,
                workspace_identity.to_owned(),
                &publication_project_root,
                owner.clone(),
            )
            .await
        {
            Ok(_) => WorkspaceDbIpcResult::ProviderOwnerProjection { owner },
            Err(message) => WorkspaceDbIpcResult::Failed {
                code: "runtime-server-provider-owner-publication-failed".to_owned(),
                message,
            },
        },
        Err(message) => WorkspaceDbIpcResult::Failed {
            code: "runtime-server-provider-owner-query-failed".to_owned(),
            message,
        },
    }
}

#[cfg(test)]
#[path = "../tests/unit/workspace_db_ipc_server_exact_projection.rs"]
mod tests;
