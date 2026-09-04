//! One authoritative, process-scoped Hook recovery selector.
//!
//! `ASP_NO_AGENT=1` is deliberately not a policy rule.  It is evaluated before
//! any generation, configuration, reader-probe, or asynchronous-runtime work.
//! A similarly named environment variable is never a compatibility alias:
//! recovery must be explicit and auditable.

use serde_json::Value;

pub const NO_AGENT_ENV: &str = "ASP_NO_AGENT";
const NO_AGENT_ASSIGNMENT: &str = "ASP_NO_AGENT=1";

/// Whether this evaluator process inherited the operator-owned recovery lane.
pub fn inherited() -> bool {
    std::env::var_os(NO_AGENT_ENV).is_some_and(|value| value == "1")
}

/// Whether a Codex Bash payload declares the exact recovery assignment in a
/// parsed shell process stage.  This recognizes shell syntax, rather than a
/// substring, so quoted text and lookalike variables cannot bypass policy.
pub fn payload_declares_process_escape(payload: &Value) -> bool {
    if payload.get("tool_name").and_then(Value::as_str) != Some("Bash") {
        return false;
    }
    let Some(command) = payload
        .get("tool_input")
        .and_then(|input| input.get("command").or_else(|| input.get("cmd")))
        .and_then(Value::as_str)
    else {
        return false;
    };
    agent_semantic_shell_parser::parse_bash_command_candidates(command).is_ok_and(|stages| {
        agent_semantic_shell_parser::command_stages_match_process_environment_assignment(
            &stages,
            &[NO_AGENT_ASSIGNMENT],
        )
    })
}
