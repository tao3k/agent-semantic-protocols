use super::{AgentDispatchMessageFields, CHOICE_PLANE_COMMAND, render_choice_plane_instruction};

#[test]
fn choice_plane_message_carries_role_and_receipt_without_agent_identity() {
    let message = render_choice_plane_instruction(AgentDispatchMessageFields {
        role: "explore",
        receipt_kind: "asp-explore-search-v1",
    });

    assert!(message.contains(CHOICE_PLANE_COMMAND));
    assert!(message.contains("role `explore`"));
    assert!(message.contains("receipt `asp-explore-search-v1`"));
    assert!(!message.contains("@asp_"));
    assert!(!message.contains("resident"));
}

#[test]
fn testing_role_uses_the_same_choice_plane_contract() {
    let message = render_choice_plane_instruction(AgentDispatchMessageFields {
        role: "testing",
        receipt_kind: "asp-testing-execution-v1",
    });

    assert!(message.contains("role `testing`"));
    assert!(message.contains("receipt `asp-testing-execution-v1`"));
    assert!(message.contains("retrying the exact operation"));
}
