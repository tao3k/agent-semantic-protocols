use agent_semantic_hook::{DecisionKind, DecisionRouteKind, ReasonKind, classify_hook};
use serde_json::json;

use crate::classifier::routes::registry_with_rust_and_python;

#[test]
fn camelcase_namespaced_direct_read_glob_routes_to_provider_query() {
    let decision = classify_hook(
        &registry_with_rust_and_python(),
        "codex",
        "pre-tool",
        &json!({
            "toolName": "functions.read_file",
            "toolInput": {"filePath": "src/**/*.tsx"}
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Deny);
    assert_eq!(decision.reason_kind, ReasonKind::DirectSourceRead);
    assert_eq!(
        decision.subject.tool_name.as_deref(),
        Some("functions.read_file")
    );
    assert_eq!(decision.subject.paths, ["src/**/*.tsx"]);
    assert_eq!(
        decision.routes[0].argv,
        [
            "asp",
            "typescript",
            "query",
            "--selector",
            "src/**/*.tsx",
            "--surface",
            "owners,tests",
            "--workspace",
            ".",
            "--view",
            "seeds"
        ]
    );
}

#[test]
fn desktop_read_aliases_route_source_paths() {
    for tool_name in ["readFile", "FsReadFile", "fs/readFile", "fs.readFile"] {
        let decision = classify_hook(
            &registry_with_rust_and_python(),
            "codex",
            "pre-tool",
            &json!({
                "tool_name": tool_name,
                "tool_input": {"path": "crates/agent-semantic-protocol/src/command/ast_patch.rs"}
            }),
        );

        assert_eq!(decision.decision, DecisionKind::Deny, "{tool_name}");
        assert_eq!(
            decision.reason_kind,
            ReasonKind::DirectSourceRead,
            "{tool_name}"
        );
        assert_eq!(decision.language_ids, ["rust".to_string()], "{tool_name}");
    }
}

#[test]
fn mcp_direct_read_glob_routes_to_provider_query() {
    let decision = classify_hook(
        &registry_with_rust_and_python(),
        "codex",
        "pre-tool",
        &json!({
            "tool_name": "mcp__filesystem__read_file",
            "tool_input": {"file_path": "packages/*/src/**/*.py"}
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Deny);
    assert_eq!(decision.reason_kind, ReasonKind::DirectSourceRead);
    assert_eq!(decision.language_ids, ["python".to_string()]);
    assert_eq!(
        decision.routes[0].argv,
        [
            "asp",
            "python",
            "query",
            "--selector",
            "packages/*/src/**/*.py",
            "--surface",
            "owners,tests",
            "--workspace",
            ".",
            "--view",
            "seeds"
        ]
    );
}

#[test]
fn command_direct_read_source_glob_routes_to_provider_query() {
    let decision = classify_hook(
        &registry_with_rust_and_python(),
        "codex",
        "pre-tool",
        &json!({
            "tool_name": "functions.exec_command",
            "tool_input": {"cmd": "rtk read *.rs"}
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Deny);
    assert_eq!(decision.reason_kind, ReasonKind::DirectSourceRead);
    assert_eq!(decision.subject.command.as_deref(), Some("rtk read *.rs"));
    assert_eq!(decision.subject.paths, ["*.rs"]);
    assert_eq!(decision.language_ids, ["rust".to_string()]);
    assert_eq!(decision.routes[0].kind, DecisionRouteKind::Query);
    assert_eq!(
        decision.routes[0].argv,
        [
            "asp",
            "rust",
            "query",
            "--selector",
            "*.rs",
            "--surface",
            "owners,tests",
            "--workspace",
            ".",
            "--view",
            "seeds"
        ]
    );
}
