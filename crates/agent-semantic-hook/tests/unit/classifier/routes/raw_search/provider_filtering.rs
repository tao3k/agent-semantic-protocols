use agent_semantic_hook::{DecisionKind, ReasonKind, classify_hook};
use serde_json::json;

use super::support::{prefixed_registry, registry};

#[test]
fn provider_output_filtering_is_allowed() {
    for command in [
        "ts-harness --help | rg -- '--code|query <owner-path>'",
        "ts-harness agent guide . | rg -- '--code'",
        "py-harness --help | rg -- '--code|query <owner-path>'",
        "rs-harness agent guide . | rg -- '--code'",
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
        assert_eq!(decision.reason_kind, ReasonKind::None, "{command}");
    }
}

#[test]
fn prefixed_provider_output_filtering_requires_full_command_prefix() {
    let allowed = "python -m tools.fake_provider agent guide . | rg -- 'search owner'";
    let allowed_decision = classify_hook(
        &prefixed_registry(),
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
        &prefixed_registry(),
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
