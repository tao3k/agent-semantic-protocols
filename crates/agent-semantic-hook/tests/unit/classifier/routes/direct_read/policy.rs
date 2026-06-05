use agent_semantic_hook::{ActionPolicy, DecisionKind, ReasonKind, classify_hook};
use serde_json::json;

use crate::classifier::registry;

#[test]
fn action_policy_can_allow_direct_source_read_without_disabling_other_policies() {
    let mut registry = registry();
    registry.providers[0].policy.direct_source_read = ActionPolicy::Allow;

    let read_decision = classify_hook(
        &registry,
        "codex",
        "pre-tool",
        &json!({ "tool_name": "Read", "tool_input": { "path": "src/cli/agent-hooks.ts" } }),
    );
    let search_decision = classify_hook(
        &registry,
        "codex",
        "pre-tool",
        &json!({
            "tool_name": "functions.exec_command",
            "tool_input": { "cmd": "rg -n -g '*.ts' WorkflowExecution src" }
        }),
    );

    assert_eq!(read_decision.decision, DecisionKind::Allow);
    assert_eq!(search_decision.decision, DecisionKind::Deny);
    assert_eq!(search_decision.reason_kind, ReasonKind::RawBroadSearch);
}
