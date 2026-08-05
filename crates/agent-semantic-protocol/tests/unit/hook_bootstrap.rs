use super::{
    event_is_observational, exact_canonical_binary_install, hook_event,
    hook_event_is_runtime_server_recovery, is_hook_event_dispatch, runtime_server_hook_unavailable,
    validate_hook_args,
};
use std::ffi::OsString;

#[test]
fn lifecycle_failures_degrade_open_but_enforcement_failures_do_not() {
    for event in [
        "session-start",
        "user-prompt",
        "post-tool",
        "subagent-start",
        "subagent-stop",
        "stop",
    ] {
        let args = vec![OsString::from("hook"), OsString::from(event)];
        assert!(event_is_observational(&args), "{event}");
    }
    for event in ["pre-tool", "permission-request"] {
        let args = vec![OsString::from("hook"), OsString::from(event)];
        assert!(!event_is_observational(&args), "{event}");
    }
}

#[test]
fn recovery_kernel_admits_only_the_exact_canonical_binary_install_target() {
    let canonical = std::path::Path::new("/state/runtime/bin/asp");
    let words = |command: &str| {
        agent_semantic_command_match::parse_bash_command_candidates(command)
            .expect("parse recovery command")[0]
            .words()
            .to_vec()
    };
    assert!(exact_canonical_binary_install(
        &words("/tmp/candidate/asp install binary --target /state/runtime/bin/asp"),
        0,
        canonical,
    ));
    for command in [
        "/tmp/candidate/asp install binary --target /tmp/asp",
        "/tmp/candidate/asp install binary /state/runtime/bin/asp",
        "/tmp/candidate/asp install binary --target /state/runtime/bin/asp --force",
    ] {
        assert!(!exact_canonical_binary_install(
            &words(command),
            0,
            canonical,
        ));
    }
}

#[test]
fn bootstrap_rejects_non_hook_dispatch() {
    let valid = vec![OsString::from("hook"), OsString::from("stop")];
    assert_eq!(hook_event(&valid), Some("stop"));
    assert!(validate_hook_args(&valid).is_ok());
    let flags_before_event = vec![
        OsString::from("hook"),
        OsString::from("--client"),
        OsString::from("codex"),
        OsString::from("pre-tool"),
    ];
    assert_eq!(hook_event(&flags_before_event), Some("pre-tool"));
    assert!(validate_hook_args(&flags_before_event).is_ok());
    assert!(validate_hook_args(&[OsString::from("server"), OsString::from("stop")]).is_err());
}

#[test]
fn bootstrap_intercepts_events_but_not_lifecycle_diagnostics() {
    for args in [
        vec!["hook", "pre-tool"],
        vec!["hook", "event", "stop"],
        vec!["hook", "--client", "codex", "--event", "permission-request"],
    ] {
        assert!(
            is_hook_event_dispatch(args),
            "event dispatch must bootstrap"
        );
    }
    for args in [
        vec!["hook", "doctor"],
        vec!["hook", "paths"],
        vec!["hook", "--help"],
        vec!["server", "reconcile"],
    ] {
        assert!(
            !is_hook_event_dispatch(args),
            "non-event dispatch must use the ordinary CLI parser"
        );
    }
}

fn pre_tool(command: &str) -> (Vec<OsString>, Vec<u8>) {
    (
        vec![OsString::from("hook"), OsString::from("pre-tool")],
        serde_json::to_vec(&serde_json::json!({
            "tool_name": "Bash",
            "tool_input": { "cmd": command }
        }))
        .expect("hook payload"),
    )
}

#[test]
fn runtime_server_recovery_is_a_configuration_independent_escape_edge() {
    for command in [
        "asp server status",
        "asp server reconcile",
        "/tmp/runtime/bin/asp server restart",
        "direnv exec . asp server reconcile",
    ] {
        let (args, input) = pre_tool(command);
        assert!(
            hook_event_is_runtime_server_recovery(&args, &input),
            "{command}"
        );
    }
}

#[test]
fn recovery_kernel_rejects_chains_extra_arguments_and_non_control_commands() {
    for command in [
        "asp server reconcile && touch /tmp/escaped",
        "asp server reconcile --force",
        "cargo test",
        "other-asp server restart",
    ] {
        let (args, input) = pre_tool(command);
        assert!(
            !hook_event_is_runtime_server_recovery(&args, &input),
            "{command}"
        );
    }
}

#[test]
fn observational_events_cannot_claim_the_recovery_escape_edge() {
    let args = vec![OsString::from("hook"), OsString::from("stop")];
    let (_, input) = pre_tool("asp server reconcile");
    assert!(!hook_event_is_runtime_server_recovery(&args, &input));
}

#[test]
fn unavailable_resident_authority_is_typed_and_names_the_escape_edge() {
    let failure = runtime_server_hook_unavailable("pre-tool", "endpoint refused");
    let value: serde_json::Value = serde_json::from_str(&failure).expect("typed failure JSON");
    assert_eq!(
        value["schemaId"],
        "agent.semantic-protocols.hook-control-plane-unavailable.v1"
    );
    assert_eq!(
        value["reasonKind"],
        "runtime-server-hook-authority-unavailable"
    );
    assert_eq!(value["recoveryCommand"], "asp server reconcile");
    assert_eq!(value["recoveryCommands"].as_array().map(Vec::len), Some(2));
    assert!(value["canonicalBinaryInstallTarget"].is_string());
}
