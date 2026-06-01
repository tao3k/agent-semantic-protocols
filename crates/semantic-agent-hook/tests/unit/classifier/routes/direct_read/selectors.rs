use semantic_agent_hook::{DecisionKind, DecisionRouteKind, ReasonKind, classify_hook};
use serde_json::json;

use crate::classifier::registry;

use crate::classifier::routes::registry_with_rust_and_python;

#[test]
fn structured_direct_read_source_glob_routes_to_prime() {
    let decision = classify_hook(
        &registry(),
        "codex",
        "pre-tool",
        &json!({
            "tool_name": "Read",
            "tool_input": {"path": "*.ts"}
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Deny);
    assert_eq!(decision.reason_kind, ReasonKind::DirectSourceRead);
    assert_eq!(decision.subject.paths, ["*.ts"]);
    assert_eq!(
        decision.routes[0].argv,
        ["ts-harness", "search", "prime", "."]
    );
    assert_eq!(
        decision.message,
        "direct-source-read denied; provider guide: ts-harness => ts-harness agent guide ."
    );
}

#[test]
fn structured_direct_read_language_globs_route_to_profile_prime() {
    for (selector, language_id, binary) in [
        ("*.rs", "rust", "rs-harness"),
        ("*.ts", "typescript", "ts-harness"),
        ("*.py", "python", "py-harness"),
    ] {
        let decision = classify_hook(
            &registry_with_rust_and_python(),
            "codex",
            "pre-tool",
            &json!({
                "tool_name": "Read",
                "tool_input": {"path": selector}
            }),
        );

        assert_eq!(decision.decision, DecisionKind::Deny, "{selector}");
        assert_eq!(decision.reason_kind, ReasonKind::DirectSourceRead);
        assert_eq!(decision.language_ids, [language_id.to_string()]);
        assert_eq!(decision.routes[0].kind, DecisionRouteKind::Prime);
        assert_eq!(
            decision.routes[0].argv,
            [binary, "search", "prime", "."],
            "{selector}"
        );
    }
}

#[test]
fn structured_direct_read_globs_are_matched_by_suffix_not_project_shape() {
    for (selector, binary) in [
        (
            "third_party/vendor/acme/generated/deep/**/*.rs",
            "rs-harness",
        ),
        ("warehouse/arbitrary/layout/plugins/**/*.py", "py-harness"),
        ("not-a-source-root/custom/frontend/**/*.[jt]s", "ts-harness"),
    ] {
        let decision = classify_hook(
            &registry_with_rust_and_python(),
            "codex",
            "pre-tool",
            &json!({
                "tool_name": "Read",
                "tool_input": {"path": selector}
            }),
        );

        assert_eq!(decision.decision, DecisionKind::Deny, "{selector}");
        assert_eq!(decision.reason_kind, ReasonKind::DirectSourceRead);
        assert_eq!(decision.routes[0].binary, binary, "{selector}");
        assert_eq!(decision.routes[0].kind, DecisionRouteKind::Prime);
    }
}

#[test]
fn structured_direct_read_globs_without_suffix_are_not_language_evidence() {
    for selector in ["crates/**", "src/**", "**/*"] {
        let decision = classify_hook(
            &registry_with_rust_and_python(),
            "codex",
            "pre-tool",
            &json!({
                "tool_name": "Read",
                "tool_input": {"path": selector}
            }),
        );

        assert_eq!(decision.decision, DecisionKind::Allow, "{selector}");
    }
}

#[test]
fn structured_direct_read_brace_glob_routes_to_all_matching_profiles() {
    let decision = classify_hook(
        &registry_with_rust_and_python(),
        "codex",
        "pre-tool",
        &json!({
            "tool_name": "Read",
            "tool_input": {"path": "*.{rs,py}"}
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Deny);
    assert_eq!(decision.reason_kind, ReasonKind::DirectSourceRead);
    assert_eq!(
        decision.language_ids,
        vec!["rust".to_string(), "python".to_string()]
    );
    assert_eq!(decision.routes.len(), 2);
    assert_eq!(
        decision.routes[0].argv,
        ["rs-harness", "search", "prime", "."]
    );
    assert_eq!(
        decision.routes[1].argv,
        ["py-harness", "search", "prime", "."]
    );
    assert_eq!(
        decision.message,
        "direct-source-read denied; provider guide: rs-harness => rs-harness agent guide .; py-harness => py-harness agent guide ."
    );
}

#[test]
fn structured_direct_read_paths_array_ignores_non_source_and_routes_source() {
    let decision = classify_hook(
        &registry_with_rust_and_python(),
        "codex",
        "pre-tool",
        &json!({
            "tool_name": "Read",
            "tool_input": {"paths": ["README.md", "*.py"]}
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Deny);
    assert_eq!(decision.reason_kind, ReasonKind::DirectSourceRead);
    assert_eq!(decision.subject.paths, ["README.md", "*.py"]);
    assert_eq!(decision.language_ids, ["python".to_string()]);
    assert_eq!(
        decision.routes[0].argv,
        ["py-harness", "search", "prime", "."]
    );
}

#[test]
fn non_source_direct_read_glob_is_allowed() {
    let decision = classify_hook(
        &registry_with_rust_and_python(),
        "codex",
        "pre-tool",
        &json!({
            "tool_name": "Read",
            "tool_input": {"path": "docs/**/*.md"}
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Allow);
}
