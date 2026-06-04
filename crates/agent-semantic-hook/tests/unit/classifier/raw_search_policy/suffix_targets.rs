use agent_semantic_hook::{DecisionKind, ReasonKind, classify_hook};
use serde_json::json;

use super::support::{assert_allowed, assert_raw_search_denied, polyglot_registry};

#[test]
fn non_source_extension_raw_search_is_allowed() {
    for command in [
        "rg --files -g '*.md' docs",
        "rg -t markdown WorkflowExecution docs",
        "fd -e md src",
        "find . -name '*.md' -print",
    ] {
        assert_allowed(command);
    }
}

#[test]
fn source_extension_raw_search_is_suffix_driven() {
    for (command, binary) in [
        ("rg --files -g '*.ts' src", "ts-harness"),
        ("rg --files -g 'src/**/*.tsx'", "ts-harness"),
        ("rg --files -g 'packages/*/src/**/*.py'", "py-harness"),
        ("rg --files -g 'vendor/acme/**/SourceFile.rs'", "rs-harness"),
        ("rg --files -g 'crates/**/lib.rs'", "rs-harness"),
        ("rg --files -g '*.[jt]s' src", "ts-harness"),
        ("rg --files -g '**/*.{rs,py,js,ts}'", "ts-harness"),
        ("rg --type=typescript WorkflowExecution src", "ts-harness"),
        ("rg -t rust HookToolName crates", "rs-harness"),
        ("fd -e py runner packages", "py-harness"),
        ("fd '*.rs'", "rs-harness"),
        ("fd '**/*.{js,ts}'", "ts-harness"),
        (
            "find external-layout -path '*/custom/**/*.py' -print",
            "py-harness",
        ),
        ("find . -name '*.rs' -print", "rs-harness"),
        ("find . -name '*.{rs,py}' -print", "rs-harness"),
        ("grep -R --include='*.py' StepRunner packages", "py-harness"),
        ("git grep WorkflowExecution -- '*.tsx'", "ts-harness"),
        ("git ls-files '*.py'", "py-harness"),
    ] {
        assert_raw_search_denied(command, binary);
    }
}

#[test]
fn exact_source_file_raw_search_is_denied_for_each_provider() {
    for (command, binary) in [
        ("rg -n WorkflowExecution src/cli/protocol.ts", "ts-harness"),
        (
            "rg -n HookToolName crates/agent-semantic-hook/src/lib.rs",
            "rs-harness",
        ),
        (
            "rg -n StepRunner packages/python/src/tools/semantic_sandtable/runner.py",
            "py-harness",
        ),
    ] {
        assert_raw_search_denied(command, binary);
    }
}

#[test]
fn source_like_pattern_text_does_not_create_language_match() {
    for command in [
        "rg -n 'protocol.ts' docs",
        "rg -e 'runner.py' docs",
        "grep -R 'lib.rs' README.md",
    ] {
        assert_allowed(command);
    }
}

#[test]
fn brace_glob_targets_all_matching_language_providers() {
    let decision = classify_hook(
        &polyglot_registry(),
        "codex",
        "pre-tool",
        &json!({
            "tool_name": "functions.exec_command",
            "tool_input": {"cmd": "rg --files -g '**/*.{rs,py,js,ts}' packages"}
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Deny);
    assert_eq!(decision.reason_kind, ReasonKind::RawBroadSearch);
    assert_eq!(
        decision.language_ids,
        vec![
            "typescript".to_string(),
            "rust".to_string(),
            "python".to_string()
        ]
    );
}
