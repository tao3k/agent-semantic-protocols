use std::path::Path;

pub(crate) async fn ensure(
    project_root: &Path,
    request_id: String,
) -> Result<
    agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationAdmissionReceipt,
    String,
> {
    let state = agent_semantic_runtime::state_core::ResolvedState::resolve(project_root)?;
    let endpoint = agent_semantic_client_db::runtime_server_control::read_runtime_server_endpoint(
        &state.state_home,
    )?
    .ok_or_else(|| "Runtime Server endpoint is unavailable".to_owned())?;
    agent_semantic_client_db::runtime_server_control::ensure_runtime_server_workspace(
        &endpoint,
        project_root,
        request_id.clone(),
    )
    .await
    .map_err(|error| format!("Runtime global ensure-workspace failed: {error}"))?;
    let session =
        agent_semantic_client_db::workspace_db_ipc::connect_runtime_server_workspace_session(
            project_root,
        )
        .await
        .map_err(|error| format!("Runtime workspace owner connection failed: {error}"))?;
    session
        .ensure_runtime_generation_ready(request_id)
        .await
        .map_err(|error| format!("Runtime CompleteGeneration admission failed: {error}"))
}

pub(crate) async fn submit(
    project_root: &Path,
    mutation_id: String,
    changed_paths: Vec<String>,
) -> Result<
    agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationMutationSubmissionReceipt,
    String,
>{
    let session =
        agent_semantic_client_db::workspace_db_ipc::connect_runtime_server_workspace_session(
            project_root,
        )
        .await?;
    session
        .submit_runtime_generation_mutation(mutation_id, changed_paths)
        .await
}
