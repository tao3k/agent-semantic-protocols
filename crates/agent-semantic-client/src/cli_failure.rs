//! Agent-facing materialization for failures that occur before an ASP evidence frame exists.

/// Preserve ordinary command errors, but turn a bare OS `EPERM` into an
/// explicit boundary receipt that cannot be mistaken for a Hook policy deny.
#[must_use]
pub fn materialize_cli_failure(message: &str) -> String {
    let lower = message.to_ascii_lowercase();
    if !lower.contains("operation not permitted") && !lower.contains("os error 1") {
        return message.to_owned();
    }
    serde_json::json!({
        "schemaId": "agent.semantic-protocols.cli-failure",
        "schemaVersion": 1,
        "state": "blocked",
        "reasonKind": "host-operation-not-permitted",
        "failureLayer": "asp-cli-startup-or-ipc-boundary",
        "osError": "EPERM",
        "message": "ASP could not produce semantic evidence because the Host OS denied a process, IPC, or filesystem operation. This is not a Hook policy denial; the failing lower layer is not attributable until a typed ASP frame exists.",
        "recovery": "Repair Host filesystem/socket permission and rerun the exact ASP command. ASP_NO_AGENT selects the minimal single-thread server-first IPC lane, recovers a dead owner from the durable applied V1 activation, and then continues the original request. It does not override operator-stop or endpoint identity denial.",
        "originalError": message,
    })
    .to_string()
}

#[cfg(test)]
#[path = "../tests/unit/cli_failure.rs"]
mod tests;
