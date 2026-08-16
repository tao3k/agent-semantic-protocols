use std::ffi::OsString;

use super::{
    hook_event_is_canonical_recovery, is_synchronous_hook_dispatch,
    is_synchronous_hook_dispatch_with_override,
};

fn pre_tool_args() -> Vec<OsString> {
    ["hook", "pre-tool", "--client", "codex"]
        .into_iter()
        .map(OsString::from)
        .collect()
}

fn bash_payload(command: &str) -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({
        "tool_name": "Bash",
        "tool_input": { "command": command },
    }))
    .expect("encode Hook recovery payload")
}

#[test]
fn structured_hook_refresh_is_a_canonical_local_recovery_edge() {
    assert!(hook_event_is_canonical_recovery(
        &pre_tool_args(),
        &bash_payload("direnv exec . asp hook refresh --client codex"),
    ));
}

#[test]
fn unrelated_hook_commands_are_not_recovery_edges() {
    assert!(!hook_event_is_canonical_recovery(
        &pre_tool_args(),
        &bash_payload("asp hook paths ."),
    ));
}

#[test]
fn enforcing_and_host_publication_actions_are_synchronous_before_runtime_construction() {
    assert!(is_synchronous_hook_dispatch([
        "hook", "pre-tool", "--client", "codex",
    ]));
    assert!(is_synchronous_hook_dispatch([
        "hook",
        "permission-request",
        "--client",
        "codex",
    ]));
    assert!(is_synchronous_hook_dispatch([
        "hook",
        "subagent-start",
        "--client",
        "codex",
    ]));
    assert!(is_synchronous_hook_dispatch([
        "hook",
        "subagent-stop",
        "--client",
        "codex",
    ]));
}

#[test]
fn process_override_only_short_circuits_actual_hook_dispatches() {
    let version = [OsString::from("--version")];
    assert!(!is_synchronous_hook_dispatch_with_override(&version, true));

    let lifecycle = [
        OsString::from("hook"),
        OsString::from("subagent-start"),
        OsString::from("--client"),
        OsString::from("codex"),
    ];
    assert!(is_synchronous_hook_dispatch_with_override(&lifecycle, true));
}
