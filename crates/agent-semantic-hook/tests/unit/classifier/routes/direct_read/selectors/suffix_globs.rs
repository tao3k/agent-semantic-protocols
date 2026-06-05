use agent_semantic_hook::{DecisionKind, DecisionRouteKind, ReasonKind, classify_hook};
use serde_json::json;

use crate::classifier::routes::registry_with_rust_and_python;

#[test]
fn structured_direct_read_globs_are_matched_by_suffix_not_project_shape() {
    for (selector, language_id, provider_id) in [
        (
            "third_party/vendor/acme/generated/deep/**/*.rs",
            "rust",
            "rs-harness",
        ),
        (
            "warehouse/arbitrary/layout/plugins/**/*.py",
            "python",
            "py-harness",
        ),
        (
            "not-a-source-root/custom/frontend/**/*.[jt]s",
            "typescript",
            "ts-harness",
        ),
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
        assert_eq!(decision.routes[0].binary, "asp", "{selector}");
        assert_eq!(decision.routes[0].provider_id, provider_id, "{selector}");
        assert_eq!(decision.routes[0].kind, DecisionRouteKind::Query);
        assert_eq!(
            decision.routes[0].argv,
            [
                "asp",
                language_id,
                "query",
                "--from-hook",
                "direct-source-read",
                "--selector",
                selector,
                "--surface",
                "owners,tests",
                "--view",
                "seeds",
                "."
            ],
            "{selector}"
        );
    }
}

#[test]
fn structured_direct_read_brace_glob_routes_to_all_matching_providers() {
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
    assert_eq!(decision.routes[0].provider_id, "rs-harness");
    assert_eq!(decision.routes[1].provider_id, "py-harness");
    assert_eq!(
        decision.routes[0].argv,
        [
            "asp",
            "rust",
            "query",
            "--from-hook",
            "direct-source-read",
            "--selector",
            "*.{rs,py}",
            "--surface",
            "owners,tests",
            "--view",
            "seeds",
            "."
        ]
    );
    assert_eq!(
        decision.routes[1].argv,
        [
            "asp",
            "python",
            "query",
            "--from-hook",
            "direct-source-read",
            "--selector",
            "*.{rs,py}",
            "--surface",
            "owners,tests",
            "--view",
            "seeds",
            "."
        ]
    );
    assert!(decision.message.starts_with("# ASP Hook Recovery"));
    assert!(decision.message.contains("`asp rust guide .`"));
    assert!(decision.message.contains("`asp python guide .`"));
    assert!(decision.message.contains(
        "asp rust query --from-hook direct-source-read --selector '*.{rs,py}' --surface 'owners,tests' --view seeds ."
    ));
    assert!(decision.message.contains(
        "asp python query --from-hook direct-source-read --selector '*.{rs,py}' --surface 'owners,tests' --view seeds ."
    ));
    assert!(!decision.message.contains("|run-next"));
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
