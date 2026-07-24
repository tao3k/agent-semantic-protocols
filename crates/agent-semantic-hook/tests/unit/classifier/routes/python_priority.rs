use agent_semantic_hook::{DecisionKind, DecisionRouteKind, ReasonKind, classify_hook};
use serde_json::json;

use super::registry_with_python;

#[test]
fn namespaced_python_explicit_read_routes_to_owner_frontier() {
    let decision = classify_hook(
        &registry_with_python(),
        "codex",
        "pre-tool",
        &json!({
            "toolName": "functions.read_file",
            "toolInput": {"path": "src/tools/semantic_sandtable/receipt_reports.py"}
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Deny);
    assert_eq!(decision.reason_kind, ReasonKind::DirectSourceRead);
    assert_eq!(decision.language_ids, ["python"]);
    assert_eq!(decision.routes[0].kind, DecisionRouteKind::Owner);
    assert_eq!(decision.routes[0].provider_id, "py-harness");
    assert_eq!(
        decision.routes[0].argv,
        [
            "asp",
            "python",
            "search",
            "owner",
            "src/tools/semantic_sandtable/receipt_reports.py",
            "items",
            "--query",
            "src/tools/semantic_sandtable/receipt_reports.py",
            "--workspace",
            ".",
            "--view",
            "seeds"
        ]
    );
    assert!(
        !decision.routes[0]
            .argv
            .iter()
            .any(|arg| matches!(arg.as_str(), "query" | "--code" | "--content"))
    );
}

#[test]
fn exact_document_reads_route_to_owner_discovery_without_projection_flags() {
    for (path, language_id) in [("docs/guide.org", "org"), ("docs/guide.md", "md")] {
        let decision = classify_hook(
            &super::registry_with_documents(),
            "codex",
            "pre-tool",
            &json!({
                "toolName": "functions.read_file",
                "toolInput": {"path": path}
            }),
        );

        assert_eq!(decision.decision, DecisionKind::Deny, "{path}");
        assert_eq!(decision.reason_kind, ReasonKind::DirectSourceRead, "{path}");
        assert_eq!(decision.language_ids, [language_id], "{path}");
        assert_eq!(decision.routes.len(), 1, "{path}");
        let route = &decision.routes[0];
        assert_eq!(route.kind, DecisionRouteKind::Owner, "{path}");
        assert_eq!(route.language_id, language_id, "{path}");
        assert_eq!(
            &route.argv[..4],
            ["asp", language_id, "search", "owner"],
            "{path}"
        );
        assert!(
            route.argv.iter().any(|arg| arg == path),
            "{path}: {:?}",
            route.argv
        );
        assert!(
            !route.argv.iter().any(|arg| matches!(
                arg.as_str(),
                "query" | "--content" | "--verbatim" | "--code"
            )),
            "{path}: {:?}",
            route.argv
        );
    }
}

#[test]
fn python_pattern_read_routes_to_lexical_discovery() {
    let decision = classify_hook(
        &registry_with_python(),
        "codex",
        "pre-tool",
        &json!({
            "toolName": "functions.read_file",
            "toolInput": {"path": "src/tools/**/*.py"}
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Deny);
    assert_eq!(decision.reason_kind, ReasonKind::DirectSourceRead);
    assert_eq!(decision.language_ids, ["python"]);
    assert_eq!(decision.routes[0].kind, DecisionRouteKind::Lexical);
    assert_eq!(decision.routes[0].provider_id, "py-harness");
    assert!(
        !decision.routes[0]
            .argv
            .iter()
            .any(|arg| matches!(arg.as_str(), "query" | "--code" | "--content"))
    );
}

#[test]
fn python_embedded_read_text_routes_to_owner_frontier() {
    let decision = classify_hook(
        &registry_with_python(),
        "codex",
        "pre-tool",
        &json!({
            "tool_name": "functions.exec_command",
            "tool_input": {"cmd": "python3 - <<'PY'\nfrom pathlib import Path\npath = Path('src/tools/semantic_sandtable/receipt_reports.py')\nprint(path.read_text())\nPY"}
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Deny);
    assert_eq!(decision.reason_kind, ReasonKind::BulkSourceDump);
    assert_eq!(decision.language_ids, ["python"]);
    assert_eq!(
        decision.subject.command.as_deref(),
        Some(
            "python3 - <<'PY'\nfrom pathlib import Path\npath = Path('src/tools/semantic_sandtable/receipt_reports.py')\nprint(path.read_text())\nPY"
        )
    );
    assert_eq!(
        decision.subject.paths,
        ["src/tools/semantic_sandtable/receipt_reports.py"]
    );
    assert_eq!(decision.routes[0].kind, DecisionRouteKind::Owner);
    assert_eq!(decision.routes[0].provider_id, "py-harness");
    assert_eq!(
        decision.routes[0].argv,
        [
            "asp",
            "python",
            "search",
            "owner",
            "src/tools/semantic_sandtable/receipt_reports.py",
            "items",
            "--query",
            "src/tools/semantic_sandtable/receipt_reports.py",
            "--workspace",
            ".",
            "--view",
            "seeds"
        ]
    );
}

#[test]
fn python_nested_package_read_text_routes_to_provider_root() {
    let mut registry = registry_with_python();
    registry.providers[1].package_roots = vec![
        ".".to_string(),
        "languages/python-lang-project-harness".to_string(),
    ];

    let decision = classify_hook(
        &registry,
        "codex",
        "pre-tool",
        &json!({
            "tool_name": "functions.exec_command",
            "tool_input": {"cmd": "python3 - <<'PY'\nfrom pathlib import Path\nprint(Path('languages/python-lang-project-harness/src/python_lang_project_harness/_cli_query_args.py').read_text())\nPY"}
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Deny);
    assert_eq!(decision.reason_kind, ReasonKind::BulkSourceDump);
    assert_eq!(
        decision.subject.paths,
        [
            "languages/python-lang-project-harness/src/python_lang_project_harness/_cli_query_args.py"
        ]
    );
    assert_eq!(decision.routes[0].kind, DecisionRouteKind::Owner);
    assert_eq!(decision.routes[0].provider_id, "py-harness");
    assert_eq!(
        decision.routes[0].argv,
        [
            "asp",
            "python",
            "search",
            "owner",
            "src/python_lang_project_harness/_cli_query_args.py",
            "items",
            "--query",
            "languages/python-lang-project-harness/src/python_lang_project_harness/_cli_query_args.py",
            "--workspace",
            "languages/python-lang-project-harness",
            "--view",
            "seeds"
        ]
    );
}
