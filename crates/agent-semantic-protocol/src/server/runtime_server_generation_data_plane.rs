use std::path::Path;

/// Admits one Runtime generation, then reads its exact projection synchronously.
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
    let resident_generation_pointer = session.runtime_generation_pointer_path().ok_or_else(|| {
        "Runtime exact projection requires an admitted resident generation: reasonKind=runtime-generation-not-ready"
            .to_owned()
    })?;
    let resident_read =
        agent_semantic_client_db::runtime_resident_read::RuntimeResidentReadClient::open(
            &resident_generation_pointer,
            project_root,
        )
        .await?;
    let read_started = std::time::Instant::now();
    let read = resident_read.read_runtime_selector(projection_kind, structural_selector)?;
    let read_elapsed_micros = u64::try_from(read_started.elapsed().as_micros()).unwrap_or(u64::MAX);
    if read_elapsed_micros >= 1_000 {
        return Err(format!(
            "Runtime exact projection exceeded the synchronous mmap budget: elapsedMicros={read_elapsed_micros} budgetExclusiveMicros=1000"
        ));
    }
    let work = resident_read.work_counters();
    if work.database_read_count != 0
        || work.filesystem_read_count != 0
        || work.provider_process_count != 0
        || work.scheduler_task_count != 0
        || work.socket_operation_count != 0
    {
        return Err(format!(
            "Runtime exact projection violated synchronous mmap authority: databaseReads={} filesystemReads={} providerProcesses={} schedulerTasks={} socketOperations={}",
            work.database_read_count,
            work.filesystem_read_count,
            work.provider_process_count,
            work.scheduler_task_count,
            work.socket_operation_count,
        ));
    }
    resident_read.try_record_read_observation(
        "runtime-exact-projection",
        "qualified",
        structural_selector,
        Some(&language_id),
        "exact-selector",
        read_elapsed_micros,
        1_000,
        "within-budget",
    );
    Ok(read)
}
