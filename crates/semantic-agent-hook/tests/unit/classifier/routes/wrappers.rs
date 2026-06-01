use semantic_agent_hook::{DecisionKind, DecisionRouteKind, ReasonKind, classify_hook};
use serde_json::json;

use crate::classifier::registry;

#[test]
fn wrapper_raw_search_routes_to_ingest() {
    for command in [
        "DIRENV_SILENCE=1 direnv exec . rg -n WorkflowExecution src",
        "direnv exec . rg -n WorkflowExecution src",
        "env CODEX=1 rg -n WorkflowExecution src",
        "rtk --ultra-compact rg -n WorkflowExecution src",
        "rtk proxy rg -n WorkflowExecution src",
        "rtk run -c 'rg -n WorkflowExecution src'",
        "uv run --with ./languages/typescript-lang-project-harness rg -n WorkflowExecution src",
        "/usr/bin/rg -n WorkflowExecution src",
    ] {
        let decision = classify_hook(
            &registry(),
            "codex",
            "pre-tool",
            &json!({
                "tool_name": "functions.exec_command",
                "tool_input": {"cmd": command}
            }),
        );

        assert_eq!(decision.decision, DecisionKind::Deny, "{command}");
        assert_eq!(decision.reason_kind, ReasonKind::RawBroadSearch);
        assert_eq!(decision.routes[0].kind, DecisionRouteKind::Ingest);
    }
}

#[test]
fn shell_path_wrapper_routes_content_dump_to_provider_owner_search() {
    let decision = classify_hook(
        &registry(),
        "codex",
        "pre-tool",
        &json!({
            "tool_name": "command_execution",
            "tool_input": {"command": "/bin/zsh -lc \"sed -n '1,8p' src/cli/agent-hooks.ts\""}
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Deny);
    assert_eq!(decision.reason_kind, ReasonKind::BulkSourceDump);
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
fn rtk_read_routes_to_provider_owner_search() {
    let decision = classify_hook(
        &registry(),
        "codex",
        "pre-tool",
        &json!({
            "tool_name": "functions.exec_command",
            "tool_input": {"cmd": "rtk read src/cli/agent-hooks.ts"}
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Deny);
    assert_eq!(decision.reason_kind, ReasonKind::DirectSourceRead);
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
fn nested_parallel_exec_command_routes_to_provider_owner_search() {
    let decision = classify_hook(
        &registry(),
        "codex",
        "pre-tool",
        &json!({
            "tool_name": "multi_tool_use.parallel",
            "tool_input": {
                "tool_uses": [{
                    "recipient_name": "functions.exec_command",
                    "parameters": {"cmd": "rtk read src/cli/agent-hooks.ts"}
                }]
            }
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Deny);
    assert_eq!(decision.reason_kind, ReasonKind::DirectSourceRead);
    assert_eq!(
        decision.subject.tool_name.as_deref(),
        Some("functions.exec_command")
    );
    assert_eq!(
        decision.subject.command.as_deref(),
        Some("rtk read src/cli/agent-hooks.ts")
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
