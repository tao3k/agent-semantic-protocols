use agent_semantic_hook::{DecisionKind, DecisionRouteKind, classify_hook};
use serde_json::json;

use super::support::{
    assert_allowed, assert_direct_read_denied, document_registry, polyglot_registry,
};

#[test]
fn rtk_read_source_globs_are_denied_for_each_provider() {
    for (command, binary) in [
        ("rtk read -n *.rs", "rs-harness"),
        ("rtk read *.ts", "ts-harness"),
        ("rtk read '*.py'", "py-harness"),
        ("rtk read -n 'src/**/*.tsx'", "ts-harness"),
        ("rtk read 'packages/*/src/**/*.py'", "py-harness"),
        ("rtk read 'crates/**/lib.rs'", "rs-harness"),
        ("rtk read '*.[jt]s'", "ts-harness"),
        ("rtk read '*.{ts,py}'", "ts-harness"),
    ] {
        assert_direct_read_denied(command, binary);
    }
}

#[test]
fn rtk_read_brace_glob_targets_all_matching_language_providers() {
    let decision = classify_hook(
        &polyglot_registry(),
        "codex",
        "pre-tool",
        &json!({
            "tool_name": "functions.exec_command",
            "tool_input": {"cmd": "rtk read -n '*.{rs,py,js,ts}'"}
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Deny);
    assert_eq!(
        decision.language_ids,
        vec![
            "typescript".to_string(),
            "rust".to_string(),
            "python".to_string()
        ]
    );
    assert!(
        decision
            .routes
            .iter()
            .all(|route| { route.kind == DecisionRouteKind::Query })
    );
}

#[test]
fn rtk_read_scans_all_path_arguments() {
    for (command, binary) in [
        (
            "rtk read README.md crates/agent-semantic-hook/src/lib.rs",
            "rs-harness",
        ),
        (
            "rtk read README.md crates/agent-semantic-hook/src/lib.rs:10-20",
            "rs-harness",
        ),
        ("rtk read docs/guide.md src/cli/protocol.ts", "ts-harness"),
        (
            "rtk read README.md packages/python/src/tools/semantic_sandtable/runner.py",
            "py-harness",
        ),
    ] {
        assert_direct_read_denied(command, binary);
    }
}

#[test]
fn rtk_read_line_locator_routes_to_provider_query_with_range() {
    let decision = classify_hook(
        &polyglot_registry(),
        "codex",
        "pre-tool",
        &json!({
            "tool_name": "functions.exec_command",
            "tool_input": {"cmd": "rtk read crates/agent-semantic-hook/src/lib.rs:10-20"}
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Deny);
    assert_eq!(
        decision.subject.paths,
        [
            "crates/agent-semantic-hook/src/lib.rs:10-20",
            "crates/agent-semantic-hook/src/lib.rs"
        ]
    );
    assert_eq!(decision.routes[0].kind, DecisionRouteKind::Query);
    assert_eq!(
        decision.routes[0].argv,
        vec![
            "asp",
            "rust",
            "query",
            "--selector",
            "crates/agent-semantic-hook/src/lib.rs:10-20",
            "--workspace",
            ".",
            "--code",
        ]
    );
}

#[test]
fn rtk_read_whole_source_file_routes_to_selector_code_query() {
    let decision = classify_hook(
        &polyglot_registry(),
        "codex",
        "pre-tool",
        &json!({
            "tool_name": "functions.exec_command",
            "tool_input": {"cmd": "rtk read crates/agent-semantic-hook/src/lib.rs"}
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Deny);
    assert_eq!(decision.routes[0].kind, DecisionRouteKind::Query);
    assert_eq!(
        decision.routes[0].argv,
        vec![
            "asp",
            "rust",
            "query",
            "--selector",
            "crates/agent-semantic-hook/src/lib.rs",
            "--workspace",
            ".",
            "--code",
        ]
    );
}

#[test]
fn document_direct_read_routes_to_selector_content_query() {
    let decision = classify_hook(
        &document_registry(),
        "codex",
        "pre-tool",
        &json!({
            "tool_name": "functions.exec_command",
            "tool_input": {"cmd": "rtk read README.md:1-4"}
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Deny);
    assert_eq!(decision.routes[0].kind, DecisionRouteKind::Query);
    assert_eq!(
        decision.routes[0].argv,
        vec![
            "asp",
            "md",
            "query",
            "--selector",
            "README.md:1-4",
            "--workspace",
            ".",
            "--content"
        ]
    );
    assert!(
        !decision.routes[0]
            .argv
            .windows(2)
            .any(|pair| pair[0] == "--from-hook" && pair[1] == "direct-source-read"),
        "{:?}",
        decision.routes[0].argv
    );
}

#[test]
fn rtk_read_non_source_files_stays_allowed() {
    for command in ["rtk read README.md", "rtk read -n docs/*.org"] {
        assert_allowed(command);
    }
}

#[test]
fn rtk_read_source_globs_route_to_provider_query_not_owner() {
    let decision = classify_hook(
        &polyglot_registry(),
        "codex",
        "pre-tool",
        &json!({
            "tool_name": "functions.exec_command",
            "tool_input": {"cmd": "rtk read 'packages/*/src/**/*.py'"}
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Deny);
    assert_eq!(decision.routes[0].kind, DecisionRouteKind::Query);
    assert_eq!(
        decision.routes[0].argv,
        vec![
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
