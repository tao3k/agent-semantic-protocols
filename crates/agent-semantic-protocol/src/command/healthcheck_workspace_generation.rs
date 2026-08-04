#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct WorkspaceGenerationHealthCheck {
    pub(super) status: String,
    pub(super) workspace_identity: Option<String>,
    pub(super) generation_digest: Option<String>,
    pub(super) reconciled: Option<bool>,
    pub(super) durability_state: Option<
        agent_semantic_client_db::runtime_server_workspace::WorkspaceGenerationDurabilityState,
    >,
    pub(super) durability_failure: Option<String>,
    pub(super) elapsed_micros: u64,
    pub(super) error: Option<String>,
}

pub(super) fn read_cache_control_status(
    should_read: bool,
    workspace_identity: String,
    workspace_root: &std::path::Path,
    project_root: &std::path::Path,
) -> Result<
    Option<Result<agent_semantic_client_db::workspace_db_ipc::WorkspaceDbIpcResult, String>>,
    String,
> {
    if !should_read {
        return Ok(None);
    }
    let result = super::super::runtime_server::block_on_runtime_server_client(async {
        let session = super::super::runtime_server::runtime_server_workspace_session_for_resolved_admission_async(
            workspace_identity,
            workspace_root,
        )
        .await?;
        session
            .cache_control(
                agent_semantic_client_db::workspace_db_ipc::RuntimeCacheControlRequest::Status {
                    project_root: project_root.to_string_lossy().into_owned(),
                },
            )
            .await
    })?;
    Ok(Some(result.map(|receipt| {
        agent_semantic_client_db::workspace_db_ipc::WorkspaceDbIpcResult::CacheControl { receipt }
    })))
}
