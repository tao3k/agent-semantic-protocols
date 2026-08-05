//! Hook command routing owned by the `asp` binary.

use super::hook_runtime::{
    materialize_runtime_generation_observation, observe_runtime_generation_for_hook_client,
    read_hook_input_bounded, run_hook_runtime_args,
};
use super::runtime_server::runtime_server_hook_evaluation_client;
use agent_semantic_hook::{HookDecision, parse_payload, render_platform_response};
use std::path::PathBuf;

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
    if matches!(args.first().map(String::as_str), Some("doctor" | "paths")) {
        return run_hook_runtime_args(forwarded);
    }

    let input = read_hook_input_bounded()
        .map_err(|error| format!("failed to read hook payload from stdin: {error}"))?;
    let project_root = hook_event_project_root(&input);
    let event = canonical_hook_event(args)?;
    let output = evaluate_hook_event_via_runtime(event, &project_root, forwarded, input)?;
    print!("{output}");
    Ok(())
}

pub(crate) fn evaluate_hook_event_via_runtime(
    event: &str,
    project_root: &std::path::Path,
    mut forwarded: Vec<String>,
    input: String,
) -> Result<String, String> {
    let payload =
        parse_payload(&input).map_err(|error| format!("invalid hook payload JSON: {error:?}"))?;
    let (explicit_asp_workspace, observation) =
        observe_runtime_generation_for_hook_client(event, project_root, &payload);
    let emit = force_decision_emit(&mut forwarded);
    let output = runtime_server_hook_evaluation_client(project_root, forwarded, input)?;
    let mut decision: HookDecision = serde_json::from_str(&output).map_err(|error| {
        format!("failed to decode resident Hook decision before client rendering: {error}")
    })?;
    materialize_runtime_generation_observation(&mut decision, explicit_asp_workspace, observation);
    let rendered = match emit.as_str() {
        "decision" => serde_json::to_value(&decision)
            .map_err(|error| format!("failed to serialize Hook decision: {error}"))?,
        "platform" => render_platform_response(&decision)
            .map_err(|error| format!("failed to render Hook response: {error:?}"))?,
        other => {
            return Err(format!(
                "unsupported --emit value: {other}; expected platform or decision"
            ));
        }
    };
    serde_json::to_string(&rendered)
        .map_err(|error| format!("failed to serialize Hook response: {error}"))
}

fn canonical_hook_event(args: &[String]) -> Result<&str, String> {
    match args.first().map(String::as_str) {
        Some("event") => args
            .get(1)
            .map(String::as_str)
            .ok_or_else(|| "usage: asp hook event <event> ...".to_owned()),
        Some(event) if HOOK_EVENTS.contains(&event) => Ok(event),
        _ => Err(usage()),
    }
}

fn force_decision_emit(args: &mut Vec<String>) -> String {
    if let Some(index) = args.iter().position(|argument| argument == "--emit") {
        let emit = args
            .get(index + 1)
            .cloned()
            .unwrap_or_else(|| "platform".to_owned());
        if let Some(value) = args.get_mut(index + 1) {
            *value = "decision".to_owned();
        } else {
            args.push("decision".to_owned());
        }
        return emit;
    }
    if let Some((index, emit)) = args.iter().enumerate().find_map(|(index, argument)| {
        argument
            .strip_prefix("--emit=")
            .map(|emit| (index, emit.to_owned()))
    }) {
        args[index] = "--emit=decision".to_owned();
        return emit;
    }
    args.extend(["--emit".to_owned(), "decision".to_owned()]);
    "platform".to_owned()
}

fn hook_event_project_root(input: &str) -> PathBuf {
    serde_json::from_str::<serde_json::Value>(input)
        .ok()
        .and_then(|payload| {
            payload
                .get("cwd")
                .and_then(serde_json::Value::as_str)
                .map(PathBuf::from)
        })
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
}

pub(super) fn is_help_request(args: &[String]) -> bool {
    args.len() == 1 && matches!(args[0].as_str(), "help" | "--help" | "-h")
}

pub(super) fn is_lifecycle_help_request(args: &[String]) -> bool {
    matches!(args.first().map(String::as_str), Some("doctor" | "paths"))
        && is_help_request(&args[1..])
}

fn forwarded_hook_lifecycle_args(command: &str, args: &[String]) -> Result<Vec<String>, String> {
    match command {
        "doctor" | "paths" => {
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
        lifecycle @ ("doctor" | "paths") => forwarded_hook_lifecycle_args(lifecycle, &args[1..]),
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
    "usage: asp install hook --client claude [PROJECT_ROOT] [--subagent-model MODEL]\n       asp hook doctor --client <codex|claude> ...\n       asp hook paths [PROJECT_ROOT]\n       asp hook --client <codex|claude> --event <event> ...\n       asp hook <pre-tool|post-tool|stop|event> ...\n       asp install plugin --codex".to_string()
}
