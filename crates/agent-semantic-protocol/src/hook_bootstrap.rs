//! Configuration-independent hook bootstrap and bounded self-repair.
//!
//! This entrypoint deliberately does not parse the hook matcher document.  It
//! can therefore repair an older `asp` binary after the matcher contract has
//! advanced beyond that binary's enum vocabulary.

use std::ffi::OsString;
use std::io::{Read, Write};

const MAX_HOOK_INPUT_BYTES: usize = 1024 * 1024;
const TRACE_ENV: &str = "ASP_HOOK_BOOTSTRAP_TRACE";
const HOOK_EVENTS: &[&str] = &[
    "pre-tool",
    "permission-request",
    "post-tool",
    "stop",
    "notification",
    "user-prompt",
    "session-start",
    "subagent-start",
    "subagent-stop",
];

/// Return whether the public CLI arguments name an actual Hook event.
///
/// Lifecycle diagnostics such as `asp hook doctor` and `asp hook paths` must
/// reach the ordinary command parser.  Only event evaluation enters the
/// configuration-independent bootstrap boundary.
#[doc(hidden)]
pub fn is_hook_event_dispatch<I, S>(args: I) -> bool
where
    I: IntoIterator<Item = S>,
    S: Into<OsString>,
{
    let args = args.into_iter().map(Into::into).collect::<Vec<_>>();
    args.first().and_then(|arg| arg.to_str()) == Some("hook") && hook_event(&args).is_some()
}

/// Run the canonical hook command, repairing the development artifact on a
/// recognized binary/config contract drift before replaying the event once.
#[doc(hidden)]
pub async fn run_hook_bootstrap_from_env() -> i32 {
    match run_hook_bootstrap(std::env::args_os().skip(1).collect()).await {
        Ok(code) => code,
        Err(error) => {
            eprintln!("[asp-hook] status=failed error={error}");
            2
        }
    }
}

/// Terminate after the response owner has flushed its output, without running
/// process-global exit handlers that could extend the host's strict deadline.
#[doc(hidden)]
pub fn terminate_hook_process(code: i32) -> ! {
    if std::env::var_os(TRACE_ENV).is_some() {
        eprintln!("[asp-hook] route=bootstrap-immediate-termination");
    }
    #[cfg(unix)]
    {
        unsafe extern "C" {
            fn _exit(status: i32) -> !;
        }
        // SAFETY: all Hook-owned output is flushed above and `_exit` accepts
        // the same process status domain as the public command boundary.
        unsafe { _exit(code) }
    }
    #[cfg(not(unix))]
    std::process::exit(code)
}

