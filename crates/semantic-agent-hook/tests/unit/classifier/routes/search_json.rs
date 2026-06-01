use semantic_agent_hook::{DecisionKind, ReasonKind, classify_hook};
use serde_json::json;

use crate::classifier::registry;

#[test]
fn search_json_routes_to_compact_search() {
    let decision = classify_hook(
        &registry(),
        "codex",
        "pre-tool",
        &json!({
            "tool_name": "functions.exec_command",
            "tool_input": {"cmd": "ts-harness search text projectRoot owner tests --json ."}
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Deny);
    assert_eq!(decision.reason_kind, ReasonKind::AgentSearchJson);
    assert_eq!(
        decision.routes[0].argv,
        [
            "ts-harness",
            "search",
            "text",
            "projectRoot",
            "owner",
            "tests",
            "--view",
            "seeds",
            "."
        ]
    );
    assert_eq!(
        decision.message,
        "agent-search-json denied; provider guide: ts-harness => ts-harness agent guide ."
    );
}
