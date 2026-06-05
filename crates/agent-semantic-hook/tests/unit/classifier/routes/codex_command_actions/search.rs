use agent_semantic_hook::{DecisionKind, DecisionRouteKind, ReasonKind, classify_hook};
use serde_json::json;

use crate::classifier::routes::registry_with_rust_and_python as registry;

#[test]
fn codex_search_command_action_routes_as_raw_search() {
    let decision = classify_hook(
        &registry(),
        "codex",
        "pre-tool",
        &json!({
            "toolName": "command_execution",
            "tool_name": "command_execution",
            "toolInput": {
                "commandActions": [{
                    "type": "search",
                    "command": "rg -n -g '*.rs' HookDecision crates/agent-semantic-hook/src",
                    "query": "HookDecision",
                    "path": "crates/agent-semantic-hook/src"
                }]
            },
            "tool_input": {
                "commandActions": [{
                    "type": "search",
                    "command": "rg -n -g '*.rs' HookDecision crates/agent-semantic-hook/src",
                    "query": "HookDecision",
                    "path": "crates/agent-semantic-hook/src"
                }]
            }
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Deny);
    assert_eq!(decision.reason_kind, ReasonKind::RawBroadSearch);
    assert_eq!(decision.routes[0].kind, DecisionRouteKind::Query);
}
