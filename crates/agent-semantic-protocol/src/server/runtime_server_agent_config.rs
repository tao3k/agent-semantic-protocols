//! Runtime Server reconciliation for the canonical agent configuration projection.

use std::path::Path;

pub(super) async fn synchronize_for_reconcile(
    artifact_catalog: &agent_semantic_runtime::runtime_artifact_catalog::RuntimeArtifactCatalog,
    state_home: &Path,
) -> Result<Option<serde_json::Value>, String> {
    let agent_semantic_config::runtime_dev::RuntimeArtifactMode::Dev { .. } =
        artifact_catalog.mode()
    else {
        return Ok(None);
    };
    let state_home = state_home.to_path_buf();
    let receipt =
        agent_semantic_client_db::runtime_server_runtime::RuntimeServerOwnedTask::spawn_blocking(
            "runtime-agent-config-sync",
            move || crate::command::synchronize_embedded_agent_config(&state_home),
        )
        .join()
        .await??;
    Ok(Some(receipt))
}
