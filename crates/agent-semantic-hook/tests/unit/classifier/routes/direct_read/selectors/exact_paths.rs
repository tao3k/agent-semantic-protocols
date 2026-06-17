use agent_semantic_hook::{DecisionKind, DecisionRouteKind, ReasonKind, classify_hook};
use serde_json::json;

use crate::classifier::routes::registry_with_rust_and_python;

#[test]
fn exact_direct_read_source_suffixes_route_by_language() {
    for (selector, language_id, provider_id) in [
        ("lib.rs", "rust", "rs-harness"),
        ("main.ts", "typescript", "ts-harness"),
        ("main.js", "typescript", "ts-harness"),
        ("test_receipt_token_cost.py", "python", "py-harness"),
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
        assert_eq!(decision.routes[0].kind, DecisionRouteKind::Query);
        assert_eq!(decision.routes[0].binary, "asp", "{selector}");
        assert_eq!(decision.routes[0].provider_id, provider_id, "{selector}");
    }
}

#[test]
fn rust_basename_reads_route_through_asp_facade() {
    for selector in [
        "mod.rs",
        "intent.rs",
        "search_json.rs",
        "raw_search.rs",
        "classifier.rs",
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
        assert_eq!(decision.language_ids, ["rust".to_string()]);
        assert_eq!(decision.routes[0].binary, "asp", "{selector}");
        assert_eq!(decision.routes[0].provider_id, "rs-harness", "{selector}");
        assert_eq!(
            decision.routes[0].argv,
            [
                "asp",
                "rust",
                "query",
                "--selector",
                selector,
                "--workspace",
                ".",
                "--code",
            ],
            "{selector}"
        );
    }
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
        [
            "asp",
            "python",
            "query",
            "--selector",
            "*.py",
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
fn direct_read_path_field_variants_route_source_paths() {
    for (tool_name, tool_input, selector) in [
        (
            "functions.read_file",
            json!({"absolutePath": "profile_registry.rs"}),
            "profile_registry.rs",
        ),
        (
            "functions.read",
            json!({"file": "event_state.rs"}),
            "event_state.rs",
        ),
        (
            "read",
            json!({"resource": {"path": "hook_runtime.rs"}}),
            "hook_runtime.rs",
        ),
        (
            "Read",
            json!({"resources": [{"filePath": "tool_action.rs"}]}),
            "tool_action.rs",
        ),
        (
            "Read",
            json!({"uris": ["file://profile_registry.rs"]}),
            "profile_registry.rs",
        ),
    ] {
        let decision = classify_hook(
            &registry_with_rust_and_python(),
            "codex",
            "pre-tool",
            &json!({
                "tool_name": tool_name,
                "tool_input": tool_input
            }),
        );

        assert_eq!(decision.decision, DecisionKind::Deny, "{selector}");
        assert_eq!(decision.reason_kind, ReasonKind::DirectSourceRead);
        assert_eq!(decision.subject.paths, [selector.to_string()]);
        assert_eq!(decision.language_ids, ["rust".to_string()]);
        assert_eq!(
            decision.routes[0].argv,
            [
                "asp",
                "rust",
                "query",
                "--selector",
                selector,
                "--workspace",
                ".",
                "--code",
            ],
            "{selector}"
        );
    }
}
