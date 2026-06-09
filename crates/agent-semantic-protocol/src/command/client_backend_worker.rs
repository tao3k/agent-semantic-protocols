//! Large-stack execution boundary for the local ASP client backend.

use std::path::PathBuf;

const CLIENT_BACKEND_WORKER_STACK_BYTES: usize = 64 * 1024 * 1024;

pub(in crate::command) fn run_client_backend_on_worker(
    language_id: &str,
    client_args: Vec<String>,
    project_root: PathBuf,
) -> Result<(), String> {
    let language_id = agent_semantic_client::LanguageId::from(language_id);
    std::thread::Builder::new()
        .name("asp-client-backend".to_string())
        .stack_size(CLIENT_BACKEND_WORKER_STACK_BYTES)
        .spawn(move || {
            agent_semantic_client::run_cli_args(Some(language_id), client_args, project_root)
        })
        .map_err(|error| format!("failed to start ASP client backend worker: {error}"))?
        .join()
        .map_err(|panic| {
            if let Some(message) = panic.downcast_ref::<&str>() {
                format!("ASP client backend worker panicked: {message}")
            } else if let Some(message) = panic.downcast_ref::<String>() {
                format!("ASP client backend worker panicked: {message}")
            } else {
                "ASP client backend worker panicked".to_string()
            }
        })?
}
