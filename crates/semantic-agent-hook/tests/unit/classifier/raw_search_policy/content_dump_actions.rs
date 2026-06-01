use semantic_agent_hook::{DecisionKind, ReasonKind, classify_hook};
use serde_json::json;

use super::support::{assert_allowed, assert_content_dump_denied, polyglot_registry};

#[test]
fn content_dump_actions_block_source_filename_selectors() {
    for (command, binary) in [
        ("sed -n '1,20p' **/*.rs", "rs-harness"),
        ("less '**/*.{py,rs}'", "rs-harness"),
        ("head -n 20 'src/**/*.tsx'", "ts-harness"),
        ("tail -n 40 'packages/**/runner.py'", "py-harness"),
        ("nl -ba '**/*.[jt]s'", "ts-harness"),
        ("bat '**/*.py'", "py-harness"),
    ] {
        assert_content_dump_denied(command, binary);
    }
}

#[test]
fn content_dump_actions_do_not_block_directory_globs_without_source_suffix() {
    for command in ["less docs/**", "sed -n '1,20p' crates/**", "head -n 5 **/*"] {
        assert_allowed(command);
    }
}

#[test]
fn content_dump_action_with_multi_language_selector_routes_all_matches() {
    let decision = classify_hook(
        &polyglot_registry(),
        "codex",
        "pre-tool",
        &json!({
            "tool_name": "functions.exec_command",
            "tool_input": {"cmd": "less '**/*.{rs,py,js,ts}'"}
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Deny);
    assert_eq!(decision.reason_kind, ReasonKind::BulkSourceDump);
    assert_eq!(
        decision.language_ids,
        vec![
            "typescript".to_string(),
            "rust".to_string(),
            "python".to_string()
        ]
    );
}
