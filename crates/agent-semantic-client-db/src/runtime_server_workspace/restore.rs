use super::{RuntimeServerWorkspaceRegistry, WorkspaceRecoveryReceipt};

pub async fn restore_active_turso_generation(
    memory_registry: &RuntimeServerWorkspaceRegistry,
    session: &crate::ProviderSearchWorkspaceSession,
    request_id: impl Into<String>,
    workspace_identity: &str,
    project_root: &std::path::Path,
) -> Result<WorkspaceRecoveryReceipt, String> {
    if session.workspace_identity() != workspace_identity {
        return Err(format!(
            "resident Turso session identity mismatch: expected={workspace_identity} actual={}",
            session.workspace_identity()
        ));
    }
    let materialization = session
        .load_active_workspace_generation_materialization(project_root)
        .await?
        .ok_or_else(|| {
            format!(
                "active workspace generation materialization is unavailable: workspaceIdentity={workspace_identity}"
            )
        })?;
    materialization.validate_persisted(workspace_identity)?;
    memory_registry
        .ensure_canonical_generation(request_id, workspace_identity, materialization)
        .await
}
