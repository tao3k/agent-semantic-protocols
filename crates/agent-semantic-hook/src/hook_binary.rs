//! Process boundary for the canonical Runtime Hook executable.

use std::ffi::OsString;

const NO_AGENT_ENV: &str = "ASP_NO_AGENT";

/// Run the Hook evaluator while guaranteeing one valid Host JSON terminal.
///
/// The inherited no-agent lane is deliberately checked before payload,
/// embedded policy, reader-probe, or async-runtime work. This is
/// distinct from the parser-proven command-local process-environment rule.
pub fn run_from_env() -> std::process::ExitCode {
    let arguments = std::env::args_os().skip(1).collect::<Vec<_>>();
    if inherited_no_agent_bypass() && hook_event(&arguments).is_some() {
        println!("{{}}");
        return std::process::ExitCode::SUCCESS;
    }
    let event = hook_event(&arguments).map(str::to_owned);
    match std::panic::catch_unwind(crate::aot_evaluator_cli::main_entry) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(payload) => {
            println!(
                "{}",
                panic_terminal(event.as_deref(), panic_message(payload.as_ref()))
            );
            std::process::ExitCode::SUCCESS
        }
    }
}

fn inherited_no_agent_bypass() -> bool {
    std::env::var_os(NO_AGENT_ENV).is_some_and(|value| value == "1")
}

fn hook_event(arguments: &[OsString]) -> Option<&str> {
    let first = arguments.first()?.to_str()?;
    matches!(
        first,
        "pre-tool"
            | "permission-request"
            | "post-tool"
            | "stop"
            | "notification"
            | "user-prompt"
            | "session-start"
            | "subagent-start"
            | "subagent-stop"
    )
    .then_some(first)
}

fn panic_terminal(event: Option<&str>, message: String) -> serde_json::Value {
    let system_message = format!("ASP Hook failed before a policy terminal: {message}");
    match event {
        Some("pre-tool") => serde_json::json!({
            "hookSpecificOutput": {
                "hookEventName": "PreToolUse",
                "permissionDecision": "deny",
                "permissionDecisionReason": system_message,
            },
            "systemMessage": system_message,
        }),
        Some("permission-request") => serde_json::json!({
            "hookSpecificOutput": {
                "hookEventName": "PermissionRequest",
                "decision": {
                    "behavior": "deny",
                    "message": system_message,
                },
            },
            "systemMessage": system_message,
        }),
        _ => serde_json::json!({}),
    }
}

fn panic_message(payload: &(dyn std::any::Any + Send)) -> String {
    payload
        .downcast_ref::<String>()
        .cloned()
        .or_else(|| {
            payload
                .downcast_ref::<&'static str>()
                .map(|message| (*message).to_owned())
        })
        .unwrap_or_else(|| "non-string panic payload".to_owned())
}

#[cfg(test)]
#[path = "../tests/unit/hook_binary.rs"]
mod tests;
