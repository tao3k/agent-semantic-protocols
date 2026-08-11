//! Process argument entrypoints for the `asp` CLI.

use crate::command::run_protocol_command;
use std::env;

/// Run the `asp` CLI using process arguments.
pub async fn run_cli_from_env() -> Result<(), String> {
    run_cli_from_env_started(tokio::time::Instant::now()).await
}

pub(crate) async fn run_cli_from_env_started(
    process_started: tokio::time::Instant,
) -> Result<(), String> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    crate::command::run_protocol_command_started(args.clone(), process_started)
        .await
        .map_err(|error| render_cli_error(&args, error))
}

/// Run the `asp` CLI using caller-provided arguments.
pub async fn run_cli_args<I, S>(args: I) -> Result<(), String>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let args = args.into_iter().map(Into::into).collect::<Vec<_>>();
    run_protocol_command(args.clone())
        .await
        .map_err(|error| render_cli_error(&args, error))
}

fn render_cli_error(args: &[String], error: String) -> String {
    let argv = std::iter::once("asp".to_owned())
        .chain(args.iter().cloned())
        .collect::<Vec<_>>();
    if error.contains("reasonKind=active-workspace-generation-required") {
        return serde_json::json!({
            "schemaId": "agent.semantic-protocols.workspace-generation-required",
            "schemaVersion": "1",
            "state": "deferred",
            "reasonKind": "active-workspace-generation-required",
            "retryPolicy": "do-not-retry-in-current-generation",
            "choicePlaneCommand": "asp session --agents choice-plane",
            "argv": argv,
            "nextAction": "Run `asp session --agents choice-plane`, execute the Host create/register or Call/resume action it selects from this command's tags, require a nonzero generation bound to the same workspace, then retry the preserved argv. Do not retry in the current generation.",
            "cause": error,
        })
        .to_string();
    }
    if !agent_semantic_client_db::workspace_db_ipc::is_host_local_ipc_permission_denied(&error) {
        return error;
    }
    serde_json::json!({
        "schemaId": "agent.semantic-protocols.host-native-execution-required",
        "schemaVersion": "1",
        "state": "deferred",
        "reasonKind": agent_semantic_client_db::workspace_db_ipc::HOST_LOCAL_IPC_PERMISSION_DENIED_REASON_KIND,
        "executionAuthority": "host-native",
        "retryPolicy": "do-not-retry-in-current-sandbox",
        "argv": argv,
        "message": "The current sandbox cannot connect to the Runtime data endpoint. Execute this exact ASP argv with Host-native command authority; do not create or resume another Agent, retry in the current sandbox, open the Runtime database directly, or refresh the plugin.",
        "cause": error,
    })
    .to_string()
}

#[cfg(test)]
#[path = "../tests/unit/cli.rs"]
mod tests;
