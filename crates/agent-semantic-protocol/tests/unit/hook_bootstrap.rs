use super::{
    event_is_observational, exact_canonical_binary_install, hook_event,
    hook_event_is_canonical_recovery, hook_event_requires_policy_evaluation,
    is_hook_event_dispatch, local_hook_policy_unavailable, local_hook_policy_unavailable_deny,
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
fn canonical_recovery_is_a_configuration_independent_escape_edge() {
    for command in [
        "asp server status",
        "asp server reconcile",
        "/tmp/runtime/bin/asp server restart",
        "direnv exec . asp server reconcile",
        "asp hook doctor --client codex",
        "direnv exec . asp hook doctor --client codex",
    ] {
        let (args, input) = pre_tool(command);
        assert!(hook_event_is_canonical_recovery(&args, &input), "{command}");
    }
}

#[test]
fn recovery_kernel_rejects_chains_extra_arguments_and_non_control_commands() {
    for command in [
        "asp server reconcile && touch /tmp/escaped",
        "asp server reconcile --force",
        "asp hook doctor --client claude",
        "asp hook doctor --client codex --json",
        "cargo test",
        "other-asp server restart",
    ] {
        let (args, input) = pre_tool(command);
        assert!(
            !hook_event_is_canonical_recovery(&args, &input),
            "{command}"
        );
    }
}

#[test]
fn observational_events_cannot_claim_the_recovery_escape_edge() {
    let args = vec![OsString::from("hook"), OsString::from("stop")];
    let (_, input) = pre_tool("asp server reconcile");
    assert!(!hook_event_is_canonical_recovery(&args, &input));
}

#[test]
fn unrelated_codex_actions_never_enter_the_runtime_server_path() {
    for tool_name in ["update_plan", "view_image", "collaboration.send_message"] {
        let input = serde_json::to_vec(&serde_json::json!({
            "tool_name": tool_name,
            "tool_input": {"value": "not a source or command action"}
        }))
        .expect("Hook payload");
        for event in ["pre-tool", "permission-request", "post-tool"] {
            assert!(
                !hook_event_requires_policy_evaluation(event, &input)
                    .expect("local action classification"),
                "{event} {tool_name} must be an in-process passthrough"
            );
        }
    }
}

#[test]
fn policy_bearing_codex_actions_enter_the_local_typed_evaluator() {
    for (tool_name, tool_input) in [
        ("Read", serde_json::json!({"path": "src/lib.rs"})),
        (
            "Bash",
            serde_json::json!({"command": "sed -n '1,20p' src/lib.rs"}),
        ),
        (
            "functions.exec",
            serde_json::json!({"code": "await tools.exec_command({cmd: 'cargo test'})"}),
        ),
    ] {
        let input = serde_json::to_vec(&serde_json::json!({
            "tool_name": tool_name,
            "tool_input": tool_input
        }))
        .expect("Hook payload");
        assert!(
            hook_event_requires_policy_evaluation("pre-tool", &input)
                .expect("local action classification"),
            "{tool_name} must reach typed ASP policy"
        );
    }
}

#[test]
fn lifecycle_and_malformed_tool_events_cannot_bypass_typed_policy() {
    let lifecycle = br#"{"cwd":"/tmp/workspace"}"#;
    assert!(hook_event_requires_policy_evaluation("session-start", lifecycle).unwrap());
    assert!(hook_event_requires_policy_evaluation("pre-tool", lifecycle).unwrap());
}

#[test]
fn unavailable_local_policy_authority_is_typed_and_names_the_escape_edge() {
    let failure = local_hook_policy_unavailable("pre-tool", "config invalid");
    let value: serde_json::Value = serde_json::from_str(&failure).expect("typed failure JSON");
    assert_eq!(
        value["schemaId"],
        "agent.semantic-protocols.hook-local-policy-unavailable.v1"
    );
    assert_eq!(
        value["reasonKind"],
        "local-hook-policy-authority-unavailable"
    );
    assert!(
        value["recoveryCommand"]
            .as_str()
            .is_some_and(|command| command.contains("install binary --target"))
    );
    assert_eq!(value["recoveryCommands"].as_array().map(Vec::len), Some(2));
    assert_eq!(
        value["recoveryCommands"][0],
        "asp hook doctor --client codex"
    );
    assert_eq!(value["recoveryCommands"][1], value["recoveryCommand"]);
    assert!(value["canonicalBinaryInstallTarget"].is_string());
}

#[test]
fn unavailable_enforcement_authority_is_a_host_protocol_deny() {
    for (event, host_event) in [
        ("pre-tool", "PreToolUse"),
        ("permission-request", "PermissionRequest"),
    ] {
        let response = local_hook_policy_unavailable_deny(event, "config invalid");
        let value: serde_json::Value =
            serde_json::from_str(&response).expect("typed unavailable deny JSON");
        assert_eq!(value["hookSpecificOutput"]["hookEventName"], host_event);
        assert_eq!(value["hookSpecificOutput"]["permissionDecision"], "deny");
        assert!(
            value["hookSpecificOutput"]["additionalContext"]
                .as_str()
                .is_some_and(|context| context
                    .contains("agent.semantic-protocols.hook-local-policy-unavailable.v1")),
            "{response}"
        );
    }
}
