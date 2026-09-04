//! Runtime-owned workspace Search planner entrypoint.

use std::path::PathBuf;

pub(crate) async fn run_workspace_search_playbook(args: &[String]) -> Result<(), String> {
    let mut parser_args = vec!["search".to_owned()];
    parser_args.extend_from_slice(args);
    let request = agent_semantic_search::parse_search_playbook_args(&parser_args)?;
    let project_root = resolve_playbook_workspace(&request.workspace)?;

    let state_home = agent_semantic_runtime::state_core::resolve_state_home()?;
    #[cfg(unix)]
    let client = crate::AspClient::new_from_host_capability(state_home, &project_root)?;
    #[cfg(not(unix))]
    let client = crate::AspClient::new(state_home, &project_root);
    let frame = client
        .dispatch_method(
            agent_semantic_client_protocol::WORKSPACE_SEARCH_PLAYBOOK_METHOD.to_owned(),
            serde_json::to_value(
                agent_semantic_client_protocol::AspClientWorkspaceSearchPlaybookRequest {
                    schema_id:
                        "agent.semantic-protocols.asp-client-workspace-search-playbook-request"
                            .to_owned(),
                    schema_version: "1".to_owned(),
                    language: request.language,
                    intent: request.intent,
                    query: request.query,
                    scope: request.scope,
                    coverage: request.coverage,
                    max_owners: request.max_owners,
                    deadline_ms: request.deadline_ms,
                    explain: request.explain,
                },
            )
            .map_err(|error| format!("encode Search playbook request: {error}"))?,
        )
        .await?;
    println!(
        "{}",
        serde_json::to_string_pretty(&frame)
            .map_err(|error| format!("encode workspace Search playbook response: {error}"))?
    );
    Ok(())
}

fn resolve_playbook_workspace(workspace: &str) -> Result<PathBuf, String> {
    let workspace = PathBuf::from(workspace);
    let workspace = if workspace.is_absolute() {
        workspace
    } else {
        std::env::current_dir()
            .map_err(|error| format!("failed to resolve current project directory: {error}"))?
            .join(workspace)
    };
    if !workspace.is_dir() {
        return Err(format!(
            "search playbook workspace must be a directory: {}",
            workspace.display()
        ));
    }
    std::fs::canonicalize(&workspace).map_err(|error| {
        format!(
            "failed to canonicalize Search playbook workspace {}: {error}",
            workspace.display()
        )
    })
}
