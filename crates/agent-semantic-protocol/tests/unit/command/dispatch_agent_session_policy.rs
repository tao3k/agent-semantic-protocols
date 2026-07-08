#[path = "../../../src/command/dispatch_agent_session_policy.rs"]
mod dispatch_agent_session_policy;

use dispatch_agent_session_policy::is_agent_session_control_json_command;

fn args(tokens: &[&str]) -> Vec<String> {
    tokens.iter().map(|token| (*token).to_string()).collect()
}

#[test]
fn agent_session_control_json_command_is_whitelisted() {
    let status_command = args(&[
        "agent",
        "session",
        "status",
        "--name",
        "asp-explore",
        "--json",
    ]);
    let guide_command = args(&["agent", "session", "register", "--guide"]);

    assert!(is_agent_session_control_json_command(&status_command));
    assert!(is_agent_session_control_json_command(&guide_command));
}

#[test]
fn non_agent_session_json_command_is_not_whitelisted() {
    let healthcheck_command = args(&["healthcheck", "--json", "."]);
    let query_command = args(&["rust", "query", "--json"]);

    assert!(!is_agent_session_control_json_command(&healthcheck_command));
    assert!(!is_agent_session_control_json_command(&query_command));
}
