// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

//! Hook command routing owned by the `asp` binary.

use super::hook_runtime::run_hook_runtime_args;

pub(crate) async fn run_hook_command(args: &[String]) -> Result<(), String> {
    if matches!(args.first().map(String::as_str), Some("break-glass")) {
        return super::hook_break_glass::run_hook_break_glass(&args[1..]);
    }
    if is_help_request(args) || is_lifecycle_help_request(args) {
        println!("{}", usage());
        return Ok(());
    }
    let forwarded = forwarded_hook_args(args)?;
    run_hook_runtime_args(forwarded).await
}

/// Hook configuration and recovery controls are local control-plane work.
/// Their execution must remain reachable when Runtime activation or provider
/// data planes are unavailable.
pub(crate) fn is_runtime_independent_control_command(args: &[String]) -> bool {
    matches!(
        args.first().map(String::as_str),
        Some("accept-host" | "break-glass" | "doctor" | "enablement" | "paths" | "refresh")
    )
}

pub(super) fn is_help_request(args: &[String]) -> bool {
    args.len() == 1 && matches!(args[0].as_str(), "help" | "--help" | "-h")
}

pub(super) fn is_lifecycle_help_request(args: &[String]) -> bool {
    matches!(
        args.first().map(String::as_str),
        Some("accept-host" | "doctor" | "enablement" | "paths" | "refresh")
    ) && is_help_request(&args[1..])
}

fn forwarded_hook_lifecycle_args(command: &str, args: &[String]) -> Result<Vec<String>, String> {
    match command {
        "accept-host" | "doctor" | "enablement" | "paths" | "refresh" => {
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
        lifecycle @ ("accept-host" | "doctor" | "enablement" | "paths" | "refresh") => {
            forwarded_hook_lifecycle_args(lifecycle, &args[1..])
        }
        _ => Err(usage()),
    }
}

fn usage() -> String {
    "usage: asp install hook --client claude [PROJECT_ROOT] [--subagent-model MODEL]\n       asp hook accept-host --host-rollout PATH --hook-events PATH --host-probe-path PATH --host-sentinel TOKEN\n       asp hook doctor --client <codex|claude>\n       asp hook enablement [PROJECT_ROOT] [--json]\n       asp hook paths [PROJECT_ROOT]\n       asp hook break-glass mint --defect-kind <KIND> --command <COMMAND> [PROJECT_ROOT]\n       asp install plugin <status|publish> --codex [PROJECT_ROOT]\n\nHost events are accepted only by the standalone `asp-hook <event> ...` executable.".to_string()
}
