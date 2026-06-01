use semantic_agent_hook::{DecisionKind, DecisionRouteKind, ReasonKind, classify_hook};
use serde_json::json;

use crate::classifier::routes::registry_with_rust_and_python;

#[test]
fn camelcase_namespaced_direct_read_glob_routes_to_prime() {
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
        ["ts-harness", "search", "prime", "."]
    );
}

#[test]
fn mcp_direct_read_glob_routes_to_profile_prime() {
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
        ["py-harness", "search", "prime", "."]
    );
}

#[test]
fn command_direct_read_source_glob_routes_to_profile_prime() {
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
    assert_eq!(decision.routes[0].kind, DecisionRouteKind::Prime);
    assert_eq!(
        decision.routes[0].argv,
        ["rs-harness", "search", "prime", "."]
    );
}
