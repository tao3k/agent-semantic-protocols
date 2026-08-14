use crate::command::hook_runtime::is_child_self_registration_command;

#[test]
fn exact_child_self_registration_command_is_owned() {
    let payload = serde_json::json!({
        "tool_input": {"cmd": "/tmp/candidate/asp session register-current-child"}
    });
    assert!(is_child_self_registration_command(&payload));
}

#[test]
fn compound_or_argument_bearing_registration_commands_are_not_owned() {
    for command in [
        "asp session register-current-child --root forged",
        "asp session register-current-child; echo bypass",
        "asp session register-current-child | tee receipt",
        "other session register-current-child",
    ] {
        let payload = serde_json::json!({"tool_input": {"cmd": command}});
        assert!(!is_child_self_registration_command(&payload), "{command}");
    }
}
