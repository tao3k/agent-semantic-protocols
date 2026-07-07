use agent_semantic_hook::{DecisionKind, DecisionRouteKind, ReasonKind, classify_hook};
use serde_json::json;

use crate::classifier::registry;

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
    assert_eq!(
        decision.subject.paths,
        ["src/cli/agent-hooks.ts:1:8", "src/cli/agent-hooks.ts"]
    );
    assert_eq!(decision.routes[0].kind, DecisionRouteKind::Owner);
    assert_eq!(decision.routes[0].provider_id, "ts-harness");
    assert_eq!(
        decision.routes[0].argv,
        [
            "asp",
            "typescript",
            "search",
            "owner",
            "src/cli/agent-hooks.ts",
            "items",
            "--workspace",
            ".",
            "--view",
            "seeds"
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
    assert_eq!(decision.routes[0].kind, DecisionRouteKind::Owner);
    assert_eq!(decision.routes[0].provider_id, "ts-harness");
    assert_eq!(
        decision.routes[0].argv,
        [
            "asp",
            "typescript",
            "search",
            "owner",
            "languages/typescript-lang-project-harness/src/cli/semantic-search/item-query.ts",
            "items",
            "--workspace",
            ".",
            "--view",
            "seeds"
        ]
    );
}
