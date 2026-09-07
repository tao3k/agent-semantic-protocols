// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Process boundary for the canonical Runtime Hook executable.

use std::ffi::OsString;

/// Run the Hook evaluator while guaranteeing one valid Host JSON terminal.
///
/// Hook enablement is read once from the global State Home configuration before
/// payload, policy, reader-probe, or asynchronous Runtime work.
pub fn run_from_env() -> std::process::ExitCode {
    let arguments = std::env::args_os().skip(1).collect::<Vec<_>>();
    let event = hook_event(&arguments).map(str::to_owned);
    if event.is_some() {
        match configured_hook_engine_enabled() {
            Ok(false) => {
                println!("{{}}");
                return std::process::ExitCode::SUCCESS;
            }
            Ok(true) => {}
            Err(error) => {
                println!("{}", panic_terminal(event.as_deref(), error));
                return std::process::ExitCode::SUCCESS;
            }
        }
    }
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

fn configured_hook_engine_enabled() -> Result<bool, String> {
    let state_home = agent_semantic_artifacts::StateHomeLayout::from_process_environment()?;
    Ok(
        agent_semantic_artifacts::load_asp_global_config(state_home.root())?
            .hook_engine()
            .enabled(),
    )
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
