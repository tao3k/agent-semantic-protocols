use super::{
    event_is_observational, hook_event, hook_event_requires_policy_evaluation,
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
        "agent.semantic-protocols.hook-local-policy-unavailable"
    );
    assert_eq!(
        value["reasonKind"],
        "local-hook-policy-authority-unavailable"
    );
    assert!(
        value["recoveryCommand"]
            .as_str()
            .is_some_and(|command| command.contains("install binary"))
    );
    assert_eq!(value["recoveryCommands"].as_array().map(Vec::len), Some(1));
    assert!(
        value["recoveryCommands"][0]
            .as_str()
            .is_some_and(|command| command.contains("install binary"))
    );
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
                    .contains("agent.semantic-protocols.hook-local-policy-unavailable")),
            "{response}"
        );
    }
}
