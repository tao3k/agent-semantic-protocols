use agent_semantic_hook::{DecisionKind, ReasonKind, classify_hook};
use serde_json::json;

use super::support::{prefixed_registry, registry};

#[test]
fn provider_output_filtering_is_allowed() {
    for command in [
        "rs-harness search prime . | rg -g '*.rs' WorkflowExecution",
        "py-harness search prime . | rg -g '*.py' Session",
        "env | rg PATH",
        "printenv | rg '^PATH='",
    ] {
        let decision = classify_hook(
            &registry(),
            "codex",
            "pre-tool",
            &json!({ "tool_name": "functions.exec_command", "tool_input": { "cmd": command } }),
        );
        assert_eq!(decision.decision, DecisionKind::Allow, "{command}");
        assert_eq!(decision.reason_kind, ReasonKind::None, "{command}");
    }

    let source_listing = classify_hook(
        &registry(),
        "codex",
        "pre-tool",
        &json!({
            "tool_name": "functions.exec_command",
            "tool_input": {"cmd": "fd -t f shell crates/agent-semantic-hook"}
        }),
    );
    assert_eq!(source_listing.decision, DecisionKind::Deny);
    assert_eq!(source_listing.reason_kind, ReasonKind::RawBroadSearch);
}

#[test]
fn prefixed_provider_output_filtering_requires_full_command_prefix() {
    let allowed = "python -m tools.fake_provider search prime . | rg -g '*.py' Session";
    let allowed_decision = classify_hook(
        &prefixed_registry(),
        "codex",
        "pre-tool",
        &json!({ "tool_name": "functions.exec_command", "tool_input": { "cmd": allowed } }),
    );
    assert_eq!(allowed_decision.decision, DecisionKind::Allow);
    assert_eq!(allowed_decision.reason_kind, ReasonKind::None);

    let denied = "tools.fake_provider search prime . | rg -g '*.py' Session";
    let denied_decision = classify_hook(
        &prefixed_registry(),
        "codex",
        "pre-tool",
        &json!({ "tool_name": "functions.exec_command", "tool_input": { "cmd": denied } }),
    );
    assert_eq!(denied_decision.decision, DecisionKind::Deny);
    assert_eq!(denied_decision.reason_kind, ReasonKind::RawBroadSearch);
}
