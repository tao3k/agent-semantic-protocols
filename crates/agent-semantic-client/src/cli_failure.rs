//! Agent-facing materialization for failures that occur before an ASP evidence frame exists.

/// Preserve ordinary command errors, but turn a bare OS `EPERM` into an
/// explicit boundary receipt that cannot be mistaken for a Hook policy deny.
#[must_use]
pub fn materialize_cli_failure(message: &str) -> String {
    let lower = message.to_ascii_lowercase();
    if lower.contains("reasonkind=host-operation-not-permitted")
        && lower.contains("failurelayer=runtime-verified-endpoint-transport")
    {
        return serde_json::json!({
            "schemaId": "agent.semantic-protocols.cli-failure",
            "schemaVersion": 1,
            "state": "blocked",
            "reasonKind": "host-operation-not-permitted",
            "failureLayer": "runtime-verified-endpoint-transport",
            "osError": "EPERM",
            "message": "ASP proved the Runtime serving identity, then the Host OS denied the endpoint connection.",
            "recovery": "Repair Host socket permission and rerun the exact ASP command. ASP did not infer a Runtime lifecycle transition.",
            "originalError": message,
        })
        .to_string();
    }
    if lower.contains("reasonkind=transport-unavailable") {
        return serde_json::json!({
            "schemaId": "agent.semantic-protocols.cli-failure",
            "schemaVersion": 1,
            "state": "blocked",
            "reasonKind": "transport-unavailable",
            "failureLayer": "runtime-transport-capability",
            "message": "ASP could not enter the Runtime ClientFrame service because no usable Host-declared transport capability was available.",
            "originalError": message,
        })
        .to_string();
    }
    if lower.contains("reasonkind=runtime-status-observation-failed") {
        return serde_json::json!({
            "schemaId": "agent.semantic-protocols.cli-failure",
            "schemaVersion": 1,
            "state": "blocked",
            "reasonKind": "runtime-status-observation-failed",
            "failureLayer": "runtime-status-observation",
            "message": "ASP could not authenticate a Runtime status observation. No lifecycle state was inferred.",
            "originalError": message,
        })
        .to_string();
    }
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
        "recovery": "Repair Host filesystem/socket permission and rerun the exact ASP command. ASP_NO_AGENT is a Hook-policy recovery escape only; normal Search and Query always use the verified Runtime serving endpoint. It does not override operator-stop or endpoint identity denial.",
        "originalError": message,
    })
    .to_string()
}

#[cfg(test)]
#[path = "../tests/unit/cli_failure.rs"]
mod tests;
