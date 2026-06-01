use semantic_agent_hook::{
    DecisionKind, DecisionRouteKind, ProfileRegistry, ReasonKind, classify_hook, parse_profiles,
};
use serde_json::json;

use super::registry;

#[test]
fn direct_read_routes_to_owner_search() {
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
            "."
        ]
    );
}

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
}

#[test]
fn broad_raw_search_routes_to_ingest() {
    let decision = classify_hook(
        &registry(),
        "codex",
        "pre-tool",
        &json!({
            "tool_name": "functions.exec_command",
            "tool_input": {"cmd": "rg -n WorkflowExecution src tests"}
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Deny);
    assert_eq!(decision.reason_kind, ReasonKind::RawBroadSearch);
    assert_eq!(decision.routes[0].kind, DecisionRouteKind::Ingest);
}

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
fn rtk_read_routes_to_owner_search() {
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
            "."
        ]
    );
}

#[test]
fn nested_parallel_exec_command_routes_to_owner_search() {
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
            "."
        ]
    );
}

#[test]
fn content_dump_file_extension_beats_shared_source_root() {
    let decision = classify_hook(
        &registry_with_python(),
        "codex",
        "pre-tool",
        &json!({
            "tool_name": "functions.exec_command",
            "tool_input": {"cmd": "sed -n '1,80p' src/tools/semantic_sandtable/receipt_reports.py"}
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Deny);
    assert_eq!(decision.reason_kind, ReasonKind::BulkSourceDump);
    assert_eq!(decision.language_ids, ["python"]);
    assert_eq!(decision.routes[0].provider_id, "py-harness");
    assert_eq!(
        decision.routes[0].argv,
        [
            "py-harness",
            "search",
            "owner",
            "src/tools/semantic_sandtable/receipt_reports.py",
            "."
        ]
    );
}

fn registry_with_python() -> ProfileRegistry {
    let mut value = super::registry_value();
    value["profiles"].as_array_mut().unwrap().push(json!({
        "languageId": "python",
        "providerId": "py-harness",
        "binary": "py-harness",
        "namespace": "agent.semantic-protocols.languages.python.py-harness",
        "sourceExtensions": [".py", ".pyi"],
        "configFiles": ["pyproject.toml"],
        "sourceRoots": ["src", "tests"],
        "ignoredPathPrefixes": [".venv", "__pycache__"],
        "commands": {
            "prime": {"argv": ["py-harness", "search", "prime", "."]},
            "owner": {"argv": ["py-harness", "search", "owner", "{path}", "."]},
            "text": {"argv": ["py-harness", "search", "text", "{query}", "owner", "tests", "--view", "seeds", "."]},
            "ingest": {"argv": ["py-harness", "search", "ingest", "."], "stdinMode": "pipe-candidates"},
            "checkChanged": {"argv": ["py-harness", "check", "--changed", "."]}
        }
    }));
    parse_profiles(&value.to_string()).unwrap()
}
