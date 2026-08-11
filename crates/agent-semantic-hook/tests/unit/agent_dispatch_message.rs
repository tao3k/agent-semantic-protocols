#[path = "../../src/agent_dispatch_message.rs"]
mod subject;

use subject::{AgentDispatchMessageFields, CHOICE_PLANE_COMMAND, render_choice_plane_instruction};

#[test]
fn explorer_message_keeps_only_the_native_call_target_in_identity_backticks() {
    let rendered = render_choice_plane_instruction(AgentDispatchMessageFields {
        agent_kind: "Subagent",
        call_target: "@asp_explorer",
        role: "Evidence Explorer",
        description: "for code and evidence search",
    });

    assert_eq!(
        rendered,
        "Please use `asp session --agents choice-plane` to create or resume the Subagent `@asp_explorer` (Evidence Explorer; for code and evidence search)."
    );
}

#[test]
fn testing_message_uses_the_same_four_slot_grammar() {
    let rendered = render_choice_plane_instruction(AgentDispatchMessageFields {
        agent_kind: "Subagent",
        call_target: "@asp_testing",
        role: "Test Runner",
        description: "for build and test jobs",
    });

    assert_eq!(
        rendered,
        "Please use `asp session --agents choice-plane` to create or resume the Subagent `@asp_testing` (Test Runner; for build and test jobs)."
    );
    assert_eq!(CHOICE_PLANE_COMMAND, "asp session --agents choice-plane");
}

#[test]
fn native_call_target_is_canonicalized_with_one_at_sign() {
    let rendered = render_choice_plane_instruction(AgentDispatchMessageFields {
        agent_kind: "Subagent",
        call_target: "asp_explorer",
        role: "Evidence Explorer",
        description: "for code and evidence search",
    });

    assert_eq!(
        rendered,
        "Please use `asp session --agents choice-plane` to create or resume the Subagent `@asp_explorer` (Evidence Explorer; for code and evidence search)."
    );
}

#[test]
fn provider_identity_never_changes_the_subagent_message_grammar() {
    let agent = AgentDispatchMessageFields {
        agent_kind: "Subagent",
        call_target: "@asp_explorer",
        role: "Evidence Explorer",
        description: "for code and evidence search",
    };
    let expected = render_choice_plane_instruction(agent);

    for (provider_id, language_id) in [
        ("rs-harness", "rust"),
        ("ts-harness", "typescript"),
        ("py-harness", "python"),
        ("julia-lang-project-harness", "julia"),
        ("gslph", "gerbil-scheme"),
    ] {
        assert_eq!(render_choice_plane_instruction(agent), expected);
        assert!(!expected.contains(provider_id));
        assert!(!expected.contains(language_id));
    }
}
