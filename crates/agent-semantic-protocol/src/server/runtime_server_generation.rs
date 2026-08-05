//! Runtime Server generation readiness bridges for exact projection reads.

use std::path::Path;

use super::runtime_server::runtime_server_workspace_session_async;

pub(crate) async fn ensure_runtime_generation_ready_for_projection_async(
    project_root: &Path,
) -> Result<
    agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationReadinessReceipt,
    String,
> {
    runtime_server_workspace_session_async(project_root)
        .await?
        .ensure_runtime_generation_ready()
        .await
}

pub(crate) async fn ensure_runtime_generation_owner_ready_for_projection_async(
    project_root: &Path,
    owner_path: &str,
    admitted_content_digest: &str,
) -> Result<
    agent_semantic_client_db::runtime_server_admission::WorkspaceOwnerGenerationReadinessReceipt,
    String,
> {
    runtime_server_workspace_session_async(project_root)
        .await?
        .ensure_runtime_generation_owner_ready(owner_path, admitted_content_digest)
        .await
}
