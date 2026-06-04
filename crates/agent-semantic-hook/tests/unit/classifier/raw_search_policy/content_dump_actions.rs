use agent_semantic_hook::{DecisionKind, ReasonKind, classify_hook};
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
fn sed_content_dump_routes_source_suffixes_across_languages() {
    for (command, binary) in [
        ("sed -n '1,20p' src/lib.rs", "rs-harness"),
        ("sed -n '1,20p' src/cli/main.ts", "ts-harness"),
        ("sed -n '1,20p' src/cli/main.js", "ts-harness"),
        (
            "sed -n '1,20p' packages/python/src/tools/semantic_sandtable/receipts.py",
            "py-harness",
        ),
        (
            "sed -n '1,20p' tests/unit/semantic_sandtable/test_receipt_token_cost.py",
            "py-harness",
        ),
    ] {
        assert_content_dump_denied(command, binary);
    }
}

#[test]
fn range_content_dump_routes_source_suffixes_across_languages() {
    for (command, provider_id, selector) in [
        (
            "awk 'NR>=10 && NR<=20' src/lib.rs",
            "rs-harness",
            "src/lib.rs:10:20",
        ),
        (
            "head -n 20 src/cli/main.ts",
            "ts-harness",
            "src/cli/main.ts:1:20",
        ),
        (
            "head -n 20 src/cli/main.js",
            "ts-harness",
            "src/cli/main.js:1:20",
        ),
        (
            "tail -n +10 packages/python/src/tools/semantic_sandtable/receipts.py | head -n 21",
            "py-harness",
            "packages/python/src/tools/semantic_sandtable/receipts.py:10:30",
        ),
        (
            "nl -ba tests/unit/semantic_sandtable/test_receipt_token_cost.py | sed -n '5,12p'",
            "py-harness",
            "tests/unit/semantic_sandtable/test_receipt_token_cost.py:5:12",
        ),
    ] {
        let decision = classify_hook(
            &polyglot_registry(),
            "codex",
            "pre-tool",
            &json!({
                "tool_name": "functions.exec_command",
                "tool_input": {"cmd": command}
            }),
        );

        assert_eq!(decision.decision, DecisionKind::Deny, "{command}");
        assert_eq!(
            decision.reason_kind,
            ReasonKind::BulkSourceDump,
            "{command}"
        );
        assert_eq!(decision.routes[0].binary, "asp", "{command}");
        assert_eq!(decision.routes[0].provider_id, provider_id, "{command}");
        let selector_index = decision.routes[0]
            .argv
            .iter()
            .position(|arg| arg == "--selector")
            .expect("selector flag");
        assert_eq!(
            decision.routes[0]
                .argv
                .get(selector_index + 1)
                .map(String::as_str),
            Some(selector),
            "{command}"
        );
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
            "tool_input": {"cmd": "sed -n '1,20p' '**/*.{rs,py,js,ts}'"}
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
