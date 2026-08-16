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
    match service
        .provider_owner(
            workspace_identity.to_owned(),
            Path::new(&project_root).to_path_buf(),
            language_id.to_string(),
            owner_path,
        )
        .await
    {
        Ok(owner) => WorkspaceDbIpcResult::ProviderOwnerProjection { owner },
        Err(message) => WorkspaceDbIpcResult::Failed {
            code: "runtime-server-provider-owner-query-failed".to_owned(),
            message,
        },
    }
}
