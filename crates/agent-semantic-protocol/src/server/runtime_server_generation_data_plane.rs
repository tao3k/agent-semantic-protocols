use std::path::Path;

/// Requires one admitted Runtime generation, then reads its exact projection synchronously.
///
/// Runtime lifecycle and generation authority remain Tokio-owned. Once the
/// generation pointer is admitted, the warm read is a synchronous mmap lookup
/// in the current admitted task: it never enters a service channel, waits for a
/// provider, or uses a timeout as a recovery mechanism.
#[tracing::instrument(
    name = "runtime_server.exact_projection",
    skip(project_root),
    fields(
        language_id = %language_id,
        projection_kind = ?projection_kind,
        projection_scope = "production",
        structural_selector = structural_selector
    )
)]
pub(crate) async fn runtime_server_workspace_exact_projection_async(
    project_root: &Path,
    language_id: agent_semantic_client_core::LanguageId,
    projection_kind: agent_semantic_client_db::runtime_server_workspace::ExactProjectionKind,
    structural_selector: &str,
) -> Result<agent_semantic_client_db::runtime_server_workspace::WorkspaceRuntimeSelectorRead, String>
{
    let session =
        crate::server::runtime_server::runtime_server_workspace_session_for_admission_async(
            project_root,
        )
        .await?;
    // The exact-read operation is the single authority boundary.  Its Server
    // dispatch either leases the resident Ready generation or submits the
    // Runtime-owned cold admission before any mmap read.  A protocol-side
    // generation preflight would reject Missing workspaces before that
    // admission path can run and would introduce a TOCTOU check.
    let read = session
        .read_runtime_exact_projection(
            language_id,
            projection_kind,
            agent_semantic_client_db::runtime_server_workspace::RuntimeProjectionScope::Production,
            structural_selector,
        )
        .await?;
    require_runtime_exact_projection_budget(read.evidence.elapsed_micros)?;
    Ok(read.value)
}

fn require_runtime_exact_projection_budget(read_elapsed_micros: u64) -> Result<(), String> {
    if read_elapsed_micros >= 1_000 {
        return Err(format!(
            "Runtime exact projection exceeded the synchronous mmap budget: elapsedMicros={read_elapsed_micros} budgetExclusiveMicros=1000"
        ));
    }
    Ok(())
}

#[cfg(test)]
#[path = "../../tests/unit/runtime_server_generation_data_plane.rs"]
mod tests;
