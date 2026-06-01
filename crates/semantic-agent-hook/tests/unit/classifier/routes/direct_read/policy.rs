use semantic_agent_hook::{DecisionKind, ReasonKind, classify_hook, parse_profiles};
use serde_json::json;

use crate::classifier::registry_value;

#[test]
fn action_policy_can_allow_direct_source_read_without_disabling_other_policies() {
    let mut value = registry_value();
    value["profiles"][0]["policy"]["directSourceRead"] = json!("allow");
    let registry = parse_profiles(&value.to_string()).unwrap();

    let read_decision = classify_hook(
        &registry,
        "codex",
        "pre-tool",
        &json!({
            "tool_name": "Read",
            "tool_input": {"path": "src/cli/agent-hooks.ts"}
        }),
    );
    let search_decision = classify_hook(
        &registry,
        "codex",
        "pre-tool",
        &json!({
            "tool_name": "functions.exec_command",
            "tool_input": {"cmd": "rg -n WorkflowExecution src"}
        }),
    );

    assert_eq!(read_decision.decision, DecisionKind::Allow);
    assert_eq!(search_decision.decision, DecisionKind::Deny);
    assert_eq!(search_decision.reason_kind, ReasonKind::RawBroadSearch);
}
