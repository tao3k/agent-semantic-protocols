use std::path::Path;

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

pub(crate) async fn submit_pending_wall_failure(
    value: serde_json::Value,
    project_root: &Path,
) -> Result<(), String> {
    let Some(mut observation) = agent_semantic_client_db::runtime_server_opentelemetry::RuntimePerformanceObservation::from_agent_facing_wall_failure(&value)? else {
        return Ok(());
    };
    let state_home = crate::server::runtime_server::state_home()?;
    let catalog_path = state_home
        .join("runtime")
        .join("server")
        .join("workspace-admissions.v1.json");
    let admission = agent_semantic_client_db::runtime_server_admission_catalog::RuntimeWorkspaceAdmissionCatalog::resolve_mapped(
        &catalog_path,
        project_root,
    )
    .map_err(|error| format!("performance failure workspace admission is unavailable: {error}"))?;
    observation.workspace_identity = Some(admission.workspace_identity);
    observation.seal_budget_failure_identity();
    let socket_path =
        crate::server::runtime_server::runtime_server_telemetry_socket_path(&state_home);
    let receipt = tokio::time::timeout(
        std::time::Duration::from_millis(25),
        agent_semantic_client_db::runtime_server_opentelemetry::admit_to_runtime(
            &socket_path,
            &observation,
        ),
    )
    .await
    .map_err(|_| "Runtime Server performance ingress exceeded 25ms".to_owned())??;
    if std::env::var_os("ASP_HOOK_BOOTSTRAP_TRACE").is_some() {
        eprintln!(
            "[asp-hook] route=performance-failure-reconcile state={} workspaceIdentity={} surface={} stage={}",
            receipt.state, receipt.workspace_identity, receipt.surface, receipt.stage
        );
    }
    Ok(())
}
