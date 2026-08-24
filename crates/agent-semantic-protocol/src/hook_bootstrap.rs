//! Configuration-independent hook bootstrap and bounded self-repair.
//!
//! This entrypoint deliberately does not parse the hook matcher document.  It
//! can therefore repair an older `asp` binary after the matcher contract has
//! advanced beyond that binary's enum vocabulary.

use std::ffi::OsString;
use std::future::Future;
use std::io::Write;

const MAX_HOOK_INPUT_BYTES: usize = 1024 * 1024;
const TRACE_ENV: &str = "ASP_HOOK_BOOTSTRAP_TRACE";
const NO_AGENT_ENV: &str = "ASP_NO_AGENT";
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

#[cfg(test)]
#[path = "../tests/unit/hook_bootstrap_publication_recovery.rs"]
mod publication_recovery_tests;

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

/// Policy-enforcing Host actions are mmap-backed synchronous work and must
/// complete before any Tokio runtime is constructed. Lifecycle events retain
/// the Tokio runtime needed for typed Runtime Server IPC. A process-level
/// recovery override has the same precedence for every Hook event.
#[doc(hidden)]
pub fn is_synchronous_hook_dispatch<I, S>(args: I) -> bool
where
    I: IntoIterator<Item = S>,
    S: Into<OsString>,
{
    let args = args.into_iter().map(Into::into).collect::<Vec<_>>();
    is_synchronous_hook_dispatch_with_override(&args, no_agent_bypass_requested())
}

fn no_agent_bypass_requested() -> bool {
    std::env::var_os(NO_AGENT_ENV).is_some_and(|value| value == "1")
}

/// Completes the explicit no-agent escape at process entry, before the Hook
/// watchdog thread, Tokio runtime, stdin payload, matcher, or state locks exist.
#[doc(hidden)]
pub fn process_entry_no_agent_bypass(args: &[OsString]) -> Option<Result<(), String>> {
    if !no_agent_bypass_requested() || !is_hook_event_dispatch(args.iter().cloned()) {
        return None;
    }
    if std::env::var_os(TRACE_ENV).is_some() {
        eprintln!("[asp-hook] route=process-entry-no-agent-bypass");
    }
    Some(emit_empty_success())
}

fn is_synchronous_hook_dispatch_with_override(args: &[OsString], override_present: bool) -> bool {
    if !is_hook_event_dispatch(args.iter().cloned()) {
        return false;
    }
    override_present
        || args
            .iter()
            .filter_map(|arg| arg.to_str())
            .any(|arg| matches!(arg, "pre-tool" | "permission-request"))
}

/// Poll the synchronous policy data plane without a Tokio runtime. Reaching
/// `Pending` is a contract violation; only lifecycle events may own async work.
#[doc(hidden)]
pub fn run_synchronous_hook_bootstrap_from_env() -> i32 {
    let mut future = std::pin::pin!(run_hook_bootstrap_from_env());
    let mut context = std::task::Context::from_waker(std::task::Waker::noop());
    match future.as_mut().poll(&mut context) {
        std::task::Poll::Ready(code) => code,
        std::task::Poll::Pending => {
            eprintln!(
                "[asp-hook] status=failed error=synchronous-policy-data-plane-yielded-to-runtime"
            );
            2
        }
    }
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
    hard_exit(code)
}

#[cfg(unix)]
fn hard_exit(code: i32) -> ! {
    unsafe extern "C" {
        fn _exit(status: i32) -> !;
    }
    // SAFETY: Hook-owned diagnostics are emitted before this boundary and
    // `_exit` accepts the same process status domain as the public command.
    unsafe { _exit(code) }
}

#[cfg(not(unix))]
fn hard_exit(code: i32) -> ! {
    std::process::exit(code)
}

async fn run_hook_bootstrap(args: Vec<OsString>) -> Result<i32, String> {
    let started = std::time::Instant::now();
    validate_hook_args(&args)?;
    if no_agent_bypass_requested() {
        if std::env::var_os(TRACE_ENV).is_some() {
            eprintln!("[asp-hook] route=bootstrap-no-agent-bypass");
        }
        emit_empty_success()?;
        return Ok(0);
    }
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
    if agent_semantic_hook::canonical_recovery_admission(hook_event(&args), &input) {
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
    let canonical_install_command = "<validated-candidate-asp> install binary";
    serde_json::json!({
        "schemaId": "agent.semantic-protocols.hook-local-policy-unavailable",
        "schemaVersion": "1",
        "surface": "hook",
        "event": event,
        "state": "unavailable",
        "reasonKind": "local-hook-policy-authority-unavailable",
        "recoveryCommand": canonical_install_command,
        "recoveryCommands": [canonical_install_command],
        "canonicalBinaryInstallTarget": canonical_install_command,
        "error": single_line(error),
    })
    .to_string()
}

fn local_hook_policy_unavailable_deny(event: &str, error: &str) -> String {
    let failure = local_hook_policy_unavailable(event, error);
    let reason = "ASP local Hook policy authority is unavailable; the enforcing event is denied until canonical recovery completes.";
    if event == "permission-request" {
        return serde_json::json!({
            "hookSpecificOutput": {
                "hookEventName": "PermissionRequest",
                "decision": {
                    "behavior": "deny",
                    "message": reason,
                },
            },
            "systemMessage": reason,
        })
        .to_string();
    }
    serde_json::json!({
        "hookSpecificOutput": {
            "hookEventName": "PreToolUse",
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

fn read_bounded_stdin() -> Result<Vec<u8>, String> {
    let input = crate::command::hook_runtime::read_hook_input_bounded()?.into_bytes();
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
