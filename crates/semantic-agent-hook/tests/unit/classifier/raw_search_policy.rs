use semantic_agent_hook::{DecisionKind, ReasonKind, classify_hook};
use serde_json::json;

use super::registry;

#[test]
fn non_source_extension_raw_search_is_allowed() {
    for command in [
        "rg --files -g '*.md' src",
        "rg -t markdown WorkflowExecution docs",
        "fd -e md src",
        "find . -name '*.md' -print",
    ] {
        let decision = classify_hook(
            &registry(),
            "codex",
            "pre-tool",
            &json!({
                "tool_name": "functions.exec_command",
                "tool_input": {"cmd": command}
            }),
        );

        assert_eq!(decision.decision, DecisionKind::Allow, "{command}");
    }
}

#[test]
fn source_extension_raw_search_is_denied() {
    for command in [
        "rg --files -g '*.ts' src",
        "rg -t ts WorkflowExecution src",
        "rg --type=typescript WorkflowExecution src",
        "fd -e ts src",
        "find . -name '*.ts' -print",
    ] {
        let decision = classify_hook(
            &registry(),
            "codex",
            "pre-tool",
            &json!({
                "tool_name": "functions.exec_command",
                "tool_input": {"cmd": command}
            }),
        );

        assert_eq!(decision.decision, DecisionKind::Deny, "{command}");
        assert_eq!(decision.reason_kind, ReasonKind::RawBroadSearch);
    }
}

#[test]
fn raw_search_piped_to_ingest_is_allowed() {
    let decision = classify_hook(
        &registry(),
        "codex",
        "pre-tool",
        &json!({
            "tool_name": "functions.exec_command",
            "tool_input": {"cmd": "rg -n WorkflowExecution src | ts-harness search ingest owner tests --view seeds ."}
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Allow);
}
