use semantic_agent_hook::{DecisionKind, DecisionRouteKind, ReasonKind, classify_hook};
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
            "py-harness",
            "query",
            "--from-hook",
            "direct-source-read",
            "--selector",
            "src/tools/semantic_sandtable/receipt_reports.py",
            "."
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
            "py-harness",
            "query",
            "--from-hook",
            "direct-source-read",
            "--selector",
            "src/tools/semantic_sandtable/receipt_reports.py",
            "."
        ]
    );
}