async fn run_hook_bootstrap(args: Vec<OsString>) -> Result<i32, String> {
    let started = std::time::Instant::now();
    validate_hook_args(&args)?;
    let input = read_bounded_stdin()?;
    match crate::hook_break_glass::evaluate_hook_break_glass(&input) {
        crate::hook_break_glass::HookBreakGlassEvaluation::Authorized(capability) => {
            if std::env::var_os(TRACE_ENV).is_some() {
                eprintln!(
                    "[asp-hook] route=bootstrap-one-shot-break-glass nonce={} defectKind={}",
                    capability.nonce, capability.defect_kind,
                );
            }
            emit_empty_success()?;
            return Ok(0);
        }
        crate::hook_break_glass::HookBreakGlassEvaluation::Rejected(error) => {
            if std::env::var_os(TRACE_ENV).is_some() {
                eprintln!(
                    "[asp-hook] route=bootstrap-break-glass-rejected error={}",
                    single_line(&error),
                );
            }
        }
        crate::hook_break_glass::HookBreakGlassEvaluation::NotRequested => {}
    }
    let hook_args = args
        .iter()
        .map(|arg| {
            arg.to_str()
                .map(ToOwned::to_owned)
                .ok_or_else(|| "bootstrap hook arguments must be UTF-8".to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
    let hook_input = String::from_utf8(input.clone())
        .map_err(|error| format!("hook payload must be UTF-8 JSON: {error}"))?;
    if hook_event_is_canonical_recovery(&args, &input) {
        if std::env::var_os(TRACE_ENV).is_some() {
            eprintln!("[asp-hook] route=bootstrap-canonical-recovery");
        }
        emit_empty_success()?;
        return Ok(0);
    }
    let event = hook_event(&args).unwrap_or("unknown");
    if !hook_event_requires_policy_evaluation(event, &input)? {
        if std::env::var_os(TRACE_ENV).is_some() {
            eprintln!(
                "[asp-hook] route=bootstrap-local-action-passthrough elapsedMicros={}",
                started.elapsed().as_micros()
            );
        }
        emit_empty_success()?;
        if std::env::var_os(TRACE_ENV).is_some() {
            eprintln!(
                "[asp-hook] route=bootstrap-local-action-emitted elapsedMicros={}",
                started.elapsed().as_micros()
            );
        }
        return Ok(0);
    }

    match crate::command::evaluate_hook_event_locally(&hook_args[1..], hook_input).await {
        Ok(()) => {
            if std::env::var_os(TRACE_ENV).is_some() {
                eprintln!("[asp-hook] route=local-policy-evaluator");
            }
            Ok(0)
        }
        Err(error) if event_is_observational(&args) => {
            eprintln!(
                "[asp-hook] status=degraded-open event={} serverError={} policy=observational-liveness",
                hook_event(&args).unwrap_or("unknown"),
                single_line(&error),
            );
            Ok(0)
        }
        Err(error) => {
            let event = hook_event(&args).unwrap_or("unknown");
            println!("{}", local_hook_policy_unavailable_deny(event, &error));
            Ok(0)
        }
    }
}

fn emit_empty_success() -> Result<(), String> {
    let mut stdout = std::io::stdout().lock();
    stdout
        .write_all(b"{}\n")
        .and_then(|()| stdout.flush())
        .map_err(|error| format!("write Hook passthrough response: {error}"))
}

fn hook_event_requires_policy_evaluation(event: &str, input: &[u8]) -> Result<bool, String> {
    if !matches!(event, "pre-tool" | "permission-request" | "post-tool") {
        return Ok(true);
    }
    let payload: serde_json::Value = serde_json::from_slice(input)
        .map_err(|error| format!("hook payload must be JSON before action routing: {error}"))?;
    Ok(agent_semantic_hook::codex_tool_event_requires_policy_evaluation(&payload).unwrap_or(true))
}

fn local_hook_policy_unavailable(event: &str, error: &str) -> String {
    let canonical_install_target = agent_semantic_runtime::resolve_state_home()
        .ok()
        .map(|state_home| state_home.join("runtime/bin/asp"));
    let recovery_command = canonical_install_target
        .as_ref()
        .map(|target| {
            format!(
                "<validated-candidate-asp> install binary --target {}",
                target.display()
            )
        })
        .unwrap_or_else(|| {
            "<validated-candidate-asp> install binary --target <canonicalBinaryInstallTarget>"
                .to_owned()
        });
    serde_json::json!({
        "schemaId": "agent.semantic-protocols.hook-local-policy-unavailable.v1",
        "schemaVersion": "1",
        "surface": "hook",
        "event": event,
        "state": "unavailable",
        "reasonKind": "local-hook-policy-authority-unavailable",
        "recoveryCommand": recovery_command,
        "recoveryCommands": [
            "asp hook doctor --client codex",
            recovery_command
        ],
        "canonicalBinaryInstallTarget": canonical_install_target,
        "error": single_line(error),
    })
    .to_string()
}

fn local_hook_policy_unavailable_deny(event: &str, error: &str) -> String {
    let failure = local_hook_policy_unavailable(event, error);
    let hook_event_name = match event {
        "pre-tool" => "PreToolUse",
        "permission-request" => "PermissionRequest",
        _ => "PreToolUse",
    };
    let reason = "ASP local Hook policy authority is unavailable; the enforcing event is denied until canonical recovery completes.";
    serde_json::json!({
        "hookSpecificOutput": {
            "hookEventName": hook_event_name,
            "permissionDecision": "deny",
            "permissionDecisionReason": reason,
            "additionalContext": format!("[agent-hook-local-policy-unavailable] {failure}"),
        },
        "systemMessage": reason,
    })
    .to_string()
}

fn validate_hook_args(args: &[OsString]) -> Result<(), String> {
    if args.first().and_then(|arg| arg.to_str()) != Some("hook") {
        return Err("bootstrap accepts only `hook <event> ...` dispatch".to_string());
    }
    if hook_event(args).is_none() {
        return Err("bootstrap hook event is missing or non-UTF-8".to_string());
    }
    Ok(())
}

fn hook_event_is_canonical_recovery(args: &[OsString], input: &[u8]) -> bool {
    if !matches!(hook_event(args), Some("pre-tool" | "permission-request")) {
        return false;
    }
    let Ok(payload) = serde_json::from_slice::<serde_json::Value>(input) else {
        return false;
    };
    let Some(command) = hook_payload_command(&payload) else {
        return false;
    };
    let Ok(stages) = agent_semantic_command_match::parse_bash_command_candidates(command) else {
        return false;
    };
    if stages.len() != 1 {
        return false;
    }
    let words = stages[0].words();
    let Some(asp_index) = words.iter().position(|word| {
        word.rsplit(['/', '\\'])
            .next()
            .is_some_and(|name| name == "asp")
    }) else {
        return false;
    };
    let trusted_prefix = asp_index == 0
        || (asp_index == 3
            && words[0].rsplit(['/', '\\']).next() == Some("direnv")
            && words[1] == "exec"
            && words[2] == ".");
    if !trusted_prefix {
        return false;
    }
    let exact_server_control = words.get(asp_index + 1).map(String::as_str) == Some("server")
        && matches!(
            words.get(asp_index + 2).map(String::as_str),
            Some("status" | "reconcile" | "restart")
        )
        && words.len() == asp_index + 3;
    if exact_server_control {
        return true;
    }
    let exact_hook_doctor = words.get(asp_index + 1).map(String::as_str) == Some("hook")
        && words.get(asp_index + 2).map(String::as_str) == Some("doctor")
        && words.get(asp_index + 3).map(String::as_str) == Some("--client")
        && words.get(asp_index + 4).map(String::as_str) == Some("codex")
        && words.len() == asp_index + 5;
    if exact_hook_doctor {
        return true;
    }
    let Ok(state_home) = agent_semantic_runtime::resolve_state_home() else {
        return false;
    };
    exact_canonical_binary_install(words, asp_index, &state_home.join("runtime/bin/asp"))
}

fn exact_canonical_binary_install(
    words: &[String],
    asp_index: usize,
    canonical_target: &std::path::Path,
) -> bool {
    words.get(asp_index + 1).map(String::as_str) == Some("install")
        && words.get(asp_index + 2).map(String::as_str) == Some("binary")
        && words.get(asp_index + 3).map(String::as_str) == Some("--target")
        && words
            .get(asp_index + 4)
            .is_some_and(|target| std::path::Path::new(target) == canonical_target)
        && words.len() == asp_index + 5
}

fn hook_payload_command(payload: &serde_json::Value) -> Option<&str> {
    let tool_input = payload
        .get("tool_input")
        .or_else(|| payload.get("toolInput"));
    tool_input
        .and_then(|input| input.get("cmd").or_else(|| input.get("command")))
        .and_then(serde_json::Value::as_str)
        .or_else(|| payload.get("command").and_then(serde_json::Value::as_str))
}

fn read_bounded_stdin() -> Result<Vec<u8>, String> {
    let mut input = Vec::new();
    std::io::stdin()
        .take((MAX_HOOK_INPUT_BYTES + 1) as u64)
        .read_to_end(&mut input)
        .map_err(|error| format!("read hook stdin: {error}"))?;
    if input.len() > MAX_HOOK_INPUT_BYTES {
        return Err(format!(
            "hook payload exceeds {MAX_HOOK_INPUT_BYTES} byte bootstrap bound"
        ));
    }
    Ok(input)
}

fn hook_event(args: &[OsString]) -> Option<&str> {
    args.iter()
        .skip(1)
        .filter_map(|arg| arg.to_str())
        .find(|arg| HOOK_EVENTS.contains(arg))
}

fn event_is_observational(args: &[OsString]) -> bool {
    matches!(
        hook_event(args),
        Some(
            "session-start"
                | "user-prompt"
                | "post-tool"
                | "subagent-start"
                | "subagent-stop"
                | "stop"
                | "notification"
        )
    )
}

fn single_line(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
#[path = "../tests/unit/hook_bootstrap.rs"]
mod tests;
