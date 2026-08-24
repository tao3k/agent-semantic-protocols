use super::is_synchronous_hook_dispatch_with_override;
use std::ffi::OsString;

#[test]
fn only_policy_enforcement_is_synchronous_before_runtime_construction() {
    let dispatch = |event: &str| {
        let args = [
            OsString::from("hook"),
            OsString::from(event),
            OsString::from("--client"),
            OsString::from("codex"),
        ];
        is_synchronous_hook_dispatch_with_override(&args, false)
    };
    assert!(dispatch("pre-tool"));
    assert!(dispatch("permission-request"));
    assert!(!dispatch("subagent-start"));
    assert!(!dispatch("subagent-stop"));
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
