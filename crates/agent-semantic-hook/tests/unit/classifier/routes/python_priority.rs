use agent_semantic_hook::{DecisionKind, DecisionRouteKind, ReasonKind, classify_hook};
use serde_json::json;

use super::registry_with_python;

#[test]
fn content_dump_file_extension_beats_shared_source_root() {
    let decision = classify_hook(
        &registry_with_python(),
        "codex",
        "pre-tool",
        &json!({
            "tool_name": "functions.exec_command",
            "tool_input": {"cmd": "sed -n '1,80p' src/tools/semantic_sandtable/receipt_reports.py"}
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Deny);
    assert_eq!(decision.reason_kind, ReasonKind::BulkSourceDump);
    assert_eq!(decision.language_ids, ["python"]);
    assert_eq!(decision.routes[0].kind, DecisionRouteKind::Query);
    assert_eq!(decision.routes[0].provider_id, "py-harness");
    assert_eq!(
        decision.routes[0].argv,
        [
            "asp",
            "python",
            "query",
            "--selector",
            "src/tools/semantic_sandtable/receipt_reports.py:1:80",
            "--workspace",
            ".",
            "--code",
        ]
    );
}

#[test]
fn namespaced_python_direct_read_routes_to_provider_query() {
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
    assert_eq!(
        decision.subject.tool_name.as_deref(),
        Some("functions.read_file")
    );
    assert_eq!(
        decision.subject.paths,
        ["src/tools/semantic_sandtable/receipt_reports.py"]
    );
    assert_eq!(decision.routes[0].kind, DecisionRouteKind::Query);
    assert_eq!(decision.routes[0].provider_id, "py-harness");
    assert_eq!(
        decision.routes[0].argv,
        [
            "asp",
            "python",
            "query",
            "--selector",
            "src/tools/semantic_sandtable/receipt_reports.py",
            "--workspace",
            ".",
            "--code",
        ]
    );
}

#[test]
fn python_embedded_read_text_routes_to_provider_query() {
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
    assert_eq!(decision.routes[0].kind, DecisionRouteKind::Query);
    assert_eq!(decision.routes[0].provider_id, "py-harness");
    assert_eq!(
        decision.routes[0].argv,
        [
            "asp",
            "python",
            "query",
            "--selector",
            "src/tools/semantic_sandtable/receipt_reports.py",
            "--workspace",
            ".",
            "--code"
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
    assert_eq!(
        decision.routes[0].argv,
        [
            "asp",
            "python",
            "query",
            "--selector",
            "src/python_lang_project_harness/_cli_query_args.py",
            "--workspace",
            "languages/python-lang-project-harness",
            "--code"
        ]
    );
}
