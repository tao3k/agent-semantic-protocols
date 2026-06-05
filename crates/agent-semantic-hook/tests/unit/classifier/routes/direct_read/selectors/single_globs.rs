use agent_semantic_hook::{DecisionKind, DecisionRouteKind, ReasonKind, classify_hook};
use serde_json::json;

use crate::classifier::registry;
use crate::classifier::routes::registry_with_rust_and_python;

#[test]
fn structured_direct_read_source_glob_routes_to_provider_query() {
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
        [
            "asp",
            "typescript",
            "query",
            "--from-hook",
            "direct-source-read",
            "--selector",
            "*.ts",
            "--surface",
            "owners,tests",
            "--view",
            "seeds",
            "."
        ]
    );
    assert!(decision.message.starts_with("# ASP Hook Recovery"));
    assert!(decision.message.contains("`asp typescript guide .`"));
    assert!(decision.message.contains(
        "asp typescript query --from-hook direct-source-read --selector '*.ts' --surface 'owners,tests' --view seeds ."
    ));
    assert!(!decision.message.contains("|run-next"));
}

#[test]
fn structured_direct_read_language_globs_route_to_provider_query() {
    for (selector, language_id, provider_id) in [
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
        assert_eq!(decision.routes[0].kind, DecisionRouteKind::Query);
        assert_eq!(decision.routes[0].binary, "asp", "{selector}");
        assert_eq!(decision.routes[0].provider_id, provider_id, "{selector}");
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
