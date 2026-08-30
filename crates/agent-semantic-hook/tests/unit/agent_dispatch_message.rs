use super::render_collaboration_instruction;

#[test]
fn collaboration_message_names_the_configured_agent_and_native_tool() {
    let message = render_collaboration_instruction(Some("asp_explorer"));

    assert!(message.contains("collaboration.spawn_agent"));
    assert!(message.contains("collaboration.list_agents"));
    assert!(message.contains("agent_type: \"asp_explorer\""));
    assert!(message.contains("task_name: \"asp_explorer\""));
    assert!(message.contains("/root/asp_explorer"));
    assert!(message.contains("collaboration.list_agents({\n  path_prefix: \"/root\"\n})"));
    assert!(message.contains("collaboration.followup_task({"));
    assert!(message.contains("collaboration.send_message({"));
    assert!(message.contains("standardized JSON"));
    assert!(message.contains("Host result remains the lifecycle authority"));
    assert!(message.contains("diagnostic live-Agent snapshot"));
    assert!(message.contains("start a new turn on that same canonical Agent path"));
    assert!(message.contains("without starting a second turn"));
    assert!(message.contains("do not invent an Agent path"));
    assert!(!message.contains("{{{"));
    assert!(!message.contains("}}}"));
    assert!(!message.contains("choice-plane"));
    assert!(!message.contains("self-registration"));
}
