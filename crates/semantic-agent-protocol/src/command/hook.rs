//! Delegation from `semantic-agent-protocol hook` to `semantic-agent-hook`.

const HOOK_EVENTS: &[&str] = &[
    "pre-tool",
    "post-tool",
    "stop",
    "notification",
    "user-prompt-submit",
    "session-start",
];

pub(crate) fn run_hook_command(args: &[String]) -> Result<(), String> {
    let forwarded = forwarded_hook_args(args)?;
    semantic_agent_hook::run_cli_args(forwarded)
}

pub(super) fn forwarded_hook_args(args: &[String]) -> Result<Vec<String>, String> {
    let Some(command) = args.first().map(String::as_str) else {
        return Err(usage());
    };

    match command {
        "install" | "doctor" => {
            let mut forwarded = vec![command.to_string()];
            forwarded.extend(args[1..].iter().cloned());
            Ok(forwarded)
        }
        "event" => {
            let Some(event) = args.get(1) else {
                return Err("usage: semantic-agent-protocol hook event <event> ...".to_string());
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
    "usage: semantic-agent-protocol hook <install|doctor|pre-tool|post-tool|stop|event> ..."
        .to_string()
}
