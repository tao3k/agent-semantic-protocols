use super::{AgentDispatchMessageFields, CHOICE_PLANE_COMMAND, render_choice_plane_instruction};

#[test]
fn choice_plane_message_carries_agent_route_and_receipt() {
    let message = render_choice_plane_instruction(AgentDispatchMessageFields {
        agent: "asp_explorer",
        symbol: Some("@asp_explorer"),
        receipt_kind: "asp-explore-search-v1",
    });

    assert!(message.contains(CHOICE_PLANE_COMMAND));
    assert!(message.contains("`@asp_explorer`"));
    assert!(message.contains("registered Agent `asp_explorer`"));
    assert!(message.contains("receipt `asp-explore-search-v1`"));
    assert!(!message.contains("resident"));
}

#[test]
fn testing_agent_uses_the_same_choice_plane_contract() {
    let message = render_choice_plane_instruction(AgentDispatchMessageFields {
        agent: "asp_testing",
        symbol: Some("@agent-asp-testing"),
        receipt_kind: "asp-testing-execution-v1",
    });

    assert!(message.contains("`@agent-asp-testing`"));
    assert!(message.contains("receipt `asp-testing-execution-v1`"));
    assert!(message.contains("retrying the exact operation"));
}
