use semantic_agent_hook::{DecisionKind, DecisionRouteKind, ReasonKind, classify_hook};
use serde_json::json;

use crate::classifier::registry;

#[test]
fn wrapper_raw_search_routes_to_provider_query_when_supported() {
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
        assert_eq!(decision.routes[0].kind, DecisionRouteKind::Query);
    }
}

#[test]
fn shell_path_wrapper_routes_content_dump_to_provider_query() {
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
    assert_eq!(decision.routes[0].kind, DecisionRouteKind::Query);
    assert_eq!(decision.routes[0].provider_id, "ts-harness");
    assert_eq!(
        decision.routes[0].argv,
        [
            "ts-harness",
            "query",
            "--from-hook",
            "direct-source-read",
            "--selector",
            "src/cli/agent-hooks.ts",
            "--code",
            "."
        ]
    );
}

#[test]
fn node_eval_read_file_sync_routes_content_dump_to_provider_query() {
    let command = concat!(
        "node -e \"const fs=require('fs'); ",
        "const f='languages/typescript-lang-project-harness/src/cli/semantic-search/item-query.ts'; ",
        "const lines=fs.readFileSync(f,'utf8').split('\\\\n'); ",
        "for (let i=453;i<=505;i++) console.log(String(i).padStart(4), lines[i-1]);\"",
    );
    let decision = classify_hook(
        &registry(),
        "codex",
        "pre-tool",
        &json!({
            "tool_name": "functions.exec_command",
            "tool_input": {"cmd": command}
        }),
    );
    assert_eq!(decision.decision, DecisionKind::Deny);
    assert_eq!(decision.reason_kind, ReasonKind::BulkSourceDump);
    assert_eq!(
        decision.subject.paths,
        ["languages/typescript-lang-project-harness/src/cli/semantic-search/item-query.ts"]
    );
    assert_eq!(decision.routes[0].kind, DecisionRouteKind::Query);
    assert_eq!(decision.routes[0].provider_id, "ts-harness");
    assert_eq!(
        decision.routes[0].argv,
        [
            "ts-harness",
            "query",
            "--from-hook",
            "direct-source-read",
            "--selector",
            "languages/typescript-lang-project-harness/src/cli/semantic-search/item-query.ts",
            "--code",
            "."
        ]
    );
}

#[test]
fn rtk_read_routes_to_provider_query() {
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
    assert_eq!(decision.routes[0].kind, DecisionRouteKind::Query);
    assert_eq!(decision.routes[0].provider_id, "ts-harness");
    assert_eq!(
        decision.routes[0].argv,
        [
            "ts-harness",
            "query",
            "--from-hook",
            "direct-source-read",
            "--selector",
            "src/cli/agent-hooks.ts",
            "--code",
            "."
        ]
    );
}

#[test]
fn rtk_read_routes_display_locator_to_clean_provider_selector() {
    let decision = classify_hook(
        &registry(),
        "codex",
        "pre-tool",
        &json!({
            "tool_name": "functions.exec_command",
            "tool_input": {"cmd": "rtk read 0:src/cli/agent-hooks.ts:190\u{2013}330"}
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Deny);
    assert_eq!(decision.reason_kind, ReasonKind::DirectSourceRead);
    assert_eq!(decision.routes[0].kind, DecisionRouteKind::Query);
    assert_eq!(decision.routes[0].provider_id, "ts-harness");
    assert_eq!(
        decision.routes[0].argv,
        [
            "ts-harness",
            "query",
            "--from-hook",
            "direct-source-read",
            "--selector",
            "src/cli/agent-hooks.ts",
            "--code",
            "."
        ]
    );
}

#[test]
fn nested_parallel_exec_command_routes_to_provider_query() {
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
    assert_eq!(decision.routes[0].kind, DecisionRouteKind::Query);
    assert_eq!(decision.routes[0].provider_id, "ts-harness");
    assert_eq!(
        decision.routes[0].argv,
        [
            "ts-harness",
            "query",
            "--from-hook",
            "direct-source-read",
            "--selector",
            "src/cli/agent-hooks.ts",
            "--code",
            "."
        ]
    );
}
