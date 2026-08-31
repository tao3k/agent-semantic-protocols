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
    if let Some(reason_kind) = [
        "runtime-client-connect-deadline-exceeded",
        "runtime-client-session-deadline-exceeded",
        "runtime-client-response-deadline-exceeded",
    ]
    .into_iter()
    .find(|reason_kind| error.contains(&format!("reasonKind={reason_kind}")))
    {
        return serde_json::json!({
            "schemaId": "agent.semantic-protocols.runtime-client-terminal",
            "schemaVersion": "1",
            "state": "failed",
            "reasonKind": reason_kind,
            "retryAdmitted": false,
            "argv": argv,
            "cause": error,
        })
        .to_string();
    }
    if error.contains("reasonKind=active-workspace-generation-required") {
        return serde_json::json!({
            "schemaId": "agent.semantic-protocols.workspace-generation-required",
            "schemaVersion": "1",
            "state": "in-progress",
            "reasonKind": "active-workspace-generation-required",
            "admissionTrigger": "query-demand",
            "buildOwner": "runtime-server",
            "requestLifetimeIndependent": true,
            "retryPolicy": "retry-after-runtime-progress",
            "argv": argv,
            "nextAction": "Runtime Server owns the detached cold admission. Retry the preserved argv after Runtime progress; no client-side recovery action is required.",
            "cause": error,
        })
        .to_string();
    }
    if !agent_semantic_client_db::workspace_db_ipc::is_host_local_ipc_permission_denied(&error) {
        return error;
    }
    let command_digest = agent_semantic_hook::host_native_handoff::argv_digest(&argv)
        .unwrap_or_else(|_| "unavailable".to_owned());
    serde_json::json!({
        "schemaId": "agent.semantic-protocols.host-native-execution-required",
        "schemaVersion": "1",
        "state": "deferred",
        "reasonKind": agent_semantic_client_db::workspace_db_ipc::HOST_LOCAL_IPC_PERMISSION_DENIED_REASON_KIND,
        "executionAuthority": "host-native",
        "retryPolicy": "do-not-retry-in-current-sandbox",
        "argv": argv,
        "commandDigest": command_digest,
        "message": "The current sandbox cannot connect to the Runtime data endpoint. Execute this exact ASP argv with Host-native command authority; do not create or resume another Agent, retry in the current sandbox, open the Runtime database directly, or refresh the plugin.",
        "cause": error,
    })
    .to_string()
}

#[cfg(test)]
#[path = "../tests/unit/cli.rs"]
mod tests;
