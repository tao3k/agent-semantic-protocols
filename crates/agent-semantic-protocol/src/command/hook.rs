//! Hook command routing owned by the `asp` binary.

use super::hook_runtime::run_hook_runtime_args;

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

pub(crate) fn run_hook_command(args: &[String]) -> Result<(), String> {
    if is_help_request(args) || is_lifecycle_help_request(args) {
        println!("{}", usage());
        return Ok(());
    }
    let forwarded = forwarded_hook_args(args)?;
    run_hook_runtime_args(forwarded)
}

pub(super) fn is_help_request(args: &[String]) -> bool {
    args.len() == 1 && matches!(args[0].as_str(), "help" | "--help" | "-h")
}

pub(super) fn is_lifecycle_help_request(args: &[String]) -> bool {
    matches!(args.first().map(String::as_str), Some("install" | "doctor"))
        && is_help_request(&args[1..])
}

fn forwarded_hook_lifecycle_args(command: &str, args: &[String]) -> Result<Vec<String>, String> {
    match command {
        "install" | "doctor" => {
            let mut forwarded = vec![command.to_string()];
            forwarded.extend(args.iter().cloned());
            Ok(forwarded)
        }
        _ => Err(usage()),
    }
}

pub(super) fn forwarded_hook_args(args: &[String]) -> Result<Vec<String>, String> {
    let Some(command) = args.first().map(String::as_str) else {
        return Err(usage());
    };

    match command {
        "help" | "--help" | "-h" => Err(usage()),
        lifecycle @ ("install" | "doctor") => forwarded_hook_lifecycle_args(lifecycle, &args[1..]),
        "event" => {
            let Some(event) = args.get(1) else {
                return Err("usage: asp hook event <event> ...".to_string());
            };
            forwarded_event_args(event, &args[2..])
        }
        event if HOOK_EVENTS.contains(&event) => forwarded_event_args(event, &args[1..]),
        flag if flag.starts_with('-') => {
            let mut forwarded = vec!["hook".to_string()];
            forwarded.extend(args.iter().cloned());
            Ok(forwarded)
        }
        _ => Err(usage()),
    }
}

fn forwarded_event_args(event: &str, rest: &[String]) -> Result<Vec<String>, String> {
    if !HOOK_EVENTS.contains(&event) {
        return Err(format!("unsupported hook event: {event}"));
    }
    let mut forwarded = vec!["hook".to_string(), "--event".to_string(), event.to_string()];
    forwarded.extend(rest.iter().cloned());
    Ok(forwarded)
}

fn usage() -> String {
    "usage: asp hook <install|doctor> --client <codex|claude> [--subagent-model MODEL] ...\n       asp hook --client <codex|claude> --event <event> ...\n       asp hook <pre-tool|post-tool|stop|event> ...".to_string()
}
