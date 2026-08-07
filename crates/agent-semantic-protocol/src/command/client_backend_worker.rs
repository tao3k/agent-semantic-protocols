//! Large-stack execution boundary for the local ASP client backend.

use std::path::PathBuf;

pub(in crate::command) async fn run_client_backend_on_worker(
    language_id: &str,
    client_args: Vec<String>,
    project_root: PathBuf,
) -> Result<(), String> {
    let language_id = agent_semantic_client::LanguageId::from(language_id);
    agent_semantic_client::run_cli_args(Some(language_id), client_args, project_root).await
}
