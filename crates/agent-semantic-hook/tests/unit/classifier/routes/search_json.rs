use agent_semantic_hook::{DecisionKind, DecisionRouteKind, ReasonKind, classify_hook};
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
            "tool_input": {"cmd": "ts-harness search lexical projectRoot owner tests --json ."}
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Deny);
    assert_eq!(decision.reason_kind, ReasonKind::AgentSearchJson);
    assert_eq!(
        decision.fields.get("configRuleId"),
        Some(&json!("deny-agent-search-json"))
    );
    assert_eq!(decision.routes[0].kind, DecisionRouteKind::Lexical);
    assert_eq!(
        decision.routes[0].argv,
        [
            "asp",
            "typescript",
            "search",
            "lexical",
            "projectRoot",
            "owner",
            "tests",
            "--workspace",
            ".",
            "--view",
            "seeds"
        ]
    );
    assert_eq!(
        decision.message,
        "agent-search-json denied; route: asp typescript search lexical projectRoot owner tests --workspace . --view seeds"
    );
}

#[test]
fn search_json_owner_routes_to_provider_owner_query() {
    let decision = classify_hook(
        &registry(),
        "codex",
        "pre-tool",
        &json!({
            "tool_name": "functions.exec_command",
            "tool_input": {"cmd": "ts-harness search owner src/cli/agent-hooks.ts items --query agent-hooks --json ."}
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Deny);
    assert_eq!(decision.reason_kind, ReasonKind::AgentSearchJson);
    assert_eq!(
        decision.fields.get("configRuleId"),
        Some(&json!("deny-agent-search-json"))
    );
    assert_eq!(decision.routes[0].kind, DecisionRouteKind::Owner);
    assert_eq!(
        decision.routes[0].argv,
        [
            "asp",
            "typescript",
            "search",
            "owner",
            "src/cli/agent-hooks.ts",
            "--workspace",
            ".",
            "--view",
            "seeds"
        ]
    );
    assert_eq!(
        decision.message,
        "agent-search-json denied; route: asp typescript search owner src/cli/agent-hooks.ts --workspace . --view seeds"
    );
}

#[test]
fn patch_text_with_search_json_example_is_not_a_command() {
    let patch_body = concat!(
        "*** Begin Patch\n",
        "*** Update File: docs/example.org\n",
        "+",
        "ts-",
        "harness search lexical projectRoot owner tests ",
        "--",
        "json .\n",
        "*** End Patch\n"
    );
    let decision = classify_hook(
        &registry(),
        "codex",
        "pre-tool",
        &json!({
            "tool_name": "functions.apply_patch",
            "tool_input": {"cmd": patch_body}
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Allow);
}
