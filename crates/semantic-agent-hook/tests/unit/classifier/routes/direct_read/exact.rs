use semantic_agent_hook::{DecisionKind, ReasonKind, classify_hook};
use serde_json::json;

use crate::classifier::registry;

#[test]
fn direct_read_routes_to_provider_owner_search() {
    let decision = classify_hook(
        &registry(),
        "codex",
        "pre-tool",
        &json!({
            "tool_name": "Read",
            "tool_input": {"path": "src/cli/agent-hooks.ts"}
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Deny);
    assert_eq!(decision.reason_kind, ReasonKind::DirectSourceRead);
    assert_eq!(
        decision.routes[0].argv,
        [
            "ts-harness",
            "search",
            "owner",
            "src/cli/agent-hooks.ts",
            "items",
            "--query",
            "agent-hooks|AgentHooks|agentHooks",
            "."
        ]
    );
}

#[test]
fn namespaced_direct_read_routes_to_provider_owner_search() {
    let decision = classify_hook(
        &registry(),
        "codex",
        "pre-tool",
        &json!({
            "toolName": "functions.read_file",
            "toolInput": {"path": "src/cli/agent-hooks.ts"}
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Deny);
    assert_eq!(decision.reason_kind, ReasonKind::DirectSourceRead);
    assert_eq!(
        decision.subject.tool_name.as_deref(),
        Some("functions.read_file")
    );
    assert_eq!(decision.subject.paths, ["src/cli/agent-hooks.ts"]);
    assert_eq!(
        decision.routes[0].argv,
        [
            "ts-harness",
            "search",
            "owner",
            "src/cli/agent-hooks.ts",
            "items",
            "--query",
            "agent-hooks|AgentHooks|agentHooks",
            "."
        ]
    );
}

#[test]
fn typescript_direct_read_infers_pascal_case_query_from_component_path() {
    let decision = classify_hook(
        &registry(),
        "codex",
        "pre-tool",
        &json!({
            "tool_name": "Read",
            "tool_input": {"path": "src/components/WorkflowExecution.tsx"}
        }),
    );
    assert_eq!(decision.decision, DecisionKind::Deny);
    assert_eq!(decision.reason_kind, ReasonKind::DirectSourceRead);
    assert_eq!(
        decision.routes[0].argv,
        [
            "ts-harness",
            "search",
            "owner",
            "src/components/WorkflowExecution.tsx",
            "items",
            "--query",
            "WorkflowExecution|workflowExecution",
            "."
        ]
    );
}
