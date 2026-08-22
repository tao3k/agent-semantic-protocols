use super::{is_synchronous_hook_dispatch, is_synchronous_hook_dispatch_with_override};
use std::ffi::OsString;

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
