use semantic_agent_hook::{DecisionKind, DecisionRouteKind, ReasonKind, classify_hook};
use serde_json::json;

use crate::classifier::registry;

#[test]
fn direct_read_routes_to_provider_query() {
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
            "."
        ]
    );
}

#[test]
fn namespaced_direct_read_routes_to_provider_query() {
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
            "src/components/WorkflowExecution.tsx",
            "."
        ]
    );
}

#[test]
fn command_transcript_with_source_path_routes_to_provider_query() {
    let decision = classify_hook(
        &super::super::registry_with_rust_and_python(),
        "codex",
        "pre-tool",
        &json!({
            "tool_name": "functions.exec_command",
            "tool_input": {
                "cmd": "Ran command\n$ DIRENV_SILENCE=1 DIRENV_LOG_FORMAT= direnv exec . rs-harness query --from-hook direct-source-read --selector packages/rust/crates/xiuxian-security/src/public_plane.rs ."
            }
        }),
    );
    assert_eq!(decision.decision, DecisionKind::Deny);
    assert_eq!(decision.reason_kind, ReasonKind::DirectSourceRead);
    assert_eq!(decision.routes[0].kind, DecisionRouteKind::Query);
    assert_eq!(decision.routes[0].provider_id, "rs-harness");
    assert_eq!(
        decision.routes[0].argv,
        [
            "rs-harness",
            "query",
            "--from-hook",
            "direct-source-read",
            "--selector",
            "packages/rust/crates/xiuxian-security/src/public_plane.rs",
            "."
        ]
    );
}
