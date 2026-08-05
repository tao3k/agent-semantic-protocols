//! Configuration-independent hook bootstrap and bounded self-repair.
//!
//! This entrypoint deliberately does not parse the hook matcher document.  It
//! can therefore repair an older `asp` binary after the matcher contract has
//! advanced beyond that binary's enum vocabulary.

use std::ffi::OsString;
use std::io::Read;
use std::path::PathBuf;

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
pub fn run_hook_bootstrap_from_env() -> i32 {
    match run_hook_bootstrap(std::env::args_os().skip(1).collect()) {
        Ok(code) => code,
        Err(error) => {
            eprintln!("[asp-hook] status=failed error={error}");
            2
        }
    }
}

fn run_hook_bootstrap(args: Vec<OsString>) -> Result<i32, String> {
    validate_hook_args(&args)?;
    let input = read_bounded_stdin()?;
    if asp_no_agent_passthrough(&input, std::env::var_os("ASP_NO_AGENT").as_deref()) {
        if std::env::var_os(TRACE_ENV).is_some() {
            eprintln!("[asp-hook] route=bootstrap-no-agent-passthrough");
        }
        println!("{{}}");
        return Ok(0);
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
    if hook_event_is_runtime_server_recovery(&args, &input) {
        if std::env::var_os(TRACE_ENV).is_some() {
            eprintln!("[asp-hook] route=bootstrap-runtime-server-recovery");
        }
        println!("{{}}");
        return Ok(0);
    }

    let project_root = server_project_root(&input)?;
    match crate::command::evaluate_hook_event_via_runtime(
        hook_event(&args).unwrap_or("unknown"),
        &project_root,
        hook_args,
        hook_input,
    ) {
        Ok(output) => {
            if std::env::var_os(TRACE_ENV).is_some() {
                eprintln!("[asp-hook] route=server-resident-evaluator");
            }
            println!("{output}");
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
        Err(error) => Err(runtime_server_hook_unavailable(
            hook_event(&args).unwrap_or("unknown"),
            &error,
        )),
    }
}

fn asp_no_agent_passthrough(input: &[u8], inherited: Option<&std::ffi::OsStr>) -> bool {
    if inherited == Some(std::ffi::OsStr::new("1")) {
        return true;
    }
    let Ok(payload) = serde_json::from_slice::<serde_json::Value>(input) else {
        return false;
    };
    hook_payload_command(&payload).is_some_and(command_has_no_agent_prefix)
}

fn command_has_no_agent_prefix(command: &str) -> bool {
    let command = command.trim_start();
    command == "ASP_NO_AGENT=1"
        || command
            .strip_prefix("ASP_NO_AGENT=1")
            .and_then(|rest| rest.chars().next())
            .is_some_and(char::is_whitespace)
}

#[cfg(test)]
#[test]
fn no_agent_passthrough_has_explicit_truthy_semantics() {
    let ordinary = br#"{"tool_input":{"cmd":"cargo test"}}"#;
    assert!(asp_no_agent_passthrough(
        ordinary,
        Some(std::ffi::OsStr::new("1"))
    ));
    for value in ["", "0", "true", "yes", "on"] {
        assert!(!asp_no_agent_passthrough(
            ordinary,
            Some(std::ffi::OsStr::new(value))
        ));
    }
    assert!(asp_no_agent_passthrough(
        br#"{"tool_input":{"cmd":"ASP_NO_AGENT=1 cargo test"}}"#,
        None
    ));
    assert!(asp_no_agent_passthrough(
        br#"{"tool_input":{"cmd":"  ASP_NO_AGENT=1 cargo test"}}"#,
        None
    ));
    assert!(!asp_no_agent_passthrough(
        br#"{"tool_input":{"cmd":"cargo test; ASP_NO_AGENT=1 echo late"}}"#,
        None
    ));
    assert!(!asp_no_agent_passthrough(
        br#"{"tool_input":{"cmd":"ASP_NO_AGENT=10 cargo test"}}"#,
        None
    ));
}

fn runtime_server_hook_unavailable(event: &str, error: &str) -> String {
    let canonical_install_target = agent_semantic_runtime::resolve_state_home()
        .ok()
        .map(|state_home| state_home.join("runtime/bin/asp"));
    serde_json::json!({
        "schemaId": "agent.semantic-protocols.hook-control-plane-unavailable.v1",
        "schemaVersion": "1",
        "surface": "hook",
        "event": event,
        "state": "unavailable",
        "reasonKind": "runtime-server-hook-authority-unavailable",
        "recoveryCommand": "asp server reconcile",
        "recoveryCommands": [
            "asp server reconcile",
            "<validated-candidate-asp> install binary --target <canonicalBinaryInstallTarget>"
        ],
        "canonicalBinaryInstallTarget": canonical_install_target,
        "error": single_line(error),
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

fn hook_event_is_runtime_server_recovery(args: &[OsString], input: &[u8]) -> bool {
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

fn server_project_root(input: &[u8]) -> Result<PathBuf, String> {
    let payload: serde_json::Value = serde_json::from_slice(input)
        .map_err(|error| format!("hook payload must be JSON before server routing: {error}"))?;
    for field in ["cwd", "project_root", "projectRoot", "workspace"] {
        if let Some(path) = payload.get(field).and_then(serde_json::Value::as_str)
            && !path.trim().is_empty()
        {
            return Ok(PathBuf::from(path));
        }
    }
    std::env::current_dir().map_err(|error| format!("failed to resolve hook workspace: {error}"))
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
