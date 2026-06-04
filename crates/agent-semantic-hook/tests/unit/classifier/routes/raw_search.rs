use agent_semantic_hook::{DecisionKind, DecisionRouteKind, ReasonKind, classify_hook};
use serde_json::json;

use crate::classifier::registry;

#[test]
fn provider_output_filtering_is_allowed() {
    for command in [
        "ts-harness --help | rg -- '--code|query <owner-path>'",
        "ts-harness agent guide . | rg -- '--code'",
        "py-harness --help | rg -- '--code|query <owner-path>'",
        "rs-harness agent guide . | rg -- '--code'",
    ] {
        let decision = classify_hook(
            &super::registry_with_rust_and_python(),
            "codex",
            "pre-tool",
            &json!({
                "tool_name": "functions.exec_command",
                "tool_input": {"cmd": command}
            }),
        );
        assert_eq!(decision.decision, DecisionKind::Allow, "{command}");
        assert_eq!(decision.reason_kind, ReasonKind::None, "{command}");
    }
}

#[test]
fn prefixed_provider_output_filtering_requires_full_command_prefix() {
    let allowed = "python -m tools.fake_provider agent guide . | rg -- 'search owner'";
    let allowed_decision = classify_hook(
        &super::registry_with_prefixed_python(),
        "codex",
        "pre-tool",
        &json!({
            "tool_name": "functions.exec_command",
            "tool_input": {"cmd": allowed}
        }),
    );
    assert_eq!(allowed_decision.decision, DecisionKind::Allow);
    assert_eq!(allowed_decision.reason_kind, ReasonKind::None);

    let denied = "python -c 'print(\"raw\")' | rg -- 'raw'";
    let denied_decision = classify_hook(
        &super::registry_with_prefixed_python(),
        "codex",
        "pre-tool",
        &json!({
            "tool_name": "functions.exec_command",
            "tool_input": {"cmd": denied}
        }),
    );
    assert_eq!(denied_decision.decision, DecisionKind::Deny);
    assert_eq!(denied_decision.reason_kind, ReasonKind::RawBroadSearch);
}

#[test]
fn raw_file_listing_without_query_keeps_ingest_route() {
    let decision = classify_hook(
        &registry(),
        "codex",
        "pre-tool",
        &json!({
            "tool_name": "functions.exec_command",
            "tool_input": {"cmd": "rg --files -g '*.ts'"}
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Deny);
    assert_eq!(decision.reason_kind, ReasonKind::RawBroadSearch);
    assert_eq!(decision.routes[0].kind, DecisionRouteKind::Ingest);
}

#[test]
fn find_extension_listing_without_name_query_keeps_ingest_route() {
    let decision = classify_hook(
        &registry(),
        "codex",
        "pre-tool",
        &json!({
            "tool_name": "functions.exec_command",
            "tool_input": {"cmd": "find . -name '*.ts'"}
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Deny);
    assert_eq!(decision.reason_kind, ReasonKind::RawBroadSearch);
    assert_eq!(decision.routes[0].kind, DecisionRouteKind::Ingest);
}
